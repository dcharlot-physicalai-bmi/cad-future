//! Parametric timeline bar — bottom horizontal strip with feature history.
//!
//! Inspired by Fusion 360 timeline and SolidWorks rollback bar.
//! Displays chronological feature/action icons with a draggable rollback marker.

use crate::draw::DrawList;
use crate::font;

/// A single entry in the timeline.
#[derive(Clone, Debug)]
pub struct TimelineEntry {
    /// Display label (short, e.g. "Cube", "Extrude", "Fillet").
    pub label: String,
    /// Icon character (Unicode symbol).
    pub icon: &'static str,
    /// Whether this feature is suppressed (greyed out).
    pub suppressed: bool,
    /// Whether this entry has an error/warning.
    pub has_warning: bool,
}

impl TimelineEntry {
    pub fn new(label: &str, icon: &'static str) -> Self {
        Self {
            label: label.to_string(),
            icon,
            suppressed: false,
            has_warning: false,
        }
    }

    pub fn suppressed(mut self) -> Self {
        self.suppressed = true;
        self
    }
}

/// The parametric timeline bar.
pub struct Timeline {
    /// All entries in chronological order.
    pub entries: Vec<TimelineEntry>,
    /// Rollback marker position (index). Everything after this is "future" / rolled back.
    /// `None` means all features are active (marker at the end).
    pub rollback_pos: Option<usize>,
    /// Currently hovered entry index.
    pub hovered: Option<usize>,
    /// Whether the timeline is visible.
    pub visible: bool,
    /// Height of the timeline bar in pixels.
    pub height: f32,
    /// Scroll offset for long timelines (horizontal).
    pub scroll_x: f32,
}

impl Timeline {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            rollback_pos: None,
            hovered: None,
            visible: true,
            height: 36.0,
            scroll_x: 0.0,
        }
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn push(&mut self, entry: TimelineEntry) {
        self.entries.push(entry);
    }

    /// How many entries are "active" (before the rollback marker).
    pub fn active_count(&self) -> usize {
        self.rollback_pos.unwrap_or(self.entries.len())
    }

    /// Set the rollback position. `None` means "all active".
    pub fn set_rollback(&mut self, pos: Option<usize>) {
        self.rollback_pos = pos;
    }

    /// Hit test: which entry is under the mouse? Returns entry index.
    pub fn hit_test(&self, mx: f32, my: f32, bar_x: f32, bar_y: f32) -> Option<usize> {
        if !self.visible { return None; }
        if my < bar_y || my > bar_y + self.height { return None; }

        let item_w = 44.0;
        let start_x = bar_x + 8.0 - self.scroll_x;

        for (i, _) in self.entries.iter().enumerate() {
            let ix = start_x + i as f32 * item_w;
            if mx >= ix && mx < ix + item_w {
                return Some(i);
            }
        }
        None
    }

    /// Hit test the rollback marker drag zone.
    /// Returns true if the mouse is on the rollback marker.
    pub fn hit_test_rollback(&self, mx: f32, my: f32, bar_x: f32, bar_y: f32) -> bool {
        if !self.visible { return false; }
        let item_w = 44.0;
        let active = self.active_count();
        let marker_x = bar_x + 8.0 - self.scroll_x + active as f32 * item_w - 2.0;
        mx >= marker_x && mx < marker_x + 6.0 && my >= bar_y && my < bar_y + self.height
    }

    /// Draw the timeline bar.
    pub fn draw(
        &self,
        dl: &mut DrawList,
        bar_y: f32,
        screen_w: f32,
        bg_color: [f32; 4],
        text_color: [f32; 4],
        accent_color: [f32; 4],
    ) {
        if !self.visible { return; }

        let bar_x = 0.0;
        let bar_h = self.height;

        // Background
        dl.push_quad(bar_x, bar_y, screen_w, bar_h, bg_color);

        // Top border
        let border_color = [
            bg_color[0] + 0.1,
            bg_color[1] + 0.1,
            bg_color[2] + 0.1,
            1.0,
        ];
        dl.push_quad(bar_x, bar_y, screen_w, 1.0, border_color);

        let item_w = 44.0;
        let item_h = 28.0;
        let item_y = bar_y + (bar_h - item_h) * 0.5;
        let start_x = bar_x + 8.0 - self.scroll_x;
        let active = self.active_count();

        for (i, entry) in self.entries.iter().enumerate() {
            let ix = start_x + i as f32 * item_w;
            let is_active = i < active;
            let is_hovered = self.hovered == Some(i);

            // Item background
            let item_bg = if is_hovered {
                [accent_color[0], accent_color[1], accent_color[2], 0.3]
            } else if !is_active || entry.suppressed {
                [bg_color[0] - 0.03, bg_color[1] - 0.03, bg_color[2] - 0.03, 0.5]
            } else {
                [bg_color[0] + 0.04, bg_color[1] + 0.04, bg_color[2] + 0.04, 1.0]
            };
            dl.push_quad(ix, item_y, item_w - 2.0, item_h, item_bg);

            // Icon
            let icon_color = if !is_active || entry.suppressed {
                [text_color[0] * 0.4, text_color[1] * 0.4, text_color[2] * 0.4, 0.5]
            } else if entry.has_warning {
                [1.0, 0.7, 0.2, 1.0]
            } else {
                text_color
            };
            let icon_x = ix + (item_w - 2.0) * 0.5 - 4.0;
            emit_text(dl, entry.icon, icon_x, item_y + 2.0, 12.0, icon_color);

            // Label (truncated)
            let label = if entry.label.len() > 5 {
                &entry.label[..5]
            } else {
                &entry.label
            };
            let label_w = font::measure_text(label, 9.0, None);
            let label_x = ix + (item_w - 2.0 - label_w) * 0.5;
            emit_text(dl, label, label_x, item_y + 16.0, 9.0, icon_color);
        }

        // Rollback marker (blue vertical bar)
        let marker_x = start_x + active as f32 * item_w - 2.0;
        dl.push_quad(marker_x, bar_y + 2.0, 3.0, bar_h - 4.0, accent_color);

        // Marker handle (small diamond/triangle)
        dl.push_quad(marker_x - 2.0, bar_y, 7.0, 6.0, accent_color);

        // Label: "History" at left
        emit_text(dl, "Timeline", bar_x + screen_w - 60.0, bar_y + bar_h * 0.5 - 5.0, 10.0,
            [text_color[0], text_color[1], text_color[2], 0.5]);
    }
}

