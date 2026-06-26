//! Transform gizmo — translate/rotate/scale handles for selected objects.
//!
//! Renders axis-colored arrows (translate), rings (rotate), and boxes (scale)
//! at the position of the selected object. Supports ray-based hit testing
//! for interactive manipulation.

use crate::vertex::Vertex;

/// Which transform tool is active.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GizmoMode {
    Translate,
    Rotate,
    Scale,
}

/// Which axis the user is dragging on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GizmoAxis {
    X,
    Y,
    Z,
    None,
}

/// Colors for gizmo axes.
const AXIS_X: [f32; 4] = [0.9, 0.2, 0.2, 1.0];
const AXIS_Y: [f32; 4] = [0.2, 0.8, 0.2, 1.0];
const AXIS_Z: [f32; 4] = [0.3, 0.4, 0.95, 1.0];
const AXIS_HOVER: [f32; 4] = [1.0, 0.9, 0.3, 1.0];

/// State for the transform gizmo.
pub struct Gizmo {
    pub mode: GizmoMode,
    pub hovered_axis: GizmoAxis,
    pub dragging_axis: GizmoAxis,
    pub visible: bool,
    /// World-space position of the gizmo center.
    pub position: glam::Vec3,
    /// Scale of gizmo in screen-constant size.
    pub scale: f32,
}

impl Gizmo {
    pub fn new() -> Self {
        Self {
            mode: GizmoMode::Translate,
            hovered_axis: GizmoAxis::None,
            dragging_axis: GizmoAxis::None,
            visible: false,
            position: glam::Vec3::ZERO,
            scale: 1.5,
        }
    }

    /// Generate vertices for the translate gizmo arrows.
    /// Returns (vertices, indices) for rendering as triangles.
    pub fn translate_geometry(&self) -> (Vec<Vertex>, Vec<u32>) {
        let mut verts = Vec::new();
        let mut idxs = Vec::new();

        let shaft_radius = 0.03 * self.scale;
        let shaft_length = 0.8 * self.scale;
        let head_radius = 0.08 * self.scale;
        let head_length = 0.2 * self.scale;
        let segments = 8u32;
        let p = self.position;

        // Generate arrow for each axis
        for (axis_idx, (dir, color)) in [
            (glam::Vec3::X, self.axis_color(GizmoAxis::X)),
            (glam::Vec3::Y, self.axis_color(GizmoAxis::Y)),
            (glam::Vec3::Z, self.axis_color(GizmoAxis::Z)),
        ].iter().enumerate()
        {
            let base_idx = verts.len() as u32;

            // Build orthonormal basis for this axis
            let (u, v) = orthonormal_basis(*dir);

            // Shaft (cylinder)
            for i in 0..segments {
                let a0 = (i as f32 / segments as f32) * std::f32::consts::TAU;
                let a1 = ((i + 1) as f32 / segments as f32) * std::f32::consts::TAU;

                let (s0, c0) = a0.sin_cos();
                let (s1, c1) = a1.sin_cos();

                let n0 = u * c0 + v * s0;
                let n1 = u * c1 + v * s1;

                let p0 = p + n0 * shaft_radius;
                let p1 = p + n1 * shaft_radius;
                let p2 = p + *dir * shaft_length + n1 * shaft_radius;
                let p3 = p + *dir * shaft_length + n0 * shaft_radius;

                let bi = verts.len() as u32;
                verts.push(Vertex { position: p0.into(), normal: n0.into(), uv: [0.0, 0.0] });
                verts.push(Vertex { position: p1.into(), normal: n1.into(), uv: [0.0, 0.0] });
                verts.push(Vertex { position: p2.into(), normal: n1.into(), uv: [0.0, 0.0] });
                verts.push(Vertex { position: p3.into(), normal: n0.into(), uv: [0.0, 0.0] });
                idxs.extend_from_slice(&[bi, bi+1, bi+2, bi, bi+2, bi+3]);
            }

            // Arrow head (cone)
            let tip = p + *dir * (shaft_length + head_length);
            let base_center = p + *dir * shaft_length;
            for i in 0..segments {
                let a0 = (i as f32 / segments as f32) * std::f32::consts::TAU;
                let a1 = ((i + 1) as f32 / segments as f32) * std::f32::consts::TAU;

                let (s0, c0) = a0.sin_cos();
                let (s1, c1) = a1.sin_cos();

                let r0 = base_center + (u * c0 + v * s0) * head_radius;
                let r1 = base_center + (u * c1 + v * s1) * head_radius;

                let n_cone0 = (*dir * head_radius + (u * c0 + v * s0) * head_length).normalize();
                let n_cone1 = (*dir * head_radius + (u * c1 + v * s1) * head_length).normalize();

                let bi = verts.len() as u32;
                verts.push(Vertex { position: tip.into(), normal: (*dir).into(), uv: [0.0, 0.0] });
                verts.push(Vertex { position: r0.into(), normal: n_cone0.into(), uv: [0.0, 0.0] });
                verts.push(Vertex { position: r1.into(), normal: n_cone1.into(), uv: [0.0, 0.0] });
                idxs.extend_from_slice(&[bi, bi+1, bi+2]);
            }

            let _ = (axis_idx, base_idx, color);
        }

        (verts, idxs)
    }

