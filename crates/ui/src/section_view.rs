//! Section view — cross-section cutting plane control.
//!
//! Inspired by SolidWorks Section View, Fusion 360 Section Analysis,
//! and AutoCAD SECTIONPLANE. Provides a movable cutting plane that reveals
//! internal geometry with optional cap fill and offset control.

use crate::draw::DrawList;
use crate::font;

/// Which axis the section plane is aligned to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SectionPlane {
    X,
    Y,
    Z,
    Custom,
}

impl SectionPlane {
    pub fn label(self) -> &'static str {
        match self {
            Self::X => "YZ Plane (X)",
            Self::Y => "XZ Plane (Y)",
            Self::Z => "XY Plane (Z)",
            Self::Custom => "Custom",
        }
    }

    pub fn short(self) -> &'static str {
        match self {
            Self::X => "X",
            Self::Y => "Y",
            Self::Z => "Z",
            Self::Custom => "?",
        }
    }

    pub fn normal(self) -> [f32; 3] {
        match self {
            Self::X => [1.0, 0.0, 0.0],
            Self::Y => [0.0, 1.0, 0.0],
            Self::Z => [0.0, 0.0, 1.0],
            Self::Custom => [0.0, 1.0, 0.0],
        }
    }

    pub fn color(self) -> [f32; 4] {
        match self {
            Self::X => [0.9, 0.2, 0.2, 0.3],
            Self::Y => [0.2, 0.9, 0.2, 0.3],
            Self::Z => [0.2, 0.2, 0.9, 0.3],
            Self::Custom => [0.7, 0.5, 0.9, 0.3],
        }
    }
}

/// The section view control.
pub struct SectionView {
    /// Whether section view is active.
    pub active: bool,
    /// Current section plane axis.
    pub plane: SectionPlane,
    /// Offset along the plane normal (world units).
    pub offset: f32,
    /// Minimum offset.
    pub min_offset: f32,
    /// Maximum offset.
    pub max_offset: f32,
    /// Whether to show a cap (filled cross-section face).
    pub show_cap: bool,
    /// Whether to flip the clipping direction.
    pub flip: bool,
    /// Cap color.
    pub cap_color: [f32; 4],
    /// Whether the offset slider is being dragged.
    pub dragging: bool,
    /// Control panel width.
    pub panel_width: f32,
}

impl SectionView {
    pub fn new() -> Self {
        Self {
            active: false,
            plane: SectionPlane::Y,
            offset: 0.0,
            min_offset: -50.0,
            max_offset: 50.0,
            show_cap: true,
            flip: false,
            cap_color: [0.8, 0.6, 0.2, 0.8],
            dragging: false,
            panel_width: 200.0,
        }
    }

    /// Toggle section view on/off.
    pub fn toggle(&mut self) {
        self.active = !self.active;
    }

    /// Set the cutting plane.
    pub fn set_plane(&mut self, plane: SectionPlane) {
        self.plane = plane;
    }

    /// Cycle through planes: X -> Y -> Z -> Custom -> X.
    pub fn cycle_plane(&mut self) {
        self.plane = match self.plane {
            SectionPlane::X => SectionPlane::Y,
            SectionPlane::Y => SectionPlane::Z,
            SectionPlane::Z => SectionPlane::Custom,
            SectionPlane::Custom => SectionPlane::X,
        };
    }

    /// Set offset clamped to min/max.
    pub fn set_offset(&mut self, offset: f32) {
        self.offset = offset.clamp(self.min_offset, self.max_offset);
    }

    /// Get the plane equation [nx, ny, nz, d] for shader use.
    pub fn plane_equation(&self) -> [f32; 4] {
        let n = self.plane.normal();
        let sign = if self.flip { -1.0 } else { 1.0 };
        [n[0] * sign, n[1] * sign, n[2] * sign, -self.offset * sign]
    }

    /// Normalized slider position (0..1).
    pub fn slider_t(&self) -> f32 {
        let range = self.max_offset - self.min_offset;
        if range.abs() < 1e-6 { 0.5 } else {
            (self.offset - self.min_offset) / range
        }
    }

    /// Set offset from slider position (0..1).
    pub fn set_from_slider(&mut self, t: f32) {
        let t = t.clamp(0.0, 1.0);
        self.offset = self.min_offset + t * (self.max_offset - self.min_offset);
    }

    /// Hit test the slider area. Returns true if in slider region.
    pub fn hit_test_slider(&self, mx: f32, my: f32, panel_x: f32, panel_y: f32) -> bool {
        if !self.active { return false; }
        let slider_y = panel_y + 80.0;
        let slider_x = panel_x + 12.0;
        let slider_w = self.panel_width - 24.0;
        mx >= slider_x && mx <= slider_x + slider_w
            && my >= slider_y - 8.0 && my <= slider_y + 16.0
    }

    /// Handle slider drag — returns true if consumed.
    pub fn handle_slider_drag(&mut self, mx: f32, panel_x: f32) -> bool {
        if !self.dragging { return false; }
        let slider_x = panel_x + 12.0;
        let slider_w = self.panel_width - 24.0;
        let t = ((mx - slider_x) / slider_w).clamp(0.0, 1.0);
        self.set_from_slider(t);
        true
    }

