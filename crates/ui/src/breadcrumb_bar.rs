//! Breadcrumb bar — hierarchical navigation for Assembly > Part > Feature.
//!
//! Inspired by SolidWorks FeatureManager breadcrumb and Fusion 360 browser path.
//! Shows the current location in the document hierarchy and allows quick
//! navigation by clicking any segment.

use crate::draw::DrawList;
use crate::font;

/// A single breadcrumb segment.
#[derive(Clone, Debug)]
pub struct BreadcrumbSegment {
    /// Display label.
    pub label: String,
    /// Icon character.
    pub icon: &'static str,
    /// Navigation target ID (opaque, passed back on click).
    pub target_id: u32,
    /// Whether this is the currently active (leaf) segment.
    pub active: bool,
}

impl BreadcrumbSegment {
    pub fn new(label: &str, icon: &'static str, target_id: u32) -> Self {
        Self {
            label: label.to_string(),
            icon,
            target_id,
            active: false,
        }
    }

    pub fn active(mut self) -> Self {
        self.active = true;
        self
    }
}

/// The breadcrumb bar.
pub struct BreadcrumbBar {
    /// Breadcrumb segments (root → leaf).
    pub segments: Vec<BreadcrumbSegment>,
    /// Hovered segment index.
    pub hovered: Option<usize>,
    /// Height of the bar.
    pub height: f32,
    /// Whether the bar is visible.
    pub visible: bool,
}

impl BreadcrumbBar {
    pub fn new() -> Self {
        Self {
            segments: Vec::new(),
            hovered: None,
            height: 22.0,
            visible: true,
        }
    }

    /// Set the breadcrumb path.
    pub fn set_path(&mut self, segments: Vec<BreadcrumbSegment>) {
        self.segments = segments;
    }

    /// Clear the path.
    pub fn clear(&mut self) {
        self.segments.clear();
    }

    /// Push a segment to the end.
    pub fn push(&mut self, segment: BreadcrumbSegment) {
        // Deactivate all existing segments
        for s in &mut self.segments {
            s.active = false;
        }
        self.segments.push(segment);
    }

    /// Pop the last segment (go up one level).
    pub fn pop(&mut self) -> Option<BreadcrumbSegment> {
        let popped = self.segments.pop();
        // Mark new last as active
        if let Some(last) = self.segments.last_mut() {
            last.active = true;
        }
        popped
    }

    /// Hit test: which segment was clicked?
    pub fn hit_test(&self, mx: f32, my: f32, bar_x: f32, bar_y: f32) -> Option<usize> {
        if !self.visible { return None; }
        if my < bar_y || my > bar_y + self.height { return None; }

        let mut cx = bar_x + 8.0;
        let sep_w = 16.0; // " > " separator width

        for (i, seg) in self.segments.iter().enumerate() {
            let icon_w = if seg.icon.is_empty() { 0.0 } else { 14.0 };
            let label_w = font::measure_text(&seg.label, 11.0, None);
            let seg_w = icon_w + label_w + 4.0;

            if mx >= cx && mx < cx + seg_w {
                return Some(i);
            }
            cx += seg_w;
            if i < self.segments.len() - 1 {
                cx += sep_w;
            }
        }
        None
    }

    /// Draw the breadcrumb bar.
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
        if self.segments.is_empty() { return; }

        // Background
        dl.push_quad(bar_x, bar_y, screen_w, self.height, bg_color);

        // Bottom border
        let border = [bg_color[0] + 0.06, bg_color[1] + 0.06, bg_color[2] + 0.06, 0.8];
        dl.push_quad(bar_x, bar_y + self.height - 1.0, screen_w, 1.0, border);

        let mut cx = bar_x + 8.0;
        let text_y = bar_y + 5.0;

        for (i, seg) in self.segments.iter().enumerate() {
            let is_hovered = self.hovered == Some(i);
            let is_active = seg.active;

            let label_color = if is_active {
                accent_color
            } else if is_hovered {
                [text_color[0], text_color[1], text_color[2], 1.0]
            } else {
                [text_color[0] * 0.7, text_color[1] * 0.7, text_color[2] * 0.7, text_color[3]]
            };

            // Icon
            if !seg.icon.is_empty() {
                emit_text(dl, seg.icon, cx, text_y, 11.0, label_color);
                cx += 14.0;
            }

            // Label
            let label_w = font::measure_text(&seg.label, 11.0, None);

            // Hover underline
            if is_hovered && !is_active {
                dl.push_quad(cx, bar_y + self.height - 3.0, label_w, 1.0, label_color);
            }

            emit_text(dl, &seg.label, cx, text_y, 11.0, label_color);
            cx += label_w + 4.0;

            // Separator " > " between segments
            if i < self.segments.len() - 1 {
                let sep_color = [text_color[0] * 0.4, text_color[1] * 0.4, text_color[2] * 0.4, text_color[3]];
                emit_text(dl, ">", cx + 4.0, text_y, 11.0, sep_color);
                cx += 16.0;
            }
        }
    }
}

impl Default for BreadcrumbBar {
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
    fn empty_path() {
        let bar = BreadcrumbBar::new();
        assert!(bar.segments.is_empty());
        assert!(bar.hit_test(50.0, 10.0, 0.0, 0.0).is_none());
    }

    #[test]
    fn push_and_pop() {
        let mut bar = BreadcrumbBar::new();
        bar.push(BreadcrumbSegment::new("Assembly", "#", 0).active());
        bar.push(BreadcrumbSegment::new("Part1", "@", 1).active());
        assert_eq!(bar.segments.len(), 2);
        assert!(!bar.segments[0].active); // deactivated by push
        assert!(bar.segments[1].active);

        let popped = bar.pop();
        assert!(popped.is_some());
        assert_eq!(popped.unwrap().label, "Part1");
        assert!(bar.segments[0].active); // re-activated
    }

    #[test]
    fn set_path() {
        let mut bar = BreadcrumbBar::new();
        bar.set_path(vec![
            BreadcrumbSegment::new("Root", "#", 0),
            BreadcrumbSegment::new("Child", "@", 1).active(),
        ]);
        assert_eq!(bar.segments.len(), 2);
    }

    #[test]
    fn hit_test_segments() {
        let mut bar = BreadcrumbBar::new();
        bar.push(BreadcrumbSegment::new("Assembly", "#", 0));
        bar.push(BreadcrumbSegment::new("Part", "@", 1).active());
        // First segment starts at x=8; test a click in the first 40px
        let hit = bar.hit_test(20.0, 10.0, 0.0, 0.0);
        assert!(hit.is_some());
    }
}
