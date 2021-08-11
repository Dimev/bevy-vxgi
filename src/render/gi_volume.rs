// HOW IT WORKS
// render the world from 3 orthographic cameras
// if the angle to the camera with the triangle is good enough, rasterize it to the volume
// set opacity to 1, color to the albedo at that point * shadow
// generate mipmaps
// pass the volume texture(s)? to the

// single pass for the orthographic pass

// THIS FILE:
// extract all cascades
// make the pipeline for voxelization
// prepare voxelization, make list of projection, resolution and textures
// do voxelization

// OTHER FILE:
// pass volumes to PBR shader

use crevice::std140::AsStd140;

use crate::bundle::GiVolume;

use bevy::transform::components::{GlobalTransform, Transform};

use bevy::ecs::prelude::*;
use bevy::math::{const_vec3, Mat4, Vec3, Vec4};
use bevy::render2::{
	render_graph::{Node, NodeRunError, RenderGraphContext, SlotInfo, SlotType},
    render_asset::RenderAssets,
    render_phase::{Draw, DrawFunctions, RenderPhase, TrackedRenderPass},
    render_resource::*,
    renderer::{RenderContext, RenderDevice, RenderQueue},
    shader::Shader,
    texture::*,
};

use bevy_core_pipeline::Transparent3dPhase;
use bevy_pbr2::PbrShaders;

//use bevy::pbr2::PbrShaders;

// info for the cascade
pub struct ExtractedGiVolume {
    transform: GlobalTransform, // origin and scale
    resolution: u32,
    cascades: u8, // how many lod levels we have
    size: f32, // size of the first lod
}

// this is for *one* projection for a cascade
#[repr(C)]
#[derive(Copy, Clone, AsStd140, Default, Debug)]
pub struct GpuGiCascade {
    projection: Mat4,
    resolution: u32,
	texture_index: u32, // which part of the texture to use
}

// max number of cascades allowed in the world at the same time
const MAX_CASCADE_NUM: usize = 8;

// holds all cascades
// used for passing to the pbr shader
#[repr(C)]
#[derive(Copy, Clone, AsStd140, Default, Debug)]
pub struct GpuGiCascades {
    num_cascades: u32,
    cascades: [GpuGiCascade; MAX_CASCADE_NUM], 
}


pub struct GiShaders {
    //vertex_pipeline: ComputePipeline,
	voxelize_pipeline: ComputePipeline,
	mipmap_pipeline: ComputePipeline,
    view_layout: BindGroupLayout,
	mesh_layout: BindGroupLayout,
	mesh_model_layout: BindGroupLayout,
	volume_layout: BindGroupLayout,
	volume_sampler: Sampler,
}

