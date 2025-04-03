use glam::{Mat4, Vec3};

pub struct Camera {
    position: Vec3,
    yaw: f32,
    pitch: f32,
    speed: f32,
    sensitivity: f32,
}

impl Camera {
    pub fn new() -> Self {
        Self {
            position: Vec3::new(0.0, 5.0, 0.0),
            yaw: -90.0,
            pitch: 0.0,
            speed: 1.1,
            sensitivity: 1.0,
        }
    }

    pub fn get_view_matrix(&self) -> Mat4 {
        let direction = self.get_direction();
        Mat4::look_at_rh(self.position, self.position + direction, Vec3::Y)
    }

    pub fn get_direction(&self) -> Vec3 {
        Vec3::new(
            self.yaw.to_radians().cos() * self.pitch.to_radians().cos(),
            self.pitch.to_radians().sin(),
            self.yaw.to_radians().sin() * self.pitch.to_radians().cos(),
        )
        .normalize()
    }

    pub fn handle_mouse(&mut self, delta: &(f64, f64)) {
        self.yaw += delta.0 as f32 * self.sensitivity;
        self.pitch -= delta.1 as f32 * self.sensitivity;
        self.pitch = self.pitch.clamp(-89.0, 89.0); 
    }

    pub fn handle_input(&mut self, keys: &[winit::keyboard::KeyCode]) {
        let direction = self.get_direction();
        let right = direction.cross(Vec3::Y).normalize();

        for key in keys {
            match key {
                winit::keyboard::KeyCode::KeyW => self.position += direction * self.speed,
                winit::keyboard::KeyCode::KeyS => self.position -= direction * self.speed,
                winit::keyboard::KeyCode::KeyA => self.position -= right * self.speed,
                winit::keyboard::KeyCode::KeyD => self.position += right * self.speed,
                winit::keyboard::KeyCode::Space => self.position.y += self.speed,
                winit::keyboard::KeyCode::ShiftLeft => self.position.y -= self.speed,
                _ => {}
            }
        }
    }

    pub fn get_position(&self) -> Vec3 {
        self.position
    }
}
