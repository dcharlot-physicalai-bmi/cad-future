//! Progress overlay — modal spinner and progress bar for long operations.
//!
//! Inspired by Fusion 360 rebuild progress, SolidWorks feature rebuild bar,
//! and Ansys solve progress dialogs. Shows a centered overlay with
//! operation name, progress bar, elapsed time, and optional cancel.

use crate::draw::DrawList;
use crate::font;

/// Kind of progress display.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProgressKind {
    /// Indeterminate (spinner/pulsing bar).
    Indeterminate,
    /// Determinate (0.0 to 1.0 progress bar).
    Determinate,
}

/// A long-running operation's progress.
pub struct ProgressOverlay {
    /// Whether the overlay is visible.
    pub visible: bool,
    /// Operation name (e.g., "Rebuilding...", "Exporting STL...").
    pub label: String,
    /// Progress kind.
    pub kind: ProgressKind,
    /// Progress value (0.0 to 1.0) for determinate.
    pub progress: f32,
    /// Elapsed seconds since the operation started.
    pub elapsed: f32,
    /// Whether the operation is cancellable.
    pub cancellable: bool,
    /// Whether cancel was requested.
    pub cancel_requested: bool,
    /// Hovered on cancel button.
    pub cancel_hovered: bool,
    /// Animation timer for indeterminate spinner.
    pub anim_t: f32,
    /// Sub-label (e.g., "Processing feature 3 of 12").
    pub sub_label: String,
}

impl ProgressOverlay {
    pub fn new() -> Self {
        Self {
            visible: false,
            label: String::new(),
            kind: ProgressKind::Indeterminate,
            progress: 0.0,
            elapsed: 0.0,
            cancellable: true,
            cancel_requested: false,
            cancel_hovered: false,
            anim_t: 0.0,
            sub_label: String::new(),
        }
    }

    /// Begin an indeterminate operation.
    pub fn begin(&mut self, label: &str) {
        self.visible = true;
        self.label = label.to_string();
        self.kind = ProgressKind::Indeterminate;
        self.progress = 0.0;
        self.elapsed = 0.0;
        self.cancel_requested = false;
        self.sub_label.clear();
        self.anim_t = 0.0;
    }

    /// Begin a determinate operation.
    pub fn begin_determinate(&mut self, label: &str) {
        self.begin(label);
        self.kind = ProgressKind::Determinate;
    }

    /// Update progress (determinate only).
    pub fn set_progress(&mut self, value: f32, sub_label: &str) {
        self.progress = value.clamp(0.0, 1.0);
        self.sub_label = sub_label.to_string();
    }

    /// End the operation.
    pub fn end(&mut self) {
        self.visible = false;
        self.cancel_requested = false;
    }

    /// Update animation. Call each frame.
    pub fn update(&mut self, dt: f32) {
        if !self.visible { return; }
        self.elapsed += dt;
        self.anim_t += dt;
    }

    /// Format elapsed time as "0:05" or "1:23".
    fn format_elapsed(&self) -> String {
        let secs = self.elapsed as u32;
        let mins = secs / 60;
        let rem = secs % 60;
        format!("{}:{:02}", mins, rem)
    }

    /// Hit test for cancel button.
    pub fn hit_test_cancel(&self, mx: f32, my: f32, screen_w: f32, screen_h: f32) -> bool {
        if !self.visible || !self.cancellable { return false; }

        let panel_w = 300.0;
        let panel_h = 120.0;
        let px = (screen_w - panel_w) * 0.5;
        let py = (screen_h - panel_h) * 0.5;

        let btn_w = 70.0;
        let btn_h = 24.0;
        let btn_x = px + (panel_w - btn_w) * 0.5;
        let btn_y = py + panel_h - btn_h - 10.0;

        mx >= btn_x && mx < btn_x + btn_w && my >= btn_y && my < btn_y + btn_h
    }

