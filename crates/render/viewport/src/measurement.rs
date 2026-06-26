//! Measurement overlay — draws dimension annotations in screen space.
//!
//! Projects 3D measurement points to 2D and draws leader lines, arrows,
//! and dimension text using the UI draw list.

use glam::{Mat4, Vec3, Vec4};
use physical_ui::draw::DrawList;
use physical_ui::font;

/// A single measurement between two 3D points.
#[derive(Clone, Debug)]
pub struct Measurement {
    pub start: Vec3,
    pub end: Vec3,
    pub label: Option<String>,
    pub color: [f32; 4],
}

impl Measurement {
    pub fn distance(start: Vec3, end: Vec3) -> Self {
        let dist = (end - start).length();
        Self {
            start,
            end,
            label: Some(format!("{:.2}", dist)),
            color: [1.0, 0.85, 0.2, 1.0], // yellow
        }
    }

    pub fn with_label(mut self, label: &str) -> Self {
        self.label = Some(label.to_string());
        self
    }

    pub fn with_color(mut self, color: [f32; 4]) -> Self {
        self.color = color;
        self
    }
}

/// Manages a set of measurements and draws them.
pub struct MeasurementOverlay {
    pub measurements: Vec<Measurement>,
    pub visible: bool,
    /// Temporary measurement being placed (first point set, waiting for second).
    pub pending_start: Option<Vec3>,
}

impl MeasurementOverlay {
    pub fn new() -> Self {
        Self {
            measurements: Vec::new(),
            visible: true,
            pending_start: None,
        }
    }

    pub fn add(&mut self, m: Measurement) {
        self.measurements.push(m);
    }

    pub fn clear(&mut self) {
        self.measurements.clear();
        self.pending_start = None;
    }

    /// Draw all measurements into a DrawList.
    pub fn draw(
        &self,
        draw: &mut DrawList,
        view_proj: Mat4,
        screen_w: f32,
        screen_h: f32,
    ) {
        if !self.visible {
            return;
        }

        for m in &self.measurements {
            let (s2d, s_vis) = project(m.start, view_proj, screen_w, screen_h);
            let (e2d, e_vis) = project(m.end, view_proj, screen_w, screen_h);

            if !s_vis && !e_vis {
                continue;
            }

            // Draw dimension line
            draw_line(draw, s2d.0, s2d.1, e2d.0, e2d.1, 1.5, m.color);

            // Draw endpoint markers (small crosshairs)
            let marker_size = 4.0;
            if s_vis {
                draw_line(draw, s2d.0 - marker_size, s2d.1, s2d.0 + marker_size, s2d.1, 1.5, m.color);
                draw_line(draw, s2d.0, s2d.1 - marker_size, s2d.0, s2d.1 + marker_size, 1.5, m.color);
            }
            if e_vis {
                draw_line(draw, e2d.0 - marker_size, e2d.1, e2d.0 + marker_size, e2d.1, 1.5, m.color);
                draw_line(draw, e2d.0, e2d.1 - marker_size, e2d.0, e2d.1 + marker_size, 1.5, m.color);
            }

            // Draw label at midpoint
            if let Some(ref label) = m.label {
                let mid_x = (s2d.0 + e2d.0) * 0.5;
                let mid_y = (s2d.1 + e2d.1) * 0.5;
                let font_size = 12.0;
                let text_w = font::measure_text(label, font_size, None);

                // Background pill behind text
                let pad = 3.0;
                draw.push_quad(
                    mid_x - text_w * 0.5 - pad,
                    mid_y - font_size * 0.5 - pad,
                    text_w + pad * 2.0,
                    font_size + pad * 2.0,
                    [0.0, 0.0, 0.0, 0.7],
                );

                // Text
                let mut tx = mid_x - text_w * 0.5;
                let ty = mid_y - font_size * 0.5;
                for c in label.chars() {
                    let params = font::CharQuadParams {
                        c,
                        x: tx,
                        y: ty,
                        size: font_size,
                        color: m.color,
                        atlas: None,
                    };
                    tx += font::emit_char_quads(&params, &mut draw.vertices, &mut draw.indices);
                }
            }
        }
    }
}

impl Default for MeasurementOverlay {
    fn default() -> Self {
        Self::new()
    }
}

/// Project a 3D point to screen coordinates.
/// Returns ((x, y), visible).
fn project(point: Vec3, view_proj: Mat4, screen_w: f32, screen_h: f32) -> ((f32, f32), bool) {
    let clip = view_proj * Vec4::new(point.x, point.y, point.z, 1.0);
    if clip.w <= 0.001 {
        return ((0.0, 0.0), false);
    }
    let ndc_x = clip.x / clip.w;
    let ndc_y = clip.y / clip.w;
    let ndc_z = clip.z / clip.w;

    let sx = (ndc_x * 0.5 + 0.5) * screen_w;
    let sy = (1.0 - (ndc_y * 0.5 + 0.5)) * screen_h;

    let visible = ndc_z >= 0.0 && ndc_z <= 1.0
        && sx >= -50.0 && sx <= screen_w + 50.0
        && sy >= -50.0 && sy <= screen_h + 50.0;

    ((sx, sy), visible)
}

/// Draw a thick line as a rotated quad.
fn draw_line(draw: &mut DrawList, x1: f32, y1: f32, x2: f32, y2: f32, thickness: f32, color: [f32; 4]) {
    let dx = x2 - x1;
    let dy = y2 - y1;
    let len = (dx * dx + dy * dy).sqrt();
    if len < 0.001 {
        return;
    }

    let nx = -dy / len * thickness * 0.5;
    let ny = dx / len * thickness * 0.5;

    // Quad corners
    let v0 = (x1 + nx, y1 + ny);
    let v1 = (x1 - nx, y1 - ny);
    let v2 = (x2 - nx, y2 - ny);
    let v3 = (x2 + nx, y2 + ny);

    draw.push_quad_vertices(
        [v0.0, v0.1],
        [v1.0, v1.1],
        [v2.0, v2.1],
        [v3.0, v3.1],
        color,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn measurement_distance_label() {
        let m = Measurement::distance(Vec3::ZERO, Vec3::new(3.0, 4.0, 0.0));
        assert_eq!(m.label, Some("5.00".to_string()));
    }

    #[test]
    fn project_center() {
        let vp = Mat4::IDENTITY;
        let (pos, vis) = project(Vec3::ZERO, vp, 800.0, 600.0);
        assert!(vis);
        assert!((pos.0 - 400.0).abs() < 1.0);
        assert!((pos.1 - 300.0).abs() < 1.0);
    }

    #[test]
    fn project_behind_camera() {
        let vp = Mat4::IDENTITY;
        // w would be negative for a point behind — use a perspective matrix
        let proj = Mat4::perspective_rh(1.0, 1.0, 0.1, 100.0);
        let view = Mat4::look_at_rh(Vec3::new(0.0, 0.0, 5.0), Vec3::ZERO, Vec3::Y);
        let vp = proj * view;
        let (_, vis) = project(Vec3::new(0.0, 0.0, 10.0), vp, 800.0, 600.0);
        // Point behind the camera
        assert!(!vis);
    }

    #[test]
    fn overlay_draw_produces_geometry() {
        let mut overlay = MeasurementOverlay::new();
        overlay.add(Measurement::distance(Vec3::ZERO, Vec3::X));
        let mut draw = DrawList::new();
        overlay.draw(&mut draw, Mat4::IDENTITY, 800.0, 600.0);
        assert!(!draw.vertices.is_empty());
    }
}
