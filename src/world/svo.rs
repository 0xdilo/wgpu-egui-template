use crate::voxel::{VoxelId, AIR_VOXEL, CHUNK_SIZE, LocalVoxelPos};
use bytemuck::{Pod, Zeroable};
use glam::{Vec3, IVec3};

pub const MAX_OCTREE_DEPTH: u32 = 5; // 32 = 2^5, so max depth is 5
pub const OCTREE_NODE_POOL_SIZE: usize = 8192;

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct OctreeNode {
    pub child_mask: u32,    // Bitmask indicating which children exist  
    pub leaf_mask: u32,     // Bitmask indicating which children are leaves
    pub child_ptr: u32,     // Index to first child in node pool
    pub voxel_id: u32,      // Voxel ID if this is a leaf
}

impl OctreeNode {
    pub fn new_empty() -> Self {
        Self {
            child_mask: 0,
            leaf_mask: 0,
            child_ptr: 0,
            voxel_id: AIR_VOXEL.0,
        }
    }
    
    pub fn new_leaf(voxel_id: VoxelId) -> Self {
        Self {
            child_mask: 0,
            leaf_mask: 0,
            child_ptr: 0,
            voxel_id: voxel_id.0,
        }
    }
    
    pub fn has_child(&self, child_index: u32) -> bool {
        debug_assert!(child_index < 8);
        (self.child_mask & (1 << child_index)) != 0
    }
    
    pub fn is_leaf(&self, child_index: u32) -> bool {
        debug_assert!(child_index < 8);
        (self.leaf_mask & (1 << child_index)) != 0
    }
    
    pub fn set_child(&mut self, child_index: u32, is_leaf: bool) {
        debug_assert!(child_index < 8);
        let mask = 1 << child_index;
        self.child_mask |= mask;
        if is_leaf {
            self.leaf_mask |= mask;
        } else {
            self.leaf_mask &= !mask;
        }
    }
    
    pub fn remove_child(&mut self, child_index: u32) {
        debug_assert!(child_index < 8);
        let mask = !(1 << child_index);
        self.child_mask &= mask;
        self.leaf_mask &= mask;
    }
}

#[derive(Debug, Clone)]
pub struct SparseVoxelOctree {
    nodes: Vec<OctreeNode>,
    free_indices: Vec<u32>,
    root_size: u32, // Size of the root node (should be CHUNK_SIZE)
}

impl SparseVoxelOctree {
    pub fn new() -> Self {
        let mut nodes = Vec::with_capacity(OCTREE_NODE_POOL_SIZE);
        nodes.push(OctreeNode::new_empty()); // Root node
        
        Self {
            nodes,
            free_indices: Vec::new(),
            root_size: CHUNK_SIZE,
        }
    }
    
    pub fn clear(&mut self) {
        self.nodes.clear();
        self.nodes.push(OctreeNode::new_empty());
        self.free_indices.clear();
    }
    
    fn allocate_node(&mut self) -> u32 {
        if let Some(index) = self.free_indices.pop() {
            self.nodes[index as usize] = OctreeNode::new_empty();
            index
        } else {
            let index = self.nodes.len() as u32;
            self.nodes.push(OctreeNode::new_empty());
            index
        }
    }
    
    fn deallocate_node(&mut self, index: u32) {
        if index > 0 && (index as usize) < self.nodes.len() {
            self.nodes[index as usize] = OctreeNode::new_empty();
            self.free_indices.push(index);
        }
    }
    
    pub fn set_voxel(&mut self, pos: LocalVoxelPos, voxel_id: VoxelId) {
        let mut node_index = 0u32; // Root node
        let mut node_size = self.root_size;
        let mut node_pos = IVec3::ZERO;
        
        let target_pos = IVec3::new(pos.x as i32, pos.y as i32, pos.z as i32);
        
        // Traverse down to the target depth
        while node_size > 1 {
            let half_size = node_size / 2;
            
            // Determine which child octant the target position is in
            let child_offset = IVec3::new(
                if target_pos.x >= node_pos.x + half_size as i32 { 1 } else { 0 },
                if target_pos.y >= node_pos.y + half_size as i32 { 1 } else { 0 },
                if target_pos.z >= node_pos.z + half_size as i32 { 1 } else { 0 },
            );
            
            let child_index = (child_offset.x + child_offset.y * 2 + child_offset.z * 4) as u32;
            
            // Update node position for next iteration
            node_pos += child_offset * half_size as i32;
            
            let needs_new_child = !self.nodes[node_index as usize].has_child(child_index);
            
            if needs_new_child {
                if voxel_id.is_air() {
                    // No need to create nodes for air voxels
                    return;
                }
                
                // Create new child
                if node_size == 2 {
                    // Next level would be leaf
                    self.nodes[node_index as usize].set_child(child_index, true);
                } else {
                    // Create internal node
                    let new_node_index = self.allocate_node();
                    self.nodes[node_index as usize].set_child(child_index, false);
                    
                    // Set child pointer if this is the first child
                    if self.nodes[node_index as usize].child_mask.count_ones() == 1 {
                        self.nodes[node_index as usize].child_ptr = new_node_index;
                    }
                }
            }
            
            if node_size == 2 {
                // Next level is leaf level
                break;
            }
            
            // Move to child node
            let child_ptr = self.nodes[node_index as usize].child_ptr;
            let child_offset_in_array = (self.nodes[node_index as usize].child_mask & ((1 << child_index) - 1)).count_ones();
            node_index = child_ptr + child_offset_in_array;
            node_size = half_size;
        }
        
        // Set the voxel in the leaf
        if node_size == 1 {
            self.nodes[node_index as usize].voxel_id = voxel_id.0;
        }
    }
    
    pub fn get_voxel(&self, pos: LocalVoxelPos) -> VoxelId {
        let mut node_index = 0u32; // Root node
        let mut node_size = self.root_size;
        let mut node_pos = IVec3::ZERO;
        
        let target_pos = IVec3::new(pos.x as i32, pos.y as i32, pos.z as i32);
        
        // Traverse down to the target position
        while node_size > 1 {
            let half_size = node_size / 2;
            
            let child_offset = IVec3::new(
                if target_pos.x >= node_pos.x + half_size as i32 { 1 } else { 0 },
                if target_pos.y >= node_pos.y + half_size as i32 { 1 } else { 0 },
                if target_pos.z >= node_pos.z + half_size as i32 { 1 } else { 0 },
            );
            
            let child_index = (child_offset.x + child_offset.y * 2 + child_offset.z * 4) as u32;
            node_pos += child_offset * half_size as i32;
            
            let current_node = &self.nodes[node_index as usize];
            
            if !current_node.has_child(child_index) {
                // No child means air
                return AIR_VOXEL;
            }
            
            if current_node.is_leaf(child_index) {
                // This child is a leaf, return its voxel
                return VoxelId(current_node.voxel_id);
            }
            
            // Move to child node
            let child_ptr = current_node.child_ptr;
            let child_offset_in_array = (current_node.child_mask & ((1 << child_index) - 1)).count_ones();
            node_index = child_ptr + child_offset_in_array;
            node_size = half_size;
        }
        
        // If we get here, we're at a leaf node
        VoxelId(self.nodes[node_index as usize].voxel_id)
    }
    
    pub fn get_nodes(&self) -> &[OctreeNode] {
        &self.nodes
    }
    
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }
}

impl Default for SparseVoxelOctree {
    fn default() -> Self {
        Self::new()
    }
}