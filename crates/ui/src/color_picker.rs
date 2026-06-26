//! Color picker — material/appearance color editor widget.
//!
//! Provides HSV color picker with RGB/Hex output for assigning
//! colors and appearances to objects and materials.

use crate::draw::DrawList;
use crate::font;

/// A color represented as RGBA [0.0, 1.0].
#[derive(Clone, Copy, Debug)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Color {
    pub fn new(r: f32, g: f32, b: f32) -> Self {
        Self { r, g, b, a: 1.0 }
    }

    pub fn with_alpha(mut self, a: f32) -> Self {
        self.a = a;
        self
    }

    pub fn to_array(self) -> [f32; 4] {
        [self.r, self.g, self.b, self.a]
    }

    pub fn from_array(arr: [f32; 4]) -> Self {
        Self { r: arr[0], g: arr[1], b: arr[2], a: arr[3] }
    }

    /// Convert to HSV. Returns (hue 0-360, saturation 0-1, value 0-1).
    pub fn to_hsv(self) -> (f32, f32, f32) {
        let max = self.r.max(self.g).max(self.b);
        let min = self.r.min(self.g).min(self.b);
        let delta = max - min;

        let v = max;
        let s = if max > 0.0 { delta / max } else { 0.0 };

        let h = if delta < 0.0001 {
            0.0
        } else if (max - self.r).abs() < 0.0001 {
            60.0 * (((self.g - self.b) / delta) % 6.0)
        } else if (max - self.g).abs() < 0.0001 {
            60.0 * ((self.b - self.r) / delta + 2.0)
        } else {
            60.0 * ((self.r - self.g) / delta + 4.0)
        };

        let h = if h < 0.0 { h + 360.0 } else { h };
        (h, s, v)
    }

    /// Create from HSV. h: 0-360, s: 0-1, v: 0-1.
    pub fn from_hsv(h: f32, s: f32, v: f32) -> Self {
        let c = v * s;
        let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
        let m = v - c;

        let (r, g, b) = if h < 60.0 {
            (c, x, 0.0)
        } else if h < 120.0 {
            (x, c, 0.0)
        } else if h < 180.0 {
            (0.0, c, x)
        } else if h < 240.0 {
            (0.0, x, c)
        } else if h < 300.0 {
            (x, 0.0, c)
        } else {
            (c, 0.0, x)
        };

        Self { r: r + m, g: g + m, b: b + m, a: 1.0 }
    }

    /// Format as hex string (e.g., "#FF8040").
    pub fn to_hex(self) -> String {
        format!("#{:02X}{:02X}{:02X}",
            (self.r * 255.0) as u8,
            (self.g * 255.0) as u8,
            (self.b * 255.0) as u8,
        )
    }
}

/// Preset color swatches for quick selection.
pub const PRESET_COLORS: &[[f32; 3]] = &[
    [0.78, 0.78, 0.80], // aluminum silver
    [0.65, 0.65, 0.65], // steel gray
    [0.72, 0.53, 0.04], // brass
    [0.90, 0.90, 0.92], // white plastic
    [0.15, 0.15, 0.15], // carbon black
    [0.85, 0.12, 0.12], // red
    [0.12, 0.55, 0.85], // blue
    [0.12, 0.75, 0.22], // green
    [0.95, 0.75, 0.10], // gold
    [0.60, 0.30, 0.10], // copper
    [0.85, 0.45, 0.10], // orange
    [0.55, 0.20, 0.70], // purple
];

/// The color picker widget.
pub struct ColorPicker {
    /// Whether the picker is visible/open.
    pub visible: bool,
    /// Current selected color.
    pub color: Color,
    /// HSV components for the picker UI.
    pub hue: f32,
    pub saturation: f32,
    pub value: f32,
    /// Which part is being dragged: 0 = SV square, 1 = hue bar, 2 = alpha bar.
    pub dragging: Option<u8>,
    /// Hovered preset index.
    pub hovered_preset: Option<usize>,
    /// Position of the picker popup.
    pub x: f32,
    pub y: f32,
    /// Whether the color changed this frame.
    pub changed: bool,
}

impl ColorPicker {
    pub fn new() -> Self {
        let default = Color::new(0.78, 0.78, 0.80);
        let (h, s, v) = default.to_hsv();
        Self {
            visible: false,
            color: default,
            hue: h,
            saturation: s,
            value: v,
            dragging: None,
            hovered_preset: None,
            x: 0.0,
            y: 0.0,
            changed: false,
        }
    }

    /// Open the picker at a position with an initial color.
    pub fn open(&mut self, x: f32, y: f32, color: Color) {
        self.visible = true;
        self.x = x;
        self.y = y;
        self.color = color;
        let (h, s, v) = color.to_hsv();
        self.hue = h;
        self.saturation = s;
        self.value = v;
        self.changed = false;
    }

