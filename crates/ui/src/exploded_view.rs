//! Exploded view — step-based assembly explosion animation controller.
//!
//! Inspired by SolidWorks Exploded View, Fusion 360 Animation Workspace,
//! and CATIA Explode command. Provides step-by-step explosion with
//! direction, distance, and timeline scrubbing.

use crate::draw::DrawList;
use crate::font;

/// Direction of explosion for a step.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExplodeDirection {
    X,
    Y,
    Z,
    Radial,
    Normal,
    Custom,
}

impl ExplodeDirection {
    pub fn label(self) -> &'static str {
        match self {
            Self::X => "X",
            Self::Y => "Y",
            Self::Z => "Z",
            Self::Radial => "Radial",
            Self::Normal => "Normal",
            Self::Custom => "Custom",
        }
    }
}

/// A single explosion step.
#[derive(Clone, Debug)]
pub struct ExplodeStep {
    /// Component name or ID.
    pub component: String,
    /// Explosion direction.
    pub direction: ExplodeDirection,
    /// Distance (world units).
    pub distance: f32,
    /// Offset vector (computed from direction + distance).
    pub offset: [f32; 3],
    /// Whether this step is enabled.
    pub enabled: bool,
    /// Animation delay (0.0..1.0 in overall timeline).
    pub delay: f32,
}

impl ExplodeStep {
    pub fn new(component: &str, direction: ExplodeDirection, distance: f32) -> Self {
        let offset = match direction {
            ExplodeDirection::X => [distance, 0.0, 0.0],
            ExplodeDirection::Y => [0.0, distance, 0.0],
            ExplodeDirection::Z => [0.0, 0.0, distance],
            _ => [0.0, distance, 0.0],
        };
        Self {
            component: component.to_string(),
            direction,
            distance,
            offset,
            enabled: true,
            delay: 0.0,
        }
    }

    pub fn with_delay(mut self, delay: f32) -> Self {
        self.delay = delay;
        self
    }
}

/// The exploded view controller.
pub struct ExplodedView {
    /// Whether exploded view is active.
    pub active: bool,
    /// Explosion steps.
    pub steps: Vec<ExplodeStep>,
    /// Animation progress (0.0 = collapsed, 1.0 = fully exploded).
    pub progress: f32,
    /// Whether animation is playing.
    pub playing: bool,
    /// Animation speed (seconds for full explode).
    pub duration: f32,
    /// Selected step index.
    pub selected_step: Option<usize>,
    /// Hovered step index.
    pub hovered_step: Option<usize>,
    /// Whether to show explosion lines.
    pub show_lines: bool,
    /// Panel width.
    pub panel_width: f32,
    /// Panel visible.
    pub panel_visible: bool,
}

impl ExplodedView {
    pub fn new() -> Self {
        Self {
            active: false,
            steps: Vec::new(),
            progress: 0.0,
            playing: false,
            duration: 2.0,
            selected_step: None,
            hovered_step: None,
            show_lines: true,
            panel_width: 240.0,
            panel_visible: true,
        }
    }

    /// Toggle exploded view on/off.
    pub fn toggle(&mut self) {
        self.active = !self.active;
        if !self.active {
            self.progress = 0.0;
            self.playing = false;
        }
    }

    /// Add an explosion step.
    pub fn add_step(&mut self, step: ExplodeStep) {
        self.steps.push(step);
    }

    /// Remove a step by index.
    pub fn remove_step(&mut self, idx: usize) -> Option<ExplodeStep> {
        if idx < self.steps.len() { Some(self.steps.remove(idx)) } else { None }
    }

    /// Toggle play/pause animation.
    pub fn toggle_play(&mut self) {
        self.playing = !self.playing;
    }

    /// Reset to collapsed.
    pub fn collapse(&mut self) {
        self.progress = 0.0;
        self.playing = false;
    }

    /// Jump to fully exploded.
    pub fn explode(&mut self) {
        self.progress = 1.0;
        self.playing = false;
    }

    /// Update animation (call each frame with dt in seconds).
    pub fn update(&mut self, dt: f32) {
        if !self.playing || !self.active { return; }
        self.progress += dt / self.duration;
        if self.progress >= 1.0 {
            self.progress = 1.0;
            self.playing = false;
        }
    }

    /// Get the current offset for a step (interpolated by progress).
    pub fn step_offset(&self, idx: usize) -> [f32; 3] {
        if let Some(step) = self.steps.get(idx) {
            if !step.enabled { return [0.0; 3]; }
            let t = ((self.progress - step.delay) / (1.0 - step.delay)).clamp(0.0, 1.0);
            [step.offset[0] * t, step.offset[1] * t, step.offset[2] * t]
        } else {
            [0.0; 3]
        }
    }

    /// Count of enabled steps.
    pub fn enabled_count(&self) -> usize {
        self.steps.iter().filter(|s| s.enabled).count()
    }

