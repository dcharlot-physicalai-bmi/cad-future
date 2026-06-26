//! Feature tree — parametric feature browser with rollback, suppress, drag-reorder.
//!
//! Inspired by SolidWorks FeatureManager Design Tree, Fusion 360 Browser/Timeline,
//! and Onshape Feature List. Displays the ordered list of modeling operations
//! (sketch, extrude, fillet, chamfer, pattern, etc.) with status icons,
//! rollback bar, and suppress/unsuppress control.

use crate::draw::DrawList;
use crate::font;

/// Type of parametric feature.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FeatureKind {
    Origin,
    Sketch,
    Extrude,
    Revolve,
    Sweep,
    Loft,
    Fillet,
    Chamfer,
    Shell,
    Hole,
    Pattern,
    Mirror,
    Boolean,
    Split,
    Import,
    Reference,
}

impl FeatureKind {
    pub fn icon(self) -> &'static str {
        match self {
            Self::Origin => "O",
            Self::Sketch => "S",
            Self::Extrude => "E",
            Self::Revolve => "R",
            Self::Sweep => "W",
            Self::Loft => "L",
            Self::Fillet => "F",
            Self::Chamfer => "C",
            Self::Shell => "H",
            Self::Hole => "o",
            Self::Pattern => "P",
            Self::Mirror => "M",
            Self::Boolean => "B",
            Self::Split => "X",
            Self::Import => "I",
            Self::Reference => ">",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Origin => "Origin",
            Self::Sketch => "Sketch",
            Self::Extrude => "Extrude",
            Self::Revolve => "Revolve",
            Self::Sweep => "Sweep",
            Self::Loft => "Loft",
            Self::Fillet => "Fillet",
            Self::Chamfer => "Chamfer",
            Self::Shell => "Shell",
            Self::Hole => "Hole",
            Self::Pattern => "Pattern",
            Self::Mirror => "Mirror",
            Self::Boolean => "Boolean",
            Self::Split => "Split",
            Self::Import => "Import",
            Self::Reference => "Reference",
        }
    }
}

/// Status of a feature in the tree.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FeatureStatus {
    /// Fully resolved, up to date.
    Ok,
    /// Has warnings (e.g., over-constrained sketch).
    Warning,
    /// Failed to rebuild.
    Error,
    /// Suppressed (excluded from rebuild).
    Suppressed,
}

/// A single feature in the tree.
#[derive(Clone, Debug)]
pub struct Feature {
    /// Unique ID.
    pub id: u64,
    /// Display name (e.g., "Extrude1", "Fillet - 2mm").
    pub name: String,
    /// Feature type.
    pub kind: FeatureKind,
    /// Current status.
    pub status: FeatureStatus,
    /// Parameters summary (e.g., "D = 10mm", "R = 2mm").
    pub params: String,
    /// Whether this feature has children (for expand/collapse).
    pub has_children: bool,
    /// Whether children are expanded.
    pub expanded: bool,
    /// Nesting depth (0 = top level).
    pub depth: u32,
}

impl Feature {
    pub fn new(id: u64, name: &str, kind: FeatureKind) -> Self {
        Self {
            id,
            name: name.to_string(),
            kind,
            status: FeatureStatus::Ok,
            params: String::new(),
            has_children: false,
            expanded: true,
            depth: 0,
        }
    }

    pub fn with_params(mut self, params: &str) -> Self {
        self.params = params.to_string();
        self
    }

    pub fn with_status(mut self, status: FeatureStatus) -> Self {
        self.status = status;
        self
    }

    pub fn with_depth(mut self, depth: u32) -> Self {
        self.depth = depth;
        self
    }
}

/// The feature tree panel.
pub struct FeatureTree {
    /// Ordered list of features.
    pub features: Vec<Feature>,
    /// Whether the panel is visible.
    pub visible: bool,
    /// Panel width.
    pub width: f32,
    /// Selected feature index.
    pub selected: Option<usize>,
    /// Hovered feature index.
    pub hovered: Option<usize>,
    /// Rollback bar position (features after this index are "rolled back" / grayed).
    /// None means all features active.
    pub rollback: Option<usize>,
    /// Scroll offset (first visible feature index).
    pub scroll_offset: usize,
    /// Row height.
    pub row_height: f32,
    /// Maximum visible rows.
    pub max_visible: usize,
    /// Next auto-increment ID.
    next_id: u64,
}

impl FeatureTree {
    pub fn new() -> Self {
        Self {
            features: Vec::new(),
            visible: true,
            width: 220.0,
            selected: None,
            hovered: None,
            rollback: None,
            scroll_offset: 0,
            row_height: 22.0,
            max_visible: 30,
            next_id: 1,
        }
    }