    /// Draw the section view control panel.
    pub fn draw(
        &self,
        dl: &mut DrawList,
        panel_x: f32,
        panel_y: f32,
        bg_color: [f32; 4],
        text_color: [f32; 4],
        accent_color: [f32; 4],
    ) {
        if !self.active { return; }

        let panel_h = 140.0;

        // Background
        dl.push_quad(panel_x, panel_y, self.panel_width, panel_h, bg_color);

        // Border
        let border = [bg_color[0] + 0.1, bg_color[1] + 0.1, bg_color[2] + 0.1, 0.8];
        dl.push_quad(panel_x, panel_y, self.panel_width, 1.0, border);
        dl.push_quad(panel_x, panel_y + panel_h - 1.0, self.panel_width, 1.0, border);
        dl.push_quad(panel_x, panel_y, 1.0, panel_h, border);
        dl.push_quad(panel_x + self.panel_width - 1.0, panel_y, 1.0, panel_h, border);

        // Title
        emit_text(dl, "Section View", panel_x + 8.0, panel_y + 5.0, 11.0, text_color);

        // Plane label
        let muted = [text_color[0] * 0.6, text_color[1] * 0.6, text_color[2] * 0.6, text_color[3]];
        emit_text(dl, self.plane.label(), panel_x + 8.0, panel_y + 24.0, 9.0, muted);

        // Plane buttons (X/Y/Z)
        let btn_y = panel_y + 42.0;
        for (i, p) in [SectionPlane::X, SectionPlane::Y, SectionPlane::Z].iter().enumerate() {
            let bx = panel_x + 8.0 + i as f32 * 36.0;
            let is_active = self.plane == *p;
            let btn_bg = if is_active { accent_color } else { [0.3, 0.3, 0.3, 0.5] };
            dl.push_quad(bx, btn_y, 30.0, 20.0, btn_bg);
            let tc = if is_active { [1.0, 1.0, 1.0, 1.0] } else { text_color };
            emit_text(dl, p.short(), bx + 10.0, btn_y + 4.0, 10.0, tc);
        }

        // Flip button
        {
            let fx = panel_x + 8.0 + 3.0 * 36.0 + 8.0;
            let flip_bg = if self.flip { [0.6, 0.3, 0.3, 0.5] } else { [0.3, 0.3, 0.3, 0.5] };
            dl.push_quad(fx, btn_y, 40.0, 20.0, flip_bg);
            emit_text(dl, "Flip", fx + 8.0, btn_y + 4.0, 9.0, text_color);
        }

        // Offset slider
        let slider_y = panel_y + 80.0;
        emit_text(dl, "Offset", panel_x + 8.0, slider_y - 12.0, 8.0, muted);

        let offset_str = format!("{:.1}", self.offset);
        let ow = font::measure_text(&offset_str, 8.0, None);
        emit_text(dl, &offset_str, panel_x + self.panel_width - ow - 8.0, slider_y - 12.0, 8.0, text_color);

        let slider_x = panel_x + 12.0;
        let slider_w = self.panel_width - 24.0;

        // Track
        dl.push_quad(slider_x, slider_y + 3.0, slider_w, 4.0, [0.3, 0.3, 0.3, 0.5]);

        // Fill
        let t = self.slider_t();
        dl.push_quad(slider_x, slider_y + 3.0, slider_w * t, 4.0, accent_color);

        // Thumb
        let thumb_x = slider_x + slider_w * t - 5.0;
        dl.push_quad(thumb_x, slider_y, 10.0, 10.0, accent_color);

        // Cap toggle
        let cap_y = panel_y + 108.0;
        let cap_bg = if self.show_cap { accent_color } else { [0.3, 0.3, 0.3, 0.5] };
        dl.push_quad(panel_x + 8.0, cap_y, 12.0, 12.0, cap_bg);
        emit_text(dl, "Show cap", panel_x + 26.0, cap_y + 1.0, 9.0, text_color);

        // Cap color swatch
        dl.push_quad(panel_x + self.panel_width - 20.0, cap_y, 12.0, 12.0, self.cap_color);
    }
}

impl Default for SectionView {
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
    fn toggle_section() {
        let mut sv = SectionView::new();
        assert!(!sv.active);
        sv.toggle();
        assert!(sv.active);
        sv.toggle();
        assert!(!sv.active);
    }

    #[test]
    fn plane_equation_y() {
        let sv = SectionView::new(); // default Y plane, offset 0
        let eq = sv.plane_equation();
        assert_eq!(eq, [0.0, 1.0, 0.0, 0.0]);
    }

    #[test]
    fn plane_equation_flipped() {
        let mut sv = SectionView::new();
        sv.flip = true;
        sv.offset = 5.0;
        let eq = sv.plane_equation();
        assert_eq!(eq[0], 0.0);
        assert_eq!(eq[1], -1.0);
        assert_eq!(eq[3], 5.0);
    }

    #[test]
    fn slider_roundtrip() {
        let mut sv = SectionView::new();
        sv.set_from_slider(0.75);
        let t = sv.slider_t();
        assert!((t - 0.75).abs() < 0.01);
    }

    #[test]
    fn cycle_planes() {
        let mut sv = SectionView::new();
        sv.set_plane(SectionPlane::X);
        sv.cycle_plane();
        assert_eq!(sv.plane, SectionPlane::Y);
        sv.cycle_plane();
        assert_eq!(sv.plane, SectionPlane::Z);
        sv.cycle_plane();
        assert_eq!(sv.plane, SectionPlane::Custom);
        sv.cycle_plane();
        assert_eq!(sv.plane, SectionPlane::X);
    }
}