impl FromWorld for GiShaders {
    fn from_world(world: &mut World) -> Self {
        let render_device = world.get_resource::<RenderDevice>().unwrap();
        let pbr_shaders = world.get_resource::<PbrShaders>().unwrap();

        // make the shader
        let shader = Shader::from_wgsl(include_str!("voxelize.wgsl"));
        let shader_module = render_device.create_shader_module(&shader);

        let view_layout = render_device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            entries: &[
                // view
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStage::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: true,
                        // TODO: change this to ViewUniform::std140_size_static once crevice fixes this!
                        // Context: https://github.com/LPGhatguy/crevice/issues/29
                        min_binding_size: BufferSize::new(80), // TODO
                    },
                    count: None,
                },
            ],
            label: None,
        });

        // TODO grab from pbr
        let mesh_layout = render_device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            entries: &[
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStage::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: true,
                        min_binding_size: BufferSize::new(Mat4::std140_size_static() as u64), // TODO
                    },
                    count: None,
                },
            ], 
            label: None,
        });

		// and for the voxelizer
		let volume_layout = render_device.create_bind_group_layout(&BindGroupLayoutDescriptor {
			entries: &[ // TODO: add a buffer so we can store the gpugicascades
				BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStage::COMPUTE,
                    ty: BindingType::StorageTexture {
                        access: StorageTextureAccess::ReadWrite,
                        format: TextureFormat::Rgba32Float,
                        view_dimension: TextureViewDimension::D3,
                    },
                    count: None,
                },
			],
			label: None,
		});

		// vertex and index buffers
		let mesh_model_layout = render_device.create_bind_group_layout(&BindGroupLayoutDescriptor {
			entries: &[
				BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStage::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: true},
                        has_dynamic_offset: true,
                        min_binding_size: BufferSize::new(Mat4::std140_size_static() as u64), // TODO
                    },
                    count: None,
                },
				BindGroupLayoutEntry {
                    binding: 1,
                    visibility: ShaderStage::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: true},
                        has_dynamic_offset: true,
                        min_binding_size: BufferSize::new(Mat4::std140_size_static() as u64), // TODO
                    },
                    count: None,
                },
			],
			label: None,
		});
		
		// we'll need index buffer, vertex buffer layouts, + mesh transform, mesh material, lights and the cascades as inputs
        let pipeline_layout = render_device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: None,
            push_constant_ranges: &[],
            bind_group_layouts: &[&view_layout, &mesh_layout, &volume_layout],
        });

		let voxelize_pipeline = render_device.create_compute_pipeline(&ComputePipelineDescriptor {
			label: None,
			layout: Some(&pipeline_layout),
			entry_point: "vertex",
			module: &shader_module,
		});
		/*
		let vertex_pipeline = render_device.create_compute_pipeline(&ComputePipelineDescriptor {
			label: None,
			layout: Some(&pipeline_layout),
			entry_point: "voxelize",
			module: &shader_module,
		});
		*/
		let mipmap_pipeline = render_device.create_compute_pipeline(&ComputePipelineDescriptor {
			label: None,
			layout: Some(&pipeline_layout),
			entry_point: "mipmap",
			module: &shader_module,
		});

		/*
        let pipeline = render_device.create_render_pipeline(&RenderPipelineDescriptor {
            label: None,
            vertex: VertexState {
                buffers: &[VertexBufferLayout {
                    array_stride: 32,
                    step_mode: InputStepMode::Vertex,
                    attributes: &[
                        // Position (GOTCHA! Vertex_Position isn't first in the buffer due to how Mesh sorts attributes (alphabetically))
                        VertexAttribute {
                            format: VertexFormat::Float32x3,
                            offset: 12,
                            shader_location: 0,
                        },
                        // Normal
                        VertexAttribute {
                            format: VertexFormat::Float32x3,
                            offset: 0,
                            shader_location: 1,
                        },
                        // Uv
                        VertexAttribute {
                            format: VertexFormat::Float32x2,
                            offset: 24,
                            shader_location: 2,
                        },
                    ],
                }],
                module: &shader_module,
                entry_point: "vertex",
            },
            fragment: Some(FragmentState {
                module: &shader_module,
                entry_point: "fragment",
                targets: &[], // no targets needed here, we just need to write to the volume texture inside the shader
            }),
            depth_stencil: None,
            layout: Some(&pipeline_layout),
            multisample: MultisampleState::default(),
            primitive: PrimitiveState {
                topology: PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: PolygonMode::Fill,
                clamp_depth: false,
                conservative: false,
            },
        });
		*/

        GiShaders {
            //vertex_pipeline,
			voxelize_pipeline,
			mipmap_pipeline,
			mesh_model_layout,
            view_layout,
			mesh_layout,
			volume_layout,
			volume_sampler: render_device.create_sampler(&SamplerDescriptor {
				address_mode_u: AddressMode::ClampToEdge,
				address_mode_v: AddressMode::ClampToEdge,
				address_mode_w: AddressMode::ClampToEdge,
				mag_filter: FilterMode::Linear,
				min_filter: FilterMode::Linear,
				mipmap_filter: FilterMode::Nearest,
				..Default::default()
			})
        }
    }
}

pub fn extract_gi_cascades(
    mut commands: Commands,
    volumes: Query<(Entity, &GiVolume, &GlobalTransform)>,
) {
	
	// we only need 1
    for (_, volume, transform) in volumes.iter().take(1) {
        // here we get all active volumes
        // each cascade actually needs to render 3 times, with 3 different projections
        // these are calculated in prepare, this is just to find all active volumes, and get the cascade
		commands.insert_resource(ExtractedGiVolume {
			transform: *transform,
			resolution: volume.resolution as u32,
			cascades: volume.cascades,
			size: volume.size,
		});
        
    }
	
}

// Views are needed for every camera that renders, so here we need to store everything
/*
pub struct ViewGiVolume {
    pub volume_texture: Texture,
    pub volume_texture_view: TextureView,
}
*/
pub struct ViewGiVolumes {
	pub volume_texture: Texture,
    pub volume_texture_view: TextureView,
	pub gpu_volume_binding_index: u32,
}



#[derive(Default)]
pub struct GiCascadeMeta {
    pub view_cascades: DynamicUniformVec<GpuGiCascades>,
	pub bind_group: Option<BindGroup>,
}

