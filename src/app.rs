use crate::camera::Camera;
use crate::egui_tools::EguiRenderer;
use crate::raytracing::VoxelRenderer;
use crate::world::VoxelWorld;
use egui_wgpu::{wgpu, ScreenDescriptor};
use std::sync::Arc;
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::{DeviceEvent, ElementState, MouseButton, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::window::{Window, WindowId};

pub struct AppState {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub surface_config: wgpu::SurfaceConfiguration,
    pub surface: wgpu::Surface<'static>,
    pub scale_factor: f32,
    pub egui_renderer: EguiRenderer,
    pub window: Arc<Window>,
    camera: Camera,
    pressed_keys: Vec<winit::keyboard::KeyCode>,
    mouse_delta: (f32, f32),
    cursor_locked: bool,
    voxel_world: VoxelWorld,
    voxel_renderer: VoxelRenderer,
}

impl AppState {
    async fn new(
        instance: &wgpu::Instance,
        surface: wgpu::Surface<'static>,
        window: Arc<Window>,
        width: u32,
        height: u32,
    ) -> Self {
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                force_fallback_adapter: false,
                compatible_surface: Some(&surface),
            })
            .await
            .expect("Failed to find an appropriate adapter");

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: None,
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                    memory_hints: wgpu::MemoryHints::default(),
                },
                None,
            )
            .await
            .expect("Failed to create device");

        let swapchain_capabilities = surface.get_capabilities(&adapter);
        let selected_format = wgpu::TextureFormat::Bgra8UnormSrgb;
        let swapchain_format = swapchain_capabilities
            .formats
            .iter()
            .find(|d| **d == selected_format)
            .expect("failed to select proper surface texture format!");

        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: *swapchain_format,
            width,
            height,
            present_mode: wgpu::PresentMode::AutoVsync,
            desired_maximum_frame_latency: 2,
            alpha_mode: swapchain_capabilities.alpha_modes[0],
            view_formats: vec![],
        };

        surface.configure(&device, &surface_config);

        let egui_renderer = EguiRenderer::new(&device, surface_config.format, None, 1, &window);
        let camera = Camera::new();
        
        let voxel_world = VoxelWorld::new();
        let voxel_renderer = VoxelRenderer::new(&device, surface_config.format, width, height);

        Self {
            device,
            queue,
            surface,
            surface_config,
            egui_renderer,
            scale_factor: 1.0,
            window,
            camera,
            pressed_keys: Vec::new(),
            mouse_delta: (0.0, 0.0),
            cursor_locked: false,
            voxel_world,
            voxel_renderer,
        }
    }

    fn resize_surface(&mut self, width: u32, height: u32) {
        self.surface_config.width = width;
        self.surface_config.height = height;
        self.surface.configure(&self.device, &self.surface_config);
        self.voxel_renderer.resize(&self.device, width, height);
    }

    fn render(&mut self) {
        let surface_texture = match self.surface.get_current_texture() {
            Ok(texture) => texture,
            Err(_) => return,
        };
        let view = surface_texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

        // Update voxel renderer with world data (simplified for now)
        self.voxel_renderer.update_world_data(&self.device, &self.queue, &self.voxel_world);
        
        // Render voxels using raytracing
        self.voxel_renderer.render(
            &self.device,
            &self.queue,
            &mut encoder,
            &view,
            &self.camera,
            self.surface_config.width,
            self.surface_config.height,
        );

        // Render the eGUI menu
        {
            let window = self.window.as_ref();
            let screen_descriptor = ScreenDescriptor {
                size_in_pixels: [self.surface_config.width, self.surface_config.height],
                pixels_per_point: window.scale_factor() as f32 * self.scale_factor,
            };

            self.egui_renderer.begin_frame(window);

            egui::Window::new("Voxel Engine Controls")
                .resizable(true)
                .vscroll(true)
                .default_open(true)
                .show(self.egui_renderer.context(), |ui| {
                    ui.label("Camera Controls");
                    if ui.button("Reset Camera").clicked() {
                        self.camera = Camera::new();
                    }
                    
                    ui.separator();
                    ui.label("Voxel World Info");
                    ui.label(format!("Loaded Chunks: {}", self.voxel_world.chunk_count()));
                    ui.label(format!("Camera Position: {:.1}, {:.1}, {:.1}", 
                        self.camera.get_position().x,
                        self.camera.get_position().y,
                        self.camera.get_position().z
                    ));
                    
                    ui.separator();
                    ui.label("Render Distance");
                    let mut render_distance = 8; // Default value
                    ui.add(egui::Slider::new(&mut render_distance, 1..=16).text("chunks"));
                    self.voxel_world.set_render_distance(render_distance);
                    
                    ui.separator();
                    ui.horizontal(|ui| {
                        ui.label(format!(
                            "Pixels per point: {}",
                            self.egui_renderer.context().pixels_per_point()
                        ));
                        if ui.button("-").clicked() {
                            self.scale_factor = (self.scale_factor - 0.1).max(0.3);
                        }
                        if ui.button("+").clicked() {
                            self.scale_factor = (self.scale_factor + 0.1).min(3.0);
                        }
                    });
                    
                    ui.separator();
                    ui.label("Controls:");
                    ui.label("WASD - Move camera");
                    ui.label("Space/Shift - Move up/down");
                    ui.label("Mouse - Look around (click to lock cursor)");
                    ui.label("Escape - Unlock cursor");
                });

            self.egui_renderer.end_frame_and_draw(
                &self.device,
                &self.queue,
                &mut encoder,
                window,
                &view,
                screen_descriptor,
            );
        }

        self.queue.submit(Some(encoder.finish()));
        surface_texture.present();
    }
}

