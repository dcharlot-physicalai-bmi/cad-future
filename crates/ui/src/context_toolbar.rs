//! Context toolbar — floating toolbar near cursor on selection.
//!
//! Inspired by SolidWorks context toolbar and Shapr3D adaptive UI.
//! Appears near the selected object with relevant quick actions.

use crate::draw::DrawList;
use crate::font;

/// A button in the context toolbar.
#[derive(Clone, Debug)]
pub struct ContextButton {
    /// Display label.
    pub label: String,
    /// Action ID.
    pub action_id: &'static str,
    /// Icon character.
    pub icon: &'static str,
    /// Whether this button is a separator.
    pub separator: bool,
}

impl ContextButton {
    pub fn new(label: &str, action_id: &'static str, icon: &'static str) -> Self {
        Self {
            label: label.to_string(),
            action_id,
            icon,
            separator: false,
        }
    }

    pub fn separator() -> Self {
        Self {
            label: String::new(),
            action_id: "",
            icon: "",
            separator: true,
        }
    }
}

/// Floating context toolbar that appears near selected objects.
pub struct ContextToolbar {
    /// Whether the toolbar is visible.
    pub visible: bool,
    /// Position (screen coordinates, near the selected object).
    pub x: f32,
    pub y: f32,
    /// Buttons to display.
    pub buttons: Vec<ContextButton>,
    /// Currently hovered button index.
    pub hovered: Option<usize>,
    /// Opacity for fade-in animation.
    pub opacity: f32,
    /// Time visible (for auto-hide delay).
    pub visible_time: f32,
}

impl ContextToolbar {
    pub fn new() -> Self {
        Self {
            visible: false,
            x: 0.0,
            y: 0.0,
            buttons: Vec::new(),
            hovered: None,
            opacity: 0.0,
            visible_time: 0.0,
        }
    }

    /// Show the toolbar near a screen position with the given buttons.
    pub fn show(&mut self, x: f32, y: f32, buttons: Vec<ContextButton>) {
        self.visible = true;
        self.x = x;
        self.y = y;
        self.buttons = buttons;
        self.hovered = None;
        self.opacity = 0.0;
        self.visible_time = 0.0;
    }

    pub fn hide(&mut self) {
        self.visible = false;
        self.opacity = 0.0;
    }

    /// Update animation. Call each frame.
    pub fn update(&mut self, dt: f32) {
        if self.visible {
            self.visible_time += dt;
            // Fade in
            self.opacity = (self.opacity + dt * 6.0).min(1.0);
        } else {
            self.opacity = (self.opacity - dt * 8.0).max(0.0);
        }
    }

    /// Hit test — returns button index if mouse is over a non-separator button.
    pub fn hit_test(&self, mx: f32, my: f32) -> Option<usize> {
        if self.opacity < 0.1 { return None; }

        let btn_w = 32.0;
        let btn_h = 28.0;
        let padding = 4.0;
        let bar_h = btn_h + padding * 2.0;

        if my < self.y || my > self.y + bar_h { return None; }

        let mut cx = self.x + padding;
        for (i, btn) in self.buttons.iter().enumerate() {
            if btn.separator {
                cx += 8.0; // separator width
                continue;
            }
            if mx >= cx && mx < cx + btn_w {
                return Some(i);
            }
            cx += btn_w + 2.0;
        }
        None
    }