// and it's format
const VOLUME_TEXTURE_FORMAT: TextureFormat = TextureFormat::Rgba32Float;

pub fn prepare_gi_cascades(
    mut commands: Commands,
    mut texture_cache: ResMut<TextureCache>,
    render_device: Res<RenderDevice>,
    views: Query<Entity, With<RenderPhase<Transparent3dPhase>>>,
    mut cascade_meta: ResMut<GiCascadeMeta>,
    volume: Res<ExtractedGiVolume>,
) {
    // reserve the right amount of space for the cascades
    cascade_meta
        .view_cascades
        .reserve_and_clear(views.iter().count(), &render_device);

    // TODO: I assume I also need to get all lights here if I want to pass that to the voxelization shader?

	for entity in views.iter() {

		// get the volume texture, with the right amount of memory allocated
		let volume_texture = texture_cache.get(
			&render_device,
			TextureDescriptor {
				size: Extent3d { // TODO CHECK IF SIZE IS ALLOWED
					width: volume.resolution,
					height: volume.resolution,
					depth_or_array_layers: volume.resolution * (MAX_CASCADE_NUM as u32).min(volume.cascades as u32),
				},
				mip_level_count: volume.resolution.next_power_of_two(), // this is the same as the log of the resolution
				sample_count: 1,
				dimension: TextureDimension::D3,
				format: VOLUME_TEXTURE_FORMAT,
				usage: TextureUsage::SAMPLED | TextureUsage::STORAGE,
				label: None,
			},
		);

		// get the view for it
		let volume_texture_view = volume_texture.texture.create_view(&TextureViewDescriptor {
			label: None,
			format: None,
			dimension: Some(TextureViewDimension::D3),
			aspect: TextureAspect::All,
			base_mip_level: 0,
			mip_level_count: None,
			base_array_layer: 0,
			array_layer_count: None,
		});

		// store our view cascades
		let mut gpu_cascades = GpuGiCascades {
			num_cascades: (MAX_CASCADE_NUM as u32).min(volume.cascades as u32),
			cascades: [GpuGiCascade::default(); MAX_CASCADE_NUM],
		};

		// go over all cascades
		// we need a seperate texture for all cascades due to size
		// this is roughly similar to how light does it but not really
		for cascade in 0..((volume.cascades as usize).min(MAX_CASCADE_NUM)) {


			// get the projection matrix
			// TODO: FIX
			let projection = volume.transform;

			// store it into the gpu gi cascades
			gpu_cascades.cascades[cascade] = GpuGiCascade {
				projection: Mat4::default(),
				resolution: volume.resolution,
				texture_index: cascade as u32,
			};
		}

		// and add it to the commands
		commands.entity(entity).insert(ViewGiVolumes {
			volume_texture: volume_texture.texture,
			volume_texture_view,
			gpu_volume_binding_index: cascade_meta.view_cascades.push(gpu_cascades)
		});
	}

	cascade_meta
		.view_cascades
		.write_to_staging_buffer(&render_device);

}

pub struct VoxelizePhase;

pub struct VoxelizePassNode {
	main_view_query: QueryState<&'static ViewGiVolumes>,
	view_volume_query: QueryState<(&'static ViewGiVolumes, &'static RenderPhase<VoxelizePhase>)>,
}

impl VoxelizePassNode {

	pub const IN_VIEW: &'static str = "view";

	pub fn new(world: &mut World) -> Self {

		Self {
			main_view_query: QueryState::new(world),
			view_volume_query: QueryState::new(world),
		}
	}
}

impl Node for VoxelizePassNode {

	fn input(&self) -> Vec<SlotInfo> {
		vec![SlotInfo::new(VoxelizePassNode::IN_VIEW, SlotType::Entity)]
	}

	fn update(&mut self, world: &mut World) {
		self.main_view_query.update_archetypes(world);
		self.view_volume_query.update_archetypes(world);
	}

	fn run(&self, graph: &mut RenderGraphContext, render_context: &mut RenderContext, world: &World) -> Result<(), NodeRunError> {

		let view_entity = graph.get_input_entity(Self::IN_VIEW)?;

		if let Ok(view_volume) = self.main_view_query.get_manual(world, view_entity) {

			//let view

			// TODO: only need to get one compute pass done, because we can do all volumes + cascades in one go
			// TODO: figure out how this works
			// TODO: first, run only one compute pass on all triangles, and per triangle, write to the volume
			// THEN: generate mipmaps for the volume

		}

		Ok(())

	}
}