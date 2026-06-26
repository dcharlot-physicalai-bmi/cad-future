//! Workspace layout — Blender-style split-panel system.
//!
//! The workspace divides the screen into rectangular areas. Each area hosts
//! a panel type (Viewport, Properties, Outliner, Console, etc.). Areas can
//! be split horizontally or vertically, and borders are draggable.

use crate::draw::DrawList;
use crate::font;
use crate::theme::ThemeColors;

/// Which content a panel area displays.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelKind {
    Viewport,
    Properties,
    Outliner,
    Console,
    Materials,
    Constraints,
}

/// Axis along which a split divides two children.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitAxis {
    Horizontal, // top / bottom
    Vertical,   // left / right
}

/// A node in the workspace layout tree.
#[derive(Debug, Clone)]
pub enum LayoutNode {
    Leaf {
        kind: PanelKind,
    },
    Split {
        axis: SplitAxis,
        /// Fraction [0..1] of the parent area allocated to the first child.
        ratio: f32,
        children: [Box<LayoutNode>; 2],
    },
}

/// Rectangular screen region in pixels.
#[derive(Debug, Clone, Copy)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub fn contains(&self, px: f32, py: f32) -> bool {
        px >= self.x && px <= self.x + self.w && py >= self.y && py <= self.y + self.h
    }
}

/// A resolved panel with its screen-space bounds.
#[derive(Debug, Clone)]
pub struct ResolvedPanel {
    pub kind: PanelKind,
    pub rect: Rect,
}

/// A split border in screen space, for interactive dragging.
#[derive(Debug, Clone)]
struct SplitBorder {
    /// Screen-space position along the split axis.
    position: f32,
    /// Axis of the split this border belongs to.
    axis: SplitAxis,
    /// The bounding rect of the parent region containing this split.
    parent_rect: Rect,
    /// Path of child indices from root to reach this split node.
    path: Vec<usize>,
}

/// The workspace manages the layout tree and resolves it to screen-space panels.
pub struct Workspace {
    root: LayoutNode,
    resolved: Vec<ResolvedPanel>,
    borders: Vec<SplitBorder>,
    // Split border dragging
    dragging: Option<usize>,
    /// Cursor is near a border (for cursor style feedback).
    pub hover_border: Option<SplitAxis>,
}

impl Workspace {
    /// Default CAD workspace: viewport left (75%), properties right (25%).
    pub fn new() -> Self {
        Self {
            root: LayoutNode::Split {
                axis: SplitAxis::Vertical,
                ratio: 0.75,
                children: [
                    Box::new(LayoutNode::Leaf { kind: PanelKind::Viewport }),
                    Box::new(LayoutNode::Split {
                        axis: SplitAxis::Horizontal,
                        ratio: 0.5,
                        children: [
                            Box::new(LayoutNode::Leaf { kind: PanelKind::Properties }),
                            Box::new(LayoutNode::Leaf { kind: PanelKind::Constraints }),
                        ],
                    }),
                ],
            },
            resolved: Vec::new(),
            borders: Vec::new(),
            dragging: None,
            hover_border: None,
        }
    }

    /// Set a custom layout tree.
    pub fn set_layout(&mut self, root: LayoutNode) {
        self.root = root;
    }

    /// Resolve the layout tree into screen-space panels for the given screen size.
    pub fn resolve(&mut self, screen_w: f32, screen_h: f32) {
        self.resolved.clear();
        self.borders.clear();
        let full = Rect { x: 0.0, y: 0.0, w: screen_w, h: screen_h };
        let mut path = Vec::new();
        Self::resolve_node_with_borders(&self.root, full, &mut self.resolved, &mut self.borders, &mut path);
    }

    fn resolve_node_with_borders(
        node: &LayoutNode,
        rect: Rect,
        out: &mut Vec<ResolvedPanel>,
        borders: &mut Vec<SplitBorder>,
        path: &mut Vec<usize>,
    ) {
        match node {
            LayoutNode::Leaf { kind } => {
                out.push(ResolvedPanel { kind: *kind, rect });
            }
            LayoutNode::Split { axis, ratio, children } => {
                let position = match axis {
                    SplitAxis::Horizontal => rect.y + rect.h * ratio,
                    SplitAxis::Vertical => rect.x + rect.w * ratio,
                };

                borders.push(SplitBorder {
                    position,
                    axis: *axis,
                    parent_rect: rect,
                    path: path.clone(),
                });

                let (r1, r2) = match axis {
                    SplitAxis::Horizontal => {
                        let h1 = rect.h * ratio;
                        (
                            Rect { x: rect.x, y: rect.y, w: rect.w, h: h1 },
                            Rect { x: rect.x, y: rect.y + h1, w: rect.w, h: rect.h - h1 },
                        )
                    }
                    SplitAxis::Vertical => {
                        let w1 = rect.w * ratio;
                        (
                            Rect { x: rect.x, y: rect.y, w: w1, h: rect.h },
                            Rect { x: rect.x + w1, y: rect.y, w: rect.w - w1, h: rect.h },
                        )
                    }
                };

                path.push(0);
                Self::resolve_node_with_borders(&children[0], r1, out, borders, path);
                path.pop();
                path.push(1);
                Self::resolve_node_with_borders(&children[1], r2, out, borders, path);
                path.pop();
            }
        }
    }

