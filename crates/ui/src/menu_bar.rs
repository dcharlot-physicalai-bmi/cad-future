//! Menu bar — Fusion/SolidWorks-style top menu with dropdown submenus.
//!
//! Renders a horizontal menu strip. Clicking a menu label opens a dropdown
//! with items. Items can have shortcuts, separators, and submenus.

use crate::draw::DrawList;
use crate::font;

/// A single item in a dropdown menu.
#[derive(Clone, Debug)]
pub struct MenuItemEntry {
    pub label: String,
    pub shortcut: String,
    pub separator: bool,
    pub enabled: bool,
    pub id: &'static str,
}

impl MenuItemEntry {
    pub fn action(label: &str, id: &'static str) -> Self {
        Self {
            label: label.to_string(),
            shortcut: String::new(),
            separator: false,
            enabled: true,
            id,
        }
    }

    pub fn with_shortcut(mut self, shortcut: &str) -> Self {
        self.shortcut = shortcut.to_string();
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    pub fn separator() -> Self {
        Self {
            label: String::new(),
            shortcut: String::new(),
            separator: true,
            enabled: false,
            id: "",
        }
    }
}

/// A top-level menu (e.g., "File", "Edit").
#[derive(Clone, Debug)]
pub struct Menu {
    pub label: String,
    pub items: Vec<MenuItemEntry>,
}

impl Menu {
    pub fn new(label: &str, items: Vec<MenuItemEntry>) -> Self {
        Self {
            label: label.to_string(),
            items,
        }
    }
}

/// The full menu bar state.
pub struct MenuBar {
    pub menus: Vec<Menu>,
    /// Index of the currently open dropdown (None = all closed).
    pub open_menu: Option<usize>,
    /// Hovered item within the open dropdown.
    pub hovered_item: Option<usize>,
    /// Hovered top-level menu label.
    pub hovered_menu: Option<usize>,
    /// Menu bar height.
    pub height: f32,
}

impl MenuBar {
    pub fn new(menus: Vec<Menu>) -> Self {
        Self {
            menus,
            open_menu: None,
            hovered_item: None,
            hovered_menu: None,
            height: 24.0,
        }
    }

    /// Close all menus.
    pub fn close(&mut self) {
        self.open_menu = None;
        self.hovered_item = None;
    }

    /// Hit-test the top-level menu labels. Returns menu index if hit.
    pub fn hit_test_bar(&self, mx: f32, my: f32) -> Option<usize> {
        if my > self.height {
            return None;
        }
        let mut x = 8.0;
        for (i, menu) in self.menus.iter().enumerate() {
            let w = font::measure_text(&menu.label, 12.0, None) + 16.0;
            if mx >= x && mx < x + w && my >= 0.0 && my < self.height {
                return Some(i);
            }
            x += w;
        }
        None
    }

    /// Hit-test the open dropdown. Returns item index if hit (skips separators).
    pub fn hit_test_dropdown(&self, mx: f32, my: f32) -> Option<usize> {
        let menu_idx = self.open_menu?;
        let menu = &self.menus[menu_idx];

        let (drop_x, drop_y) = self.dropdown_position(menu_idx);
        let drop_w = self.dropdown_width(menu);
        let item_h = 24.0;
        let sep_h = 8.0;

        let mut y = drop_y;
        for (i, item) in menu.items.iter().enumerate() {
            let h = if item.separator { sep_h } else { item_h };
            if !item.separator && item.enabled && mx >= drop_x && mx < drop_x + drop_w
                && my >= y && my < y + h
            {
                return Some(i);
            }
            y += h;
        }
        None
    }

    fn dropdown_position(&self, menu_idx: usize) -> (f32, f32) {
        let mut x = 8.0;
        for i in 0..menu_idx {
            x += font::measure_text(&self.menus[i].label, 12.0, None) + 16.0;
        }
        (x, self.height)
    }

