use crate::camera::Camera;
use crate::raytracing::ray::{CameraUniforms, RaytraceParams};
use crate::voxel::VoxelMaterial;
use crate::world::{VoxelWorld, OctreeNode};
use egui_wgpu::wgpu;
use glam::{Mat4, Vec3};
use std::borrow::Cow;

pub struct VoxelRenderer {
    compute_pipeline: wgpu::ComputePipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    bind_group: Option<wgpu::BindGroup>,
    
    // Uniforms
    camera_buffer: wgpu::Buffer,
    params_buffer: wgpu::Buffer,
    
    // Output texture
    output_texture: wgpu::Texture,
    output_view: wgpu::TextureView,
    
    // Render pipeline for displaying the raytraced result
    render_pipeline: wgpu::RenderPipeline,
    sampler: wgpu::Sampler,
    
    params: RaytraceParams,
}

impl VoxelRenderer {
    pub fn new(
        device: &wgpu::Device,
        surface_format: wgpu::TextureFormat,
        width: u32,
        height: u32,
    ) -> Self {
        // Load compute shader
        let compute_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Raytracing Compute Shader"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!("../shaders/raytracer.wgsl"))),
        });
        
        // Create bind group layout
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Raytracing Bind Group Layout"),
            entries: &[
                // Camera uniforms
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // Raytrace parameters
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // Output texture
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: wgpu::TextureFormat::Rgba8Unorm,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
            ],
        });
        
        // Create compute pipeline
        let compute_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Raytracing Compute Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });
        
        let compute_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Raytracing Compute Pipeline"),
            layout: Some(&compute_pipeline_layout),
            module: &compute_shader,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });
        
        // Create uniform buffers
        let camera_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Camera Buffer"),
            size: std::mem::size_of::<CameraUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        
        let params = RaytraceParams::new();
        let params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Raytrace Params Buffer"),
            size: std::mem::size_of::<RaytraceParams>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        
        
        // Create output texture
        let output_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Raytracing Output Texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        
        let output_view = output_texture.create_view(&wgpu::TextureViewDescriptor::default());
        
        // Create render pipeline for displaying the result
        let display_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Display Shader"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(
                r#"
                struct VertexOutput {
                    @builtin(position) clip_position: vec4<f32>,
                    @location(0) uv: vec2<f32>,
                }
                
                @vertex
                fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
                    var out: VertexOutput;
                    // Create a fullscreen quad with 6 vertices (2 triangles)
                    var positions = array<vec2<f32>, 6>(
                        vec2<f32>(-1.0, -1.0), // Bottom left
                        vec2<f32>(1.0, -1.0),  // Bottom right
                        vec2<f32>(-1.0, 1.0),  // Top left
                        vec2<f32>(-1.0, 1.0),  // Top left
                        vec2<f32>(1.0, -1.0),  // Bottom right
                        vec2<f32>(1.0, 1.0)    // Top right
                    );
                    
                    var uvs = array<vec2<f32>, 6>(
                        vec2<f32>(0.0, 1.0), // Bottom left
                        vec2<f32>(1.0, 1.0), // Bottom right
                        vec2<f32>(0.0, 0.0), // Top left
                        vec2<f32>(0.0, 0.0), // Top left
                        vec2<f32>(1.0, 1.0), // Bottom right
                        vec2<f32>(1.0, 0.0)  // Top right
                    );
                    
                    out.clip_position = vec4<f32>(positions[vertex_index], 0.0, 1.0);
                    out.uv = uvs[vertex_index];
                    return out;
                }
                
                @group(0) @binding(0) var t_diffuse: texture_2d<f32>;
                @group(0) @binding(1) var s_diffuse: sampler;
                
                @fragment
                fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
                    return textureSample(t_diffuse, s_diffuse, in.uv);
                }
                "#,
            )),
        });
        
        let render_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Display Pipeline Layout"),
            bind_group_layouts: &[&device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Display Bind Group Layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            multisampled: false,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            })],
            push_constant_ranges: &[],
        });
        
        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Display Render Pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &display_shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &display_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
            cache: None,
        });
        
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        
        Self {
            compute_pipeline,
            bind_group_layout,
            bind_group: None,
            camera_buffer,
            params_buffer,
            output_texture,
            output_view,
            render_pipeline,
            sampler,
            params,
        }
    }
    
    pub fn update_world_data(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, world: &VoxelWorld) {
        // Update params
        self.params.chunk_count = world.chunk_count() as u32;
        queue.write_buffer(&self.params_buffer, 0, bytemuck::cast_slice(&[self.params]));
        
        // Recreate bind group if needed
        if self.bind_group.is_none() {
            self.bind_group = Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Raytracing Bind Group"),
                layout: &self.bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: self.camera_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: self.params_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::TextureView(&self.output_view),
                    },
                ],
            }));
        }
    }
    
    pub fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target_view: &wgpu::TextureView,
        camera: &Camera,
        screen_width: u32,
        screen_height: u32,
    ) {
        // Update camera uniforms
        let view_matrix = camera.get_view_matrix();
        let projection_matrix = Mat4::perspective_rh(
            45.0_f32.to_radians(),
            screen_width as f32 / screen_height as f32,
            0.1,
            1000.0,
        );
        
        let camera_uniforms = CameraUniforms::new(
            camera.get_position(),
            view_matrix,
            projection_matrix,
        );
        
        queue.write_buffer(&self.camera_buffer, 0, bytemuck::cast_slice(&[camera_uniforms]));
        
        // Run compute shader
        if let Some(bind_group) = &self.bind_group {
            let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("Raytracing Compute Pass"),
                timestamp_writes: None,
            });
            
            compute_pass.set_pipeline(&self.compute_pipeline);
            compute_pass.set_bind_group(0, bind_group, &[]);
            
            let workgroup_size = 8;
            let dispatch_x = (screen_width + workgroup_size - 1) / workgroup_size;
            let dispatch_y = (screen_height + workgroup_size - 1) / workgroup_size;
            
            compute_pass.dispatch_workgroups(dispatch_x, dispatch_y, 1);
        }
        
        // Display the result
        let display_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Display Bind Group"),
            layout: &self.render_pipeline.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&self.output_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        });
        
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Display Render Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.1,
                        g: 0.2,
                        b: 0.3,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        
        render_pass.set_pipeline(&self.render_pipeline);
        render_pass.set_bind_group(0, &display_bind_group, &[]);
        render_pass.draw(0..6, 0..1);
    }
    
    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        self.output_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Raytracing Output Texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        
        self.output_view = self.output_texture.create_view(&wgpu::TextureViewDescriptor::default());
        
        // Force bind group recreation
        self.bind_group = None;
    }
}