pub struct App {
    instance: wgpu::Instance,
    state: Option<AppState>,
    window: Option<Arc<Window>>,
}

impl App {
    pub fn new() -> Self {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
        Self {
            instance,
            state: None,
            window: None,
        }
    }

    async fn set_window(&mut self, window: Window) {
        let window = Arc::new(window);
        let initial_width = 1360;
        let initial_height = 768;

        let _ = window.request_inner_size(PhysicalSize::new(initial_width, initial_height));

        let surface = self
            .instance
            .create_surface(window.clone())
            .expect("Failed to create surface!");

        let state = AppState::new(
            &self.instance,
            surface,
            window.clone(),
            initial_width,
            initial_height,
        )
        .await;

        self.window = Some(window);
        self.state = Some(state);
    }

    fn handle_resized(&mut self, width: u32, height: u32) {
        if width > 0 && height > 0 {
            self.state.as_mut().unwrap().resize_surface(width, height);
        }
    }

    fn handle_redraw(&mut self) {
        let state = self.state.as_mut().unwrap();
        state.camera.handle_input(&state.pressed_keys);

        if state.cursor_locked && state.mouse_delta != (0.0, 0.0) {
            state
                .camera
                .handle_mouse(&(state.mouse_delta.0 as f64, state.mouse_delta.1 as f64));
            state.mouse_delta = (0.0, 0.0); 
        }

        state.render();
        self.window.as_ref().unwrap().request_redraw();
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window = event_loop
            .create_window(Window::default_attributes())
            .unwrap();
        pollster::block_on(self.set_window(window));
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        let state = self.state.as_mut().unwrap();
        state
            .egui_renderer
            .handle_input(self.window.as_ref().unwrap(), &event);

        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::RedrawRequested => {
                self.handle_redraw();
            }
            WindowEvent::Resized(new_size) => {
                self.handle_resized(new_size.width, new_size.height);
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => {
                if !state.cursor_locked {
                    state.cursor_locked = true;
                    self.window
                        .as_ref()
                        .unwrap()
                        .set_cursor_grab(winit::window::CursorGrabMode::Locked)
                        .unwrap_or_else(|_| {
                            self.window
                                .as_ref()
                                .unwrap()
                                .set_cursor_grab(winit::window::CursorGrabMode::Confined)
                                .unwrap();
                        });
                    self.window.as_ref().unwrap().set_cursor_visible(false);
                }
            }
            WindowEvent::KeyboardInput {
                event:
                    winit::event::KeyEvent {
                        physical_key: winit::keyboard::PhysicalKey::Code(keycode),
                        state: key_state,
                        ..
                    },
                ..
            } => {
                if keycode == winit::keyboard::KeyCode::Escape && key_state == ElementState::Pressed
                {
                    if state.cursor_locked {
                        state.cursor_locked = false;
                        self.window
                            .as_ref()
                            .unwrap()
                            .set_cursor_grab(winit::window::CursorGrabMode::None)
                            .unwrap();
                        self.window.as_ref().unwrap().set_cursor_visible(true);
                    }
                }

                match key_state {
                    ElementState::Pressed => {
                        if !state.pressed_keys.contains(&keycode) {
                            state.pressed_keys.push(keycode);
                        }
                    }
                    ElementState::Released => {
                        state.pressed_keys.retain(|&k| k != keycode);
                    }
                }
            }
            _ => (),
        }
    }

    fn device_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        _device_id: winit::event::DeviceId,
        event: DeviceEvent,
    ) {
        if let Some(state) = self.state.as_mut() {
            match event {
                DeviceEvent::MouseMotion { delta } => {
                    if state.cursor_locked {
                        state.mouse_delta = (delta.0 as f32, delta.1 as f32);
                    }
                }
                _ => (),
            }
        }
    }
}