    /// Get all resolved panels.
    pub fn panels(&self) -> &[ResolvedPanel] {
        &self.resolved
    }

    /// Find which panel contains a screen-space point.
    pub fn panel_at(&self, x: f32, y: f32) -> Option<&ResolvedPanel> {
        self.resolved.iter().find(|p| p.rect.contains(x, y))
    }

    /// Get the viewport panel rect (first Viewport found).
    pub fn viewport_rect(&self) -> Option<Rect> {
        self.resolved
            .iter()
            .find(|p| p.kind == PanelKind::Viewport)
            .map(|p| p.rect)
    }

    /// Draw panel borders and headers into a DrawList, themed.
    pub fn draw(&self, draw: &mut DrawList, theme: &ThemeColors) {
        let header_h = 24.0;
        let border_color = theme.border;
        let header_bg = theme.header_bg;
        let text_color = theme.text_muted;

        for panel in &self.resolved {
            let r = &panel.rect;

            // Panel border (1px)
            draw.push_quad(r.x, r.y, r.w, 1.0, border_color);
            draw.push_quad(r.x, r.y, 1.0, r.h, border_color);
            draw.push_quad(r.x, r.y + r.h - 1.0, r.w, 1.0, border_color);
            draw.push_quad(r.x + r.w - 1.0, r.y, 1.0, r.h, border_color);

            // Header bar
            draw.push_quad(r.x + 1.0, r.y + 1.0, r.w - 2.0, header_h, header_bg);

            // Panel title text
            let title = match panel.kind {
                PanelKind::Viewport => "Viewport",
                PanelKind::Properties => "Properties",
                PanelKind::Outliner => "Outliner",
                PanelKind::Console => "Console",
                PanelKind::Materials => "Materials",
                PanelKind::Constraints => "Constraints",
            };

            let font_size = 12.0;
            let text_x = r.x + 6.0;
            let text_y = r.y + (header_h - font_size) * 0.5 + 1.0;
            let mut cx = text_x;
            for c in title.chars() {
                let params = font::CharQuadParams {
                    c,
                    x: cx,
                    y: text_y,
                    size: font_size,
                    color: text_color,
                    atlas: None,
                };
                let advance = font::emit_char_quads(
                    &params,
                    &mut draw.vertices,
                    &mut draw.indices,
                );
                cx += advance;
            }
        }
    }

