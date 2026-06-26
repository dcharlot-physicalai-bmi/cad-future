//! Assembly browser — hierarchical component tree with instance management.
//!
//! Inspired by SolidWorks Assembly FeatureManager, Fusion 360 Browser,
//! and Onshape Assembly. Shows component hierarchy with visibility toggles,
//! instance counts, interference indicators, and isolation mode.

use crate::draw::DrawList;
use crate::font;

/// Type of assembly node.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ComponentKind {
    Assembly,
    Part,
    StandardPart,
    VirtualPart,
    Pattern,
    Reference,
}

impl ComponentKind {
    pub fn icon(self) -> &'static str {
        match self {
            Self::Assembly => "A",
            Self::Part => "P",
            Self::StandardPart => "S",
            Self::VirtualPart => "V",
            Self::Pattern => "#",
            Self::Reference => ">",
        }
    }
}

/// A node in the assembly tree.
#[derive(Clone, Debug)]
pub struct ComponentNode {
    /// Unique ID.
    pub id: u64,
    /// Display name.
    pub name: String,
    /// Component type.
    pub kind: ComponentKind,
    /// Instance count (1 for unique, >1 for repeated).
    pub instances: u32,
    /// Whether this component is visible.
    pub visible: bool,
    /// Whether it's suppressed.
    pub suppressed: bool,
    /// Whether it has interference.
    pub interference: bool,
    /// Child indices (into flat list).
    pub children: Vec<usize>,
    /// Nesting depth.
    pub depth: u32,
    /// Expanded state.
    pub expanded: bool,
}

impl ComponentNode {
    pub fn new(id: u64, name: &str, kind: ComponentKind) -> Self {
        Self {
            id,
            name: name.to_string(),
            kind,
            instances: 1,
            visible: true,
            suppressed: false,
            interference: false,
            children: Vec::new(),
            depth: 0,
            expanded: true,
        }
    }

    pub fn with_instances(mut self, count: u32) -> Self {
        self.instances = count;
        self
    }

    pub fn with_depth(mut self, depth: u32) -> Self {
        self.depth = depth;
        self
    }
}

/// The assembly browser panel.
pub struct AssemblyBrowser {
    /// All nodes in flat list.
    pub nodes: Vec<ComponentNode>,
    /// Whether the panel is visible.
    pub visible: bool,
    /// Panel width.
    pub width: f32,
    /// Selected node index.
    pub selected: Option<usize>,
    /// Hovered node index.
    pub hovered: Option<usize>,
    /// Scroll offset.
    pub scroll_offset: usize,
    /// Row height.
    pub row_height: f32,
    /// Whether isolation mode is active.
    pub isolation_mode: bool,
    /// Isolated component index.
    pub isolated_node: Option<usize>,
    /// Next auto ID.
    next_id: u64,
}