    fn dropdown_width(&self, menu: &Menu) -> f32 {
        let mut max_w: f32 = 120.0;
        for item in &menu.items {
            if item.separator {
                continue;
            }
            let label_w = font::measure_text(&item.label, 11.0, None);
            let shortcut_w = if item.shortcut.is_empty() {
                0.0
            } else {
                font::measure_text(&item.shortcut, 10.0, None) + 20.0
            };
            max_w = max_w.max(label_w + shortcut_w + 32.0);
        }
        max_w
    }

    fn dropdown_height(&self, menu: &Menu) -> f32 {
        let item_h = 24.0;
        let sep_h = 8.0;
        menu.items.iter().map(|item| if item.separator { sep_h } else { item_h }).sum()
    }

    /// Handle mouse hover — updates hovered_menu and hovered_item.
    pub fn handle_hover(&mut self, mx: f32, my: f32) {
        self.hovered_menu = self.hit_test_bar(mx, my);

        // If a menu is open and we hover a different label, switch to it
        if self.open_menu.is_some() {
            if let Some(idx) = self.hovered_menu {
                self.open_menu = Some(idx);
                self.hovered_item = None;
            }
        }

        // Hover within dropdown
        if self.open_menu.is_some() {
            self.hovered_item = self.hit_test_dropdown(mx, my);
        }
    }

    /// Handle click. Returns the action ID if a menu item was clicked.
    pub fn handle_click(&mut self, mx: f32, my: f32) -> Option<&'static str> {
        // Click on bar label
        if let Some(idx) = self.hit_test_bar(mx, my) {
            if self.open_menu == Some(idx) {
                self.close();
            } else {
                self.open_menu = Some(idx);
                self.hovered_item = None;
            }
            return None;
        }

        // Click on dropdown item
        if let Some(item_idx) = self.hit_test_dropdown(mx, my) {
            let menu_idx = self.open_menu.unwrap();
            let item = &self.menus[menu_idx].items[item_idx];
            if item.enabled && !item.separator {
                let id = item.id;
                self.close();
                return Some(id);
            }
        }

        // Click outside — close
        if self.open_menu.is_some() {
            self.close();
        }

        None
    }

