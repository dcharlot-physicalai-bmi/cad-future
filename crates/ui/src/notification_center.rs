//! Notification center — collapsible panel for warnings, errors, and info.
//!
//! Inspired by Fusion 360 notification panel, SolidWorks "What's Wrong" dialog,
//! and Ansys message window. Collects build errors, DFM warnings, and system
//! messages in a scrollable panel that can be toggled open/closed.

use crate::draw::DrawList;
use crate::font;

/// Severity level of a notification.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NotificationLevel {
    Info,
    Warning,
    Error,
}

impl NotificationLevel {
    pub fn icon(self) -> &'static str {
        match self {
            Self::Info => "i",
            Self::Warning => "!",
            Self::Error => "X",
        }
    }

    pub fn color(self) -> [f32; 4] {
        match self {
            Self::Info => [0.3, 0.7, 1.0, 0.9],
            Self::Warning => [1.0, 0.75, 0.2, 0.9],
            Self::Error => [1.0, 0.3, 0.3, 0.9],
        }
    }
}

/// A single notification entry.
#[derive(Clone, Debug)]
pub struct Notification {
    /// Message text.
    pub message: String,
    /// Severity level.
    pub level: NotificationLevel,
    /// Source/context (e.g., "DFM Check", "Rebuild", "System").
    pub source: String,
    /// Whether this notification has been read/dismissed.
    pub read: bool,
    /// Timestamp (seconds since app start).
    pub timestamp: f32,
}

impl Notification {
    pub fn new(message: &str, level: NotificationLevel, source: &str) -> Self {
        Self {
            message: message.to_string(),
            level,
            source: source.to_string(),
            read: false,
            timestamp: 0.0,
        }
    }

    pub fn with_timestamp(mut self, t: f32) -> Self {
        self.timestamp = t;
        self
    }
}

/// The notification center panel.
pub struct NotificationCenter {
    /// All notifications (newest first).
    pub entries: Vec<Notification>,
    /// Whether the panel is expanded.
    pub expanded: bool,
    /// Panel width.
    pub width: f32,
    /// Maximum visible entries when expanded.
    pub max_visible: usize,
    /// Scroll offset.
    pub scroll_offset: usize,
    /// Hovered entry index.
    pub hovered: Option<usize>,
    /// Badge count (unread).
    pub unread_count: usize,
    /// Filter: show only this level (None = all).
    pub filter: Option<NotificationLevel>,
}

