//! Keyboard shortcut editor — customizable key bindings.
//!
//! Inspired by VS Code Keyboard Shortcuts, Blender Keymap Editor,
//! and SolidWorks Customize. Provides a searchable, categorized
//! list of commands with editable key bindings.

use crate::draw::DrawList;
use crate::font;

/// A key binding entry.
#[derive(Clone, Debug)]
pub struct KeyBinding {
    /// Command ID (e.g., "edit.undo").
    pub command: String,
    /// Display label (e.g., "Undo").
    pub label: String,
    /// Category (e.g., "Edit", "View", "Insert").
    pub category: String,
    /// Key combination string (e.g., "Ctrl+Z").
    pub keys: String,
    /// Default key combination.
    pub default_keys: String,
    /// Whether this binding is modified from default.
    pub modified: bool,
}

impl KeyBinding {
    pub fn new(command: &str, label: &str, category: &str, keys: &str) -> Self {
        Self {
            command: command.to_string(),
            label: label.to_string(),
            category: category.to_string(),
            keys: keys.to_string(),
            default_keys: keys.to_string(),
            modified: false,
        }
    }

    /// Set a new key binding.
    pub fn set_keys(&mut self, keys: &str) {
        self.keys = keys.to_string();
        self.modified = self.keys != self.default_keys;
    }

    /// Reset to default.
    pub fn reset(&mut self) {
        self.keys = self.default_keys.clone();
        self.modified = false;
    }
}

/// The shortcut editor dialog.
pub struct ShortcutEditor {
    /// Whether the dialog is visible.
    pub visible: bool,
    /// All key bindings.
    pub bindings: Vec<KeyBinding>,
    /// Search query.
    pub search: String,
    /// Selected binding index.
    pub selected: Option<usize>,
    /// Hovered binding index.
    pub hovered: Option<usize>,
    /// Whether we're recording a new key combo.
    pub recording: bool,
    /// Scroll offset.
    pub scroll_offset: usize,
    /// Dialog width.
    pub width: f32,
    /// Filter category (empty = all).
    pub filter_category: String,
}

impl ShortcutEditor {
    pub fn new() -> Self {
        Self {
            visible: false,
            bindings: Vec::new(),
            search: String::new(),
            selected: None,
            hovered: None,
            recording: false,
            scroll_offset: 0,
            width: 500.0,
            filter_category: String::new(),
        }
    }

    pub fn toggle(&mut self) {
        self.visible = !self.visible;
        self.recording = false;
    }

    /// Load default bindings.
    pub fn load_defaults(&mut self) {
        self.bindings.clear();
        // Edit
        self.bindings.push(KeyBinding::new("edit.undo", "Undo", "Edit", "Ctrl+Z"));
        self.bindings.push(KeyBinding::new("edit.redo", "Redo", "Edit", "Ctrl+Y"));
        self.bindings.push(KeyBinding::new("edit.copy", "Copy", "Edit", "Ctrl+C"));
        self.bindings.push(KeyBinding::new("edit.paste", "Paste", "Edit", "Ctrl+V"));
        self.bindings.push(KeyBinding::new("edit.delete", "Delete", "Edit", "Del"));
        self.bindings.push(KeyBinding::new("edit.select_all", "Select All", "Edit", "Ctrl+A"));
        // File
        self.bindings.push(KeyBinding::new("file.new", "New", "File", "Ctrl+N"));
        self.bindings.push(KeyBinding::new("file.open", "Open", "File", "Ctrl+O"));
        self.bindings.push(KeyBinding::new("file.save", "Save", "File", "Ctrl+S"));
        self.bindings.push(KeyBinding::new("file.export", "Export", "File", "Ctrl+Shift+E"));
        // View
        self.bindings.push(KeyBinding::new("view.fit", "Fit All", "View", "F"));
        self.bindings.push(KeyBinding::new("view.front", "Front View", "View", "1"));
        self.bindings.push(KeyBinding::new("view.right", "Right View", "View", "3"));
        self.bindings.push(KeyBinding::new("view.top", "Top View", "View", "7"));
        self.bindings.push(KeyBinding::new("view.iso", "Isometric", "View", "0"));
        // Insert
        self.bindings.push(KeyBinding::new("insert.sketch", "New Sketch", "Insert", "S"));
        self.bindings.push(KeyBinding::new("insert.extrude", "Extrude", "Insert", "E"));
        self.bindings.push(KeyBinding::new("insert.hole", "Hole", "Insert", "H"));
        // Tools
        self.bindings.push(KeyBinding::new("tools.measure", "Measure", "Tools", "M"));
        self.bindings.push(KeyBinding::new("tools.section", "Section View", "Tools", "Ctrl+Shift+X"));
    }