    /// Draw the menu bar and open dropdown.
    pub fn draw(&self, draw: &mut DrawList, screen_w: f32, bar_bg: [f32; 4], text_color: [f32; 4]) {
        // Bar background
        draw.push_quad(0.0, 0.0, screen_w, self.height, bar_bg);
        // Bottom border
        draw.push_quad(0.0, self.height - 1.0, screen_w, 1.0, [0.2, 0.2, 0.25, 0.6]);

        // Menu labels
        let mut x = 8.0;
        for (i, menu) in self.menus.iter().enumerate() {
            let w = font::measure_text(&menu.label, 12.0, None) + 16.0;

            // Highlight active/hovered
            let is_open = self.open_menu == Some(i);
            let is_hover = self.hovered_menu == Some(i);
            if is_open {
                draw.push_quad(x, 0.0, w, self.height, [0.25, 0.35, 0.55, 0.8]);
            } else if is_hover {
                draw.push_quad(x, 0.0, w, self.height, [0.2, 0.2, 0.3, 0.5]);
            }

            let label_color = if is_open {
                [1.0, 1.0, 1.0, 1.0]
            } else {
                text_color
            };

            let tx = x + 8.0;
            let ty = (self.height - 12.0) * 0.5;
            let mut cx = tx;
            for c in menu.label.chars() {
                let params = font::CharQuadParams {
                    c, x: cx, y: ty, size: 12.0, color: label_color, atlas: None,
                };
                cx += font::emit_char_quads(&params, &mut draw.vertices, &mut draw.indices);
            }

            x += w;
        }

        // Dropdown
        if let Some(menu_idx) = self.open_menu {
            let menu = &self.menus[menu_idx];
            let (drop_x, drop_y) = self.dropdown_position(menu_idx);
            let drop_w = self.dropdown_width(menu);
            let drop_h = self.dropdown_height(menu);

            // Shadow
            draw.push_quad(drop_x + 2.0, drop_y + 2.0, drop_w, drop_h, [0.0, 0.0, 0.0, 0.3]);
            // Background
            draw.push_quad(drop_x, drop_y, drop_w, drop_h, [0.14, 0.14, 0.17, 0.97]);
            // Border
            draw.push_quad(drop_x, drop_y, drop_w, 1.0, [0.3, 0.3, 0.4, 0.6]);
            draw.push_quad(drop_x, drop_y + drop_h - 1.0, drop_w, 1.0, [0.3, 0.3, 0.4, 0.6]);
            draw.push_quad(drop_x, drop_y, 1.0, drop_h, [0.3, 0.3, 0.4, 0.6]);
            draw.push_quad(drop_x + drop_w - 1.0, drop_y, 1.0, drop_h, [0.3, 0.3, 0.4, 0.6]);

            let item_h = 24.0;
            let sep_h = 8.0;
            let mut y = drop_y;

            for (i, item) in menu.items.iter().enumerate() {
                if item.separator {
                    let mid_y = y + sep_h * 0.5;
                    draw.push_quad(drop_x + 8.0, mid_y, drop_w - 16.0, 1.0, [0.3, 0.3, 0.35, 0.5]);
                    y += sep_h;
                    continue;
                }

                // Hover highlight
                if self.hovered_item == Some(i) && item.enabled {
                    draw.push_quad(drop_x + 2.0, y, drop_w - 4.0, item_h, [0.25, 0.35, 0.55, 0.7]);
                }

                let alpha = if item.enabled { 1.0 } else { 0.4 };

                // Label
                let tx = drop_x + 12.0;
                let ty = y + (item_h - 11.0) * 0.5;
                let mut cx = tx;
                for c in item.label.chars() {
                    let params = font::CharQuadParams {
                        c, x: cx, y: ty, size: 11.0,
                        color: [text_color[0], text_color[1], text_color[2], alpha],
                        atlas: None,
                    };
                    cx += font::emit_char_quads(&params, &mut draw.vertices, &mut draw.indices);
                }

                // Shortcut (right-aligned)
                if !item.shortcut.is_empty() {
                    let sw = font::measure_text(&item.shortcut, 10.0, None);
                    let sx = drop_x + drop_w - sw - 12.0;
                    let mut scx = sx;
                    for c in item.shortcut.chars() {
                        let params = font::CharQuadParams {
                            c, x: scx, y: ty + 1.0, size: 10.0,
                            color: [0.5, 0.5, 0.6, alpha],
                            atlas: None,
                        };
                        scx += font::emit_char_quads(&params, &mut draw.vertices, &mut draw.indices);
                    }
                }

                y += item_h;
            }
        }
    }
}

impl Default for MenuBar {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_menus() -> Vec<Menu> {
        vec![
            Menu::new("File", vec![
                MenuItemEntry::action("New", "file.new"),
                MenuItemEntry::separator(),
                MenuItemEntry::action("Save", "file.save").with_shortcut("Ctrl+S"),
            ]),
            Menu::new("Edit", vec![
                MenuItemEntry::action("Undo", "edit.undo").with_shortcut("Ctrl+Z"),
            ]),
        ]
    }

    #[test]
    fn bar_hit_test() {
        let bar = MenuBar::new(test_menus());
        // Should hit "File" label area
        let hit = bar.hit_test_bar(20.0, 10.0);
        assert!(hit.is_some());
    }

    #[test]
    fn click_opens_menu() {
        let mut bar = MenuBar::new(test_menus());
        assert!(bar.open_menu.is_none());
        bar.handle_click(20.0, 10.0);
        assert_eq!(bar.open_menu, Some(0));
    }

    #[test]
    fn click_again_closes() {
        let mut bar = MenuBar::new(test_menus());
        bar.handle_click(20.0, 10.0);
        assert_eq!(bar.open_menu, Some(0));
        bar.handle_click(20.0, 10.0);
        assert!(bar.open_menu.is_none());
    }

    #[test]
    fn click_outside_closes() {
        let mut bar = MenuBar::new(test_menus());
        bar.open_menu = Some(0);
        bar.handle_click(500.0, 300.0);
        assert!(bar.open_menu.is_none());
    }
}