    /// Handle click — returns action ID if a button was clicked.
    pub fn handle_click(&self, mx: f32, my: f32) -> Option<&'static str> {
        self.hit_test(mx, my).map(|i| self.buttons[i].action_id)
    }

    /// Draw the floating toolbar.
    pub fn draw(
        &self,
        dl: &mut DrawList,
        bg_color: [f32; 4],
        text_color: [f32; 4],
        accent_color: [f32; 4],
    ) {
        if self.opacity < 0.01 { return; }

        let btn_w = 32.0;
        let btn_h = 28.0;
        let padding = 4.0;
        let sep_w = 8.0;

        // Compute total width
        let mut total_w = padding * 2.0;
        for btn in &self.buttons {
            if btn.separator {
                total_w += sep_w;
            } else {
                total_w += btn_w + 2.0;
            }
        }

        let bar_h = btn_h + padding * 2.0;
        let alpha = self.opacity;

        // Shadow
        let shadow = [0.0, 0.0, 0.0, 0.2 * alpha];
        dl.push_quad(self.x + 2.0, self.y + 2.0, total_w, bar_h, shadow);

        // Background
        let bg = [bg_color[0], bg_color[1], bg_color[2], bg_color[3] * alpha];
        dl.push_quad(self.x, self.y, total_w, bar_h, bg);

        // Border
        let border = [bg_color[0] + 0.15, bg_color[1] + 0.15, bg_color[2] + 0.15, 0.6 * alpha];
        dl.push_quad(self.x, self.y, total_w, 1.0, border);
        dl.push_quad(self.x, self.y + bar_h - 1.0, total_w, 1.0, border);
        dl.push_quad(self.x, self.y, 1.0, bar_h, border);
        dl.push_quad(self.x + total_w - 1.0, self.y, 1.0, bar_h, border);

        // Buttons
        let mut cx = self.x + padding;
        let by = self.y + padding;

        for (i, btn) in self.buttons.iter().enumerate() {
            if btn.separator {
                // Vertical separator line
                let sx = cx + sep_w * 0.5;
                let sep_col = [bg_color[0] + 0.1, bg_color[1] + 0.1, bg_color[2] + 0.1, 0.5 * alpha];
                dl.push_quad(sx, by + 4.0, 1.0, btn_h - 8.0, sep_col);
                cx += sep_w;
                continue;
            }

            let is_hovered = self.hovered == Some(i);

            // Button background
            if is_hovered {
                let hover_bg = [accent_color[0], accent_color[1], accent_color[2], 0.3 * alpha];
                dl.push_quad(cx, by, btn_w, btn_h, hover_bg);
            }

            // Icon
            let icon_color = if is_hovered {
                [accent_color[0], accent_color[1], accent_color[2], alpha]
            } else {
                [text_color[0], text_color[1], text_color[2], alpha]
            };
            let icon_x = cx + (btn_w - 8.0) * 0.5;
            let icon_y = by + (btn_h - 12.0) * 0.5;
            emit_text(dl, btn.icon, icon_x, icon_y, 12.0, icon_color);

            cx += btn_w + 2.0;
        }
    }
}

impl Default for ContextToolbar {
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
    fn show_and_hide() {
        let mut ct = ContextToolbar::new();
        ct.show(100.0, 200.0, vec![
            ContextButton::new("Move", "modify.move", "G"),
            ContextButton::new("Delete", "edit.delete", "X"),
        ]);
        assert!(ct.visible);
        ct.hide();
        assert!(!ct.visible);
    }

    #[test]
    fn fade_in() {
        let mut ct = ContextToolbar::new();
        ct.show(0.0, 0.0, vec![ContextButton::new("Test", "test", "T")]);
        assert_eq!(ct.opacity, 0.0);
        ct.update(0.1);
        assert!(ct.opacity > 0.0);
        ct.update(1.0);
        assert!((ct.opacity - 1.0).abs() < 0.01);
    }

    #[test]
    fn hit_test_buttons() {
        let mut ct = ContextToolbar::new();
        ct.show(0.0, 0.0, vec![
            ContextButton::new("A", "a", "A"),
            ContextButton::separator(),
            ContextButton::new("B", "b", "B"),
        ]);
        ct.opacity = 1.0;
        // First button: x=4..36, y=4..32
        assert_eq!(ct.hit_test(10.0, 10.0), Some(0));
        // After separator (4 + 32 + 2 + 8 = 46), button B starts at 46
        assert_eq!(ct.hit_test(50.0, 10.0), Some(2));
    }

    #[test]
    fn handle_click_returns_action() {
        let mut ct = ContextToolbar::new();
        ct.show(0.0, 0.0, vec![
            ContextButton::new("Move", "modify.move", "G"),
        ]);
        ct.opacity = 1.0;
        assert_eq!(ct.handle_click(10.0, 10.0), Some("modify.move"));
    }
}
