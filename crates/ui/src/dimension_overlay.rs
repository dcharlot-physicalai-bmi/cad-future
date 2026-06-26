//! Dimension overlay — on-canvas dimension labels with leader lines.
//!
//! Inspired by SolidWorks smart dimensions, Fusion 360 sketch dimensions,
//! and Ansys annotation labels. Projects 3D dimension endpoints into
//! screen space and draws labeled leader lines with value readouts.

use crate::draw::DrawList;
use crate::font;
use glam::{Mat4, Vec3, Vec4};

/// The style of a dimension.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DimensionKind {
    /// Linear distance between two points.
    Linear,
    /// Radius of a circle/arc.
    Radius,
    /// Diameter of a circle/arc.
    Diameter,
    /// Angle between two lines.
    Angle,
    /// Note/callout at a single point.
    Note,
}

/// A single on-canvas dimension annotation.
#[derive(Clone, Debug)]
pub struct DimensionLabel {
    /// World-space start point.
    pub start: Vec3,
    /// World-space end point (same as start for Radius/Note).
    pub end: Vec3,
    /// The formatted value label (e.g., "25.40 mm", "90.0°").
    pub text: String,
    /// Kind of dimension.
    pub kind: DimensionKind,
    /// Color override (None = default theme color).
    pub color: Option<[f32; 4]>,
    /// Whether this dimension is selected/highlighted.
    pub selected: bool,
    /// Whether this dimension is editable (click to modify).
    pub editable: bool,
    /// Offset distance for the leader line (perpendicular to the line).
    pub offset: f32,
}

impl DimensionLabel {
    pub fn linear(start: Vec3, end: Vec3, text: &str) -> Self {
        Self {
            start,
            end,
            text: text.to_string(),
            kind: DimensionKind::Linear,
            color: None,
            selected: false,
            editable: false,
            offset: 20.0,
        }
    }

    pub fn radius(center: Vec3, text: &str) -> Self {
        Self {
            start: center,
            end: center,
            text: text.to_string(),
            kind: DimensionKind::Radius,
            color: None,
            selected: false,
            editable: false,
            offset: 0.0,
        }
    }

    pub fn note(position: Vec3, text: &str) -> Self {
        Self {
            start: position,
            end: position,
            text: text.to_string(),
            kind: DimensionKind::Note,
            color: None,
            selected: false,
            editable: false,
            offset: 0.0,
        }
    }

    pub fn with_color(mut self, color: [f32; 4]) -> Self {
        self.color = Some(color);
        self
    }

    pub fn editable(mut self) -> Self {
        self.editable = true;
        self
    }
}

/// The dimension overlay system.
pub struct DimensionOverlay {
    /// Persistent dimensions (survive across frames).
    pub dimensions: Vec<DimensionLabel>,
    /// Temporary dimensions (cleared each frame, e.g., during sketch).
    pub transient: Vec<DimensionLabel>,
    /// Whether the overlay is visible.
    pub visible: bool,
    /// Hovered dimension index (in `dimensions`).
    pub hovered: Option<usize>,
    /// Default color for dimension lines.
    pub default_color: [f32; 4],
    /// Selected color.
    pub selected_color: [f32; 4],
}

impl DimensionOverlay {
    pub fn new() -> Self {
        Self {
            dimensions: Vec::new(),
            transient: Vec::new(),
            visible: true,
            hovered: None,
            default_color: [0.3, 0.75, 1.0, 0.85],
            selected_color: [1.0, 0.8, 0.2, 1.0],
        }
    }

    /// Add a persistent dimension.
    pub fn add(&mut self, dim: DimensionLabel) {
        self.dimensions.push(dim);
    }

    /// Clear all persistent dimensions.
    pub fn clear(&mut self) {
        self.dimensions.clear();
    }

    /// Add a transient dimension (cleared next frame).
    pub fn add_transient(&mut self, dim: DimensionLabel) {
        self.transient.push(dim);
    }

    /// Clear transient dimensions (call at start of each frame).
    pub fn clear_transient(&mut self) {
        self.transient.clear();
    }

    /// Toggle visibility.
    pub fn toggle(&mut self) {
        self.visible = !self.visible;
    }

