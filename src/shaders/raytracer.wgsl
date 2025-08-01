struct Camera {
    view_proj: mat4x4<f32>,
    view_inv: mat4x4<f32>,
    proj_inv: mat4x4<f32>,
    position: vec3<f32>,
    _padding1: f32,
}

struct RaytraceParams {
    chunk_count: u32,
    max_bounces: u32,
    sun_direction: vec3<f32>,
    _padding1: f32,
    sun_color: vec3<f32>,
    _padding2: f32,
    ambient_color: vec3<f32>,
    _padding3: f32,
}

@group(0) @binding(0) var<uniform> camera: Camera;
@group(0) @binding(1) var<uniform> params: RaytraceParams;
@group(0) @binding(2) var output_texture: texture_storage_2d<rgba8unorm, write>;

const VOXEL_SIZE: f32 = 0.125;
const CHUNK_SIZE: u32 = 32u;

fn create_ray_direction(screen_pos: vec2<f32>, screen_size: vec2<f32>) -> vec3<f32> {
    let ndc = (screen_pos / screen_size) * 2.0 - 1.0;
    let ndc_with_depth = vec4<f32>(ndc.x, -ndc.y, 1.0, 1.0);
    
    let world_pos = camera.proj_inv * ndc_with_depth;
    let world_pos_normalized = world_pos.xyz / world_pos.w;
    
    let ray_direction = normalize((camera.view_inv * vec4<f32>(world_pos_normalized, 0.0)).xyz);
    
    return ray_direction;
}

fn simple_voxel_test(ray_origin: vec3<f32>, ray_direction: vec3<f32>) -> vec3<f32> {
    let t_max = 200.0;
    let step_size = VOXEL_SIZE * 0.5;
    
    var t = 0.1;
    while t < t_max {
        let pos = ray_origin + ray_direction * t;
        
        // Create a more complex voxel world
        let voxel_pos = floor(pos / VOXEL_SIZE);
        
        // Create a procedural voxel landscape
        let height = sin(voxel_pos.x * 0.05) * 10.0 + cos(voxel_pos.z * 0.07) * 8.0;
        let noise = sin(voxel_pos.x * 0.3) * cos(voxel_pos.z * 0.4) * 3.0;
        let terrain_height = height + noise;
        
        // Check if we're in a solid voxel
        var is_solid = false;
        var material_type = 0;
        
        if voxel_pos.y <= terrain_height {
            is_solid = true;
            // Different materials based on height
            if voxel_pos.y > terrain_height - 2.0 {
                material_type = 1; // Grass
            } else if voxel_pos.y > terrain_height - 5.0 {
                material_type = 2; // Dirt  
            } else {
                material_type = 3; // Stone
            }
        }
        
        // Add some scattered structures
        let structure_noise = sin(voxel_pos.x * 0.1) * cos(voxel_pos.z * 0.1);
        if structure_noise > 0.7 && voxel_pos.y > terrain_height && voxel_pos.y < terrain_height + 10.0 {
            is_solid = true;
            material_type = 4; // Structure material
        }
        
        if is_solid {
            // Calculate lighting based on normal approximation
            let light_dir = normalize(vec3<f32>(-0.5, -0.8, -0.3));
            let sample_offset = VOXEL_SIZE * 2.0;
            
            // Simple normal estimation by sampling neighboring positions
            let height_x = sin((voxel_pos.x + 1.0) * 0.05) * 10.0 + cos(voxel_pos.z * 0.07) * 8.0;
            let height_z = sin(voxel_pos.x * 0.05) * 10.0 + cos((voxel_pos.z + 1.0) * 0.07) * 8.0;
            let normal = normalize(vec3<f32>(terrain_height - height_x, 1.0, terrain_height - height_z));
            
            let light_intensity = max(0.2, dot(normal, -light_dir));
            
            // Material colors
            var base_color = vec3<f32>(0.5, 0.5, 0.5);
            switch material_type {
                case 1: { base_color = vec3<f32>(0.3, 0.7, 0.2); } // Grass
                case 2: { base_color = vec3<f32>(0.6, 0.4, 0.2); } // Dirt
                case 3: { base_color = vec3<f32>(0.5, 0.5, 0.5); } // Stone
                case 4: { base_color = vec3<f32>(0.8, 0.6, 0.4); } // Structure
                default: { base_color = vec3<f32>(0.5, 0.5, 0.5); }
            }
            
            return base_color * light_intensity;
        }
        
        t += step_size;
    }
    
    // Sky gradient
    let sky_t = max(0.0, ray_direction.y);
    return mix(vec3<f32>(0.6, 0.8, 1.0), vec3<f32>(0.1, 0.3, 0.8), 1.0 - sky_t);
}

@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let screen_size = textureDimensions(output_texture);
    let screen_pos = vec2<f32>(global_id.xy);
    
    if global_id.x >= screen_size.x || global_id.y >= screen_size.y {
        return;
    }
    
    let ray_direction = create_ray_direction(screen_pos, vec2<f32>(screen_size));
    let color = simple_voxel_test(camera.position, ray_direction);
    
    textureStore(output_texture, vec2<i32>(global_id.xy), vec4<f32>(color, 1.0));
}