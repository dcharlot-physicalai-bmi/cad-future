//! Navigation cube — interactive orientation widget in the viewport corner.
//!
//! A small projected cube in the upper-right corner. Clickable faces snap
//! the camera to axis-aligned views (Top, Front, Right, etc.).

use physical_ui::draw::{DrawList, UiVertex};
use physical_ui::font;

/// Named view orientations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewPreset {
    Front,
    Back,
    Left,
    Right,
    Top,
    Bottom,
    Iso,
}

impl ViewPreset {
    /// Returns (yaw, pitch) for the orbit camera to achieve this view.
    pub fn yaw_pitch(self) -> (f32, f32) {
        use std::f32::consts::{FRAC_PI_2, PI};
        match self {
            ViewPreset::Front => (0.0, 0.0),
            ViewPreset::Back => (PI, 0.0),
            ViewPreset::Right => (FRAC_PI_2, 0.0),
            ViewPreset::Left => (-FRAC_PI_2, 0.0),
            ViewPreset::Top => (0.0, FRAC_PI_2 - 0.01),
            ViewPreset::Bottom => (0.0, -FRAC_PI_2 + 0.01),
            ViewPreset::Iso => (0.6, 0.4),
        }
    }
}

/// The navigation cube widget.
pub struct NavCube {
    pub size: f32,
    pub margin: f32,
    pub hovered_face: Option<ViewPreset>,
}

impl NavCube {
    pub fn new() -> Self {
        Self {
            size: 80.0,
            margin: 16.0,
            hovered_face: None,
        }
    }

    /// Draw the navigation cube into a UI draw list.
    /// Position: upper-right corner.
    pub fn draw(
        &self,
        draw: &mut DrawList,
        yaw: f32,
        pitch: f32,
        screen_w: f32,
        _screen_h: f32,
    ) {
        let cx = screen_w - self.margin - self.size * 0.5;
        let cy = self.margin + self.size * 0.5;
        let half = self.size * 0.35;

        // Simple 3D cube projection
        let cos_y = yaw.cos();
        let sin_y = yaw.sin();
        let cos_p = pitch.cos();
        let sin_p = pitch.sin();

        // 8 cube corners in local space
        let corners_3d: [[f32; 3]; 8] = [
            [-1.0, -1.0, -1.0], [1.0, -1.0, -1.0], [1.0, 1.0, -1.0], [-1.0, 1.0, -1.0],
            [-1.0, -1.0,  1.0], [1.0, -1.0,  1.0], [1.0, 1.0,  1.0], [-1.0, 1.0,  1.0],
        ];

        // Project to 2D
        let project = |p: [f32; 3]| -> [f32; 2] {
            let x = p[0] * cos_y + p[2] * sin_y;
            let z = -p[0] * sin_y + p[2] * cos_y;
            let y = p[1] * cos_p - z * sin_p;
            [cx + x * half, cy - y * half]
        };

        let corners: Vec<[f32; 2]> = corners_3d.iter().map(|c| project(*c)).collect();

        // 6 faces: [indices, normal_z_depth, label, color]
        let faces: [([usize; 4], f32, &str, [f32; 4]); 6] = [
            ([4, 5, 6, 7], cos_y * cos_p,       "Front", [0.28, 0.32, 0.38, 0.85]),  // +Z
            ([1, 0, 3, 2], -cos_y * cos_p,      "Back",  [0.22, 0.25, 0.30, 0.85]),  // -Z
            ([5, 1, 2, 6], cos_y.abs() * 0.5 + sin_y, "Right", [0.25, 0.28, 0.34, 0.85]),  // +X
            ([0, 4, 7, 3], cos_y.abs() * 0.5 - sin_y, "Left",  [0.25, 0.28, 0.34, 0.85]),  // -X
            ([3, 2, 6, 7], sin_p,                "Top",   [0.32, 0.36, 0.42, 0.85]),  // +Y
            ([0, 1, 5, 4], -sin_p,              "Bottom", [0.20, 0.22, 0.28, 0.85]),  // -Y
        ];

        // Sort by depth (back to front)
        let mut sorted: Vec<usize> = (0..6).collect();
        sorted.sort_by(|a, b| faces[*a].1.partial_cmp(&faces[*b].1).unwrap_or(std::cmp::Ordering::Equal));

        for idx in sorted {
            let (vidxs, depth, label, mut color) = faces[idx];
            if depth < -0.1 {
                continue; // back-face cull
            }

            // Highlight hovered face
            let preset = match idx {
                0 => ViewPreset::Front,
                1 => ViewPreset::Back,
                2 => ViewPreset::Right,
                3 => ViewPreset::Left,
                4 => ViewPreset::Top,
                5 => ViewPreset::Bottom,
                _ => continue,
            };
            if self.hovered_face == Some(preset) {
                color = [0.4, 0.55, 0.75, 0.9];
            }

            // Draw face as two triangles
            let c = corners.clone();
            let base = draw.vertices.len() as u32;
            for &vi in &vidxs {
                draw.vertices.push(UiVertex {
                    pos: c[vi],
                    uv: [0.0, 0.0],
                    color,
                });
            }
            draw.indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);

            // Edge lines
            let edge_color = [0.5, 0.55, 0.6, 0.9];
            for i in 0..4 {
                let j = (i + 1) % 4;
                draw_line_thin(draw, c[vidxs[i]], c[vidxs[j]], edge_color);
            }

            // Label text centered on face
            if depth > 0.2 {
                let face_cx = (c[vidxs[0]][0] + c[vidxs[1]][0] + c[vidxs[2]][0] + c[vidxs[3]][0]) / 4.0;
                let face_cy = (c[vidxs[0]][1] + c[vidxs[1]][1] + c[vidxs[2]][1] + c[vidxs[3]][1]) / 4.0;

                let font_size = 9.0;
                let text_w = font::measure_text(label, font_size, None);
                let tx = face_cx - text_w * 0.5;
                let ty = face_cy - font_size * 0.5;

                let text_color = [0.85, 0.88, 0.92, 1.0];
                for c in label.chars() {
                    let params = font::CharQuadParams {
                        c,
                        x: tx + font::measure_text(&label[..label.find(c).unwrap_or(0)], font_size, None),
                        y: ty,
                        size: font_size,
                        color: text_color,
                        atlas: None,
                    };
                    font::emit_char_quads(&params, &mut draw.vertices, &mut draw.indices);
                }
            }
        }
    }

    /// Hit-test: given a screen-space click position, return which face (if any) was clicked.
    pub fn hit_test(&self, mx: f32, my: f32, screen_w: f32, _screen_h: f32) -> Option<ViewPreset> {
        let cx = screen_w - self.margin - self.size * 0.5;
        let cy = self.margin + self.size * 0.5;
        let dx = mx - cx;
        let dy = my - cy;
        let dist = (dx * dx + dy * dy).sqrt();
        if dist > self.size * 0.55 {
            return None;
        }
        // Simplified: return the closest face based on click quadrant
        // A proper implementation would do full 3D ray-cube intersection
        if dy.abs() > dx.abs() {
            if dy < 0.0 { Some(ViewPreset::Top) } else { Some(ViewPreset::Bottom) }
        } else if dx > 0.0 {
            Some(ViewPreset::Right)
        } else {
            Some(ViewPreset::Left)
        }
    }
}

