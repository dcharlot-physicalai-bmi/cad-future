//! Orbit camera for 3D CAD viewport navigation.
//!
//! Ported from game-studio's orbit_camera. Provides view/projection matrices
//! for a camera that orbits around a target point.

use glam::{Mat4, Vec3};
use std::f32::consts::FRAC_PI_4;

pub struct OrbitCamera {
    pub target: Vec3,
    pub distance: f32,
    pub yaw: f32,
    pub pitch: f32,
    pub near: f32,
    pub far: f32,
    pub fov: f32,
    pub orthographic: bool,

    // Smooth transition state
    target_yaw: f32,
    target_pitch: f32,
    target_distance: f32,
    target_target: Vec3,
    animating: bool,
    anim_speed: f32,
}

impl OrbitCamera {
    pub fn new(target: Vec3, distance: f32) -> Self {
        Self {
            target,
            distance,
            yaw: 0.0,
            pitch: 0.3,
            near: 0.1,
            far: 1000.0,
            fov: FRAC_PI_4,
            orthographic: false,
            target_yaw: 0.0,
            target_pitch: 0.3,
            target_distance: distance,
            target_target: target,
            animating: false,
            anim_speed: 8.0,
        }
    }

    pub fn rotate(&mut self, dx: f32, dy: f32) {
        self.yaw += dx;
        self.pitch += dy;
        self.pitch = self.pitch.clamp(
            -std::f32::consts::FRAC_PI_2 + 0.01,
            std::f32::consts::FRAC_PI_2 - 0.01,
        );
    }

    pub fn zoom(&mut self, delta: f32) {
        self.distance = (self.distance - delta).clamp(0.1, 10000.0);
    }

    pub fn pan(&mut self, dx: f32, dy: f32) {
        let right = Vec3::new(self.yaw.cos(), 0.0, -self.yaw.sin());
        let up = Vec3::Y;
        self.target += right * dx * self.distance * 0.001;
        self.target += up * dy * self.distance * 0.001;
    }

    pub fn eye(&self) -> Vec3 {
        let x = self.distance * self.pitch.cos() * self.yaw.sin();
        let y = self.distance * self.pitch.sin();
        let z = self.distance * self.pitch.cos() * self.yaw.cos();
        self.target + Vec3::new(x, y, z)
    }

    pub fn view_matrix(&self) -> Mat4 {
        Mat4::look_at_rh(self.eye(), self.target, Vec3::Y)
    }

    pub fn proj_matrix(&self, aspect: f32) -> Mat4 {
        if self.orthographic {
            let half_h = self.distance * 0.5;
            let half_w = half_h * aspect;
            Mat4::orthographic_rh(-half_w, half_w, -half_h, half_h, self.near, self.far)
        } else {
            Mat4::perspective_rh(self.fov, aspect, self.near, self.far)
        }
    }

    /// Toggle between perspective and orthographic projection.
    pub fn toggle_ortho(&mut self) {
        self.orthographic = !self.orthographic;
    }

    /// Smoothly animate to a target yaw/pitch.
    pub fn animate_to(&mut self, yaw: f32, pitch: f32) {
        self.target_yaw = yaw;
        self.target_pitch = pitch;
        self.target_distance = self.distance;
        self.target_target = self.target;
        self.animating = true;
    }

    /// Smoothly animate to focus on a point at a given distance.
    pub fn focus_on(&mut self, point: Vec3, distance: f32) {
        self.target_target = point;
        self.target_distance = distance;
        self.target_yaw = self.yaw;
        self.target_pitch = self.pitch;
        self.animating = true;
    }

    /// Update animation state. Call each frame with delta time.
    pub fn update(&mut self, dt: f32) {
        if !self.animating {
            return;
        }

        let t = (self.anim_speed * dt).min(1.0);

        self.yaw = lerp(self.yaw, self.target_yaw, t);
        self.pitch = lerp(self.pitch, self.target_pitch, t);
        self.distance = lerp(self.distance, self.target_distance, t);
        self.target = self.target.lerp(self.target_target, t);

        // Check convergence
        let dy = (self.yaw - self.target_yaw).abs();
        let dp = (self.pitch - self.target_pitch).abs();
        let dd = (self.distance - self.target_distance).abs();
        let dt_dist = (self.target - self.target_target).length();

        if dy < 0.001 && dp < 0.001 && dd < 0.01 && dt_dist < 0.01 {
            self.yaw = self.target_yaw;
            self.pitch = self.target_pitch;
            self.distance = self.target_distance;
            self.target = self.target_target;
            self.animating = false;
        }
    }

