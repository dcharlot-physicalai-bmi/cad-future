//! Flyout toolbar — toolbar buttons with expandable sub-menus.
//!
//! Inspired by SolidWorks flyout toolbars and Fusion 360 panel dropdowns.
//! Each toolbar button can have a flyout with additional related commands.

use crate::draw::DrawList;
use crate::font;

/// A single item within a flyout menu.
#[derive(Clone, Debug)]
pub struct FlyoutItem {
    /// Display label.
    pub label: String,
    /// Action ID.
    pub action_id: &'static str,
    /// Icon character.
    pub icon: &'static str,
    /// Whether this is a separator.
    pub separator: bool,
}

impl FlyoutItem {
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

/// A toolbar button that can optionally expand into a flyout.
#[derive(Clone, Debug)]
pub struct FlyoutButton {
    /// Primary label (shown on the button).
    pub label: String,
    /// Primary action ID (clicked without expanding).
    pub action_id: &'static str,
    /// Icon character.
    pub icon: &'static str,
    /// Whether this button is currently active/toggled.
    pub active: bool,
    /// Flyout items (empty = no flyout).
    pub flyout: Vec<FlyoutItem>,
}

impl FlyoutButton {
    pub fn new(label: &str, action_id: &'static str, icon: &'static str) -> Self {
        Self {
            label: label.to_string(),
            action_id,
            icon,
            active: false,
            flyout: Vec::new(),
        }
    }

    pub fn with_flyout(mut self, items: Vec<FlyoutItem>) -> Self {
        self.flyout = items;
        self
    }

    pub fn has_flyout(&self) -> bool {
        !self.flyout.is_empty()
    }
}

/// The flyout toolbar.
pub struct FlyoutToolbar {
    /// Buttons in the toolbar.
    pub buttons: Vec<FlyoutButton>,
    /// Currently open flyout (button index).
    pub open_flyout: Option<usize>,
    /// Hovered button index.
    pub hovered_button: Option<usize>,
    /// Hovered flyout item index (within the open flyout).
    pub hovered_item: Option<usize>,
    /// Orientation.
    pub vertical: bool,
}

impl FlyoutToolbar {
    pub fn new(vertical: bool) -> Self {
        Self {
            buttons: Vec::new(),
            open_flyout: None,
            hovered_button: None,
            hovered_item: None,
            vertical,
        }
    }

    pub fn add(&mut self, button: FlyoutButton) {
        self.buttons.push(button);
    }

    /// Hit test the main toolbar buttons. Returns button index.
    pub fn hit_test_button(&self, mx: f32, my: f32, x: f32, y: f32) -> Option<usize> {
        let btn_size = 32.0;
        let gap = 2.0;
        for (i, _) in self.buttons.iter().enumerate() {
            let (bx, by) = if self.vertical {
                (x, y + i as f32 * (btn_size + gap))
            } else {
                (x + i as f32 * (btn_size + gap), y)
            };
            if mx >= bx && mx < bx + btn_size && my >= by && my < by + btn_size {
                return Some(i);
            }
        }
        None
    }

    /// Hit test the open flyout menu. Returns item index.
    pub fn hit_test_flyout(&self, mx: f32, my: f32, x: f32, y: f32) -> Option<usize> {
        let Some(btn_idx) = self.open_flyout else { return None };
        let btn = &self.buttons[btn_idx];
        if btn.flyout.is_empty() { return None; }

        let btn_size = 32.0;
        let gap = 2.0;
        let item_h = 26.0;
        let flyout_w = 140.0;

        // Flyout position (to the right of a vertical toolbar)
        let (fx, fy) = if self.vertical {
            (x + btn_size + 4.0, y + btn_idx as f32 * (btn_size + gap))
        } else {
            (x + btn_idx as f32 * (btn_size + gap), y + btn_size + 4.0)
        };

        if mx < fx || mx > fx + flyout_w { return None; }

        let mut cy = fy;
        for (i, item) in btn.flyout.iter().enumerate() {
            if item.separator {
                cy += 6.0;
                continue;
            }
            if my >= cy && my < cy + item_h {
                return Some(i);
            }
            cy += item_h;
        }
        None
    }

    /// Handle a click on a button. Returns action ID or opens flyout.
    pub fn handle_button_click(&mut self, btn_idx: usize) -> Option<&'static str> {
        let btn = &self.buttons[btn_idx];
        if btn.has_flyout() {
            // Toggle flyout
            if self.open_flyout == Some(btn_idx) {
                self.open_flyout = None;
            } else {
                self.open_flyout = Some(btn_idx);
            }
            None
        } else {
            self.open_flyout = None;
            Some(btn.action_id)
        }
    }