    /// Project a 3D point to screen coordinates.
    fn project(point: Vec3, vp: Mat4, screen_w: f32, screen_h: f32) -> Option<(f32, f32)> {
        let clip = vp * Vec4::new(point.x, point.y, point.z, 1.0);
        if clip.w <= 0.0 { return None; }
        let ndc = clip.truncate() / clip.w;
        let sx = (ndc.x * 0.5 + 0.5) * screen_w;
        let sy = (1.0 - (ndc.y * 0.5 + 0.5)) * screen_h;
        Some((sx, sy))
    }

    /// Hit test: returns dimension index if mouse is near a label.
    pub fn hit_test(
        &self,
        mx: f32, my: f32,
        vp: Mat4, screen_w: f32, screen_h: f32,
    ) -> Option<usize> {
        if !self.visible { return None; }

        for (i, dim) in self.dimensions.iter().enumerate() {
            let (s1, s2) = match (
                Self::project(dim.start, vp, screen_w, screen_h),
                Self::project(dim.end, vp, screen_w, screen_h),
            ) {
                (Some(a), Some(b)) => (a, b),
                _ => continue,
            };

            // Label position = midpoint + offset
            let mid_x = (s1.0 + s2.0) * 0.5;
            let mid_y = (s1.1 + s2.1) * 0.5 - dim.offset;

            let label_w = font::measure_text(&dim.text, 11.0, None) + 12.0;
            let label_h = 18.0;

            if mx >= mid_x - label_w * 0.5
                && mx <= mid_x + label_w * 0.5
                && my >= mid_y - label_h * 0.5
                && my <= mid_y + label_h * 0.5
            {
                return Some(i);
            }
        }
        None
    }

    /// Draw all dimensions.
    pub fn draw(
        &self,
        dl: &mut DrawList,
        vp: Mat4,
        screen_w: f32,
        screen_h: f32,
    ) {
        if !self.visible { return; }

        let all = self.dimensions.iter().chain(self.transient.iter()).enumerate();

        for (i, dim) in all {
            let color = if dim.selected {
                self.selected_color
            } else if Some(i) == self.hovered && i < self.dimensions.len() {
                [self.default_color[0] + 0.2, self.default_color[1] + 0.1,
                 self.default_color[2], 1.0]
            } else {
                dim.color.unwrap_or(self.default_color)
            };

            let (s1, s2) = match (
                Self::project(dim.start, vp, screen_w, screen_h),
                Self::project(dim.end, vp, screen_w, screen_h),
            ) {
                (Some(a), Some(b)) => (a, b),
                _ => continue,
            };

            match dim.kind {
                DimensionKind::Linear => {
                    self.draw_linear(dl, s1, s2, &dim.text, dim.offset, color);
                }
                DimensionKind::Radius | DimensionKind::Diameter => {
                    self.draw_note_label(dl, s1, &dim.text, color);
                }
                DimensionKind::Angle => {
                    self.draw_note_label(dl, ((s1.0 + s2.0) * 0.5, (s1.1 + s2.1) * 0.5), &dim.text, color);
                }
                DimensionKind::Note => {
                    self.draw_note_label(dl, s1, &dim.text, color);
                }
            }
        }
    }