    /// Draw the progress overlay.
    pub fn draw(&self, dl: &mut DrawList, screen_w: f32, screen_h: f32) {
        if !self.visible { return; }

        // Dim background
        dl.push_quad(0.0, 0.0, screen_w, screen_h, [0.0, 0.0, 0.0, 0.4]);

        let panel_w = 300.0;
        let panel_h = 120.0;
        let px = (screen_w - panel_w) * 0.5;
        let py = (screen_h - panel_h) * 0.5;

        // Shadow
        dl.push_quad(px + 3.0, py + 3.0, panel_w, panel_h, [0.0, 0.0, 0.0, 0.3]);

        // Panel background
        dl.push_quad(px, py, panel_w, panel_h, [0.16, 0.16, 0.18, 0.97]);

        // Border
        let border = [0.3, 0.3, 0.35, 0.8];
        dl.push_quad(px, py, panel_w, 1.0, border);
        dl.push_quad(px, py + panel_h - 1.0, panel_w, 1.0, border);
        dl.push_quad(px, py, 1.0, panel_h, border);
        dl.push_quad(px + panel_w - 1.0, py, 1.0, panel_h, border);

        // Operation label
        emit_text(dl, &self.label, px + 12.0, py + 12.0, 13.0, [0.9, 0.9, 0.9, 1.0]);

        // Elapsed time
        let elapsed = self.format_elapsed();
        let elapsed_w = font::measure_text(&elapsed, 10.0, None);
        emit_text(dl, &elapsed, px + panel_w - elapsed_w - 12.0, py + 14.0, 10.0,
            [0.5, 0.5, 0.5, 0.8]);

        // Progress bar area
        let bar_x = px + 12.0;
        let bar_y = py + 36.0;
        let bar_w = panel_w - 24.0;
        let bar_h = 8.0;

        // Bar background (track)
        dl.push_quad(bar_x, bar_y, bar_w, bar_h, [0.1, 0.1, 0.12, 1.0]);

        match self.kind {
            ProgressKind::Determinate => {
                // Filled portion
                let fill_w = bar_w * self.progress;
                dl.push_quad(bar_x, bar_y, fill_w, bar_h, [0.2, 0.6, 1.0, 0.9]);

                // Percentage label
                let pct = format!("{}%", (self.progress * 100.0) as u32);
                emit_text(dl, &pct, bar_x + bar_w + 4.0 - 30.0, bar_y - 1.0, 10.0,
                    [0.7, 0.7, 0.7, 0.9]);
            }
            ProgressKind::Indeterminate => {
                // Pulsing bar segment
                let cycle = (self.anim_t * 1.5) % 1.0;
                let seg_w = bar_w * 0.3;
                let seg_x = bar_x + (bar_w - seg_w) * cycle;
                dl.push_quad(seg_x, bar_y, seg_w, bar_h, [0.2, 0.6, 1.0, 0.8]);
            }
        }

        // Sub-label
        if !self.sub_label.is_empty() {
            emit_text(dl, &self.sub_label, bar_x, bar_y + bar_h + 6.0, 10.0,
                [0.6, 0.6, 0.6, 0.8]);
        }

        // Cancel button
        if self.cancellable {
            let btn_w = 70.0;
            let btn_h = 24.0;
            let btn_x = px + (panel_w - btn_w) * 0.5;
            let btn_y = py + panel_h - btn_h - 10.0;

            let btn_bg = if self.cancel_hovered {
                [0.7, 0.2, 0.2, 0.9]
            } else {
                [0.35, 0.15, 0.15, 0.8]
            };
            dl.push_quad(btn_x, btn_y, btn_w, btn_h, btn_bg);

            // Border
            let btn_border = [0.6, 0.2, 0.2, 0.6];
            dl.push_quad(btn_x, btn_y, btn_w, 1.0, btn_border);
            dl.push_quad(btn_x, btn_y + btn_h - 1.0, btn_w, 1.0, btn_border);
            dl.push_quad(btn_x, btn_y, 1.0, btn_h, btn_border);
            dl.push_quad(btn_x + btn_w - 1.0, btn_y, 1.0, btn_h, btn_border);

            let label = if self.cancel_requested { "Stopping..." } else { "Cancel" };
            let label_w = font::measure_text(label, 11.0, None);
            emit_text(dl, label, btn_x + (btn_w - label_w) * 0.5, btn_y + 6.0, 11.0,
                [1.0, 1.0, 1.0, 0.9]);
        }
    }
}

impl Default for ProgressOverlay {
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
    fn begin_and_end() {
        let mut po = ProgressOverlay::new();
        assert!(!po.visible);
        po.begin("Rebuilding...");
        assert!(po.visible);
        assert_eq!(po.kind, ProgressKind::Indeterminate);
        po.end();
        assert!(!po.visible);
    }

    #[test]
    fn determinate_progress() {
        let mut po = ProgressOverlay::new();
        po.begin_determinate("Exporting STL...");
        assert_eq!(po.kind, ProgressKind::Determinate);
        po.set_progress(0.5, "Face 50 of 100");
        assert!((po.progress - 0.5).abs() < 0.01);
        assert_eq!(po.sub_label, "Face 50 of 100");
    }

    #[test]
    fn elapsed_format() {
        let mut po = ProgressOverlay::new();
        po.elapsed = 65.0;
        assert_eq!(po.format_elapsed(), "1:05");
    }

    #[test]
    fn cancel_request() {
        let mut po = ProgressOverlay::new();
        po.begin("Solving...");
        po.cancellable = true;
        assert!(!po.cancel_requested);
        po.cancel_requested = true;
        assert!(po.cancel_requested);
    }

    #[test]
    fn animation_updates() {
        let mut po = ProgressOverlay::new();
        po.begin("Working...");
        po.update(0.5);
        assert!(po.elapsed > 0.4);
        assert!(po.anim_t > 0.4);
    }
}
