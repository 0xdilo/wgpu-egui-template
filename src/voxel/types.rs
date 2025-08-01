use bytemuck::{Pod, Zeroable};
use glam::{IVec3, Vec3};

pub const VOXEL_SIZE: f32 = 0.125; // 1/8 of minecraft block
pub const CHUNK_SIZE: u32 = 32; // 32x32x32 voxels per chunk
pub const CHUNK_SIZE_F32: f32 = CHUNK_SIZE as f32;
pub const CHUNK_VOLUME: usize = (CHUNK_SIZE * CHUNK_SIZE * CHUNK_SIZE) as usize;

pub const MAX_MATERIALS: u32 = 255;
pub const AIR_VOXEL: VoxelId = VoxelId(0);

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Pod, Zeroable)]
pub struct VoxelId(pub u32);

impl VoxelId {
    pub fn is_air(self) -> bool {
        self.0 == 0
    }
    
    pub fn is_solid(self) -> bool {
        self.0 != 0
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct VoxelMaterial {
    pub color: [f32; 3],
    pub roughness: f32,
    pub metallic: f32,
    pub emission: f32,
    pub _padding: [f32; 2], // Align to 32 bytes
}

impl Default for VoxelMaterial {
    fn default() -> Self {
        Self {
            color: [0.5, 0.5, 0.5],
            roughness: 0.8,
            metallic: 0.0,
            emission: 0.0,
            _padding: [0.0; 2],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ChunkPos {
    pub x: i32,
    pub y: i32, 
    pub z: i32,
}

impl ChunkPos {
    pub fn new(x: i32, y: i32, z: i32) -> Self {
        Self { x, y, z }
    }
    
    pub fn from_world_pos(world_pos: Vec3) -> Self {
        let chunk_x = (world_pos.x / (CHUNK_SIZE_F32 * VOXEL_SIZE)).floor() as i32;
        let chunk_y = (world_pos.y / (CHUNK_SIZE_F32 * VOXEL_SIZE)).floor() as i32;
        let chunk_z = (world_pos.z / (CHUNK_SIZE_F32 * VOXEL_SIZE)).floor() as i32;
        Self::new(chunk_x, chunk_y, chunk_z)
    }
    
    pub fn to_world_pos(self) -> Vec3 {
        Vec3::new(
            self.x as f32 * CHUNK_SIZE_F32 * VOXEL_SIZE,
            self.y as f32 * CHUNK_SIZE_F32 * VOXEL_SIZE,
            self.z as f32 * CHUNK_SIZE_F32 * VOXEL_SIZE,
        )
    }
}

impl From<IVec3> for ChunkPos {
    fn from(pos: IVec3) -> Self {
        Self::new(pos.x, pos.y, pos.z)
    }
}

impl From<ChunkPos> for IVec3 {
    fn from(pos: ChunkPos) -> Self {
        IVec3::new(pos.x, pos.y, pos.z)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LocalVoxelPos {
    pub x: u32,
    pub y: u32,
    pub z: u32,
}

impl LocalVoxelPos {
    pub fn new(x: u32, y: u32, z: u32) -> Self {
        debug_assert!(x < CHUNK_SIZE);
        debug_assert!(y < CHUNK_SIZE);
        debug_assert!(z < CHUNK_SIZE);
        Self { x, y, z }
    }
    
    pub fn to_index(self) -> usize {
        (self.z as usize * CHUNK_SIZE as usize * CHUNK_SIZE as usize) +
        (self.y as usize * CHUNK_SIZE as usize) +
        (self.x as usize)
    }
    
    pub fn from_index(index: usize) -> Self {
        let z = (index / (CHUNK_SIZE * CHUNK_SIZE) as usize) as u32;
        let y = ((index % (CHUNK_SIZE * CHUNK_SIZE) as usize) / CHUNK_SIZE as usize) as u32;
        let x = (index % CHUNK_SIZE as usize) as u32;
        Self::new(x, y, z)
    }
    
    pub fn to_vec3(self) -> Vec3 {
        Vec3::new(self.x as f32, self.y as f32, self.z as f32)
    }
}

pub fn world_to_chunk_and_local(world_pos: Vec3) -> (ChunkPos, LocalVoxelPos) {
    let chunk_pos = ChunkPos::from_world_pos(world_pos);
    let chunk_world_pos = chunk_pos.to_world_pos();
    
    let local_pos = (world_pos - chunk_world_pos) / VOXEL_SIZE;
    let local_voxel = LocalVoxelPos::new(
        (local_pos.x.max(0.0) as u32).min(CHUNK_SIZE - 1),
        (local_pos.y.max(0.0) as u32).min(CHUNK_SIZE - 1),
        (local_pos.z.max(0.0) as u32).min(CHUNK_SIZE - 1),
    );
    
    (chunk_pos, local_voxel)
}