impl AssemblyBrowser {
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            visible: true,
            width: 240.0,
            selected: None,
            hovered: None,
            scroll_offset: 0,
            row_height: 22.0,
            isolation_mode: false,
            isolated_node: None,
            next_id: 1,
        }
    }

    /// Add a node and return its ID.
    pub fn add(&mut self, mut node: ComponentNode) -> u64 {
        let id = self.next_id;
        node.id = id;
        self.next_id += 1;
        self.nodes.push(node);
        id
    }

    /// Toggle visibility of a node.
    pub fn toggle_visibility(&mut self, idx: usize) {
        if let Some(n) = self.nodes.get_mut(idx) {
            n.visible = !n.visible;
        }
    }

    /// Toggle expand/collapse.
    pub fn toggle_expand(&mut self, idx: usize) {
        if let Some(n) = self.nodes.get_mut(idx) {
            n.expanded = !n.expanded;
        }
    }

    /// Enter isolation mode (show only this component).
    pub fn isolate(&mut self, idx: usize) {
        self.isolation_mode = true;
        self.isolated_node = Some(idx);
    }

    /// Exit isolation mode.
    pub fn exit_isolation(&mut self) {
        self.isolation_mode = false;
        self.isolated_node = None;
    }

    /// Count total instances.
    pub fn total_instances(&self) -> u32 {
        self.nodes.iter().map(|n| n.instances).sum()
    }

    /// Count interference nodes.
    pub fn interference_count(&self) -> usize {
        self.nodes.iter().filter(|n| n.interference).count()
    }

    /// Get visible (non-collapsed) node indices.
    pub fn visible_nodes(&self) -> Vec<usize> {
        let mut result = Vec::new();
        let mut skip_depth: Option<u32> = None;
        for (i, node) in self.nodes.iter().enumerate() {
            if let Some(sd) = skip_depth {
                if node.depth > sd { continue; }
                skip_depth = None;
            }
            result.push(i);
            if !node.expanded && !node.children.is_empty() {
                skip_depth = Some(node.depth);
            }
        }
        result
    }

    /// Hit test.
    pub fn hit_test(&self, mx: f32, my: f32, panel_x: f32, panel_y: f32) -> Option<usize> {
        if !self.visible { return None; }
        let header_h = 28.0;
        let ry = my - panel_y - header_h;
        if ry < 0.0 || mx < panel_x || mx > panel_x + self.width { return None; }
        let visible = self.visible_nodes();
        let row = (ry / self.row_height) as usize + self.scroll_offset;
        visible.get(row).copied()
    }

    /// Draw the assembly browser.
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

        let header_h = 28.0;

        // Background
        dl.push_quad(panel_x, panel_y, self.width, panel_h, bg_color);

        // Header
        let hdr_bg = [bg_color[0] + 0.03, bg_color[1] + 0.03, bg_color[2] + 0.03, bg_color[3]];
        dl.push_quad(panel_x, panel_y, self.width, header_h, hdr_bg);
        emit_text(dl, "Assembly", panel_x + 8.0, panel_y + 7.0, 11.0, text_color);

        let muted = [text_color[0] * 0.5, text_color[1] * 0.5, text_color[2] * 0.5, text_color[3]];

        // Isolation indicator
        if self.isolation_mode {
            emit_text(dl, "[isolated]", panel_x + 80.0, panel_y + 9.0, 8.0,
                [0.9, 0.6, 0.1, 0.8]);
        }

        // Count
        let count = format!("{}", self.nodes.len());
        let cw = font::measure_text(&count, 9.0, None);
        emit_text(dl, &count, panel_x + self.width - cw - 8.0, panel_y + 9.0, 9.0, muted);

        // Nodes
        let visible = self.visible_nodes();
        let max_rows = ((panel_h - header_h) / self.row_height) as usize;
        let end = (self.scroll_offset + max_rows).min(visible.len());

        for vis_i in self.scroll_offset..end {
            let node_idx = visible[vis_i];
            let node = &self.nodes[node_idx];
            let row = (vis_i - self.scroll_offset) as f32;
            let ry = panel_y + header_h + row * self.row_height;

            let is_sel = self.selected == Some(node_idx);
            let is_hov = self.hovered == Some(node_idx);

            if is_sel {
                dl.push_quad(panel_x, ry, self.width, self.row_height,
                    [accent_color[0] * 0.3, accent_color[1] * 0.3, accent_color[2] * 0.3, 0.5]);
            } else if is_hov {
                dl.push_quad(panel_x, ry, self.width, self.row_height, [1.0, 1.0, 1.0, 0.05]);
            }

            let indent = 8.0 + node.depth as f32 * 14.0;

            // Expand arrow
            if !node.children.is_empty() {
                let arrow = if node.expanded { "v" } else { ">" };
                emit_text(dl, arrow, panel_x + indent - 10.0, ry + 5.0, 8.0, muted);
            }

            // Visibility eye
            let eye_color = if node.visible { [0.3, 0.8, 0.3, 0.7] } else { [0.5, 0.5, 0.5, 0.3] };
            dl.push_quad(panel_x + indent, ry + 6.0, 6.0, 6.0, eye_color);

            // Kind icon
            let icon_bg = match node.kind {
                ComponentKind::Assembly => [0.2, 0.5, 0.8, 0.7],
                ComponentKind::Part => [0.5, 0.7, 0.3, 0.7],
                ComponentKind::StandardPart => [0.7, 0.7, 0.3, 0.7],
                _ => [0.5, 0.5, 0.5, 0.5],
            };
            dl.push_quad(panel_x + indent + 10.0, ry + 5.0, 12.0, 12.0, icon_bg);
            emit_text(dl, node.kind.icon(), panel_x + indent + 12.0, ry + 5.0, 8.0, [1.0, 1.0, 1.0, 0.9]);

            // Name
            let name_alpha = if node.suppressed { 0.4 } else { 1.0 };
            let nc = [text_color[0], text_color[1], text_color[2], name_alpha];
            let name = if node.name.len() > 18 { &node.name[..18] } else { &node.name };
            emit_text(dl, name, panel_x + indent + 26.0, ry + 5.0, 9.0, nc);

            // Instance count
            if node.instances > 1 {
                let inst = format!("x{}", node.instances);
                let iw = font::measure_text(&inst, 7.0, None);
                emit_text(dl, &inst, panel_x + self.width - iw - 8.0, ry + 7.0, 7.0, muted);
            }

            // Interference indicator
            if node.interference {
                dl.push_quad(panel_x + self.width - 6.0, ry + 3.0, 4.0, 4.0, [0.9, 0.2, 0.2, 0.8]);
            }
        }

        // Border
        dl.push_quad(panel_x + self.width - 1.0, panel_y, 1.0, panel_h, [bg_color[0] + 0.1, bg_color[1] + 0.1, bg_color[2] + 0.1, 0.8]);
    }
}

impl Default for AssemblyBrowser {
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
    fn add_nodes() {
        let mut ab = AssemblyBrowser::new();
        let id1 = ab.add(ComponentNode::new(0, "Top Assembly", ComponentKind::Assembly));
        let id2 = ab.add(ComponentNode::new(0, "Bracket", ComponentKind::Part));
        assert_ne!(id1, id2);
        assert_eq!(ab.nodes.len(), 2);
    }

    #[test]
    fn toggle_visibility() {
        let mut ab = AssemblyBrowser::new();
        ab.add(ComponentNode::new(0, "Part1", ComponentKind::Part));
        assert!(ab.nodes[0].visible);
        ab.toggle_visibility(0);
        assert!(!ab.nodes[0].visible);
    }

    #[test]
    fn isolation_mode() {
        let mut ab = AssemblyBrowser::new();
        ab.add(ComponentNode::new(0, "Part1", ComponentKind::Part));
        ab.isolate(0);
        assert!(ab.isolation_mode);
        assert_eq!(ab.isolated_node, Some(0));
        ab.exit_isolation();
        assert!(!ab.isolation_mode);
    }

    #[test]
    fn total_instances() {
        let mut ab = AssemblyBrowser::new();
        ab.add(ComponentNode::new(0, "Bolt", ComponentKind::StandardPart).with_instances(12));
        ab.add(ComponentNode::new(0, "Bracket", ComponentKind::Part).with_instances(4));
        assert_eq!(ab.total_instances(), 16);
    }

    #[test]
    fn interference_count() {
        let mut ab = AssemblyBrowser::new();
        ab.add(ComponentNode::new(0, "Part1", ComponentKind::Part));
        let mut n = ComponentNode::new(0, "Part2", ComponentKind::Part);
        n.interference = true;
        ab.add(n);
        assert_eq!(ab.interference_count(), 1);
    }
}