    /// Draw the exploded view control panel.
    pub fn draw(
        &self,
        dl: &mut DrawList,
        panel_x: f32,
        panel_y: f32,
        bg_color: [f32; 4],
        text_color: [f32; 4],
        accent_color: [f32; 4],
    ) {
        if !self.active || !self.panel_visible { return; }

        let header_h = 56.0;
        let row_h = 24.0;
        let rows = self.steps.len().min(12);
        let panel_h = header_h + rows as f32 * row_h + 40.0;

        // Background
        dl.push_quad(panel_x, panel_y, self.panel_width, panel_h, bg_color);

        // Border
        let border = [bg_color[0] + 0.1, bg_color[1] + 0.1, bg_color[2] + 0.1, 0.8];
        dl.push_quad(panel_x + self.panel_width - 1.0, panel_y, 1.0, panel_h, border);

        // Title
        emit_text(dl, "Exploded View", panel_x + 8.0, panel_y + 5.0, 11.0, text_color);

        let muted = [text_color[0] * 0.5, text_color[1] * 0.5, text_color[2] * 0.5, text_color[3]];

        // Progress bar
        let bar_y = panel_y + 24.0;
        dl.push_quad(panel_x + 8.0, bar_y, self.panel_width - 16.0, 6.0, [0.3, 0.3, 0.3, 0.5]);
        dl.push_quad(panel_x + 8.0, bar_y, (self.panel_width - 16.0) * self.progress, 6.0, accent_color);

        // Progress label
        let pct = format!("{:.0}%", self.progress * 100.0);
        let pw = font::measure_text(&pct, 8.0, None);
        emit_text(dl, &pct, panel_x + self.panel_width - pw - 8.0, bar_y + 8.0, 8.0, muted);

        // Play/Collapse/Explode buttons
        let btn_y = panel_y + 38.0;
        let play_label = if self.playing { "||" } else { ">" };
        dl.push_quad(panel_x + 8.0, btn_y, 24.0, 16.0, [0.3, 0.3, 0.3, 0.5]);
        emit_text(dl, play_label, panel_x + 14.0, btn_y + 2.0, 10.0, text_color);

        dl.push_quad(panel_x + 36.0, btn_y, 40.0, 16.0, [0.3, 0.3, 0.3, 0.5]);
        emit_text(dl, "Reset", panel_x + 40.0, btn_y + 2.0, 9.0, text_color);

        dl.push_quad(panel_x + 80.0, btn_y, 48.0, 16.0, [0.3, 0.3, 0.3, 0.5]);
        emit_text(dl, "Explode", panel_x + 84.0, btn_y + 2.0, 9.0, text_color);

        // Steps list
        for (i, step) in self.steps.iter().enumerate().take(rows) {
            let ry = panel_y + header_h + i as f32 * row_h;

            let is_sel = self.selected_step == Some(i);
            let is_hov = self.hovered_step == Some(i);

            if is_sel {
                dl.push_quad(panel_x, ry, self.panel_width, row_h,
                    [accent_color[0] * 0.2, accent_color[1] * 0.2, accent_color[2] * 0.2, 0.4]);
            } else if is_hov {
                dl.push_quad(panel_x, ry, self.panel_width, row_h, [1.0, 1.0, 1.0, 0.05]);
            }

            // Enabled indicator
            let en_color = if step.enabled { [0.3, 0.8, 0.3, 0.8] } else { [0.5, 0.5, 0.5, 0.3] };
            dl.push_quad(panel_x + 8.0, ry + 7.0, 8.0, 8.0, en_color);

            // Component name
            let name = if step.component.len() > 16 { &step.component[..16] } else { &step.component };
            let nc = if step.enabled { text_color } else { muted };
            emit_text(dl, name, panel_x + 22.0, ry + 5.0, 9.0, nc);

            // Distance
            let dist = format!("{:.1}", step.distance);
            let dw = font::measure_text(&dist, 8.0, None);
            emit_text(dl, &dist, panel_x + self.panel_width - dw - 8.0, ry + 6.0, 8.0, muted);
        }

        // Lines toggle
        let footer_y = panel_y + header_h + rows as f32 * row_h + 4.0;
        let line_bg = if self.show_lines { accent_color } else { [0.3, 0.3, 0.3, 0.5] };
        dl.push_quad(panel_x + 8.0, footer_y, 12.0, 12.0, line_bg);
        emit_text(dl, "Show lines", panel_x + 26.0, footer_y + 1.0, 9.0, text_color);
    }
}

impl Default for ExplodedView {
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
    fn toggle_and_progress() {
        let mut ev = ExplodedView::new();
        assert!(!ev.active);
        ev.toggle();
        assert!(ev.active);
        assert_eq!(ev.progress, 0.0);
    }

    #[test]
    fn animation_update() {
        let mut ev = ExplodedView::new();
        ev.toggle();
        ev.add_step(ExplodeStep::new("Part1", ExplodeDirection::Y, 50.0));
        ev.playing = true;
        ev.duration = 1.0;
        ev.update(0.5);
        assert!((ev.progress - 0.5).abs() < 0.01);
        ev.update(0.6);
        assert_eq!(ev.progress, 1.0);
        assert!(!ev.playing); // auto-stop at end
    }

    #[test]
    fn step_offset_interpolation() {
        let mut ev = ExplodedView::new();
        ev.active = true;
        ev.add_step(ExplodeStep::new("Part1", ExplodeDirection::X, 100.0));
        ev.progress = 0.5;
        let off = ev.step_offset(0);
        assert!((off[0] - 50.0).abs() < 0.1);
        assert_eq!(off[1], 0.0);
    }

    #[test]
    fn collapse_and_explode() {
        let mut ev = ExplodedView::new();
        ev.active = true;
        ev.explode();
        assert_eq!(ev.progress, 1.0);
        ev.collapse();
        assert_eq!(ev.progress, 0.0);
    }

    #[test]
    fn enabled_count() {
        let mut ev = ExplodedView::new();
        ev.add_step(ExplodeStep::new("A", ExplodeDirection::X, 10.0));
        let mut s = ExplodeStep::new("B", ExplodeDirection::Y, 20.0);
        s.enabled = false;
        ev.add_step(s);
        assert_eq!(ev.enabled_count(), 1);
    }
}