impl NotificationCenter {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            expanded: false,
            width: 320.0,
            max_visible: 8,
            scroll_offset: 0,
            hovered: None,
            unread_count: 0,
            filter: None,
        }
    }

    /// Push a new notification.
    pub fn push(&mut self, notification: Notification) {
        self.unread_count += 1;
        self.entries.insert(0, notification); // newest first
        // Cap at 100 entries
        if self.entries.len() > 100 {
            self.entries.pop();
        }
    }

    /// Convenience: push an info message.
    pub fn info(&mut self, message: &str, source: &str, time: f32) {
        self.push(Notification::new(message, NotificationLevel::Info, source).with_timestamp(time));
    }

    /// Convenience: push a warning.
    pub fn warn(&mut self, message: &str, source: &str, time: f32) {
        self.push(Notification::new(message, NotificationLevel::Warning, source).with_timestamp(time));
    }

    /// Convenience: push an error.
    pub fn error(&mut self, message: &str, source: &str, time: f32) {
        self.push(Notification::new(message, NotificationLevel::Error, source).with_timestamp(time));
    }

    /// Toggle expanded/collapsed.
    pub fn toggle(&mut self) {
        self.expanded = !self.expanded;
        if self.expanded {
            // Mark all as read
            self.unread_count = 0;
            for entry in &mut self.entries {
                entry.read = true;
            }
        }
    }

    /// Clear all notifications.
    pub fn clear_all(&mut self) {
        self.entries.clear();
        self.unread_count = 0;
    }

    /// Count entries matching current filter.
    pub fn filtered_count(&self) -> usize {
        match self.filter {
            None => self.entries.len(),
            Some(level) => self.entries.iter().filter(|e| e.level == level).count(),
        }
    }

    /// Count by level.
    pub fn count_by_level(&self, level: NotificationLevel) -> usize {
        self.entries.iter().filter(|e| e.level == level).count()
    }

    /// Hit test the toggle button (badge). Returns true if clicked.
    pub fn hit_test_badge(&self, mx: f32, my: f32, badge_x: f32, badge_y: f32) -> bool {
        let badge_w = 28.0;
        let badge_h = 22.0;
        mx >= badge_x && mx < badge_x + badge_w && my >= badge_y && my < badge_y + badge_h
    }

    /// Hit test an entry in the expanded panel. Returns entry index.
    pub fn hit_test_entry(
        &self, mx: f32, my: f32,
        panel_x: f32, panel_y: f32,
    ) -> Option<usize> {
        if !self.expanded { return None; }

        let entry_h = 36.0;
        let header_h = 28.0;

        let entries: Vec<_> = match self.filter {
            None => self.entries.iter().enumerate().collect(),
            Some(level) => self.entries.iter().enumerate()
                .filter(|(_, e)| e.level == level).collect(),
        };

        for (vis_i, (real_i, _)) in entries.iter()
            .skip(self.scroll_offset)
            .take(self.max_visible)
            .enumerate()
        {
            let ey = panel_y + header_h + vis_i as f32 * entry_h;
            if mx >= panel_x && mx < panel_x + self.width
                && my >= ey && my < ey + entry_h
            {
                return Some(*real_i);
            }
        }
        None
    }

    /// Draw the notification center.
    pub fn draw(
        &self,
        dl: &mut DrawList,
        panel_x: f32,
        panel_y: f32,
        bg_color: [f32; 4],
        text_color: [f32; 4],
    ) {
        let entry_h = 36.0;
        let header_h = 28.0;

        if !self.expanded {
            // Draw just the badge/icon
            self.draw_badge(dl, panel_x, panel_y, text_color);
            return;
        }

        // Collect filtered entries
        let entries: Vec<_> = match self.filter {
            None => self.entries.iter().enumerate().collect(),
            Some(level) => self.entries.iter().enumerate()
                .filter(|(_, e)| e.level == level).collect(),
        };

        let visible_count = entries.len().min(self.max_visible);
        let panel_h = header_h + visible_count as f32 * entry_h + 4.0;

        // Shadow
        dl.push_quad(panel_x + 2.0, panel_y + 2.0, self.width, panel_h, [0.0, 0.0, 0.0, 0.25]);

        // Background
        dl.push_quad(panel_x, panel_y, self.width, panel_h, bg_color);

        // Border
        let border = [bg_color[0] + 0.1, bg_color[1] + 0.1, bg_color[2] + 0.1, 0.8];
        dl.push_quad(panel_x, panel_y, self.width, 1.0, border);
        dl.push_quad(panel_x, panel_y + panel_h - 1.0, self.width, 1.0, border);
        dl.push_quad(panel_x, panel_y, 1.0, panel_h, border);
        dl.push_quad(panel_x + self.width - 1.0, panel_y, 1.0, panel_h, border);

        // Header
        let title = format!(
            "Notifications ({})  E:{} W:{} I:{}",
            self.entries.len(),
            self.count_by_level(NotificationLevel::Error),
            self.count_by_level(NotificationLevel::Warning),
            self.count_by_level(NotificationLevel::Info),
        );
        emit_text(dl, &title, panel_x + 8.0, panel_y + 7.0, 11.0, text_color);

        // Header separator
        dl.push_quad(panel_x + 4.0, panel_y + header_h - 1.0, self.width - 8.0, 1.0, border);

        // Entries
        for (vis_i, (_real_i, entry)) in entries.iter()
            .skip(self.scroll_offset)
            .take(self.max_visible)
            .enumerate()
        {
            let ey = panel_y + header_h + vis_i as f32 * entry_h;

            // Hover highlight
            if self.hovered == Some(*_real_i) {
                dl.push_quad(panel_x + 2.0, ey, self.width - 4.0, entry_h,
                    [bg_color[0] + 0.06, bg_color[1] + 0.06, bg_color[2] + 0.06, 1.0]);
            }

            // Alternating row bg
            if vis_i % 2 == 1 {
                dl.push_quad(panel_x + 2.0, ey, self.width - 4.0, entry_h,
                    [bg_color[0] + 0.02, bg_color[1] + 0.02, bg_color[2] + 0.02, 0.5]);
            }

            // Level icon
            let icon_color = entry.level.color();
            emit_text(dl, entry.level.icon(), panel_x + 8.0, ey + 4.0, 12.0, icon_color);

            // Unread indicator
            if !entry.read {
                dl.push_quad(panel_x + 4.0, ey + 6.0, 3.0, 3.0, icon_color);
            }

            // Source label
            let source_color = [text_color[0] * 0.6, text_color[1] * 0.6, text_color[2] * 0.6, text_color[3]];
            emit_text(dl, &entry.source, panel_x + 22.0, ey + 4.0, 9.0, source_color);

            // Message text (truncate if too long)
            let max_chars = (self.width as usize - 40) / 6;
            let display_msg = if entry.message.len() > max_chars {
                format!("{}...", &entry.message[..max_chars.saturating_sub(3)])
            } else {
                entry.message.clone()
            };
            emit_text(dl, &display_msg, panel_x + 22.0, ey + 18.0, 10.0, text_color);

            // Separator
            dl.push_quad(panel_x + 8.0, ey + entry_h - 1.0, self.width - 16.0, 1.0,
                [border[0], border[1], border[2], 0.3]);
        }

        // Scroll indicator
        if entries.len() > self.max_visible {
            let track_h = visible_count as f32 * entry_h;
            let thumb_h = (self.max_visible as f32 / entries.len() as f32 * track_h).max(10.0);
            let thumb_y = panel_y + header_h
                + (self.scroll_offset as f32 / entries.len() as f32 * track_h);
            dl.push_quad(
                panel_x + self.width - 4.0, thumb_y,
                3.0, thumb_h,
                [0.5, 0.5, 0.5, 0.3],
            );
        }
    }

    /// Draw the compact badge (when collapsed).
    fn draw_badge(&self, dl: &mut DrawList, x: f32, y: f32, text_color: [f32; 4]) {
        let badge_w = 28.0;
        let badge_h = 22.0;

        // Icon
        let icon_color = if self.unread_count > 0 {
            [1.0, 0.75, 0.2, 1.0]
        } else {
            [text_color[0] * 0.5, text_color[1] * 0.5, text_color[2] * 0.5, text_color[3]]
        };
        emit_text(dl, "!", x + 8.0, y + 5.0, 12.0, icon_color);

        // Unread badge count
        if self.unread_count > 0 {
            let count_str = if self.unread_count > 9 {
                "9+".to_string()
            } else {
                self.unread_count.to_string()
            };
            let cw = font::measure_text(&count_str, 8.0, None);
            let cx = x + badge_w - cw - 2.0;
            let cy = y + 1.0;
            dl.push_quad(cx - 2.0, cy, cw + 4.0, 10.0, [0.9, 0.2, 0.2, 0.9]);
            emit_text(dl, &count_str, cx, cy + 1.0, 8.0, [1.0, 1.0, 1.0, 1.0]);
        }

        let _ = badge_h; // suppress unused
    }
}

