//! Keyboard shortcut help overlay — toggleable cheatsheet.
//!
//! Shows all available shortcuts in a categorized overlay.

use crate::draw::DrawList;
use crate::font;

/// A shortcut entry for display.
pub struct ShortcutEntry {
    pub key: &'static str,
    pub description: &'static str,
}

/// The help overlay state.
pub struct ShortcutHelp {
    pub visible: bool,
}

impl ShortcutHelp {
    pub fn new() -> Self {
        Self { visible: false }
    }

    pub fn toggle(&mut self) {
        self.visible = !self.visible;
    }

    pub fn draw(&self, draw: &mut DrawList, screen_w: f32, screen_h: f32) {
        if !self.visible {
            return;
        }

        let sections: &[(&str, &[ShortcutEntry])] = &[
            ("Navigation", &[
                ShortcutEntry { key: "RMB Drag", description: "Orbit camera" },
                ShortcutEntry { key: "MMB Drag", description: "Pan camera" },
                ShortcutEntry { key: "Scroll", description: "Zoom" },
                ShortcutEntry { key: "1 / 3 / 7 / 9", description: "Front / Right / Top / Iso" },
                ShortcutEntry { key: "2 / 4 / 6 / 8", description: "Back / Left / Bottom / Rear" },
                ShortcutEntry { key: "5", description: "Toggle Ortho/Persp" },
                ShortcutEntry { key: "Home", description: "Zoom to fit" },
                ShortcutEntry { key: "Numpad .", description: "Focus selected" },
            ]),
            ("Selection", &[
                ShortcutEntry { key: "LMB", description: "Select object" },
                ShortcutEntry { key: "Shift+LMB", description: "Add to selection" },
                ShortcutEntry { key: "LMB Drag (empty)", description: "Box select" },
                ShortcutEntry { key: "Ctrl+A", description: "Select all" },
                ShortcutEntry { key: "Esc", description: "Deselect" },
            ]),
            ("Tools", &[
                ShortcutEntry { key: "Q", description: "Select tool" },
                ShortcutEntry { key: "G", description: "Move tool" },
                ShortcutEntry { key: "R", description: "Rotate tool" },
                ShortcutEntry { key: "S", description: "Scale tool" },
            ]),
            ("Edit", &[
                ShortcutEntry { key: "Ctrl+Z", description: "Undo" },
                ShortcutEntry { key: "Ctrl+Shift+Z", description: "Redo" },
                ShortcutEntry { key: "Ctrl+D", description: "Duplicate" },
                ShortcutEntry { key: "Del / Backspace", description: "Delete" },
            ]),
            ("View", &[
                ShortcutEntry { key: "Z", description: "Cycle shading mode" },
                ShortcutEntry { key: "M", description: "Toggle measurements" },
                ShortcutEntry { key: "N", description: "Toggle object labels" },
                ShortcutEntry { key: "Ctrl+Shift+S", description: "Toggle snap" },
                ShortcutEntry { key: "Ctrl+Shift+C", description: "Cross-section plane" },
                ShortcutEntry { key: "Ctrl+Shift+X", description: "Cycle clip axis" },
                ShortcutEntry { key: "Ctrl+Shift+P", description: "Performance HUD" },
                ShortcutEntry { key: "Ctrl+K / F3", description: "Command palette" },
                ShortcutEntry { key: "F1", description: "This help" },
            ]),
            ("Numeric Input (after G/R/S)", &[
                ShortcutEntry { key: "0-9 / .", description: "Type value" },
                ShortcutEntry { key: "X / Y / Z", description: "Constrain axis" },
                ShortcutEntry { key: "Tab", description: "Cycle axis" },
                ShortcutEntry { key: "-", description: "Negate" },
                ShortcutEntry { key: "Enter", description: "Confirm" },
                ShortcutEntry { key: "Esc", description: "Cancel" },
            ]),
        ];

        let panel_w = 420.0;
        let row_h = 18.0;
        let section_gap = 8.0;
        let header_h = 24.0;
        let padding = 16.0;
        let font_size = 11.0;

        // Calculate total height
        let mut total_rows = 0;
        for (_, entries) in sections {
            total_rows += 1 + entries.len(); // header + entries
        }
        let panel_h = header_h + padding + (total_rows as f32 * row_h) + (sections.len() as f32 * section_gap);

        let x = (screen_w - panel_w) * 0.5;
        let y = (screen_h - panel_h) * 0.5;

        // Backdrop
        draw.push_quad(0.0, 0.0, screen_w, screen_h, [0.0, 0.0, 0.0, 0.5]);

        // Panel background
        draw.push_quad(x, y, panel_w, panel_h, [0.12, 0.12, 0.14, 0.95]);

        // Panel border
        draw.push_quad(x, y, panel_w, 1.0, [0.3, 0.4, 0.6, 0.8]);
        draw.push_quad(x, y + panel_h - 1.0, panel_w, 1.0, [0.3, 0.4, 0.6, 0.8]);
        draw.push_quad(x, y, 1.0, panel_h, [0.3, 0.4, 0.6, 0.8]);
        draw.push_quad(x + panel_w - 1.0, y, 1.0, panel_h, [0.3, 0.4, 0.6, 0.8]);

        // Title
        let title = "Keyboard Shortcuts";
        let title_w = font::measure_text(title, 14.0, None);
        let mut tx = x + (panel_w - title_w) * 0.5;
        let ty = y + (header_h - 14.0) * 0.5;
        for c in title.chars() {
            let params = font::CharQuadParams {
                c, x: tx, y: ty, size: 14.0, color: [0.9, 0.9, 0.95, 1.0], atlas: None,
            };
            tx += font::emit_char_quads(&params, &mut draw.vertices, &mut draw.indices);
        }

        // Sections
        let mut cy = y + header_h + padding * 0.5;
        let key_col_x = x + padding;
        let desc_col_x = x + 160.0;

        for (section_name, entries) in sections {
            // Section header
            let mut sx = key_col_x;
            for c in section_name.chars() {
                let params = font::CharQuadParams {
                    c, x: sx, y: cy, size: font_size,
                    color: [0.5, 0.7, 1.0, 1.0], atlas: None,
                };
                sx += font::emit_char_quads(&params, &mut draw.vertices, &mut draw.indices);
            }
            cy += row_h;

            // Entries
            for entry in *entries {
                // Key
                let mut kx = key_col_x + 8.0;
                for c in entry.key.chars() {
                    let params = font::CharQuadParams {
                        c, x: kx, y: cy, size: font_size,
                        color: [0.85, 0.85, 0.7, 1.0], atlas: None,
                    };
                    kx += font::emit_char_quads(&params, &mut draw.vertices, &mut draw.indices);
                }

                // Description
                let mut dx = desc_col_x;
                for c in entry.description.chars() {
                    let params = font::CharQuadParams {
                        c, x: dx, y: cy, size: font_size,
                        color: [0.7, 0.7, 0.7, 1.0], atlas: None,
                    };
                    dx += font::emit_char_quads(&params, &mut draw.vertices, &mut draw.indices);
                }
                cy += row_h;
            }

            cy += section_gap;
        }

        // Footer
        let footer = "Press F1 or Esc to close";
        let footer_w = font::measure_text(footer, 10.0, None);
        let mut fx = x + (panel_w - footer_w) * 0.5;
        let fy = y + panel_h - 18.0;
        for c in footer.chars() {
            let params = font::CharQuadParams {
                c, x: fx, y: fy, size: 10.0,
                color: [0.5, 0.5, 0.5, 0.8], atlas: None,
            };
            fx += font::emit_char_quads(&params, &mut draw.vertices, &mut draw.indices);
        }
    }
}

impl Default for ShortcutHelp {
    fn default() -> Self {
        Self::new()
    }
}
