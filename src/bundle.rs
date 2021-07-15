use bevy::ecs::bundle::Bundle;
use bevy::transform::components::{GlobalTransform, Transform};

// TODO: should we only have one volume?

/// Gi volume, for rendering global illumination via voxel cone tracing
/// 
/// the volume is updated each frame, and support multiple cascades
/// each cascade is twice the size of the base volume, which is a cube from -1 to 1 by default
#[derive(Copy, Clone)]
pub struct GiVolume {

	/// resolution of a cascade
	pub resolution: u8, 

	/// number of cascades
	pub cascades: u8,

}

#[derive(Copy, Clone, Bundle)]
pub struct GiVolumeBundle {

	pub transform: Transform,
	pub global_transform : GlobalTransform,
}
