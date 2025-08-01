use crate::voxel::{VoxelId, VoxelMaterial, ChunkPos, LocalVoxelPos, CHUNK_VOLUME, AIR_VOXEL, MAX_MATERIALS};
use crate::world::svo::SparseVoxelOctree;
use ahash::AHashMap;
use glam::Vec3;
use noise::{NoiseFn, Perlin};
use rayon::prelude::*;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct VoxelChunk {
    pub position: ChunkPos,
    pub octree: SparseVoxelOctree,
    pub is_dirty: bool,
    pub is_generated: bool,
}

impl VoxelChunk {
    pub fn new(position: ChunkPos) -> Self {
        Self {
            position,
            octree: SparseVoxelOctree::new(),
            is_dirty: true,
            is_generated: false,
        }
    }
    
    pub fn set_voxel(&mut self, local_pos: LocalVoxelPos, voxel_id: VoxelId) {
        self.octree.set_voxel(local_pos, voxel_id);
        self.is_dirty = true;
    }
    
    pub fn get_voxel(&self, local_pos: LocalVoxelPos) -> VoxelId {
        self.octree.get_voxel(local_pos)
    }
    
    pub fn generate_terrain(&mut self, noise: &Perlin, materials: &[VoxelMaterial]) {
        if self.is_generated {
            return;
        }
        
        let chunk_world_pos = self.position.to_world_pos();
        
        // Generate terrain using 3D noise
        for z in 0..32u32 {
            for y in 0..32u32 {
                for x in 0..32u32 {
                    let local_pos = LocalVoxelPos::new(x, y, z);
                    let world_pos = chunk_world_pos + local_pos.to_vec3() * crate::voxel::VOXEL_SIZE;
                    
                    // Use 3D Perlin noise for terrain generation
                    let density = noise.get([world_pos.x as f64 * 0.05, world_pos.y as f64 * 0.05, world_pos.z as f64 * 0.05]);
                    
                    // Create varied terrain with height-based materials
                    let voxel_id = if density > 0.0 {
                        // Choose material based on height and noise
                        let height_factor = world_pos.y / 100.0;
                        let material_noise = noise.get([world_pos.x as f64 * 0.1, world_pos.z as f64 * 0.1, 0.0]);
                        
                        let material_index = if height_factor < -0.5 {
                            1 // Stone-like material
                        } else if height_factor < 0.0 {
                            if material_noise > 0.3 { 2 } else { 3 } // Mixed materials
                        } else if height_factor < 0.5 {
                            4 // Grass-like material
                        } else {
                            5 // Mountain material
                        };
                        
                        VoxelId(material_index.min(materials.len() as u32 - 1))
                    } else {
                        AIR_VOXEL
                    };
                    
                    if voxel_id.is_solid() {
                        self.set_voxel(local_pos, voxel_id);
                    }
                }
            }
        }
        
        self.is_generated = true;
        self.is_dirty = true;
    }
    
    pub fn is_empty(&self) -> bool {
        self.octree.node_count() <= 1 && self.octree.get_nodes()[0].child_mask == 0
    }
}

pub struct VoxelWorld {
    chunks: AHashMap<ChunkPos, VoxelChunk>,
    materials: Vec<VoxelMaterial>,
    noise: Perlin,
    render_distance: i32,
}

impl VoxelWorld {
    pub fn new() -> Self {
        let mut materials = Vec::with_capacity(MAX_MATERIALS as usize);
        
        // Add default air material
        materials.push(VoxelMaterial::default());
        
        // Add some basic materials
        materials.push(VoxelMaterial {
            color: [0.5, 0.5, 0.5],
            roughness: 0.9,
            metallic: 0.0,
            emission: 0.0,
            _padding: [0.0; 2],
        }); // Stone
        
        materials.push(VoxelMaterial {
            color: [0.8, 0.6, 0.4],
            roughness: 0.8,
            metallic: 0.0,
            emission: 0.0,
            _padding: [0.0; 2],
        }); // Sand
        
        materials.push(VoxelMaterial {
            color: [0.6, 0.4, 0.2],
            roughness: 0.9,
            metallic: 0.0,
            emission: 0.0,
            _padding: [0.0; 2],
        }); // Dirt
        
        materials.push(VoxelMaterial {
            color: [0.3, 0.7, 0.2],
            roughness: 0.8,
            metallic: 0.0,
            emission: 0.0,
            _padding: [0.0; 2],
        }); // Grass
        
        materials.push(VoxelMaterial {
            color: [0.4, 0.4, 0.4],
            roughness: 0.7,
            metallic: 0.1,
            emission: 0.0,
            _padding: [0.0; 2],
        }); // Mountain rock
        
        Self {
            chunks: AHashMap::new(),
            materials,
            noise: Perlin::new(12345),
            render_distance: 8,
        }
    }
    