    /// Add a feature and return its ID.
    pub fn add(&mut self, mut feature: Feature) -> u64 {
        let id = self.next_id;
        feature.id = id;
        self.next_id += 1;
        self.features.push(feature);
        id
    }

    /// Remove a feature by index.
    pub fn remove(&mut self, idx: usize) -> Option<Feature> {
        if idx < self.features.len() {
            Some(self.features.remove(idx))
        } else {
            None
        }
    }

    /// Toggle suppress on feature at index.
    pub fn toggle_suppress(&mut self, idx: usize) {
        if let Some(f) = self.features.get_mut(idx) {
            f.status = match f.status {
                FeatureStatus::Suppressed => FeatureStatus::Ok,
                _ => FeatureStatus::Suppressed,
            };
        }
    }

    /// Set rollback bar position. None = show all.
    pub fn set_rollback(&mut self, pos: Option<usize>) {
        self.rollback = pos;
    }

    /// Move a feature from one index to another (drag reorder).
    pub fn reorder(&mut self, from: usize, to: usize) {
        if from < self.features.len() && to <= self.features.len() && from != to {
            let feat = self.features.remove(from);
            let insert_at = if to > from { to - 1 } else { to };
            self.features.insert(insert_at, feat);
        }
    }

    /// Count of active (non-rolled-back) features.
    pub fn active_count(&self) -> usize {
        match self.rollback {
            Some(pos) => pos.min(self.features.len()),
            None => self.features.len(),
        }
    }

    /// Count of suppressed features.
    pub fn suppressed_count(&self) -> usize {
        self.features.iter()
            .filter(|f| f.status == FeatureStatus::Suppressed)
            .count()
    }

    /// Count of features with errors.
    pub fn error_count(&self) -> usize {
        self.features.iter()
            .filter(|f| f.status == FeatureStatus::Error)
            .count()
    }

    /// Toggle expand/collapse of a feature's children.
    pub fn toggle_expand(&mut self, idx: usize) {
        if let Some(f) = self.features.get_mut(idx) {
            if f.has_children {
                f.expanded = !f.expanded;
            }
        }
    }

    /// Hit test: which feature row was clicked?
    pub fn hit_test(
        &self, mx: f32, my: f32,
        panel_x: f32, panel_y: f32,
    ) -> Option<usize> {
        if !self.visible { return None; }

        let header_h = 24.0;
        let ry = my - panel_y - header_h;
        if ry < 0.0 || mx < panel_x || mx > panel_x + self.width { return None; }

        let row = (ry / self.row_height) as usize + self.scroll_offset;
        if row < self.features.len() {
            Some(row)
        } else {
            None
        }
    }

    /// Draw the feature tree.
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

        let header_h = 24.0;

        // Background
        dl.push_quad(panel_x, panel_y, self.width, panel_h, bg_color);

        // Header
        let header_bg = [bg_color[0] + 0.03, bg_color[1] + 0.03, bg_color[2] + 0.03, bg_color[3]];
        dl.push_quad(panel_x, panel_y, self.width, header_h, header_bg);
        emit_text(dl, "Feature Tree", panel_x + 8.0, panel_y + 5.0, 11.0, text_color);

        // Feature count
        let count = format!("{}", self.features.len());
        let muted = [text_color[0] * 0.5, text_color[1] * 0.5, text_color[2] * 0.5, text_color[3]];
        let cw = font::measure_text(&count, 9.0, None);
        emit_text(dl, &count, panel_x + self.width - cw - 8.0, panel_y + 7.0, 9.0, muted);

        // Feature rows
        let visible_rows = ((panel_h - header_h) / self.row_height) as usize;
        let end = (self.scroll_offset + visible_rows).min(self.features.len());