    /// Get filtered bindings.
    pub fn filtered(&self) -> Vec<(usize, &KeyBinding)> {
        self.bindings.iter().enumerate()
            .filter(|(_, b)| {
                if !self.search.is_empty() {
                    let q = self.search.to_lowercase();
                    if !b.label.to_lowercase().contains(&q)
                        && !b.command.to_lowercase().contains(&q)
                        && !b.keys.to_lowercase().contains(&q) {
                        return false;
                    }
                }
                if !self.filter_category.is_empty() && b.category != self.filter_category {
                    return false;
                }
                true
            })
            .collect()
    }

    /// Get unique categories.
    pub fn categories(&self) -> Vec<String> {
        let mut cats: Vec<String> = self.bindings.iter()
            .map(|b| b.category.clone())
            .collect();
        cats.sort();
        cats.dedup();
        cats
    }

    /// Count of modified bindings.
    pub fn modified_count(&self) -> usize {
        self.bindings.iter().filter(|b| b.modified).count()
    }

    /// Reset all to defaults.
    pub fn reset_all(&mut self) {
        for b in &mut self.bindings {
            b.reset();
        }
    }

    /// Draw the shortcut editor.
    pub fn draw(
        &self,
        dl: &mut DrawList,
        screen_w: f32,
        screen_h: f32,
        bg_color: [f32; 4],
        text_color: [f32; 4],
        accent_color: [f32; 4],
    ) {
        if !self.visible { return; }

        let panel_h = 440.0;
        let px = (screen_w - self.width) * 0.5;
        let py = (screen_h - panel_h) * 0.5;

        // Shadow + background
        dl.push_quad(px + 3.0, py + 3.0, self.width, panel_h, [0.0, 0.0, 0.0, 0.25]);
        dl.push_quad(px, py, self.width, panel_h, bg_color);

        let border = [bg_color[0] + 0.1, bg_color[1] + 0.1, bg_color[2] + 0.1, 0.8];
        dl.push_quad(px, py, self.width, 1.0, border);
        dl.push_quad(px, py + panel_h - 1.0, self.width, 1.0, border);
        dl.push_quad(px, py, 1.0, panel_h, border);
        dl.push_quad(px + self.width - 1.0, py, 1.0, panel_h, border);

        let muted = [text_color[0] * 0.5, text_color[1] * 0.5, text_color[2] * 0.5, text_color[3]];

        // Title
        emit_text(dl, "Keyboard Shortcuts", px + 8.0, py + 6.0, 12.0, text_color);

        if self.modified_count() > 0 {
            let mod_str = format!("{} modified", self.modified_count());
            let mw = font::measure_text(&mod_str, 8.0, None);
            emit_text(dl, &mod_str, px + self.width - mw - 8.0, py + 9.0, 8.0, accent_color);
        }

        // Search bar
        dl.push_quad(px + 8.0, py + 26.0, self.width - 16.0, 20.0, [0.15, 0.15, 0.15, 0.5]);
        let search_text = if self.search.is_empty() { "Search shortcuts..." } else { &self.search };
        emit_text(dl, search_text, px + 12.0, py + 30.0, 9.0,
            if self.search.is_empty() { muted } else { text_color });

        // Category filter tabs
        let tab_y = py + 52.0;
        let mut tx = px + 8.0;
        // "All" tab
        {
            let is_active = self.filter_category.is_empty();
            let tab_color = if is_active { accent_color } else { muted };
            emit_text(dl, "All", tx, tab_y + 2.0, 9.0, tab_color);
            if is_active {
                dl.push_quad(tx, tab_y + 14.0, font::measure_text("All", 9.0, None), 2.0, accent_color);
            }
            tx += 32.0;
        }
        for cat in &["Edit", "File", "View", "Insert", "Tools"] {
            let is_active = self.filter_category == *cat;
            let tab_color = if is_active { accent_color } else { muted };
            emit_text(dl, cat, tx, tab_y + 2.0, 9.0, tab_color);
            if is_active {
                dl.push_quad(tx, tab_y + 14.0, font::measure_text(cat, 9.0, None), 2.0, accent_color);
            }
            tx += font::measure_text(cat, 9.0, None) + 12.0;
        }

        // Column headers
        let header_y = py + 72.0;
        dl.push_quad(px + 8.0, header_y, self.width - 16.0, 18.0,
            [bg_color[0] + 0.02, bg_color[1] + 0.02, bg_color[2] + 0.02, 1.0]);
        emit_text(dl, "Command", px + 12.0, header_y + 3.0, 8.0, muted);
        emit_text(dl, "Shortcut", px + 280.0, header_y + 3.0, 8.0, muted);
        emit_text(dl, "Category", px + 400.0, header_y + 3.0, 8.0, muted);

        // Binding rows
        let row_h = 22.0;
        let filtered = self.filtered();
        let max_rows = ((panel_h - 132.0) / row_h) as usize;
        let end = (self.scroll_offset + max_rows).min(filtered.len());

        for vis_i in self.scroll_offset..end {
            let (real_i, binding) = filtered[vis_i];
            let row = (vis_i - self.scroll_offset) as f32;
            let ry = py + 92.0 + row * row_h;

            let is_sel = self.selected == Some(real_i);
            let is_hov = self.hovered == Some(real_i);

            if is_sel {
                dl.push_quad(px + 8.0, ry, self.width - 16.0, row_h,
                    [accent_color[0] * 0.2, accent_color[1] * 0.2, accent_color[2] * 0.2, 0.4]);
            } else if is_hov {
                dl.push_quad(px + 8.0, ry, self.width - 16.0, row_h, [1.0, 1.0, 1.0, 0.04]);
            }

            // Modified indicator
            if binding.modified {
                dl.push_quad(px + 8.0, ry + 8.0, 3.0, 3.0, accent_color);
            }

            // Label
            emit_text(dl, &binding.label, px + 16.0, ry + 4.0, 9.0, text_color);

            // Keys
            let key_bg = if is_sel && self.recording {
                [accent_color[0] * 0.3, accent_color[1] * 0.3, accent_color[2] * 0.3, 0.5]
            } else {
                [0.2, 0.2, 0.2, 0.4]
            };
            let kw = font::measure_text(&binding.keys, 9.0, None);
            dl.push_quad(px + 276.0, ry + 2.0, kw + 8.0, 16.0, key_bg);
            let kc = if binding.modified { accent_color } else { text_color };
            emit_text(dl, &binding.keys, px + 280.0, ry + 4.0, 9.0, kc);

            // Category
            emit_text(dl, &binding.category, px + 400.0, ry + 4.0, 8.0, muted);
        }

        // Recording indicator
        if self.recording {
            let rec_y = py + panel_h - 60.0;
            dl.push_quad(px + 8.0, rec_y, self.width - 16.0, 20.0,
                [accent_color[0] * 0.15, accent_color[1] * 0.15, accent_color[2] * 0.15, 0.8]);
            emit_text(dl, "Press key combination... (Esc to cancel)", px + 12.0, rec_y + 4.0, 9.0, accent_color);
        }

        // Close button
        let btn_y = py + panel_h - 34.0;
        dl.push_quad(px + self.width - 76.0, btn_y, 68.0, 24.0, [0.4, 0.4, 0.4, 0.6]);
        emit_text(dl, "Close", px + self.width - 60.0, btn_y + 5.0, 11.0, text_color);

        // Reset all
        dl.push_quad(px + 8.0, btn_y, 80.0, 24.0, [0.4, 0.3, 0.3, 0.5]);
        emit_text(dl, "Reset All", px + 16.0, btn_y + 5.0, 10.0, text_color);
    }
}