    /// Handle a click on a flyout item. Returns action ID.
    pub fn handle_flyout_click(&mut self, item_idx: usize) -> Option<&'static str> {
        let Some(btn_idx) = self.open_flyout else { return None };
        let btn = &self.buttons[btn_idx];
        let item = &btn.flyout[item_idx];
        if item.separator { return None; }
        self.open_flyout = None;
        Some(item.action_id)
    }

    /// Close any open flyout.
    pub fn close_flyout(&mut self) {
        self.open_flyout = None;
    }

    /// Draw the toolbar and any open flyout.
    pub fn draw(
        &self,
        dl: &mut DrawList,
        x: f32,
        y: f32,
        bg_color: [f32; 4],
        text_color: [f32; 4],
        accent_color: [f32; 4],
    ) {
        let btn_size = 32.0;
        let gap = 2.0;

        // Draw buttons
        for (i, btn) in self.buttons.iter().enumerate() {
            let (bx, by) = if self.vertical {
                (x, y + i as f32 * (btn_size + gap))
            } else {
                (x + i as f32 * (btn_size + gap), y)
            };

            let is_hovered = self.hovered_button == Some(i);
            let is_open = self.open_flyout == Some(i);

            // Button background
            let bg = if btn.active || is_open {
                [accent_color[0], accent_color[1], accent_color[2], 0.25]
            } else if is_hovered {
                [bg_color[0] + 0.08, bg_color[1] + 0.08, bg_color[2] + 0.08, 1.0]
            } else {
                bg_color
            };
            dl.push_quad(bx, by, btn_size, btn_size, bg);

            // Icon
            let icon_color = if btn.active { accent_color } else { text_color };
            let ix = bx + (btn_size - 10.0) * 0.5;
            let iy = by + (btn_size - 12.0) * 0.5;
            emit_text(dl, btn.icon, ix, iy, 13.0, icon_color);

            // Flyout indicator (small triangle at corner)
            if btn.has_flyout() {
                let tri_size = 5.0;
                let tri_x = bx + btn_size - tri_size - 2.0;
                let tri_y = by + btn_size - tri_size - 2.0;
                dl.push_quad(tri_x, tri_y, tri_size, tri_size,
                    [text_color[0], text_color[1], text_color[2], 0.4]);
            }
        }

        // Draw open flyout
        if let Some(btn_idx) = self.open_flyout {
            let btn = &self.buttons[btn_idx];
            if btn.flyout.is_empty() { return; }

            let item_h = 26.0;
            let flyout_w = 140.0;
            let sep_h = 6.0;

            // Compute flyout height
            let mut total_h = 4.0; // padding
            for item in &btn.flyout {
                total_h += if item.separator { sep_h } else { item_h };
            }
            total_h += 4.0;

            // Flyout position
            let (fx, fy) = if self.vertical {
                (x + btn_size + 4.0, y + btn_idx as f32 * (btn_size + gap))
            } else {
                (x + btn_idx as f32 * (btn_size + gap), y + btn_size + 4.0)
            };

            // Shadow
            dl.push_quad(fx + 2.0, fy + 2.0, flyout_w, total_h, [0.0, 0.0, 0.0, 0.25]);

            // Background
            dl.push_quad(fx, fy, flyout_w, total_h, bg_color);

            // Border
            let border = [bg_color[0] + 0.12, bg_color[1] + 0.12, bg_color[2] + 0.12, 0.8];
            dl.push_quad(fx, fy, flyout_w, 1.0, border);
            dl.push_quad(fx, fy + total_h - 1.0, flyout_w, 1.0, border);
            dl.push_quad(fx, fy, 1.0, total_h, border);
            dl.push_quad(fx + flyout_w - 1.0, fy, 1.0, total_h, border);

            // Items
            let mut cy = fy + 4.0;
            for (i, item) in btn.flyout.iter().enumerate() {
                if item.separator {
                    let sep_y = cy + sep_h * 0.5;
                    dl.push_quad(fx + 8.0, sep_y, flyout_w - 16.0, 1.0, border);
                    cy += sep_h;
                    continue;
                }

                let is_hovered = self.hovered_item == Some(i);
                if is_hovered {
                    dl.push_quad(fx + 2.0, cy, flyout_w - 4.0, item_h, accent_color);
                }

                let label_color = if is_hovered {
                    [1.0, 1.0, 1.0, 1.0]
                } else {
                    text_color
                };

                emit_text(dl, item.icon, fx + 8.0, cy + 6.0, 12.0, label_color);
                emit_text(dl, &item.label, fx + 24.0, cy + 7.0, 11.0, label_color);

                cy += item_h;
            }
        }
    }
}

impl Default for FlyoutToolbar {
    fn default() -> Self {
        Self::new(true)
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
    fn button_without_flyout() {
        let mut tb = FlyoutToolbar::new(true);
        tb.add(FlyoutButton::new("Select", "tool.select", "S"));
        let action = tb.handle_button_click(0);
        assert_eq!(action, Some("tool.select"));
    }

    #[test]
    fn button_with_flyout_opens() {
        let mut tb = FlyoutToolbar::new(true);
        tb.add(FlyoutButton::new("Insert", "insert", "+").with_flyout(vec![
            FlyoutItem::new("Cube", "insert.cube", "#"),
            FlyoutItem::new("Sphere", "insert.sphere", "@"),
        ]));
        let action = tb.handle_button_click(0);
        assert!(action.is_none()); // opens flyout instead
        assert_eq!(tb.open_flyout, Some(0));
    }

    #[test]
    fn flyout_item_click() {
        let mut tb = FlyoutToolbar::new(true);
        tb.add(FlyoutButton::new("Insert", "insert", "+").with_flyout(vec![
            FlyoutItem::new("Cube", "insert.cube", "#"),
            FlyoutItem::new("Sphere", "insert.sphere", "@"),
        ]));
        tb.open_flyout = Some(0);
        let action = tb.handle_flyout_click(1);
        assert_eq!(action, Some("insert.sphere"));
        assert!(tb.open_flyout.is_none());
    }

    #[test]
    fn hit_test_vertical() {
        let tb = FlyoutToolbar::new(true);
        // No buttons = no hits
        assert!(tb.hit_test_button(10.0, 10.0, 0.0, 0.0).is_none());
    }
}
