//! Keyboard-driven numeric transform input — Blender-style precision entry.
//!
//! When a transform mode is active (G/R/S), the user can:
//! - Type a number (e.g., "5") → apply that magnitude
//! - Press X/Y/Z → constrain to an axis
//! - Press Enter → confirm
//! - Press Esc → cancel
//! - Press Tab → cycle axis
//! - Press minus → negate

use crate::draw::DrawList;
use crate::font;

/// Transform input mode (which tool is active).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransformMode {
    Translate,
    Rotate,
    Scale,
}

impl TransformMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Translate => "Move",
            Self::Rotate => "Rotate",
            Self::Scale => "Scale",
        }
    }

    pub fn unit(self) -> &'static str {
        match self {
            Self::Translate => "m",
            Self::Rotate => "°",
            Self::Scale => "×",
        }
    }
}

/// Axis constraint for numeric input.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AxisConstraint {
    None,
    X,
    Y,
    Z,
}

impl AxisConstraint {
    pub fn label(self) -> &'static str {
        match self {
            Self::None => "",
            Self::X => "X",
            Self::Y => "Y",
            Self::Z => "Z",
        }
    }

    pub fn color(self) -> [f32; 4] {
        match self {
            Self::None => [0.8, 0.8, 0.8, 1.0],
            Self::X => [1.0, 0.3, 0.3, 1.0],
            Self::Y => [0.3, 1.0, 0.3, 1.0],
            Self::Z => [0.3, 0.5, 1.0, 1.0],
        }
    }

    pub fn cycle(self) -> Self {
        match self {
            Self::None => Self::X,
            Self::X => Self::Y,
            Self::Y => Self::Z,
            Self::Z => Self::None,
        }
    }
}

/// State for keyboard-driven numeric transform entry.
pub struct TransformInput {
    /// Whether numeric input is actively being captured.
    pub active: bool,
    /// The transform mode that triggered this input.
    pub mode: TransformMode,
    /// Current typed numeric string.
    pub buffer: String,
    /// Axis constraint.
    pub axis: AxisConstraint,
    /// Whether negative is toggled.
    pub negative: bool,
}

impl TransformInput {
    pub fn new() -> Self {
        Self {
            active: false,
            mode: TransformMode::Translate,
            buffer: String::new(),
            axis: AxisConstraint::None,
            negative: false,
        }
    }

    /// Start numeric input for a given mode.
    pub fn begin(&mut self, mode: TransformMode) {
        self.active = true;
        self.mode = mode;
        self.buffer.clear();
        self.axis = AxisConstraint::None;
        self.negative = false;
    }

    /// Cancel and discard input.
    pub fn cancel(&mut self) {
        self.active = false;
        self.buffer.clear();
    }

    /// Confirm input. Returns the parsed value (or None if empty/invalid).
    pub fn confirm(&mut self) -> Option<f32> {
        self.active = false;
        let val = self.value();
        self.buffer.clear();
        val
    }

    /// Insert a digit or decimal point.
    pub fn push_char(&mut self, c: char) {
        if c.is_ascii_digit() || (c == '.' && !self.buffer.contains('.')) {
            self.buffer.push(c);
        }
    }

    /// Remove last character.
    pub fn backspace(&mut self) {
        self.buffer.pop();
    }

    /// Toggle negative.
    pub fn toggle_negative(&mut self) {
        self.negative = !self.negative;
    }

    /// Set axis constraint.
    pub fn set_axis(&mut self, axis: AxisConstraint) {
        self.axis = axis;
    }

    /// Parse the current buffer to a numeric value.
    pub fn value(&self) -> Option<f32> {
        if self.buffer.is_empty() {
            return None;
        }
        self.buffer.parse::<f32>().ok().map(|v| {
            if self.negative { -v } else { v }
        })
    }

    /// Display string showing current input state.
    pub fn display(&self) -> String {
        let sign = if self.negative { "-" } else { "" };
        let val = if self.buffer.is_empty() { "..." } else { &self.buffer };
        let axis = self.axis.label();
        let unit = self.mode.unit();

        if axis.is_empty() {
            format!("{} {}{} {}", self.mode.label(), sign, val, unit)
        } else {
            format!("{} {} {}{} {}", self.mode.label(), axis, sign, val, unit)
        }
    }

    /// Draw the input overlay — a compact banner near the cursor or at screen center-bottom.
    pub fn draw(&self, draw: &mut DrawList, screen_w: f32, screen_h: f32) {
        if !self.active {
            return;
        }

        let text = self.display();
        let font_size = 14.0;
        let pad = 12.0;
        let text_w = font::measure_text(&text, font_size, None);
        let banner_w = text_w + pad * 2.0;
        let banner_h = 28.0;

        let x = (screen_w - banner_w) * 0.5;
        let y = screen_h - 80.0;

        // Background
        draw.push_quad(x, y, banner_w, banner_h, [0.1, 0.1, 0.15, 0.9]);

        // Accent bar (axis-colored)
        draw.push_quad(x, y, banner_w, 2.0, self.axis.color());

        // Text
        let tx = x + pad;
        let ty = y + (banner_h - font_size) * 0.5;
        let mut cx = tx;
        for c in text.chars() {
            let params = font::CharQuadParams {
                c,
                x: cx,
                y: ty,
                size: font_size,
                color: [0.95, 0.95, 0.95, 1.0],
                atlas: None,
            };
            cx += font::emit_char_quads(&params, &mut draw.vertices, &mut draw.indices);
        }
    }
}

impl Default for TransformInput {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numeric_entry() {
        let mut ti = TransformInput::new();
        ti.begin(TransformMode::Translate);
        ti.push_char('5');
        ti.push_char('.');
        ti.push_char('3');
        assert_eq!(ti.value(), Some(5.3));
    }

    #[test]
    fn negative_toggle() {
        let mut ti = TransformInput::new();
        ti.begin(TransformMode::Translate);
        ti.push_char('2');
        ti.toggle_negative();
        assert_eq!(ti.value(), Some(-2.0));
    }

    #[test]
    fn no_double_decimal() {
        let mut ti = TransformInput::new();
        ti.begin(TransformMode::Scale);
        ti.push_char('1');
        ti.push_char('.');
        ti.push_char('.');
        ti.push_char('5');
        assert_eq!(ti.buffer, "1.5");
    }

    #[test]
    fn confirm_clears() {
        let mut ti = TransformInput::new();
        ti.begin(TransformMode::Rotate);
        ti.push_char('9');
        ti.push_char('0');
        let val = ti.confirm();
        assert_eq!(val, Some(90.0));
        assert!(!ti.active);
    }

    #[test]
    fn display_format() {
        let mut ti = TransformInput::new();
        ti.begin(TransformMode::Translate);
        ti.set_axis(AxisConstraint::X);
        ti.push_char('3');
        assert_eq!(ti.display(), "Move X 3 m");
    }
}