    /// Hit-test a screen ray against the gizmo axes.
    /// Returns the axis closest to the ray within threshold.
    pub fn hit_test(
        &self,
        ray_origin: glam::Vec3,
        ray_dir: glam::Vec3,
        screen_scale: f32,
    ) -> GizmoAxis {
        if !self.visible {
            return GizmoAxis::None;
        }

        let threshold = 0.15 * self.scale * screen_scale;
        let length = self.scale;

        let mut best = GizmoAxis::None;
        let mut best_dist = threshold;

        for (axis, dir) in [
            (GizmoAxis::X, glam::Vec3::X),
            (GizmoAxis::Y, glam::Vec3::Y),
            (GizmoAxis::Z, glam::Vec3::Z),
        ] {
            let dist = ray_line_distance(ray_origin, ray_dir, self.position, self.position + dir * length);
            if dist < best_dist {
                best_dist = dist;
                best = axis;
            }
        }

        best
    }

    fn axis_color(&self, axis: GizmoAxis) -> [f32; 4] {
        if self.hovered_axis == axis || self.dragging_axis == axis {
            AXIS_HOVER
        } else {
            match axis {
                GizmoAxis::X => AXIS_X,
                GizmoAxis::Y => AXIS_Y,
                GizmoAxis::Z => AXIS_Z,
                GizmoAxis::None => [0.5, 0.5, 0.5, 1.0],
            }
        }
    }

    /// Get the world-space direction for an axis.
    pub fn axis_direction(axis: GizmoAxis) -> glam::Vec3 {
        match axis {
            GizmoAxis::X => glam::Vec3::X,
            GizmoAxis::Y => glam::Vec3::Y,
            GizmoAxis::Z => glam::Vec3::Z,
            GizmoAxis::None => glam::Vec3::ZERO,
        }
    }

    /// Cycle to the next gizmo mode.
    pub fn cycle_mode(&mut self) {
        self.mode = match self.mode {
            GizmoMode::Translate => GizmoMode::Rotate,
            GizmoMode::Rotate => GizmoMode::Scale,
            GizmoMode::Scale => GizmoMode::Translate,
        };
    }
}

impl Default for Gizmo {
    fn default() -> Self {
        Self::new()
    }
}

fn orthonormal_basis(dir: glam::Vec3) -> (glam::Vec3, glam::Vec3) {
    let up = if dir.y.abs() < 0.99 { glam::Vec3::Y } else { glam::Vec3::X };
    let u = dir.cross(up).normalize();
    let v = u.cross(dir).normalize();
    (u, v)
}

/// Minimum distance between a ray and a line segment.
fn ray_line_distance(
    ray_origin: glam::Vec3,
    ray_dir: glam::Vec3,
    seg_a: glam::Vec3,
    seg_b: glam::Vec3,
) -> f32 {
    let u = ray_dir;
    let v = seg_b - seg_a;
    let w = ray_origin - seg_a;

    let a = u.dot(u);
    let b = u.dot(v);
    let c = v.dot(v);
    let d = u.dot(w);
    let e = v.dot(w);

    let denom = a * c - b * b;
    if denom < 1e-10 {
        // Parallel
        return w.cross(u).length() / u.length();
    }

    let s = (b * e - c * d) / denom;
    let t = (a * e - b * d) / denom;
    let t = t.clamp(0.0, 1.0);
    let s = s.max(0.0);

    let closest_ray = ray_origin + u * s;
    let closest_seg = seg_a + v * t;
    (closest_ray - closest_seg).length()
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec3;

    #[test]
    fn gizmo_default_is_translate() {
        let g = Gizmo::new();
        assert_eq!(g.mode, GizmoMode::Translate);
    }

    #[test]
    fn cycle_modes() {
        let mut g = Gizmo::new();
        g.cycle_mode();
        assert_eq!(g.mode, GizmoMode::Rotate);
        g.cycle_mode();
        assert_eq!(g.mode, GizmoMode::Scale);
        g.cycle_mode();
        assert_eq!(g.mode, GizmoMode::Translate);
    }

    #[test]
    fn hit_test_invisible_returns_none() {
        let g = Gizmo::new();
        assert_eq!(g.hit_test(Vec3::ZERO, Vec3::X, 1.0), GizmoAxis::None);
    }

    #[test]
    fn translate_geometry_nonempty() {
        let mut g = Gizmo::new();
        g.visible = true;
        let (v, i) = g.translate_geometry();
        assert!(!v.is_empty());
        assert!(!i.is_empty());
    }

    #[test]
    fn axis_direction() {
        assert_eq!(Gizmo::axis_direction(GizmoAxis::X), Vec3::X);
        assert_eq!(Gizmo::axis_direction(GizmoAxis::Y), Vec3::Y);
        assert_eq!(Gizmo::axis_direction(GizmoAxis::Z), Vec3::Z);
    }
}