    pub fn is_animating(&self) -> bool {
        self.animating
    }

    pub fn view_proj(&self, aspect: f32) -> Mat4 {
        self.proj_matrix(aspect) * self.view_matrix()
    }

    /// Unproject a screen-space point (pixels) into a world-space ray.
    /// Returns (origin, direction). `screen_w`/`screen_h` are in physical pixels.
    pub fn screen_to_ray(&self, sx: f32, sy: f32, screen_w: f32, screen_h: f32) -> (Vec3, Vec3) {
        let aspect = screen_w / screen_h.max(1.0);
        let inv_vp = self.view_proj(aspect).inverse();

        // NDC: [-1, 1]
        let ndc_x = (sx / screen_w) * 2.0 - 1.0;
        let ndc_y = 1.0 - (sy / screen_h) * 2.0; // flip Y

        let near = inv_vp.project_point3(Vec3::new(ndc_x, ndc_y, -1.0));
        let far = inv_vp.project_point3(Vec3::new(ndc_x, ndc_y, 1.0));
        let dir = (far - near).normalize();

        (near, dir)
    }

    /// Find where a ray hits the Y=`y` ground plane. Returns the XZ hit point.
    pub fn ray_ground_intersect(origin: Vec3, dir: Vec3, y: f32) -> Option<Vec3> {
        if dir.y.abs() < 1e-6 {
            return None;
        }
        let t = (y - origin.y) / dir.y;
        if t < 0.0 {
            return None;
        }
        Some(origin + dir * t)
    }
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::{FRAC_PI_2, PI};

    #[test]
    fn new_sets_target_and_distance() {
        let cam = OrbitCamera::new(Vec3::ZERO, 5.0);
        assert_eq!(cam.target, Vec3::ZERO);
        assert_eq!(cam.distance, 5.0);
    }

    #[test]
    fn zoom_clamps() {
        let mut cam = OrbitCamera::new(Vec3::ZERO, 1.0);
        cam.zoom(100.0);
        assert!((cam.distance - 0.1).abs() < f32::EPSILON);
    }

    #[test]
    fn pitch_clamps_near_poles() {
        let mut cam = OrbitCamera::new(Vec3::ZERO, 5.0);
        cam.rotate(0.0, 100.0);
        assert!(cam.pitch < FRAC_PI_2);
        cam.rotate(0.0, -200.0);
        assert!(cam.pitch > -FRAC_PI_2);
    }

    #[test]
    fn eye_at_zero_yaw_zero_pitch() {
        let mut cam = OrbitCamera::new(Vec3::ZERO, 10.0);
        cam.pitch = 0.0;
        cam.yaw = 0.0;
        let eye = cam.eye();
        assert!(eye.x.abs() < 1e-5);
        assert!(eye.y.abs() < 1e-5);
        assert!((eye.z - 10.0).abs() < 1e-5);
    }

    #[test]
    fn eye_distance_matches() {
        let cam = OrbitCamera::new(Vec3::new(1.0, 2.0, 3.0), 8.0);
        let dist = (cam.eye() - cam.target).length();
        assert!((dist - cam.distance).abs() < 1e-4);
    }

    #[test]
    fn view_proj_determinant_nonzero() {
        let cam = OrbitCamera::new(Vec3::ZERO, 5.0);
        let det = cam.view_proj(16.0 / 9.0).determinant();
        assert!(det.abs() > 1e-10);
    }

    #[test]
    fn pan_moves_target() {
        let mut cam = OrbitCamera::new(Vec3::ZERO, 10.0);
        let orig = cam.target;
        cam.pan(100.0, 50.0);
        assert_ne!(cam.target, orig);
    }

    #[test]
    fn full_revolution() {
        let mut cam = OrbitCamera::new(Vec3::ZERO, 10.0);
        cam.pitch = 0.0;
        cam.yaw = 0.0;
        let eye_start = cam.eye();
        cam.yaw = 2.0 * PI;
        let eye_end = cam.eye();
        assert!((eye_start - eye_end).length() < 1e-4);
    }
}
