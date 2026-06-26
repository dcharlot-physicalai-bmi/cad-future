//! Performance HUD — real-time rendering statistics overlay.
//!
//! Displays FPS, frame time, vertex count, object count, and draw calls
//! in a compact overlay anchored to the viewport corner.

use crate::draw::DrawList;
use crate::font;

/// Rolling performance statistics.
pub struct PerfHud {
    pub visible: bool,
    // Timing
    frame_times: [f32; 60],
    frame_idx: usize,
    fps: f32,
    frame_ms: f32,
    // Per-frame stats (set externally)
    pub vertices: u32,
    pub triangles: u32,
    pub objects: u32,
    pub draw_calls: u32,
}

impl PerfHud {
    pub fn new() -> Self {
        Self {
            visible: false,
            frame_times: [16.67; 60],
            frame_idx: 0,
            fps: 60.0,
            frame_ms: 16.67,
            vertices: 0,
            triangles: 0,
            objects: 0,
            draw_calls: 0,
        }
    }

    /// Record a frame's delta time (seconds) and recompute rolling average.
    pub fn record_frame(&mut self, dt: f32) {
        let ms = dt * 1000.0;
        self.frame_times[self.frame_idx] = ms;
        self.frame_idx = (self.frame_idx + 1) % self.frame_times.len();

        let sum: f32 = self.frame_times.iter().sum();
        let avg_ms = sum / self.frame_times.len() as f32;
        self.frame_ms = avg_ms;
        self.fps = if avg_ms > 0.001 { 1000.0 / avg_ms } else { 0.0 };
    }

    pub fn toggle(&mut self) {
        self.visible = !self.visible;
    }

    pub fn draw(&self, draw: &mut DrawList, screen_w: f32, _screen_h: f32) {
        if !self.visible {
            return;
        }

        let hud_w = 160.0;
        let hud_h = 94.0;
        let margin = 8.0;
        let x = screen_w - hud_w - margin;
        let y = margin + 30.0; // below viewport header, above nav cube area
        let font_size = 10.0;
        let line_h = 14.0;

        // Background
        draw.push_quad(x, y, hud_w, hud_h, [0.0, 0.0, 0.0, 0.65]);

        // FPS bar (colored based on performance)
        let fps_color = if self.fps >= 55.0 {
            [0.2, 0.9, 0.3, 1.0] // green
        } else if self.fps >= 30.0 {
            [1.0, 0.8, 0.2, 1.0] // yellow
        } else {
            [1.0, 0.3, 0.2, 1.0] // red
        };

        let bar_w = (self.fps / 60.0).clamp(0.0, 1.0) * (hud_w - 8.0);
        draw.push_quad(x + 4.0, y + 4.0, bar_w, 3.0, fps_color);

        let lines = [
            format!("{:.0} FPS  ({:.1} ms)", self.fps, self.frame_ms),
            format!("Verts: {}  Tris: {}", fmt_count(self.vertices), fmt_count(self.triangles)),
            format!("Objects: {}  Draws: {}", self.objects, self.draw_calls),
        ];

        let mut cy = y + 12.0;
        for line in &lines {
            let mut cx = x + 6.0;
            for c in line.chars() {
                let params = font::CharQuadParams {
                    c,
                    x: cx,
                    y: cy,
                    size: font_size,
                    color: [0.85, 0.85, 0.85, 0.9],
                    atlas: None,
                };
                cx += font::emit_char_quads(&params, &mut draw.vertices, &mut draw.indices);
            }
            cy += line_h;
        }

        // GPU memory estimate line
        let mem_kb = self.vertices as f32 * 32.0 / 1024.0; // rough: 32 bytes/vert
        let mem_line = format!("VRAM: ~{:.0} KB", mem_kb);
        let mut cx = x + 6.0;
        for c in mem_line.chars() {
            let params = font::CharQuadParams {
                c,
                x: cx,
                y: cy,
                size: font_size,
                color: [0.6, 0.6, 0.7, 0.8],
                atlas: None,
            };
            cx += font::emit_char_quads(&params, &mut draw.vertices, &mut draw.indices);
        }
    }
}

impl Default for PerfHud {
    fn default() -> Self {
        Self::new()
    }
}

fn fmt_count(n: u32) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f32 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}K", n as f32 / 1_000.0)
    } else {
        n.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fps_calculation() {
        let mut hud = PerfHud::new();
        for _ in 0..120 {
            hud.record_frame(1.0 / 60.0);
        }
        assert!((hud.fps - 60.0).abs() < 1.0);
    }

    #[test]
    fn fmt_count_works() {
        assert_eq!(fmt_count(500), "500");
        assert_eq!(fmt_count(1500), "1.5K");
        assert_eq!(fmt_count(2_500_000), "2.5M");
    }

    #[test]
    fn toggle() {
        let mut hud = PerfHud::new();
        assert!(!hud.visible);
        hud.toggle();
        assert!(hud.visible);
    }
}