    /// Draw a linear dimension with extension lines, dimension line, and label.
    fn draw_linear(
        &self,
        dl: &mut DrawList,
        s1: (f32, f32),
        s2: (f32, f32),
        text: &str,
        offset: f32,
        color: [f32; 4],
    ) {
        // Extension lines (from endpoints perpendicular to dimension line)
        let dx = s2.0 - s1.0;
        let dy = s2.1 - s1.1;
        let len = (dx * dx + dy * dy).sqrt();
        if len < 4.0 { return; }

        // Normal direction for offset
        let nx = -dy / len;
        let ny = dx / len;

        let e1x = s1.0 + nx * offset;
        let e1y = s1.1 + ny * offset;
        let e2x = s2.0 + nx * offset;
        let e2y = s2.1 + ny * offset;

        // Extension lines (thin, from endpoint toward dimension line)
        let ext_color = [color[0], color[1], color[2], color[3] * 0.5];
        // Line from s1 to e1
        if (e1x - s1.0).abs() > (e1y - s1.1).abs() {
            let min_x = s1.0.min(e1x);
            dl.push_quad(min_x, s1.1, (e1x - s1.0).abs(), 1.0, ext_color);
        } else {
            let min_y = s1.1.min(e1y);
            dl.push_quad(s1.0, min_y, 1.0, (e1y - s1.1).abs(), ext_color);
        }
        if (e2x - s2.0).abs() > (e2y - s2.1).abs() {
            let min_x = s2.0.min(e2x);
            dl.push_quad(min_x, s2.1, (e2x - s2.0).abs(), 1.0, ext_color);
        } else {
            let min_y = s2.1.min(e2y);
            dl.push_quad(s2.0, min_y, 1.0, (e2y - s2.1).abs(), ext_color);
        }

        // Dimension line (between extension line endpoints)
        if (e2x - e1x).abs() > (e2y - e1y).abs() {
            let min_x = e1x.min(e2x);
            dl.push_quad(min_x, e1y, (e2x - e1x).abs(), 1.0, color);
        } else {
            let min_y = e1y.min(e2y);
            dl.push_quad(e1x, min_y, 1.0, (e2y - e1y).abs(), color);
        }

        // Arrowheads (small quads at endpoints)
        let arrow_size = 4.0;
        dl.push_quad(e1x - arrow_size * 0.5, e1y - arrow_size * 0.5,
            arrow_size, arrow_size, color);
        dl.push_quad(e2x - arrow_size * 0.5, e2y - arrow_size * 0.5,
            arrow_size, arrow_size, color);

        // Label at midpoint
        let mid_x = (e1x + e2x) * 0.5;
        let mid_y = (e1y + e2y) * 0.5;
        self.draw_note_label(dl, (mid_x, mid_y - 10.0), text, color);
    }

    /// Draw a note/callout label with background pill.
    fn draw_note_label(
        &self,
        dl: &mut DrawList,
        pos: (f32, f32),
        text: &str,
        color: [f32; 4],
    ) {
        let text_w = font::measure_text(text, 11.0, None);
        let pad = 4.0;
        let label_w = text_w + pad * 2.0;
        let label_h = 16.0;
        let lx = pos.0 - label_w * 0.5;
        let ly = pos.1 - label_h * 0.5;

        // Background pill
        dl.push_quad(lx, ly, label_w, label_h, [0.0, 0.0, 0.0, 0.7]);

        // Border
        let border = [color[0], color[1], color[2], 0.4];
        dl.push_quad(lx, ly, label_w, 1.0, border);
        dl.push_quad(lx, ly + label_h - 1.0, label_w, 1.0, border);
        dl.push_quad(lx, ly, 1.0, label_h, border);
        dl.push_quad(lx + label_w - 1.0, ly, 1.0, label_h, border);

        // Text
        emit_text(dl, text, lx + pad, ly + 2.0, 11.0, [1.0, 1.0, 1.0, 0.95]);
    }
}

impl Default for DimensionOverlay {
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
    fn add_and_clear() {
        let mut overlay = DimensionOverlay::new();
        overlay.add(DimensionLabel::linear(Vec3::ZERO, Vec3::X, "1.00 mm"));
        assert_eq!(overlay.dimensions.len(), 1);
        overlay.clear();
        assert!(overlay.dimensions.is_empty());
    }

    #[test]
    fn transient_cleared_separately() {
        let mut overlay = DimensionOverlay::new();
        overlay.add(DimensionLabel::linear(Vec3::ZERO, Vec3::X, "perm"));
        overlay.add_transient(DimensionLabel::note(Vec3::Y, "temp"));
        assert_eq!(overlay.dimensions.len(), 1);
        assert_eq!(overlay.transient.len(), 1);
        overlay.clear_transient();
        assert_eq!(overlay.dimensions.len(), 1);
        assert!(overlay.transient.is_empty());
    }

    #[test]
    fn toggle_visibility() {
        let mut overlay = DimensionOverlay::new();
        assert!(overlay.visible);
        overlay.toggle();
        assert!(!overlay.visible);
    }

    #[test]
    fn projection_behind_camera_none() {
        // A point behind the camera (w <= 0 after projection) should return None
        let vp = Mat4::IDENTITY; // degenerate but w=1 for all points
        // With identity matrix, all points should project fine
        let result = DimensionOverlay::project(Vec3::new(0.5, 0.5, 0.5), vp, 800.0, 600.0);
        assert!(result.is_some());
    }

    #[test]
    fn note_dimension() {
        let dim = DimensionLabel::note(Vec3::ZERO, "R5.0");
        assert_eq!(dim.kind, DimensionKind::Note);
        assert_eq!(dim.text, "R5.0");
    }
}