impl Default for Timeline {
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
    fn new_timeline() {
        let tl = Timeline::new();
        assert!(tl.entries.is_empty());
        assert_eq!(tl.active_count(), 0);
    }

    #[test]
    fn push_and_active() {
        let mut tl = Timeline::new();
        tl.push(TimelineEntry::new("Cube", "#"));
        tl.push(TimelineEntry::new("Extrude", "E"));
        tl.push(TimelineEntry::new("Fillet", "F"));
        assert_eq!(tl.active_count(), 3);
        tl.set_rollback(Some(1));
        assert_eq!(tl.active_count(), 1);
    }

    #[test]
    fn hit_test_entry() {
        let mut tl = Timeline::new();
        tl.push(TimelineEntry::new("Cube", "#"));
        tl.push(TimelineEntry::new("Extrude", "E"));
        // Entry 0 starts at x=8, width=44
        assert_eq!(tl.hit_test(20.0, 10.0, 0.0, 0.0), Some(0));
        assert_eq!(tl.hit_test(55.0, 10.0, 0.0, 0.0), Some(1));
        assert_eq!(tl.hit_test(200.0, 10.0, 0.0, 0.0), None);
    }

    #[test]
    fn rollback_marker_hit() {
        let mut tl = Timeline::new();
        tl.push(TimelineEntry::new("A", "#"));
        tl.push(TimelineEntry::new("B", "#"));
        // Rollback at end: marker at x = 8 + 2*44 - 2 = 94
        assert!(tl.hit_test_rollback(95.0, 10.0, 0.0, 0.0));
        assert!(!tl.hit_test_rollback(20.0, 10.0, 0.0, 0.0));
    }
}
