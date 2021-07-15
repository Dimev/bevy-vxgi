// HOW IT WORKS
// render the world from 3 orthographic cameras
// if the angle to the camera with the triangle is good enough, rasterize it to the volume 
// set opacity to 1, color to the albedo at that point * shadow
// generate mipmaps
// pass the volume texture(s)? to the 

// single pass for the orthographic pass

//use bevy::crevice::std140::AsStd140;

use crate::bundle::GiVolume;

use bevy::crates::crevice::std140::AsStd140;

use bevy::transform::components::{GlobalTransform, Transform};

use bevy::math::{const_vec3, Mat4, Vec3, Vec4};
use bevy::ecs::prelude::*;
use bevy::render2::{
	shader::Shader,
	render_resource::*,
	renderer::{RenderContext, RenderDevice}
};

use bevy::pbr2::PbrShaders;

pub struct ExtractedGiCascade {
	projection_x: Mat4,
	projection_y: Mat4,
	projection_z: Mat4,
	resolution: u32,
}

// this is for *one* projection
#[repr(C)]
#[derive(Copy, Clone, /*AsStd140,*/ Default, Debug)]
pub struct GpuGiCascade {
	projection: Mat4,
	resolution: u32,
}

// TODO: struct that sends all of them to the pbr shader

pub struct GiShaders {
	pipeline: RenderPipeline,
	view_layout: BindGroupLayout,
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
					visibility: ShaderStage::FRAGMENT,
					ty: BindingType::Buffer {
						ty: BufferBindingType::Uniform,
						has_dynamic_offset: true,
						// TODO: change this to ViewUniform::std140_size_static once crevice fixes this!
                        // Context: https://github.com/LPGhatguy/crevice/issues/29
                        min_binding_size: BufferSize::new(80),
					},
					count: None,
				},
			],
			label: None,
		});

		// TODO, change
		let mesh_layout = render_device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            entries: &[BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStage::VERTEX,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Uniform,
                    has_dynamic_offset: true,
                    min_binding_size: BufferSize::new(Mat4::std140_size_static() as u64),
                },
                count: None,
            }],
            label: None,
        });

		let pipeline_layout = render_device.create_pipeline_layout(&PipelineLayoutDescriptor {
			label: None,
			push_constant_ranges: &[],
			bind_group_layouts: &[&view_layout, &mesh_layout],
		});

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

		GiShaders {
			pipeline,
			view_layout,
		}
	}
}

pub fn extract_gi_cascades(
	mut commands: Commands,
	volumes: Query<(Entity, &GiVolume, &GlobalTransform)>
) {

	for (entity, volume, transform) in volumes.iter() {

		// here we get all active volumes
		// each one has 3 matrices for each projection face
		// these are calculated in prepare, this is just to find all active volumes, and get the cascade

	}

}