    pub fn close(&mut self) {
        self.visible = false;
        self.dragging = None;
    }

    /// Update the color from the current HSV state.
    fn sync_color(&mut self) {
        self.color = Color::from_hsv(self.hue, self.saturation, self.value);
        self.changed = true;
    }

    /// Handle mouse interaction. Returns true if the picker consumed the click.
    pub fn handle_mouse(&mut self, mx: f32, my: f32, pressed: bool) -> bool {
        if !self.visible { return false; }

        let picker_w = 220.0;
        let picker_h = 260.0;
        let px = self.x;
        let py = self.y;

        // Check if mouse is inside picker
        if mx < px || mx > px + picker_w || my < py || my > py + picker_h {
            if pressed { self.close(); }
            return false;
        }

        let sv_x = px + 8.0;
        let sv_y = py + 28.0;
        let sv_size = 150.0;

        let hue_x = sv_x + sv_size + 8.0;
        let hue_y = sv_y;
        let hue_w = 20.0;
        let hue_h = sv_size;

        // SV square
        if mx >= sv_x && mx < sv_x + sv_size && my >= sv_y && my < sv_y + sv_size {
            if pressed || self.dragging == Some(0) {
                self.dragging = Some(0);
                self.saturation = ((mx - sv_x) / sv_size).clamp(0.0, 1.0);
                self.value = 1.0 - ((my - sv_y) / sv_size).clamp(0.0, 1.0);
                self.sync_color();
            }
        }

        // Hue bar
        if mx >= hue_x && mx < hue_x + hue_w && my >= hue_y && my < hue_y + hue_h {
            if pressed || self.dragging == Some(1) {
                self.dragging = Some(1);
                self.hue = ((my - hue_y) / hue_h * 360.0).clamp(0.0, 359.9);
                self.sync_color();
            }
        }

        // Preset swatches
        let swatch_y = py + 28.0 + sv_size + 12.0;
        let swatch_size = 16.0;
        let swatch_gap = 2.0;
        let cols = 6;
        for (i, preset) in PRESET_COLORS.iter().enumerate() {
            let col = i % cols;
            let row = i / cols;
            let sx = px + 8.0 + col as f32 * (swatch_size + swatch_gap);
            let sy = swatch_y + row as f32 * (swatch_size + swatch_gap);
            if mx >= sx && mx < sx + swatch_size && my >= sy && my < sy + swatch_size {
                self.hovered_preset = Some(i);
                if pressed {
                    self.color = Color::new(preset[0], preset[1], preset[2]);
                    let (h, s, v) = self.color.to_hsv();
                    self.hue = h;
                    self.saturation = s;
                    self.value = v;
                    self.changed = true;
                }
                return true;
            }
        }
        self.hovered_preset = None;

        if !pressed {
            self.dragging = None;
        }

        true
    }

