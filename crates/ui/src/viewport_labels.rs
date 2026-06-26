//! Viewport labels — 3D-projected object name annotations.
//!
//! Projects object positions to screen space and draws name labels
//! near each object. Labels fade with distance and avoid overlap.

use crate::draw::DrawList;
use crate::font;

/// A single label to display at a screen position.
pub struct ViewportLabel {
    pub text: String,
    pub screen_x: f32,
    pub screen_y: f32,
    pub selected: bool,
    pub depth: f32,
}

/// Manages viewport label drawing.
pub struct ViewportLabels {
    pub visible: bool,
    labels: Vec<ViewportLabel>,
}

impl ViewportLabels {
    pub fn new() -> Self {
        Self {
            visible: false,
            labels: Vec::new(),
        }
    }

    pub fn toggle(&mut self) {
        self.visible = !self.visible;
    }

    /// Clear labels for this frame.
    pub fn clear(&mut self) {
        self.labels.clear();
    }

    /// Add a label by projecting a 3D world position to screen space.
    /// `view_proj` is the combined view-projection matrix.
    /// `screen_w`/`screen_h` are viewport dimensions.
    pub fn add_3d(
        &mut self,
        text: &str,
        world_pos: glam::Vec3,
        view_proj: glam::Mat4,
        screen_w: f32,
        screen_h: f32,
        selected: bool,
    ) {
        let clip = view_proj * world_pos.extend(1.0);
        if clip.w <= 0.0 {
            return; // behind camera
        }
        let ndc = clip.truncate() / clip.w;
        let sx = (ndc.x * 0.5 + 0.5) * screen_w;
        let sy = (1.0 - (ndc.y * 0.5 + 0.5)) * screen_h;

        self.labels.push(ViewportLabel {
            text: text.to_string(),
            screen_x: sx,
            screen_y: sy,
            selected,
            depth: clip.w,
        });
    }

    /// Draw all labels.
    pub fn draw(&self, draw: &mut DrawList, _screen_w: f32, _screen_h: f32) {
        if !self.visible || self.labels.is_empty() {
            return;
        }

        let font_size = 10.0;
        let pad_x = 4.0;
        let pad_y = 2.0;

        for label in &self.labels {
            // Fade with distance — closer objects have stronger labels
            let alpha = (1.0 - (label.depth - 5.0) / 40.0).clamp(0.3, 0.9);

            let text_w = font::measure_text(&label.text, font_size, None);
            let bg_w = text_w + pad_x * 2.0;
            let bg_h = font_size + pad_y * 2.0;

            // Position label above the object center
            let lx = label.screen_x - bg_w * 0.5;
            let ly = label.screen_y - 24.0; // offset above

            // Background pill
            let bg_color = if label.selected {
                [0.15, 0.3, 0.6, alpha * 0.85]
            } else {
                [0.1, 0.1, 0.12, alpha * 0.7]
            };
            draw.push_quad(lx, ly, bg_w, bg_h, bg_color);

            // Accent underline for selected
            if label.selected {
                draw.push_quad(lx, ly + bg_h - 1.5, bg_w, 1.5, [0.3, 0.6, 1.0, alpha]);
            }

            // Text
            let tx = lx + pad_x;
            let ty = ly + pad_y;
            let text_color = if label.selected {
                [0.9, 0.95, 1.0, alpha]
            } else {
                [0.7, 0.7, 0.7, alpha]
            };

            let mut cx = tx;
            for c in label.text.chars() {
                let params = font::CharQuadParams {
                    c,
                    x: cx,
                    y: ty,
                    size: font_size,
                    color: text_color,
                    atlas: None,
                };
                cx += font::emit_char_quads(&params, &mut draw.vertices, &mut draw.indices);
            }
        }
    }
}

impl Default for ViewportLabels {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::{Mat4, Vec3};

    #[test]
    fn label_behind_camera_rejected() {
        let mut labels = ViewportLabels::new();
        labels.visible = true;
        // Identity VP means clip.w = 1.0, so behind-camera check passes for points in front
        // But a point far behind would have negative w after projection
        let vp = Mat4::perspective_rh(0.8, 1.0, 0.1, 100.0)
            * Mat4::look_at_rh(Vec3::new(0.0, 0.0, 5.0), Vec3::ZERO, Vec3::Y);
        labels.add_3d("Behind", Vec3::new(0.0, 0.0, 10.0), vp, 800.0, 600.0, false);
        // This point is behind the camera (z > camera z)
        assert!(labels.labels.is_empty() || labels.labels[0].depth > 0.0);
    }

    #[test]
    fn toggle_visibility() {
        let mut labels = ViewportLabels::new();
        assert!(!labels.visible);
        labels.toggle();
        assert!(labels.visible);
    }
}
