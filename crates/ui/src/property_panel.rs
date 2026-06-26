//! Property panel — right-side sliding panel for object parameters.
//!
//! Inspired by SolidWorks PropertyManager / Fusion 360 inspector.
//! Shows context-sensitive properties based on selection.

use crate::draw::DrawList;
use crate::font;
use crate::theme::ThemeColors;

/// A single property entry to display.
#[derive(Clone, Debug)]
pub struct PropertyEntry {
    pub label: String,
    pub value: String,
    pub editable: bool,
    pub category: &'static str,
}

impl PropertyEntry {
    pub fn new(label: &str, value: &str) -> Self {
        Self {
            label: label.to_string(),
            value: value.to_string(),
            editable: false,
            category: "",
        }
    }

    pub fn editable(mut self) -> Self {
        self.editable = true;
        self
    }

    pub fn in_category(mut self, cat: &'static str) -> Self {
        self.category = cat;
        self
    }
}

/// Section header within the property panel.
#[derive(Clone, Debug)]
pub struct PropertySection {
    pub title: String,
    pub collapsed: bool,
    pub entries: Vec<PropertyEntry>,
}

impl PropertySection {
    pub fn new(title: &str) -> Self {
        Self {
            title: title.to_string(),
            collapsed: false,
            entries: Vec::new(),
        }
    }

    pub fn add(&mut self, entry: PropertyEntry) {
        self.entries.push(entry);
    }
}

/// Right-side sliding property panel.
pub struct PropertyPanel {
    /// Whether the panel is open/visible.
    pub visible: bool,
    /// Panel width in pixels.
    pub width: f32,
    /// Current slide animation progress (0.0 = hidden, 1.0 = fully open).
    pub slide_t: f32,
    /// Sections to display.
    pub sections: Vec<PropertySection>,
    /// Scroll offset within the panel.
    pub scroll_y: f32,
    /// Hovered section index (for collapse toggle).
    pub hovered_section: Option<usize>,
    /// Title text.
    pub title: String,
}

impl PropertyPanel {
    pub fn new() -> Self {
        Self {
            visible: false,
            width: 260.0,
            slide_t: 0.0,
            sections: Vec::new(),
            scroll_y: 0.0,
            hovered_section: None,
            title: String::new(),
        }
    }

    pub fn open(&mut self) {
        self.visible = true;
    }

    pub fn close(&mut self) {
        self.visible = false;
    }

    pub fn toggle(&mut self) {
        self.visible = !self.visible;
    }

    /// Animate the slide-in/out. Call each frame.
    pub fn update(&mut self, dt: f32) {
        let target = if self.visible { 1.0 } else { 0.0 };
        let speed = 8.0;
        self.slide_t += (target - self.slide_t) * speed * dt;
        if (self.slide_t - target).abs() < 0.01 {
            self.slide_t = target;
        }
    }

    /// Clear all sections (call before rebuilding each frame).
    pub fn clear(&mut self) {
        self.sections.clear();
    }

    pub fn add_section(&mut self, section: PropertySection) {
        self.sections.push(section);
    }

    /// Hit test for section header collapse toggle.
    /// Returns section index if a header was clicked.
    pub fn hit_test_header(&self, mx: f32, my: f32, screen_w: f32, top_y: f32) -> Option<usize> {
        if self.slide_t < 0.05 { return None; }
        let panel_x = screen_w - self.width * self.slide_t;
        if mx < panel_x || mx > panel_x + self.width { return None; }

        let header_h = 24.0;
        let row_h = 20.0;
        let padding = 8.0;
        let mut cy = top_y + 28.0 + padding; // after title

        for (i, section) in self.sections.iter().enumerate() {
            if my >= cy && my < cy + header_h {
                return Some(i);
            }
            cy += header_h;
            if !section.collapsed {
                cy += section.entries.len() as f32 * row_h + padding;
            }
        }
        None
    }

    /// Toggle collapse on a section.
    pub fn toggle_section(&mut self, idx: usize) {
        if idx < self.sections.len() {
            self.sections[idx].collapsed = !self.sections[idx].collapsed;
        }
    }

