//! Quick access bar — customizable pinned command toolbar at the top.
//!
//! Inspired by SolidWorks CommandManager quick access, Fusion 360 quick toolbar,
//! and AutoCAD Quick Access Toolbar. A thin row of small icon buttons above the
//! ribbon/menu that the user can customize with their most-used commands.

use crate::draw::DrawList;
use crate::font;

/// A single button in the quick access bar.
#[derive(Clone, Debug)]
pub struct QuickButton {
    /// Icon character.
    pub icon: &'static str,
    /// Tooltip text.
    pub tooltip: String,
    /// Action ID dispatched on click.
    pub action_id: &'static str,
    /// Whether this is a separator.
    pub separator: bool,
    /// Whether this button is toggled on.
    pub toggled: bool,
}

impl QuickButton {
    pub fn new(icon: &'static str, tooltip: &str, action_id: &'static str) -> Self {
        Self {
            icon,
            tooltip: tooltip.to_string(),
            action_id,
            separator: false,
            toggled: false,
        }
    }

    pub fn separator() -> Self {
        Self {
            icon: "",
            tooltip: String::new(),
            action_id: "",
            separator: true,
            toggled: false,
        }
    }

    pub fn toggled(mut self) -> Self {
        self.toggled = true;
        self
    }
}

/// The quick access bar.
pub struct QuickAccessBar {
    /// Buttons in the bar.
    pub buttons: Vec<QuickButton>,
    /// Hovered button index.
    pub hovered: Option<usize>,
    /// Height of the bar.
    pub height: f32,
    /// Whether the bar is visible.
    pub visible: bool,
    /// Button size.
    pub button_size: f32,
}

impl QuickAccessBar {
    pub fn new() -> Self {
        Self {
            buttons: Vec::new(),
            hovered: None,
            height: 24.0,
            visible: true,
            button_size: 22.0,
        }
    }

    /// Add a button.
    pub fn add(&mut self, button: QuickButton) {
        self.buttons.push(button);
    }

    /// Set default CAD quick access buttons.
    pub fn set_defaults(&mut self) {
        self.buttons.clear();
        self.add(QuickButton::new("N", "New Scene", "file.new"));
        self.add(QuickButton::separator());
        self.add(QuickButton::new("U", "Undo", "edit.undo"));
        self.add(QuickButton::new("R", "Redo", "edit.redo"));
        self.add(QuickButton::separator());
        self.add(QuickButton::new("S", "Save", "file.save"));
        self.add(QuickButton::new("E", "Export", "file.export_stl"));
        self.add(QuickButton::separator());
        self.add(QuickButton::new("F", "Zoom to Fit", "view.fit"));
        self.add(QuickButton::new("G", "Toggle Grid", "view.grid"));
        self.add(QuickButton::new("M", "Measurements", "view.measurements"));
    }

    /// Hit test: which button was clicked?
    pub fn hit_test(&self, mx: f32, my: f32, bar_x: f32, bar_y: f32) -> Option<usize> {
        if !self.visible { return None; }
        if my < bar_y || my > bar_y + self.height { return None; }

        let gap = 2.0;
        let sep_w = 8.0;
        let mut cx = bar_x + 4.0;

        for (i, btn) in self.buttons.iter().enumerate() {
            if btn.separator {
                cx += sep_w;
                continue;
            }
            if mx >= cx && mx < cx + self.button_size {
                return Some(i);
            }
            cx += self.button_size + gap;
        }
        None
    }

