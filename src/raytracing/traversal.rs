use crate::raytracing::ray::{Ray, RayHit, ray_aabb_intersect};
use crate::voxel::{ChunkPos, LocalVoxelPos, CHUNK_SIZE, VOXEL_SIZE, AIR_VOXEL};
use crate::world::{VoxelChunk, SparseVoxelOctree};
use glam::{Vec3, IVec3};

pub const MAX_RAY_STEPS: u32 = 1000;
pub const MIN_DISTANCE: f32 = 0.001;
pub const MAX_DISTANCE: f32 = 1000.0;

pub struct ChunkRaytracer;

impl ChunkRaytracer {
    pub fn trace_chunk(ray: &Ray, chunk: &VoxelChunk) -> RayHit {
        let chunk_world_pos = chunk.position.to_world_pos();
        let chunk_size = CHUNK_SIZE as f32 * VOXEL_SIZE;
        let chunk_min = chunk_world_pos;
        let chunk_max = chunk_world_pos + Vec3::splat(chunk_size);
        
        // Check if ray intersects chunk bounds
        if let Some((t_near, t_far)) = ray_aabb_intersect(ray, chunk_min, chunk_max) {
            if t_far > 0.0 {
                return Self::dda_traverse(ray, chunk, chunk_min, t_near.max(0.001));
            }
        }
        
        RayHit::new_miss()
    }
    
    fn dda_traverse(ray: &Ray, chunk: &VoxelChunk, chunk_min: Vec3, t_start: f32) -> RayHit {
        let entry_point = ray.at(t_start);
        let local_pos = (entry_point - chunk_min) / VOXEL_SIZE;
        
        // Current voxel position (integer coordinates)
        let mut current_pos = local_pos.floor();
        
        // Ray direction signs and absolute values
        let direction = ray.direction_vec3();
        let step = direction.signum();
        let delta = (1.0 / direction).abs();
        
        // Distance to next voxel boundary along each axis
        let mut side_dist = (step * (current_pos - local_pos) + (step * 0.5) + 0.5) * delta;
        
        for _ in 0..MAX_RAY_STEPS {
            // Check bounds
            if current_pos.x < 0.0 || current_pos.x >= CHUNK_SIZE as f32 ||
               current_pos.y < 0.0 || current_pos.y >= CHUNK_SIZE as f32 ||
               current_pos.z < 0.0 || current_pos.z >= CHUNK_SIZE as f32 {
                break;
            }
            
            // Get voxel at current position
            let local_voxel_pos = LocalVoxelPos::new(
                current_pos.x as u32,
                current_pos.y as u32,
                current_pos.z as u32,
            );
            
            let voxel_id = chunk.get_voxel(local_voxel_pos);
            
            if voxel_id.is_solid() {
                // Calculate hit information
                let voxel_world_pos = chunk_min + (current_pos + Vec3::splat(0.5)) * VOXEL_SIZE;
                let distance = (voxel_world_pos - ray.origin_vec3()).length();
                
                // Calculate normal based on which face was hit
                let normal = if side_dist.x - delta.x < side_dist.y - delta.y && 
                               side_dist.x - delta.x < side_dist.z - delta.z {
                    Vec3::new(-step.x, 0.0, 0.0)
                } else if side_dist.y - delta.y < side_dist.z - delta.z {
                    Vec3::new(0.0, -step.y, 0.0)
                } else {
                    Vec3::new(0.0, 0.0, -step.z)
                };
                
                return RayHit::new_hit(voxel_world_pos, normal, voxel_id.0, distance);
            }
            
            // Step to next voxel
            if side_dist.x < side_dist.y && side_dist.x < side_dist.z {
                side_dist.x += delta.x;
                current_pos.x += step.x;
            } else if side_dist.y < side_dist.z {
                side_dist.y += delta.y;
                current_pos.y += step.y;
            } else {
                side_dist.z += delta.z;
                current_pos.z += step.z;
            }
        }
        
        RayHit::new_miss()
    }
}

