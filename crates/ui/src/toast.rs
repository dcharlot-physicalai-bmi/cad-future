//! Toast notification system — transient feedback messages.
//!
//! Shows short-lived messages that fade out automatically.
//! Used for undo/redo feedback, action confirmations, warnings.

use crate::draw::DrawList;
use crate::font;

/// Severity level for styling.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToastLevel {
    Info,
    Success,
    Warning,
    Error,
}

impl ToastLevel {
    pub fn color(self) -> [f32; 4] {
        match self {
            Self::Info => [0.15, 0.15, 0.2, 0.9],
            Self::Success => [0.1, 0.25, 0.15, 0.9],
            Self::Warning => [0.3, 0.25, 0.1, 0.9],
            Self::Error => [0.3, 0.1, 0.1, 0.9],
        }
    }

    pub fn accent(self) -> [f32; 4] {
        match self {
            Self::Info => [0.4, 0.6, 1.0, 1.0],
            Self::Success => [0.3, 0.9, 0.4, 1.0],
            Self::Warning => [1.0, 0.8, 0.2, 1.0],
            Self::Error => [1.0, 0.3, 0.3, 1.0],
        }
    }
}

/// A single toast message.
#[derive(Clone, Debug)]
pub struct Toast {
    pub message: String,
    pub level: ToastLevel,
    pub remaining: f32,
    pub duration: f32,
}

impl Toast {
    pub fn new(message: &str, level: ToastLevel, duration: f32) -> Self {
        Self {
            message: message.to_string(),
            level,
            remaining: duration,
            duration,
        }
    }

    /// Opacity based on remaining time (fade out in last 0.5s).
    pub fn opacity(&self) -> f32 {
        if self.remaining < 0.5 {
            (self.remaining / 0.5).clamp(0.0, 1.0)
        } else {
            1.0
        }
    }
}

/// Manages a stack of toast notifications.
pub struct ToastManager {
    pub toasts: Vec<Toast>,
    pub max_visible: usize,
}

impl ToastManager {
    pub fn new() -> Self {
        Self {
            toasts: Vec::new(),
            max_visible: 5,
        }
    }

    pub fn push(&mut self, message: &str, level: ToastLevel) {
        self.toasts.push(Toast::new(message, level, 3.0));
    }

    pub fn info(&mut self, message: &str) {
        self.push(message, ToastLevel::Info);
    }

    pub fn success(&mut self, message: &str) {
        self.push(message, ToastLevel::Success);
    }

    pub fn warning(&mut self, message: &str) {
        self.push(message, ToastLevel::Warning);
    }

    /// Update timers and remove expired toasts.
    pub fn update(&mut self, dt: f32) {
        for toast in &mut self.toasts {
            toast.remaining -= dt;
        }
        self.toasts.retain(|t| t.remaining > 0.0);
    }

    /// Draw toasts stacked from the bottom-right.
    pub fn draw(&self, draw: &mut DrawList, screen_w: f32, screen_h: f32) {
        let toast_w = 280.0;
        let toast_h = 32.0;
        let gap = 4.0;
        let margin = 12.0;
        let font_size = 12.0;

        let visible = self.toasts.iter().rev().take(self.max_visible);
        let mut y = screen_h - 30.0 - margin; // above status bar

        for toast in visible {
            let opacity = toast.opacity();
            let bg = toast.level.color();
            let accent = toast.level.accent();
            let x = screen_w - toast_w - margin;

            // Background with opacity
            draw.push_quad(x, y - toast_h, toast_w, toast_h,
                [bg[0], bg[1], bg[2], bg[3] * opacity]);

            // Left accent bar
            draw.push_quad(x, y - toast_h, 3.0, toast_h,
                [accent[0], accent[1], accent[2], accent[3] * opacity]);

            // Text
            let tx = x + 10.0;
            let ty = y - toast_h + (toast_h - font_size) * 0.5;
            let text_color = [0.9, 0.9, 0.9, opacity];
            let mut cx = tx;
            for c in toast.message.chars() {
                if cx > x + toast_w - 10.0 { break; } // clip
                let params = font::CharQuadParams {
                    c, x: cx, y: ty, size: font_size, color: text_color, atlas: None,
                };
                cx += font::emit_char_quads(&params, &mut draw.vertices, &mut draw.indices);
            }

            y -= toast_h + gap;
        }
    }
}

impl Default for ToastManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toast_expires() {
        let mut mgr = ToastManager::new();
        mgr.info("test");
        assert_eq!(mgr.toasts.len(), 1);
        mgr.update(4.0);
        assert!(mgr.toasts.is_empty());
    }

    #[test]
    fn toast_opacity_fades() {
        let toast = Toast::new("hi", ToastLevel::Info, 3.0);
        assert!((toast.opacity() - 1.0).abs() < 0.01);

        let mut fading = toast.clone();
        fading.remaining = 0.25;
        assert!(fading.opacity() < 1.0);
        assert!(fading.opacity() > 0.0);
    }

    #[test]
    fn max_visible() {
        let mut mgr = ToastManager::new();
        for i in 0..10 {
            mgr.info(&format!("msg {}", i));
        }
        let mut draw = DrawList::new();
        mgr.draw(&mut draw, 800.0, 600.0);
        // Should still render (just shows max_visible)
        assert!(!draw.vertices.is_empty());
    }
}