impl Default for ShortcutEditor {
    fn default() -> Self { Self::new() }
}

fn emit_text(dl: &mut DrawList, text: &str, x: f32, y: f32, size: f32, color: [f32; 4]) {
    let mut cx = x;
    for c in text.chars() {
        let params = font::CharQuadParams { c, x: cx, y, size, color, atlas: None };
        cx += font::emit_char_quads(&params, &mut dl.vertices, &mut dl.indices);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_defaults() {
        let mut se = ShortcutEditor::new();
        se.load_defaults();
        assert!(se.bindings.len() >= 15);
    }

    #[test]
    fn search_filter() {
        let mut se = ShortcutEditor::new();
        se.load_defaults();
        se.search = "undo".to_string();
        let filtered = se.filtered();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].1.command, "edit.undo");
    }

    #[test]
    fn modify_binding() {
        let mut kb = KeyBinding::new("test", "Test", "Test", "Ctrl+T");
        assert!(!kb.modified);
        kb.set_keys("Ctrl+Shift+T");
        assert!(kb.modified);
        kb.reset();
        assert!(!kb.modified);
        assert_eq!(kb.keys, "Ctrl+T");
    }

    #[test]
    fn categories_list() {
        let mut se = ShortcutEditor::new();
        se.load_defaults();
        let cats = se.categories();
        assert!(cats.contains(&"Edit".to_string()));
        assert!(cats.contains(&"View".to_string()));
    }

    #[test]
    fn modified_count() {
        let mut se = ShortcutEditor::new();
        se.load_defaults();
        assert_eq!(se.modified_count(), 0);
        se.bindings[0].set_keys("Ctrl+Shift+Z");
        assert_eq!(se.modified_count(), 1);
        se.reset_all();
        assert_eq!(se.modified_count(), 0);
    }
}