    /// The actual X position of the panel's left edge (for layout).
    pub fn panel_x(&self, screen_w: f32) -> f32 {
        screen_w - self.width * self.slide_t
    }

    /// Draw the property panel.
    pub fn draw(
        &self,
        dl: &mut DrawList,
        screen_w: f32,
        screen_h: f32,
        top_y: f32,
        theme: &ThemeColors,
    ) {
        if self.slide_t < 0.01 { return; }

        let panel_x = screen_w - self.width * self.slide_t;
        let panel_h = screen_h - top_y - 24.0; // leave room for status bar

        // Panel background
        dl.push_quad(panel_x, top_y, self.width, panel_h, theme.panel_bg);

        // Left border
        dl.push_quad(panel_x, top_y, 1.0, panel_h, theme.border);

        // Title bar
        let title_h = 28.0;
        dl.push_quad(panel_x, top_y, self.width, title_h, theme.header_bg);
        let title_text = if self.title.is_empty() { "Properties" } else { &self.title };
        emit_text(dl, title_text, panel_x + 8.0, top_y + 7.0, 13.0, theme.text);

        // Close button (X) at top-right
        let close_x = panel_x + self.width - 22.0;
        emit_text(dl, "X", close_x, top_y + 7.0, 12.0, theme.text_muted);

        // Sections
        let padding = 8.0;
        let header_h = 24.0;
        let row_h = 20.0;
        let mut cy = top_y + title_h + padding;
        let content_w = self.width - padding * 2.0;
        let label_w = content_w * 0.45;

        for section in &self.sections {
            // Section header
            let arrow = if section.collapsed { ">" } else { "v" };
            dl.push_quad(panel_x + 1.0, cy, self.width - 2.0, header_h, theme.header_bg);
            emit_text(dl, arrow, panel_x + padding, cy + 5.0, 12.0, theme.text_muted);
            emit_text(dl, &section.title, panel_x + padding + 14.0, cy + 5.0, 12.0, theme.text);
            cy += header_h;

            if section.collapsed { continue; }

            // Entries
            for entry in &section.entries {
                // Alternate row bg
                let row_bg = [
                    theme.panel_bg[0] + 0.02,
                    theme.panel_bg[1] + 0.02,
                    theme.panel_bg[2] + 0.02,
                    theme.panel_bg[3],
                ];
                dl.push_quad(panel_x + 1.0, cy, self.width - 2.0, row_h, row_bg);

                // Label
                emit_text(dl, &entry.label, panel_x + padding, cy + 3.0, 11.0, theme.text_muted);

                // Value (right-aligned region)
                let val_x = panel_x + padding + label_w;
                let val_color = if entry.editable { theme.accent } else { theme.text };
                emit_text(dl, &entry.value, val_x, cy + 3.0, 11.0, val_color);

                cy += row_h;
            }
            cy += padding;
        }
    }
}

impl Default for PropertyPanel {
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
    fn panel_toggle() {
        let mut panel = PropertyPanel::new();
        assert!(!panel.visible);
        panel.toggle();
        assert!(panel.visible);
        panel.toggle();
        assert!(!panel.visible);
    }

    #[test]
    fn section_collapse() {
        let mut panel = PropertyPanel::new();
        panel.add_section(PropertySection::new("Transform"));
        panel.add_section(PropertySection::new("Material"));
        panel.toggle_section(0);
        assert!(panel.sections[0].collapsed);
        assert!(!panel.sections[1].collapsed);
    }

    #[test]
    fn clear_sections() {
        let mut panel = PropertyPanel::new();
        panel.add_section(PropertySection::new("Test"));
        assert_eq!(panel.sections.len(), 1);
        panel.clear();
        assert!(panel.sections.is_empty());
    }

    #[test]
    fn slide_animation() {
        let mut panel = PropertyPanel::new();
        panel.visible = true;
        panel.update(0.1);
        assert!(panel.slide_t > 0.0);
        panel.visible = false;
        for _ in 0..100 {
            panel.update(0.016);
        }
        assert!(panel.slide_t < 0.02);
    }
}