pub struct OctreeRaytracer;

impl OctreeRaytracer {
    pub fn trace_octree(ray: &Ray, octree: &SparseVoxelOctree, chunk_min: Vec3) -> RayHit {
        let chunk_size = CHUNK_SIZE as f32 * VOXEL_SIZE;
        let chunk_max = chunk_min + Vec3::splat(chunk_size);
        
        if let Some((t_near, _t_far)) = ray_aabb_intersect(ray, chunk_min, chunk_max) {
            if t_near >= 0.0 {
                return Self::traverse_node(ray, octree, 0, chunk_min, chunk_size, t_near.max(0.001));
            }
        }
        
        RayHit::new_miss()
    }
    
    fn traverse_node(
        ray: &Ray, 
        octree: &SparseVoxelOctree, 
        node_index: usize, 
        node_min: Vec3, 
        node_size: f32, 
        t_start: f32
    ) -> RayHit {
        if node_index >= octree.node_count() {
            return RayHit::new_miss();
        }
        
        let node = &octree.get_nodes()[node_index];
        
        // If this is a leaf node (size == VOXEL_SIZE), check for solid voxel
        if node_size <= VOXEL_SIZE {
            if node.voxel_id != AIR_VOXEL.0 {
                let hit_pos = ray.at(t_start);
                let distance = (hit_pos - ray.origin_vec3()).length();
                
                // Simple normal calculation (could be improved)
                let voxel_center = node_min + Vec3::splat(node_size * 0.5);
                let to_center = (hit_pos - voxel_center).normalize();
                let normal = if to_center.x.abs() > to_center.y.abs() && to_center.x.abs() > to_center.z.abs() {
                    Vec3::new(to_center.x.signum(), 0.0, 0.0)
                } else if to_center.y.abs() > to_center.z.abs() {
                    Vec3::new(0.0, to_center.y.signum(), 0.0)
                } else {
                    Vec3::new(0.0, 0.0, to_center.z.signum())
                };
                
                return RayHit::new_hit(hit_pos, normal, node.voxel_id, distance);
            } else {
                return RayHit::new_miss();
            }
        }
        
        // Internal node - traverse children
        let half_size = node_size * 0.5;
        let mut closest_hit = RayHit::new_miss();
        
        // Check all children that exist
        for child_idx in 0..8 {
            if (node.child_mask & (1 << child_idx)) != 0 {
                // Calculate child bounds
                let child_offset = Vec3::new(
                    if (child_idx & 1) != 0 { half_size } else { 0.0 },
                    if (child_idx & 2) != 0 { half_size } else { 0.0 },
                    if (child_idx & 4) != 0 { half_size } else { 0.0 },
                );
                
                let child_min = node_min + child_offset;
                let child_max = child_min + Vec3::splat(half_size);
                
                // Check if ray intersects child
                if let Some((child_t_near, _child_t_far)) = ray_aabb_intersect(ray, child_min, child_max) {
                    if child_t_near < closest_hit.distance {
                        let child_node_index = if (node.leaf_mask & (1 << child_idx)) != 0 {
                            // This child is a leaf, represented by the current node
                            node_index
                        } else {
                            // Calculate child node index
                            let children_before = (node.child_mask & ((1 << child_idx) - 1)).count_ones();
                            node.child_ptr as usize + children_before as usize
                        };
                        
                        let hit = Self::traverse_node(
                            ray, 
                            octree, 
                            child_node_index, 
                            child_min, 
                            half_size, 
                            child_t_near.max(0.001)
                        );
                        
                        if hit.hit && hit.distance < closest_hit.distance {
                            closest_hit = hit;
                        }
                    }
                }
            }
        }
        
        closest_hit
    }
}

pub fn get_child_index(pos: Vec3, center: Vec3) -> u8 {
    let mut index = 0u8;
    if pos.x >= center.x { index |= 1; }
    if pos.y >= center.y { index |= 2; }
    if pos.z >= center.z { index |= 4; }
    index
}