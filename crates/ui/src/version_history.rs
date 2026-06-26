//! Version history — cloud-native revision browser.
//!
//! Inspired by Onshape Version Manager, Git history, and Fusion 360
//! Version History. Shows a timeline of saves/versions with branching,
//! restore, and diff capabilities.

use crate::draw::DrawList;
use crate::font;

/// Type of version entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VersionKind {
    AutoSave,
    ManualSave,
    Version,
    Branch,
    Tag,
}

impl VersionKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::AutoSave => "Auto",
            Self::ManualSave => "Save",
            Self::Version => "Version",
            Self::Branch => "Branch",
            Self::Tag => "Tag",
        }
    }

    pub fn icon(self) -> &'static str {
        match self {
            Self::AutoSave => ".",
            Self::ManualSave => "o",
            Self::Version => "O",
            Self::Branch => "Y",
            Self::Tag => "#",
        }
    }
}

/// A version entry.
#[derive(Clone, Debug)]
pub struct VersionEntry {
    /// Version ID.
    pub id: String,
    /// Display name.
    pub name: String,
    /// Kind.
    pub kind: VersionKind,
    /// Author.
    pub author: String,
    /// Timestamp.
    pub timestamp: String,
    /// Description / commit message.
    pub description: String,
    /// Whether this is the current version.
    pub current: bool,
}

impl VersionEntry {
    pub fn new(id: &str, name: &str, kind: VersionKind) -> Self {
        Self {
            id: id.to_string(),
            name: name.to_string(),
            kind,
            author: String::new(),
            timestamp: String::new(),
            description: String::new(),
            current: false,
        }
    }

    pub fn with_author(mut self, author: &str) -> Self {
        self.author = author.to_string();
        self
    }

    pub fn with_timestamp(mut self, ts: &str) -> Self {
        self.timestamp = ts.to_string();
        self
    }

    pub fn with_description(mut self, desc: &str) -> Self {
        self.description = desc.to_string();
        self
    }
}

/// The version history panel.
pub struct VersionHistory {
    /// Whether the panel is visible.
    pub visible: bool,
    /// Version entries (newest first).
    pub entries: Vec<VersionEntry>,
    /// Selected entry index.
    pub selected: Option<usize>,
    /// Hovered entry index.
    pub hovered: Option<usize>,
    /// Scroll offset.
    pub scroll_offset: usize,
    /// Panel width.
    pub width: f32,
    /// Whether to show auto-saves.
    pub show_autosaves: bool,
}

impl VersionHistory {
    pub fn new() -> Self {
        Self {
            visible: false,
            entries: Vec::new(),
            selected: None,
            hovered: None,
            scroll_offset: 0,
            width: 280.0,
            show_autosaves: false,
        }
    }

    pub fn toggle(&mut self) {
        self.visible = !self.visible;
    }

    /// Add a version entry.
    pub fn add(&mut self, entry: VersionEntry) {
        self.entries.insert(0, entry); // newest first
    }

    /// Get filtered entries.
    pub fn filtered(&self) -> Vec<(usize, &VersionEntry)> {
        self.entries.iter().enumerate()
            .filter(|(_, e)| self.show_autosaves || e.kind != VersionKind::AutoSave)
            .collect()
    }

    /// Count of named versions.
    pub fn version_count(&self) -> usize {
        self.entries.iter().filter(|e| e.kind == VersionKind::Version).count()
    }

    /// Find current version index.
    pub fn current_index(&self) -> Option<usize> {
        self.entries.iter().position(|e| e.current)
    }

