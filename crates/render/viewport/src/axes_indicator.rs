//! Axes indicator — small RGB XYZ triad in the viewport corner.
//!
//! Renders 3 colored lines (red X, green Y, blue Z) as a screen-space
//! orientation reference. Positioned in the bottom-left corner of the viewport.

use physical_ui::draw::{DrawList, UiVertex};

/// Configuration for the axes indicator.
pub struct AxesIndicator {
    /// Size of the indicator in pixels.
    pub size: f32,
    /// Margin from the viewport edge in pixels.
    pub margin: f32,
}

impl AxesIndicator {
    pub fn new() -> Self {
        Self {
            size: 60.0,
            margin: 16.0,
        }
    }

    /// Draw the axes indicator into a UI draw list.
    /// `yaw` and `pitch` are the camera's orbit angles.
    /// `screen_w` and `screen_h` are the viewport dimensions in pixels.
    pub fn draw(
        &self,
        draw: &mut DrawList,
        yaw: f32,
        pitch: f32,
        screen_w: f32,
        screen_h: f32,
    ) {
        let cx = self.margin + self.size * 0.5;
        let cy = screen_h - self.margin - self.size * 0.5;
        let r = self.size * 0.4;

        // Project 3D axis directions to 2D using camera rotation
        let cos_y = yaw.cos();
        let sin_y = yaw.sin();
        let cos_p = pitch.cos();
        let sin_p = pitch.sin();

        // Camera-space projection (simplified orbit view)
        let axes = [
            // X axis (red)
            ([cos_y, sin_y * sin_p], [0.9, 0.2, 0.2, 1.0], "X"),
            // Y axis (green)
            ([0.0_f32, -cos_p], [0.2, 0.8, 0.2, 1.0], "Y"),
            // Z axis (blue)
            ([-sin_y, cos_y * sin_p], [0.3, 0.4, 0.95, 1.0], "Z"),
        ];

        // Draw background circle
        let bg_color = [0.1, 0.1, 0.12, 0.6];
        draw_circle(draw, cx, cy, self.size * 0.5, bg_color, 24);

        // Sort axes by depth (z-component) for proper ordering
        let mut sorted: Vec<(usize, f32)> = axes.iter().enumerate().map(|(i, (_proj, _, _))| {
            // Depth approximation
            let depth = match i {
                0 => sin_y * cos_p,  // X
                1 => sin_p,          // Y
                2 => cos_y * cos_p,  // Z
                _ => 0.0,
            };
            (i, depth)
        }).collect();
        sorted.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

        let _ = screen_w;

        for (idx, _depth) in &sorted {
            let (proj, color, label) = &axes[*idx];
            let ex = cx + proj[0] * r;
            let ey = cy - proj[1] * r; // flip Y for screen

            // Draw axis line
            draw_line(draw, cx, cy, ex, ey, 2.0, *color);

            // Draw axis label dot at tip
            draw_circle(draw, ex, ey, 4.0, *color, 8);

            // Draw label text
            let text_x = ex + (proj[0] * 8.0).clamp(-6.0, 6.0);
            let text_y = ey - (proj[1] * 8.0).clamp(-6.0, 6.0);
            let font_size = 10.0;
            let params = physical_ui::font::CharQuadParams {
                c: label.chars().next().unwrap(),
                x: text_x - font_size * 0.3,
                y: text_y - font_size * 0.5,
                size: font_size,
                color: *color,
                atlas: None,
            };
            physical_ui::font::emit_char_quads(&params, &mut draw.vertices, &mut draw.indices);
        }
    }
}

impl Default for AxesIndicator {
    fn default() -> Self {
        Self::new()
    }
}

/// Draw a line as a thin quad.
fn draw_line(draw: &mut DrawList, x0: f32, y0: f32, x1: f32, y1: f32, width: f32, color: [f32; 4]) {
    let dx = x1 - x0;
    let dy = y1 - y0;
    let len = (dx * dx + dy * dy).sqrt();
    if len < 0.001 {
        return;
    }
    let nx = -dy / len * width * 0.5;
    let ny = dx / len * width * 0.5;

    let base = draw.vertices.len() as u32;
    draw.vertices.push(UiVertex { pos: [x0 + nx, y0 + ny], uv: [0.0, 0.0], color });
    draw.vertices.push(UiVertex { pos: [x0 - nx, y0 - ny], uv: [0.0, 0.0], color });
    draw.vertices.push(UiVertex { pos: [x1 - nx, y1 - ny], uv: [0.0, 0.0], color });
    draw.vertices.push(UiVertex { pos: [x1 + nx, y1 + ny], uv: [0.0, 0.0], color });
    draw.indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
}

/// Draw a filled circle approximated as a triangle fan.
fn draw_circle(draw: &mut DrawList, cx: f32, cy: f32, radius: f32, color: [f32; 4], segments: u32) {
    let center_idx = draw.vertices.len() as u32;
    draw.vertices.push(UiVertex { pos: [cx, cy], uv: [0.5, 0.5], color });

    for i in 0..segments {
        let a = (i as f32 / segments as f32) * std::f32::consts::TAU;
        let x = cx + a.cos() * radius;
        let y = cy + a.sin() * radius;
        draw.vertices.push(UiVertex { pos: [x, y], uv: [0.0, 0.0], color });
    }

    for i in 0..segments {
        let next = if i + 1 < segments { i + 1 } else { 0 };
        draw.indices.push(center_idx);
        draw.indices.push(center_idx + 1 + i);
        draw.indices.push(center_idx + 1 + next);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indicator_default() {
        let ind = AxesIndicator::new();
        assert_eq!(ind.size, 60.0);
        assert_eq!(ind.margin, 16.0);
    }

    #[test]
    fn draw_produces_geometry() {
        let ind = AxesIndicator::new();
        let mut draw = DrawList::new();
        ind.draw(&mut draw, 0.3, 0.3, 1920.0, 1080.0);
        assert!(!draw.vertices.is_empty());
        assert!(!draw.indices.is_empty());
    }

    #[test]
    fn draw_line_produces_quad() {
        let mut draw = DrawList::new();
        draw_line(&mut draw, 0.0, 0.0, 100.0, 0.0, 2.0, [1.0; 4]);
        assert_eq!(draw.vertices.len(), 4);
        assert_eq!(draw.indices.len(), 6);
    }
}