    /// Handle mouse interaction for border dragging.
    /// Call each frame with mouse state. Returns true if a drag is active.
    pub fn handle_input(&mut self, mouse_x: f32, mouse_y: f32, mouse_down: bool, screen_w: f32, screen_h: f32) -> bool {
        const GRAB_DISTANCE: f32 = 5.0;
        const MIN_PANEL_SIZE: f32 = 80.0;

        if mouse_down {
            if let Some(border_idx) = self.dragging {
                // Continue drag — update ratio
                let border = &self.borders[border_idx];
                let parent = border.parent_rect;
                let new_ratio = match border.axis {
                    SplitAxis::Vertical => {
                        ((mouse_x - parent.x) / parent.w).clamp(
                            MIN_PANEL_SIZE / parent.w,
                            1.0 - MIN_PANEL_SIZE / parent.w,
                        )
                    }
                    SplitAxis::Horizontal => {
                        ((mouse_y - parent.y) / parent.h).clamp(
                            MIN_PANEL_SIZE / parent.h,
                            1.0 - MIN_PANEL_SIZE / parent.h,
                        )
                    }
                };

                // Navigate the path to find the split node and update its ratio
                let path = border.path.clone();
                Self::set_ratio_at_path(&mut self.root, &path, new_ratio);
                self.resolve(screen_w, screen_h);
                return true;
            } else {
                // Check if mouse is near a border to start dragging
                for (i, border) in self.borders.iter().enumerate() {
                    let near = match border.axis {
                        SplitAxis::Vertical => {
                            (mouse_x - border.position).abs() < GRAB_DISTANCE
                                && mouse_y >= border.parent_rect.y
                                && mouse_y <= border.parent_rect.y + border.parent_rect.h
                        }
                        SplitAxis::Horizontal => {
                            (mouse_y - border.position).abs() < GRAB_DISTANCE
                                && mouse_x >= border.parent_rect.x
                                && mouse_x <= border.parent_rect.x + border.parent_rect.w
                        }
                    };
                    if near {
                        self.dragging = Some(i);
                        self.hover_border = Some(border.axis);
                        return true;
                    }
                }
            }
        } else {
            self.dragging = None;
        }

        // Hover detection for cursor style
        self.hover_border = None;
        for border in &self.borders {
            let near = match border.axis {
                SplitAxis::Vertical => {
                    (mouse_x - border.position).abs() < GRAB_DISTANCE
                        && mouse_y >= border.parent_rect.y
                        && mouse_y <= border.parent_rect.y + border.parent_rect.h
                }
                SplitAxis::Horizontal => {
                    (mouse_y - border.position).abs() < GRAB_DISTANCE
                        && mouse_x >= border.parent_rect.x
                        && mouse_x <= border.parent_rect.x + border.parent_rect.w
                }
            };
            if near {
                self.hover_border = Some(border.axis);
                break;
            }
        }

        false
    }

    /// Navigate the layout tree by path and set the split ratio.
    fn set_ratio_at_path(node: &mut LayoutNode, path: &[usize], ratio: f32) {
        match node {
            LayoutNode::Split { ratio: r, children, .. } => {
                if path.is_empty() {
                    *r = ratio;
                } else {
                    Self::set_ratio_at_path(&mut children[path[0]], &path[1..], ratio);
                }
            }
            LayoutNode::Leaf { .. } => {}
        }
    }
}

impl Default for Workspace {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_workspace_resolves() {
        let mut ws = Workspace::new();
        ws.resolve(1920.0, 1080.0);
        let panels = ws.panels();
        assert_eq!(panels.len(), 3); // viewport, properties, constraints
    }

    #[test]
    fn viewport_is_largest_panel() {
        let mut ws = Workspace::new();
        ws.resolve(1920.0, 1080.0);
        let vp = ws.viewport_rect().unwrap();
        assert!(vp.w > 1000.0);
        assert!(vp.h > 1000.0);
    }

    #[test]
    fn panel_at_finds_correct_panel() {
        let mut ws = Workspace::new();
        ws.resolve(1920.0, 1080.0);
        // Top-left should be viewport
        let p = ws.panel_at(100.0, 100.0).unwrap();
        assert_eq!(p.kind, PanelKind::Viewport);
        // Far right should be properties or constraints
        let p = ws.panel_at(1800.0, 100.0).unwrap();
        assert!(p.kind == PanelKind::Properties || p.kind == PanelKind::Constraints);
    }

    #[test]
    fn single_leaf_fills_screen() {
        let mut ws = Workspace::new();
        ws.set_layout(LayoutNode::Leaf { kind: PanelKind::Viewport });
        ws.resolve(800.0, 600.0);
        assert_eq!(ws.panels().len(), 1);
        let r = ws.panels()[0].rect;
        assert!((r.w - 800.0).abs() < f32::EPSILON);
        assert!((r.h - 600.0).abs() < f32::EPSILON);
    }

    #[test]
    fn horizontal_split() {
        let mut ws = Workspace::new();
        ws.set_layout(LayoutNode::Split {
            axis: SplitAxis::Horizontal,
            ratio: 0.6,
            children: [
                Box::new(LayoutNode::Leaf { kind: PanelKind::Viewport }),
                Box::new(LayoutNode::Leaf { kind: PanelKind::Console }),
            ],
        });
        ws.resolve(1000.0, 1000.0);
        let panels = ws.panels();
        assert_eq!(panels.len(), 2);
        assert!((panels[0].rect.h - 600.0).abs() < 1.0);
        assert!((panels[1].rect.h - 400.0).abs() < 1.0);
    }

    #[test]
    fn rect_contains() {
        let r = Rect { x: 10.0, y: 20.0, w: 100.0, h: 50.0 };
        assert!(r.contains(50.0, 40.0));
        assert!(!r.contains(5.0, 40.0));
        assert!(!r.contains(50.0, 80.0));
    }
}