    /// Draw the version history panel.
    pub fn draw(
        &self,
        dl: &mut DrawList,
        panel_x: f32,
        panel_y: f32,
        panel_h: f32,
        bg_color: [f32; 4],
        text_color: [f32; 4],
        accent_color: [f32; 4],
    ) {
        if !self.visible { return; }

        // Background
        dl.push_quad(panel_x, panel_y, self.width, panel_h, bg_color);

        let muted = [text_color[0] * 0.5, text_color[1] * 0.5, text_color[2] * 0.5, text_color[3]];

        // Header
        let hdr_bg = [bg_color[0] + 0.03, bg_color[1] + 0.03, bg_color[2] + 0.03, bg_color[3]];
        dl.push_quad(panel_x, panel_y, self.width, 28.0, hdr_bg);
        emit_text(dl, "Version History", panel_x + 8.0, panel_y + 7.0, 11.0, text_color);

        let count = format!("{}", self.version_count());
        let cw = font::measure_text(&count, 9.0, None);
        emit_text(dl, &count, panel_x + self.width - cw - 8.0, panel_y + 9.0, 9.0, muted);

        // Auto-save toggle
        let as_bg = if self.show_autosaves { accent_color } else { [0.3, 0.3, 0.3, 0.5] };
        dl.push_quad(panel_x + self.width - 70.0, panel_y + 5.0, 56.0, 14.0, as_bg);
        emit_text(dl, "Auto", panel_x + self.width - 58.0, panel_y + 7.0, 8.0,
            if self.show_autosaves { [1.0, 1.0, 1.0, 1.0] } else { text_color });

        // Timeline line
        let line_x = panel_x + 20.0;
        let entry_start_y = panel_y + 32.0;
        let row_h = 48.0;

        let filtered = self.filtered();
        let visible_rows = ((panel_h - 32.0) / row_h) as usize;
        let end = (self.scroll_offset + visible_rows).min(filtered.len());

        // Timeline backbone
        if !filtered.is_empty() {
            let line_h = (end - self.scroll_offset) as f32 * row_h;
            dl.push_quad(line_x, entry_start_y, 2.0, line_h, [0.3, 0.3, 0.3, 0.4]);
        }

        for vis_i in self.scroll_offset..end {
            let (real_i, entry) = filtered[vis_i];
            let row = (vis_i - self.scroll_offset) as f32;
            let ry = entry_start_y + row * row_h;

            let is_sel = self.selected == Some(real_i);
            let is_hov = self.hovered == Some(real_i);
            let is_current = entry.current;

            // Row background
            if is_sel {
                dl.push_quad(panel_x, ry, self.width, row_h,
                    [accent_color[0] * 0.2, accent_color[1] * 0.2, accent_color[2] * 0.2, 0.3]);
            } else if is_hov {
                dl.push_quad(panel_x, ry, self.width, row_h, [1.0, 1.0, 1.0, 0.04]);
            }

            // Timeline dot
            let dot_size = match entry.kind {
                VersionKind::Version | VersionKind::Tag => 8.0,
                VersionKind::Branch => 8.0,
                _ => 4.0,
            };
            let dot_color = if is_current { accent_color } else {
                match entry.kind {
                    VersionKind::Version => [0.3, 0.7, 0.9, 0.9],
                    VersionKind::Tag => [0.9, 0.7, 0.2, 0.9],
                    VersionKind::Branch => [0.7, 0.3, 0.9, 0.9],
                    _ => [0.5, 0.5, 0.5, 0.5],
                }
            };
            dl.push_quad(line_x - dot_size * 0.5 + 1.0, ry + 8.0, dot_size, dot_size, dot_color);

            // Kind badge
            emit_text(dl, entry.kind.label(), panel_x + 34.0, ry + 4.0, 7.0, muted);

            // Name
            let nc = if is_current { accent_color } else { text_color };
            emit_text(dl, &entry.name, panel_x + 34.0, ry + 14.0, 10.0, nc);

            // Current indicator
            if is_current {
                emit_text(dl, "(current)", panel_x + 34.0 + font::measure_text(&entry.name, 10.0, None) + 4.0,
                    ry + 15.0, 7.0, accent_color);
            }

            // Author + timestamp
            if !entry.author.is_empty() {
                let meta = format!("{} {}", entry.author,
                    if entry.timestamp.is_empty() { "" } else { &entry.timestamp });
                emit_text(dl, &meta, panel_x + 34.0, ry + 28.0, 7.0, muted);
            }

            // Description
            if !entry.description.is_empty() {
                let desc = if entry.description.len() > 35 { &entry.description[..35] } else { &entry.description };
                emit_text(dl, desc, panel_x + 34.0, ry + 38.0, 7.0,
                    [muted[0], muted[1], muted[2], 0.6]);
            }
        }

        // Border
        dl.push_quad(panel_x, panel_y, 1.0, panel_h,
            [bg_color[0] + 0.1, bg_color[1] + 0.1, bg_color[2] + 0.1, 0.8]);
    }
}

impl Default for VersionHistory {
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
    fn add_and_count() {
        let mut vh = VersionHistory::new();
        vh.add(VersionEntry::new("v1", "Version 1", VersionKind::Version));
        vh.add(VersionEntry::new("s1", "Auto-save", VersionKind::AutoSave));
        assert_eq!(vh.version_count(), 1);
        assert_eq!(vh.entries.len(), 2);
    }

    #[test]
    fn filtered_hides_autosaves() {
        let mut vh = VersionHistory::new();
        vh.add(VersionEntry::new("v1", "V1", VersionKind::Version));
        vh.add(VersionEntry::new("a1", "Auto", VersionKind::AutoSave));
        vh.show_autosaves = false;
        assert_eq!(vh.filtered().len(), 1);
        vh.show_autosaves = true;
        assert_eq!(vh.filtered().len(), 2);
    }

    #[test]
    fn newest_first() {
        let mut vh = VersionHistory::new();
        vh.add(VersionEntry::new("v1", "First", VersionKind::Version));
        vh.add(VersionEntry::new("v2", "Second", VersionKind::Version));
        assert_eq!(vh.entries[0].name, "Second");
    }

    #[test]
    fn current_version() {
        let mut vh = VersionHistory::new();
        vh.add(VersionEntry::new("v1", "V1", VersionKind::Version));
        vh.entries[0].current = true;
        assert_eq!(vh.current_index(), Some(0));
    }

    #[test]
    fn version_kinds() {
        let kinds = [VersionKind::AutoSave, VersionKind::ManualSave, VersionKind::Version,
                    VersionKind::Branch, VersionKind::Tag];
        for k in kinds {
            assert!(!k.label().is_empty());
            assert!(!k.icon().is_empty());
        }
    }
}