impl Default for NavCube {
    fn default() -> Self {
        Self::new()
    }
}

fn draw_line_thin(draw: &mut DrawList, a: [f32; 2], b: [f32; 2], color: [f32; 4]) {
    let dx = b[0] - a[0];
    let dy = b[1] - a[1];
    let len = (dx * dx + dy * dy).sqrt();
    if len < 0.001 { return; }
    let nx = -dy / len;
    let ny = dx / len;

    let base = draw.vertices.len() as u32;
    draw.vertices.push(UiVertex { pos: [a[0] + nx, a[1] + ny], uv: [0.0, 0.0], color });
    draw.vertices.push(UiVertex { pos: [a[0] - nx, a[1] - ny], uv: [0.0, 0.0], color });
    draw.vertices.push(UiVertex { pos: [b[0] - nx, b[1] - ny], uv: [0.0, 0.0], color });
    draw.vertices.push(UiVertex { pos: [b[0] + nx, b[1] + ny], uv: [0.0, 0.0], color });
    draw.indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_size() {
        let cube = NavCube::new();
        assert_eq!(cube.size, 80.0);
    }

    #[test]
    fn draw_produces_geometry() {
        let cube = NavCube::new();
        let mut draw = DrawList::new();
        cube.draw(&mut draw, 0.3, 0.3, 1920.0, 1080.0);
        assert!(!draw.vertices.is_empty());
    }

    #[test]
    fn view_preset_yaw_pitch() {
        let (yaw, pitch) = ViewPreset::Front.yaw_pitch();
        assert!(yaw.abs() < f32::EPSILON);
        assert!(pitch.abs() < f32::EPSILON);
    }

    #[test]
    fn hit_test_outside_returns_none() {
        let cube = NavCube::new();
        assert!(cube.hit_test(0.0, 0.0, 1920.0, 1080.0).is_none());
    }
}
