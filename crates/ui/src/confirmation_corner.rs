//! Confirmation corner — OK/Cancel at viewport top-right during active operations.
//!
//! Inspired by SolidWorks confirmation corner. Shows green checkmark (OK)
//! and red X (Cancel) when an operation is in progress (extrude, fillet, mate, etc.).

use crate::draw::DrawList;
use crate::font;

/// The type of active operation being confirmed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OperationType {
    /// Transform (move/rotate/scale).
    Transform,
    /// Feature creation (extrude, fillet, etc.).
    Feature,
    /// Sketch editing.
    Sketch,
    /// Mate / constraint.
    Mate,
    /// Custom operation.
    Custom(String),
}

impl OperationType {
    pub fn label(&self) -> &str {
        match self {
            Self::Transform => "Transform",
            Self::Feature => "Feature",
            Self::Sketch => "Sketch",
            Self::Mate => "Mate",
            Self::Custom(s) => s,
        }
    }
}

/// The confirmation corner overlay.
pub struct ConfirmationCorner {
    /// Whether an operation is active.
    pub active: bool,
    /// The type of operation.
    pub operation: OperationType,
    /// Hovered button: 0 = OK, 1 = Cancel, None = neither.
    pub hovered: Option<u8>,
    /// Opacity for fade-in.
    pub opacity: f32,
    /// Operation description (shown as label).
    pub description: String,
}

impl ConfirmationCorner {
    pub fn new() -> Self {
        Self {
            active: false,
            operation: OperationType::Transform,
            hovered: None,
            opacity: 0.0,
            description: String::new(),
        }
    }

    /// Begin showing the confirmation corner for an operation.
    pub fn begin(&mut self, op: OperationType, description: &str) {
        self.active = true;
        self.operation = op;
        self.description = description.to_string();
        self.opacity = 0.0;
    }

    /// End the operation (hide the corner).
    pub fn end(&mut self) {
        self.active = false;
        self.hovered = None;
    }

    /// Update animation.
    pub fn update(&mut self, dt: f32) {
        let target = if self.active { 1.0 } else { 0.0 };
        self.opacity += (target - self.opacity) * 8.0 * dt;
        if (self.opacity - target).abs() < 0.01 {
            self.opacity = target;
        }
    }

    /// Hit test: returns 0 for OK, 1 for Cancel, None for miss.
    pub fn hit_test(&self, mx: f32, my: f32, screen_w: f32) -> Option<u8> {
        if self.opacity < 0.1 { return None; }

        let btn_size = 36.0;
        let margin = 12.0;
        let gap = 6.0;

        // OK button (green check)
        let ok_x = screen_w - margin - btn_size * 2.0 - gap;
        let ok_y = margin;
        if mx >= ok_x && mx < ok_x + btn_size && my >= ok_y && my < ok_y + btn_size {
            return Some(0);
        }

        // Cancel button (red X)
        let cancel_x = screen_w - margin - btn_size;
        if mx >= cancel_x && mx < cancel_x + btn_size && my >= ok_y && my < ok_y + btn_size {
            return Some(1);
        }

        None
    }

    /// Draw the confirmation corner.
    pub fn draw(
        &self,
        dl: &mut DrawList,
        screen_w: f32,
        top_offset: f32,
    ) {
        if self.opacity < 0.01 { return; }

        let alpha = self.opacity;
        let btn_size = 36.0;
        let margin = 12.0;
        let gap = 6.0;
        let base_y = top_offset + margin;

        // OK button (green)
        let ok_x = screen_w - margin - btn_size * 2.0 - gap;
        let ok_hovered = self.hovered == Some(0);
        let ok_bg = if ok_hovered {
            [0.15, 0.75, 0.30, alpha]
        } else {
            [0.12, 0.55, 0.22, 0.85 * alpha]
        };
        dl.push_quad(ok_x, base_y, btn_size, btn_size, ok_bg);
        // Border
        dl.push_quad(ok_x, base_y, btn_size, 1.0, [0.2, 0.8, 0.3, 0.5 * alpha]);
        dl.push_quad(ok_x, base_y + btn_size - 1.0, btn_size, 1.0, [0.2, 0.8, 0.3, 0.5 * alpha]);
        dl.push_quad(ok_x, base_y, 1.0, btn_size, [0.2, 0.8, 0.3, 0.5 * alpha]);
        dl.push_quad(ok_x + btn_size - 1.0, base_y, 1.0, btn_size, [0.2, 0.8, 0.3, 0.5 * alpha]);
        // Checkmark icon
        emit_text(dl, "OK", ok_x + 8.0, base_y + 11.0, 13.0, [1.0, 1.0, 1.0, alpha]);

        // Cancel button (red)
        let cancel_x = screen_w - margin - btn_size;
        let cancel_hovered = self.hovered == Some(1);
        let cancel_bg = if cancel_hovered {
            [0.85, 0.18, 0.18, alpha]
        } else {
            [0.65, 0.15, 0.15, 0.85 * alpha]
        };
        dl.push_quad(cancel_x, base_y, btn_size, btn_size, cancel_bg);
        dl.push_quad(cancel_x, base_y, btn_size, 1.0, [0.9, 0.2, 0.2, 0.5 * alpha]);
        dl.push_quad(cancel_x, base_y + btn_size - 1.0, btn_size, 1.0, [0.9, 0.2, 0.2, 0.5 * alpha]);
        dl.push_quad(cancel_x, base_y, 1.0, btn_size, [0.9, 0.2, 0.2, 0.5 * alpha]);
        dl.push_quad(cancel_x + btn_size - 1.0, base_y, 1.0, btn_size, [0.9, 0.2, 0.2, 0.5 * alpha]);
        emit_text(dl, "X", cancel_x + 13.0, base_y + 11.0, 13.0, [1.0, 1.0, 1.0, alpha]);

        // Operation description label (below buttons)
        if !self.description.is_empty() {
            let label_w = font::measure_text(&self.description, 10.0, None);
            let lx = screen_w - margin - label_w;
            let ly = base_y + btn_size + 4.0;
            let muted = [0.6, 0.6, 0.6, 0.7 * alpha];
            emit_text(dl, &self.description, lx, ly, 10.0, muted);
        }
    }
}

impl Default for ConfirmationCorner {
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
    fn begin_and_end() {
        let mut cc = ConfirmationCorner::new();
        assert!(!cc.active);
        cc.begin(OperationType::Transform, "Move X");
        assert!(cc.active);
        assert_eq!(cc.description, "Move X");
        cc.end();
        assert!(!cc.active);
    }

    #[test]
    fn hit_test_buttons() {
        let mut cc = ConfirmationCorner::new();
        cc.active = true;
        cc.opacity = 1.0;
        // OK button: screen_w - 12 - 72 - 6 = sw - 90 to sw - 90 + 36
        // Cancel button: screen_w - 12 - 36 = sw - 48 to sw - 48 + 36
        let sw = 800.0;
        assert_eq!(cc.hit_test(sw - 80.0, 20.0, sw), Some(0)); // OK
        assert_eq!(cc.hit_test(sw - 30.0, 20.0, sw), Some(1)); // Cancel
        assert_eq!(cc.hit_test(400.0, 20.0, sw), None); // miss
    }

    #[test]
    fn fade_animation() {
        let mut cc = ConfirmationCorner::new();
        cc.begin(OperationType::Feature, "Extrude");
        cc.update(0.5);
        assert!(cc.opacity > 0.5);
    }
}