        for i in self.scroll_offset..end {
            let feat = &self.features[i];
            let vis_row = (i - self.scroll_offset) as f32;
            let ry = panel_y + header_h + vis_row * self.row_height;

            let is_rolled_back = self.rollback.map_or(false, |rb| i >= rb);
            let is_selected = self.selected == Some(i);
            let is_hovered = self.hovered == Some(i);

            // Row background
            if is_selected {
                dl.push_quad(panel_x, ry, self.width, self.row_height,
                    [accent_color[0] * 0.3, accent_color[1] * 0.3, accent_color[2] * 0.3, 0.5]);
            } else if is_hovered {
                dl.push_quad(panel_x, ry, self.width, self.row_height,
                    [1.0, 1.0, 1.0, 0.05]);
            }

            // Indent
            let indent = 8.0 + feat.depth as f32 * 16.0;

            // Expand/collapse arrow
            if feat.has_children {
                let arrow = if feat.expanded { "v" } else { ">" };
                emit_text(dl, arrow, panel_x + indent - 12.0, ry + 4.0, 9.0, muted);
            }

            // Status icon
            let icon_color = match feat.status {
                FeatureStatus::Ok => [0.3, 0.7, 0.3, 1.0],
                FeatureStatus::Warning => [0.9, 0.7, 0.1, 1.0],
                FeatureStatus::Error => [0.9, 0.2, 0.2, 1.0],
                FeatureStatus::Suppressed => [0.5, 0.5, 0.5, 0.5],
            };
            dl.push_quad(panel_x + indent, ry + 6.0, 10.0, 10.0, icon_color);
            emit_text(dl, feat.kind.icon(), panel_x + indent + 1.0, ry + 6.0, 8.0,
                [1.0, 1.0, 1.0, 0.9]);

            // Feature name
            let name_alpha = if is_rolled_back { 0.35 } else if feat.status == FeatureStatus::Suppressed { 0.4 } else { 1.0 };
            let name_color = [text_color[0], text_color[1], text_color[2], name_alpha];
            emit_text(dl, &feat.name, panel_x + indent + 14.0, ry + 4.0, 10.0, name_color);

            // Parameters (smaller, muted)
            if !feat.params.is_empty() && !is_rolled_back {
                let param_color = [muted[0], muted[1], muted[2], 0.7];
                emit_text(dl, &feat.params, panel_x + indent + 14.0, ry + 14.0, 7.0, param_color);
            }
        }

        // Rollback bar
        if let Some(rb) = self.rollback {
            if rb > self.scroll_offset && rb <= end {
                let rb_y = panel_y + header_h + (rb - self.scroll_offset) as f32 * self.row_height;
                dl.push_quad(panel_x, rb_y - 1.0, self.width, 2.0, [0.9, 0.6, 0.1, 0.9]);
                emit_text(dl, "rollback", panel_x + self.width - 48.0, rb_y - 10.0, 7.0,
                    [0.9, 0.6, 0.1, 0.7]);
            }
        }

        // Border
        dl.push_quad(panel_x + self.width - 1.0, panel_y, 1.0, panel_h,
            [bg_color[0] + 0.1, bg_color[1] + 0.1, bg_color[2] + 0.1, 0.8]);
    }
}

impl Default for FeatureTree {
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
    fn add_and_select() {
        let mut ft = FeatureTree::new();
        let id1 = ft.add(Feature::new(0, "Sketch1", FeatureKind::Sketch));
        let id2 = ft.add(Feature::new(0, "Extrude1", FeatureKind::Extrude));
        assert_eq!(ft.features.len(), 2);
        assert_ne!(id1, id2);
        ft.selected = Some(0);
        assert_eq!(ft.features[0].name, "Sketch1");
    }

    #[test]
    fn toggle_suppress() {
        let mut ft = FeatureTree::new();
        ft.add(Feature::new(0, "Fillet1", FeatureKind::Fillet));
        assert_eq!(ft.features[0].status, FeatureStatus::Ok);
        ft.toggle_suppress(0);
        assert_eq!(ft.features[0].status, FeatureStatus::Suppressed);
        ft.toggle_suppress(0);
        assert_eq!(ft.features[0].status, FeatureStatus::Ok);
    }

    #[test]
    fn rollback_active_count() {
        let mut ft = FeatureTree::new();
        ft.add(Feature::new(0, "Sketch1", FeatureKind::Sketch));
        ft.add(Feature::new(0, "Extrude1", FeatureKind::Extrude));
        ft.add(Feature::new(0, "Fillet1", FeatureKind::Fillet));
        assert_eq!(ft.active_count(), 3);
        ft.set_rollback(Some(2));
        assert_eq!(ft.active_count(), 2);
    }

    #[test]
    fn reorder_features() {
        let mut ft = FeatureTree::new();
        ft.add(Feature::new(0, "A", FeatureKind::Sketch));
        ft.add(Feature::new(0, "B", FeatureKind::Extrude));
        ft.add(Feature::new(0, "C", FeatureKind::Fillet));
        ft.reorder(2, 0); // move C before A
        assert_eq!(ft.features[0].name, "C");
        assert_eq!(ft.features[1].name, "A");
        assert_eq!(ft.features[2].name, "B");
    }

    #[test]
    fn all_kinds_have_icons() {
        let kinds = [
            FeatureKind::Origin, FeatureKind::Sketch, FeatureKind::Extrude,
            FeatureKind::Revolve, FeatureKind::Sweep, FeatureKind::Loft,
            FeatureKind::Fillet, FeatureKind::Chamfer, FeatureKind::Shell,
            FeatureKind::Hole, FeatureKind::Pattern, FeatureKind::Mirror,
            FeatureKind::Boolean, FeatureKind::Split, FeatureKind::Import,
            FeatureKind::Reference,
        ];
        for k in kinds {
            assert!(!k.icon().is_empty());
            assert!(!k.label().is_empty());
        }
    }
}