    /// Handle a click. Returns action ID.
    pub fn handle_click(&self, idx: usize) -> Option<&'static str> {
        let btn = &self.buttons[idx];
        if btn.separator { return None; }
        Some(btn.action_id)
    }

    /// Get the tooltip for the hovered button.
    pub fn hovered_tooltip(&self) -> Option<&str> {
        self.hovered.and_then(|i| {
            let btn = &self.buttons[i];
            if btn.separator { None } else { Some(btn.tooltip.as_str()) }
        })
    }

    /// Draw the quick access bar.
    pub fn draw(
        &self,
        dl: &mut DrawList,
        bar_x: f32,
        bar_y: f32,
        screen_w: f32,
        bg_color: [f32; 4],
        text_color: [f32; 4],
        accent_color: [f32; 4],
    ) {
        if !self.visible { return; }

        // Background
        dl.push_quad(bar_x, bar_y, screen_w, self.height, bg_color);

        // Bottom border
        let border = [bg_color[0] + 0.05, bg_color[1] + 0.05, bg_color[2] + 0.05, 0.6];
        dl.push_quad(bar_x, bar_y + self.height - 1.0, screen_w, 1.0, border);

        let gap = 2.0;
        let sep_w = 8.0;
        let btn_size = self.button_size;
        let mut cx = bar_x + 4.0;

        for (i, btn) in self.buttons.iter().enumerate() {
            if btn.separator {
                // Draw separator line
                let sx = cx + sep_w * 0.5;
                dl.push_quad(sx, bar_y + 4.0, 1.0, self.height - 8.0,
                    [text_color[0] * 0.3, text_color[1] * 0.3, text_color[2] * 0.3, 0.5]);
                cx += sep_w;
                continue;
            }

            let is_hovered = self.hovered == Some(i);
            let by = bar_y + (self.height - btn_size) * 0.5;

            // Button background
            let btn_bg = if btn.toggled {
                [accent_color[0], accent_color[1], accent_color[2], 0.25]
            } else if is_hovered {
                [bg_color[0] + 0.1, bg_color[1] + 0.1, bg_color[2] + 0.1, 1.0]
            } else {
                [0.0, 0.0, 0.0, 0.0] // transparent
            };

            if btn_bg[3] > 0.0 {
                dl.push_quad(cx, by, btn_size, btn_size, btn_bg);
            }

            // Icon
            let icon_color = if btn.toggled {
                accent_color
            } else if is_hovered {
                text_color
            } else {
                [text_color[0] * 0.7, text_color[1] * 0.7, text_color[2] * 0.7, text_color[3]]
            };

            let ix = cx + (btn_size - 8.0) * 0.5;
            let iy = by + (btn_size - 10.0) * 0.5;
            emit_text(dl, btn.icon, ix, iy, 11.0, icon_color);

            cx += btn_size + gap;
        }

        // "Quick Access" label at far right (subtle)
        let label = "Quick Access";
        let label_w = font::measure_text(label, 9.0, None);
        let lx = screen_w - label_w - 8.0;
        let muted = [text_color[0] * 0.3, text_color[1] * 0.3, text_color[2] * 0.3, text_color[3]];
        emit_text(dl, label, lx, bar_y + 7.0, 9.0, muted);
    }
}

impl Default for QuickAccessBar {
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
    fn default_buttons() {
        let mut qab = QuickAccessBar::new();
        qab.set_defaults();
        assert!(!qab.buttons.is_empty());
        // Should have some separators
        assert!(qab.buttons.iter().any(|b| b.separator));
    }

    #[test]
    fn hit_test_buttons() {
        let mut qab = QuickAccessBar::new();
        qab.add(QuickButton::new("N", "New", "file.new"));
        qab.add(QuickButton::new("U", "Undo", "edit.undo"));
        // First button at x=4..26
        assert!(qab.hit_test(10.0, 10.0, 0.0, 0.0).is_some());
    }

    #[test]
    fn separator_not_clickable() {
        let mut qab = QuickAccessBar::new();
        qab.add(QuickButton::separator());
        assert!(qab.buttons[0].separator);
        assert_eq!(qab.handle_click(0), None);
    }

    #[test]
    fn handle_click_returns_action() {
        let mut qab = QuickAccessBar::new();
        qab.add(QuickButton::new("N", "New", "file.new"));
        assert_eq!(qab.handle_click(0), Some("file.new"));
    }

    #[test]
    fn tooltip_access() {
        let mut qab = QuickAccessBar::new();
        qab.add(QuickButton::new("N", "New Scene", "file.new"));
        qab.hovered = Some(0);
        assert_eq!(qab.hovered_tooltip(), Some("New Scene"));
    }
}