impl Default for NotificationCenter {
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
    fn push_and_count() {
        let mut nc = NotificationCenter::new();
        nc.info("Build complete", "System", 0.0);
        nc.warn("Thin wall detected", "DFM", 1.0);
        nc.error("Feature failed", "Rebuild", 2.0);
        assert_eq!(nc.entries.len(), 3);
        assert_eq!(nc.unread_count, 3);
        assert_eq!(nc.count_by_level(NotificationLevel::Error), 1);
        assert_eq!(nc.count_by_level(NotificationLevel::Warning), 1);
    }

    #[test]
    fn toggle_marks_read() {
        let mut nc = NotificationCenter::new();
        nc.warn("Test", "DFM", 0.0);
        assert_eq!(nc.unread_count, 1);
        nc.toggle(); // expand
        assert_eq!(nc.unread_count, 0);
        assert!(nc.entries[0].read);
    }

    #[test]
    fn clear_all() {
        let mut nc = NotificationCenter::new();
        nc.info("a", "s", 0.0);
        nc.info("b", "s", 1.0);
        nc.clear_all();
        assert!(nc.entries.is_empty());
        assert_eq!(nc.unread_count, 0);
    }

    #[test]
    fn newest_first() {
        let mut nc = NotificationCenter::new();
        nc.info("first", "s", 0.0);
        nc.info("second", "s", 1.0);
        assert_eq!(nc.entries[0].message, "second");
        assert_eq!(nc.entries[1].message, "first");
    }

    #[test]
    fn filter_count() {
        let mut nc = NotificationCenter::new();
        nc.info("a", "s", 0.0);
        nc.warn("b", "s", 0.0);
        nc.error("c", "s", 0.0);
        nc.filter = Some(NotificationLevel::Warning);
        assert_eq!(nc.filtered_count(), 1);
        nc.filter = None;
        assert_eq!(nc.filtered_count(), 3);
    }
}
