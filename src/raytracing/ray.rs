use bytemuck::{Pod, Zeroable};
use glam::{Vec3, Mat4};

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct Ray {
    pub origin: [f32; 3],
    pub direction: [f32; 3],
}

impl Ray {
    pub fn new(origin: Vec3, direction: Vec3) -> Self {
        let normalized = direction.normalize();
        Self {
            origin: [origin.x, origin.y, origin.z],
            direction: [normalized.x, normalized.y, normalized.z],
        }
    }
    
    pub fn at(&self, t: f32) -> Vec3 {
        Vec3::from_array(self.origin) + Vec3::from_array(self.direction) * t
    }
    
    pub fn origin_vec3(&self) -> Vec3 {
        Vec3::from_array(self.origin)
    }
    
    pub fn direction_vec3(&self) -> Vec3 {
        Vec3::from_array(self.direction)
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct CameraUniforms {
    pub view_proj: [[f32; 4]; 4],
    pub view_inv: [[f32; 4]; 4],
    pub proj_inv: [[f32; 4]; 4],
    pub position: [f32; 3],
    pub _padding1: f32,
}

impl CameraUniforms {
    pub fn new(position: Vec3, view_matrix: Mat4, projection_matrix: Mat4) -> Self {
        let view_proj = projection_matrix * view_matrix;
        
        Self {
            view_proj: view_proj.to_cols_array_2d(),
            view_inv: view_matrix.inverse().to_cols_array_2d(),
            proj_inv: projection_matrix.inverse().to_cols_array_2d(),
            position: [position.x, position.y, position.z],
            _padding1: 0.0,
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct RaytraceParams {
    pub chunk_count: u32,
    pub max_bounces: u32,
    pub sun_direction: [f32; 3],
    pub _padding1: f32,
    pub sun_color: [f32; 3],
    pub _padding2: f32,
    pub ambient_color: [f32; 3],
    pub _padding3: f32,
}

impl RaytraceParams {
    pub fn new() -> Self {
        let sun_direction = Vec3::new(-0.5, -0.8, -0.3).normalize();
        Self {
            chunk_count: 1,
            max_bounces: 3,
            sun_direction: [sun_direction.x, sun_direction.y, sun_direction.z],
            _padding1: 0.0,
            sun_color: [1.0, 0.9, 0.8],
            _padding2: 0.0,
            ambient_color: [0.1, 0.15, 0.2],
            _padding3: 0.0,
        }
    }
}

impl Default for RaytraceParams {
    fn default() -> Self {
        Self::new()
    }
}

pub struct RayHit {
    pub hit: bool,
    pub position: Vec3,
    pub normal: Vec3,
    pub material_id: u32,
    pub distance: f32,
}

impl RayHit {
    pub fn new_miss() -> Self {
        Self {
            hit: false,
            position: Vec3::ZERO,
            normal: Vec3::Y,
            material_id: 0,
            distance: f32::INFINITY,
        }
    }
    
    pub fn new_hit(position: Vec3, normal: Vec3, material_id: u32, distance: f32) -> Self {
        Self {
            hit: true,
            position,
            normal,
            material_id,
            distance,
        }
    }
}

pub fn ray_aabb_intersect(ray: &Ray, min: Vec3, max: Vec3) -> Option<(f32, f32)> {
    let origin = ray.origin_vec3();
    let direction = ray.direction_vec3();
    let inv_dir = 1.0 / direction;
    let t1 = (min - origin) * inv_dir;
    let t2 = (max - origin) * inv_dir;
    
    let tmin = t1.min(t2);
    let tmax = t1.max(t2);
    
    let t_near = tmin.x.max(tmin.y).max(tmin.z).max(0.0);
    let t_far = tmax.x.min(tmax.y).min(tmax.z);
    
    if t_near <= t_far && t_far > 0.0 {
        Some((t_near, t_far))
    } else {
        None
    }
}