    pub fn set_render_distance(&mut self, distance: i32) {
        self.render_distance = distance;
    }
    
    pub fn update_around_player(&mut self, player_pos: Vec3) {
        let player_chunk = ChunkPos::from_world_pos(player_pos);
        
        // Generate chunks around player
        let mut chunks_to_generate: Vec<ChunkPos> = Vec::new();
        
        for x in -self.render_distance..=self.render_distance {
            for y in -self.render_distance..=self.render_distance {
                for z in -self.render_distance..=self.render_distance {
                    let chunk_pos = ChunkPos::new(
                        player_chunk.x + x,
                        player_chunk.y + y,
                        player_chunk.z + z,
                    );
                    
                    if !self.chunks.contains_key(&chunk_pos) {
                        chunks_to_generate.push(chunk_pos);
                    }
                }
            }
        }
        
        // Generate chunks in parallel
        let new_chunks: Vec<(ChunkPos, VoxelChunk)> = chunks_to_generate
            .par_iter()
            .map(|&pos| {
                let mut chunk = VoxelChunk::new(pos);
                chunk.generate_terrain(&self.noise, &self.materials);
                (pos, chunk)
            })
            .collect();
        
        // Add generated chunks
        for (pos, chunk) in new_chunks {
            self.chunks.insert(pos, chunk);
        }
        
        // Remove distant chunks
        let chunks_to_remove: Vec<ChunkPos> = self.chunks
            .keys()
            .filter(|&&pos| {
                let dx = (pos.x - player_chunk.x).abs();
                let dy = (pos.y - player_chunk.y).abs();
                let dz = (pos.z - player_chunk.z).abs();
                dx > self.render_distance || dy > self.render_distance || dz > self.render_distance
            })
            .copied()
            .collect();
        
        for pos in chunks_to_remove {
            self.chunks.remove(&pos);
        }
    }
    
    pub fn get_chunk(&self, pos: ChunkPos) -> Option<&VoxelChunk> {
        self.chunks.get(&pos)
    }
    
    pub fn get_chunk_mut(&mut self, pos: ChunkPos) -> Option<&mut VoxelChunk> {
        self.chunks.get_mut(&pos)
    }
    
    pub fn get_loaded_chunks(&self) -> impl Iterator<Item = (&ChunkPos, &VoxelChunk)> {
        self.chunks.iter()
    }
    
    pub fn get_materials(&self) -> &[VoxelMaterial] {
        &self.materials
    }
    
    pub fn chunk_count(&self) -> usize {
        self.chunks.len()
    }
    
    pub fn set_voxel(&mut self, world_pos: Vec3, voxel_id: VoxelId) {
        let (chunk_pos, local_pos) = crate::voxel::world_to_chunk_and_local(world_pos);
        
        if let Some(chunk) = self.chunks.get_mut(&chunk_pos) {
            chunk.set_voxel(local_pos, voxel_id);
        }
    }
    
    pub fn get_voxel(&self, world_pos: Vec3) -> VoxelId {
        let (chunk_pos, local_pos) = crate::voxel::world_to_chunk_and_local(world_pos);
        
        if let Some(chunk) = self.chunks.get(&chunk_pos) {
            chunk.get_voxel(local_pos)
        } else {
            AIR_VOXEL
        }
    }
}

impl Default for VoxelWorld {
    fn default() -> Self {
        Self::new()
    }
}