    /// Draw the color picker popup.
    pub fn draw(&self, dl: &mut DrawList) {
        if !self.visible { return; }

        let picker_w = 220.0;
        let picker_h = 260.0;
        let px = self.x;
        let py = self.y;

        // Shadow
        dl.push_quad(px + 3.0, py + 3.0, picker_w, picker_h, [0.0, 0.0, 0.0, 0.3]);

        // Background
        dl.push_quad(px, py, picker_w, picker_h, [0.18, 0.18, 0.20, 0.97]);

        // Title
        emit_text(dl, "Color", px + 8.0, py + 7.0, 12.0, [0.8, 0.8, 0.8, 1.0]);

        // Current color preview
        let preview_x = px + picker_w - 50.0;
        dl.push_quad(preview_x, py + 4.0, 42.0, 18.0, self.color.to_array());

        // Hex label
        let hex = self.color.to_hex();
        emit_text(dl, &hex, preview_x - 58.0, py + 8.0, 10.0, [0.6, 0.6, 0.6, 1.0]);

        // SV square
        let sv_x = px + 8.0;
        let sv_y = py + 28.0;
        let sv_size = 150.0;

        // Draw SV gradient (approximated with horizontal strips)
        let strips = 15;
        let strip_h = sv_size / strips as f32;
        for row in 0..strips {
            let v = 1.0 - (row as f32 / strips as f32);
            for col in 0..strips {
                let s = col as f32 / strips as f32;
                let c = Color::from_hsv(self.hue, s, v);
                let sx = sv_x + col as f32 * (sv_size / strips as f32);
                let sy = sv_y + row as f32 * strip_h;
                dl.push_quad(sx, sy, sv_size / strips as f32, strip_h, c.to_array());
            }
        }

        // SV cursor
        let cursor_x = sv_x + self.saturation * sv_size;
        let cursor_y = sv_y + (1.0 - self.value) * sv_size;
        dl.push_quad(cursor_x - 4.0, cursor_y - 1.0, 8.0, 2.0, [1.0, 1.0, 1.0, 0.9]);
        dl.push_quad(cursor_x - 1.0, cursor_y - 4.0, 2.0, 8.0, [1.0, 1.0, 1.0, 0.9]);

        // Hue bar
        let hue_x = sv_x + sv_size + 8.0;
        let hue_y = sv_y;
        let hue_w = 20.0;
        let hue_h = sv_size;

        let hue_strips = 12;
        let hue_strip_h = hue_h / hue_strips as f32;
        for i in 0..hue_strips {
            let h = i as f32 / hue_strips as f32 * 360.0;
            let c = Color::from_hsv(h, 1.0, 1.0);
            dl.push_quad(hue_x, hue_y + i as f32 * hue_strip_h, hue_w, hue_strip_h, c.to_array());
        }

        // Hue cursor
        let hue_cursor_y = hue_y + (self.hue / 360.0) * hue_h;
        dl.push_quad(hue_x - 2.0, hue_cursor_y - 2.0, hue_w + 4.0, 4.0,
            [1.0, 1.0, 1.0, 0.8]);

        // Preset swatches
        let swatch_y = sv_y + sv_size + 12.0;
        let swatch_size = 16.0;
        let swatch_gap = 2.0;
        let cols = 6;

        emit_text(dl, "Presets", px + 8.0, swatch_y - 11.0, 9.0, [0.5, 0.5, 0.5, 0.7]);

        for (i, preset) in PRESET_COLORS.iter().enumerate() {
            let col = i % cols;
            let row = i / cols;
            let sx = px + 8.0 + col as f32 * (swatch_size + swatch_gap);
            let sy = swatch_y + row as f32 * (swatch_size + swatch_gap);
            let color = [preset[0], preset[1], preset[2], 1.0];
            dl.push_quad(sx, sy, swatch_size, swatch_size, color);

            // Hover highlight
            if self.hovered_preset == Some(i) {
                let border = [1.0, 1.0, 1.0, 0.8];
                dl.push_quad(sx, sy, swatch_size, 1.0, border);
                dl.push_quad(sx, sy + swatch_size - 1.0, swatch_size, 1.0, border);
                dl.push_quad(sx, sy, 1.0, swatch_size, border);
                dl.push_quad(sx + swatch_size - 1.0, sy, 1.0, swatch_size, border);
            }
        }

        // RGB values
        let rgb_y = swatch_y + ((PRESET_COLORS.len() + cols - 1) / cols) as f32 * (swatch_size + swatch_gap) + 4.0;
        let rgb_str = format!(
            "R:{:.0} G:{:.0} B:{:.0}",
            self.color.r * 255.0,
            self.color.g * 255.0,
            self.color.b * 255.0,
        );
        emit_text(dl, &rgb_str, px + 8.0, rgb_y, 10.0, [0.6, 0.6, 0.6, 1.0]);
    }
}

impl Default for ColorPicker {
    fn default() -> Self {
        Self::new()
    }
}

fn emit_text(dl: &mut DrawList, text: &str, x: f32, y: f32, size: f32, color: [f32; 4]) {
    let mut cx = x;
    for c in text.chars() {
        let params = font::CharQuadParams {
            c, x: cx, y, size, color, atlas: None,
        };
        cx += font::emit_char_quads(&params, &mut dl.vertices, &mut dl.indices);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hsv_roundtrip() {
        let c = Color::new(0.5, 0.3, 0.8);
        let (h, s, v) = c.to_hsv();
        let c2 = Color::from_hsv(h, s, v);
        assert!((c.r - c2.r).abs() < 0.02);
        assert!((c.g - c2.g).abs() < 0.02);
        assert!((c.b - c2.b).abs() < 0.02);
    }

    #[test]
    fn pure_red_hsv() {
        let c = Color::new(1.0, 0.0, 0.0);
        let (h, s, v) = c.to_hsv();
        assert!((h - 0.0).abs() < 1.0);
        assert!((s - 1.0).abs() < 0.01);
        assert!((v - 1.0).abs() < 0.01);
    }

    #[test]
    fn hex_format() {
        let c = Color::new(1.0, 0.5, 0.0);
        let hex = c.to_hex();
        assert_eq!(hex, "#FF7F00");
    }

    #[test]
    fn open_close() {
        let mut cp = ColorPicker::new();
        assert!(!cp.visible);
        cp.open(100.0, 100.0, Color::new(0.5, 0.5, 0.5));
        assert!(cp.visible);
        cp.close();
        assert!(!cp.visible);
    }

    #[test]
    fn preset_count() {
        assert!(PRESET_COLORS.len() >= 10);
    }
}
