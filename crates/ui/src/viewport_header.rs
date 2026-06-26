//! Viewport header bar — horizontal strip at top of viewport with mode controls.
//!
//! Matches the top toolbar pattern found in Blender, Zoo Studio, and Foxglove.

use crate::draw::DrawList;
use crate::font;

/// A clickable button in the viewport header.
#[derive(Clone, Debug)]
pub struct HeaderButton {
    pub label: String,
    pub active: bool,
    pub width: f32,
}

impl HeaderButton {
    pub fn new(label: &str) -> Self {
        Self {
            label: label.to_string(),
            active: false,
            width: font::measure_text(label, 11.0, None) + 16.0,
        }
    }

    pub fn active(mut self) -> Self {
        self.active = true;
        self
    }
}

/// The viewport header bar state.
pub struct ViewportHeader {
    pub buttons: Vec<HeaderButton>,
    pub hovered: Option<usize>,
}

impl ViewportHeader {
    pub fn new() -> Self {
        Self {
            buttons: Vec::new(),
            hovered: None,
        }
    }

    pub fn set_buttons(&mut self, buttons: Vec<HeaderButton>) {
        self.buttons = buttons;
    }

    pub fn hit_test(&self, mx: f32, my: f32, bar_x: f32, bar_y: f32) -> Option<usize> {
        let bar_h = 24.0;
        if my < bar_y || my > bar_y + bar_h { return None; }

        let mut cx = bar_x + 4.0;
        for (i, btn) in self.buttons.iter().enumerate() {
            if mx >= cx && mx <= cx + btn.width {
                return Some(i);
            }
            cx += btn.width + 2.0;
        }
        None
    }

    pub fn draw(
        &self,
        draw: &mut DrawList,
        x: f32, y: f32,
        w: f32,
        bg: [f32; 4],
        text_color: [f32; 4],
    ) {
        let bar_h = 24.0;
        let font_size = 11.0;

        // Background
        draw.push_quad(x, y, w, bar_h, bg);
        // Bottom border
        draw.push_quad(x, y + bar_h - 1.0, w, 1.0, [bg[0] + 0.1, bg[1] + 0.1, bg[2] + 0.1, 0.6]);

        let mut cx = x + 4.0;
        for (i, btn) in self.buttons.iter().enumerate() {
            let btn_bg = if btn.active {
                [bg[0] + 0.15, bg[1] + 0.15, bg[2] + 0.2, bg[3]]
            } else if self.hovered == Some(i) {
                [bg[0] + 0.08, bg[1] + 0.08, bg[2] + 0.1, bg[3]]
            } else {
                [0.0; 4] // transparent
            };

            if btn.active || self.hovered == Some(i) {
                draw.push_quad(cx, y + 2.0, btn.width, bar_h - 4.0, btn_bg);
            }

            // Active indicator
            if btn.active {
                draw.push_quad(cx, y + bar_h - 2.0, btn.width, 2.0, [0.3, 0.6, 1.0, 1.0]);
            }

            // Text
            let text_w = font::measure_text(&btn.label, font_size, None);
            let tx = cx + (btn.width - text_w) * 0.5;
            let ty = y + (bar_h - font_size) * 0.5;
            let color = if btn.active {
                [text_color[0], text_color[1], text_color[2], 1.0]
            } else {
                [text_color[0] * 0.7, text_color[1] * 0.7, text_color[2] * 0.7, text_color[3]]
            };

            let mut tcx = tx;
            for c in btn.label.chars() {
                let params = font::CharQuadParams {
                    c, x: tcx, y: ty, size: font_size, color, atlas: None,
                };
                tcx += font::emit_char_quads(&params, &mut draw.vertices, &mut draw.indices);
            }

            cx += btn.width + 2.0;
        }

        // Separator: right-aligned label "OpenIE"
        let brand = "OpenIE";
        let brand_w = font::measure_text(brand, font_size, None);
        let bx = x + w - brand_w - 8.0;
        let by = y + (bar_h - font_size) * 0.5;
        let brand_color = [text_color[0] * 0.4, text_color[1] * 0.4, text_color[2] * 0.4, text_color[3]];
        let mut bcx = bx;
        for c in brand.chars() {
            let params = font::CharQuadParams {
                c, x: bcx, y: by, size: font_size, color: brand_color, atlas: None,
            };
            bcx += font::emit_char_quads(&params, &mut draw.vertices, &mut draw.indices);
        }
    }
}

impl Default for ViewportHeader {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_hit_test() {
        let mut hdr = ViewportHeader::new();
        hdr.set_buttons(vec![
            HeaderButton::new("Solid"),
            HeaderButton::new("Ortho"),
        ]);
        // Should hit first button
        assert_eq!(hdr.hit_test(10.0, 10.0, 0.0, 0.0), Some(0));
        // Outside bar
        assert!(hdr.hit_test(10.0, 50.0, 0.0, 0.0).is_none());
    }

    #[test]
    fn draw_produces_geometry() {
        let mut hdr = ViewportHeader::new();
        hdr.set_buttons(vec![HeaderButton::new("Solid").active()]);
        let mut draw = DrawList::new();
        hdr.draw(&mut draw, 0.0, 0.0, 800.0, [0.1; 4], [0.8; 4]);
        assert!(!draw.vertices.is_empty());
    }
}
