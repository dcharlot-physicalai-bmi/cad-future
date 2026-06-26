//! Measure tool — point-to-point, edge, angle, and minimum distance measurement.
//!
//! Inspired by SolidWorks Measure Tool, Fusion 360 Inspect > Measure,
//! and CATIA Measure Between. Provides interactive measurement with
//! persistent results, unit conversion, and on-canvas readouts.

use crate::draw::DrawList;
use crate::font;

/// Type of measurement.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MeasureKind {
    /// Point-to-point distance.
    PointToPoint,
    /// Edge length.
    EdgeLength,
    /// Angle between two edges/faces.
    Angle,
    /// Minimum distance between two bodies/faces.
    MinDistance,
    /// Radius of a circular edge/face.
    Radius,
    /// Diameter of a circular edge/face.
    Diameter,
    /// Area of a face.
    Area,
}

impl MeasureKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::PointToPoint => "Point-to-Point",
            Self::EdgeLength => "Edge Length",
            Self::Angle => "Angle",
            Self::MinDistance => "Min Distance",
            Self::Radius => "Radius",
            Self::Diameter => "Diameter",
            Self::Area => "Area",
        }
    }

    pub fn icon(self) -> &'static str {
        match self {
            Self::PointToPoint => "|-|",
            Self::EdgeLength => "---",
            Self::Angle => "<",
            Self::MinDistance => "...",
            Self::Radius => "R",
            Self::Diameter => "D",
            Self::Area => "[]",
        }
    }

    pub fn unit_suffix(self) -> &'static str {
        match self {
            Self::Angle => "deg",
            Self::Area => "mm2",
            _ => "mm",
        }
    }
}

/// A single measurement result.
#[derive(Clone, Debug)]
pub struct Measurement {
    /// Type of measurement.
    pub kind: MeasureKind,
    /// Primary value (distance in mm, angle in degrees, area in mm^2).
    pub value: f64,
    /// Optional delta components [dx, dy, dz] for point-to-point.
    pub delta: Option<[f64; 3]>,
    /// Screen position for the readout [x, y].
    pub screen_pos: [f32; 2],
    /// Start point in world space.
    pub world_start: [f64; 3],
    /// End point in world space.
    pub world_end: [f64; 3],
    /// Label override (empty = auto).
    pub label: String,
    /// Whether this measurement is persistent (pinned).
    pub pinned: bool,
}

impl Measurement {
    pub fn distance(start: [f64; 3], end: [f64; 3]) -> Self {
        let dx = end[0] - start[0];
        let dy = end[1] - start[1];
        let dz = end[2] - start[2];
        let dist = (dx * dx + dy * dy + dz * dz).sqrt();
        Self {
            kind: MeasureKind::PointToPoint,
            value: dist,
            delta: Some([dx, dy, dz]),
            screen_pos: [0.0, 0.0],
            world_start: start,
            world_end: end,
            label: String::new(),
            pinned: false,
        }
    }

    pub fn angle(degrees: f64) -> Self {
        Self {
            kind: MeasureKind::Angle,
            value: degrees,
            delta: None,
            screen_pos: [0.0, 0.0],
            world_start: [0.0; 3],
            world_end: [0.0; 3],
            label: String::new(),
            pinned: false,
        }
    }

    pub fn radius(center: [f64; 3], r: f64) -> Self {
        Self {
            kind: MeasureKind::Radius,
            value: r,
            delta: None,
            screen_pos: [0.0, 0.0],
            world_start: center,
            world_end: center,
            label: String::new(),
            pinned: false,
        }
    }

    /// Format the value with appropriate precision and units.
    pub fn format_value(&self) -> String {
        match self.kind {
            MeasureKind::Angle => format!("{:.2}°", self.value),
            MeasureKind::Area => {
                if self.value >= 1_000_000.0 {
                    format!("{:.2} m²", self.value / 1_000_000.0)
                } else if self.value >= 100.0 {
                    format!("{:.2} cm²", self.value / 100.0)
                } else {
                    format!("{:.3} mm²", self.value)
                }
            }
            _ => {
                if self.value >= 1000.0 {
                    format!("{:.3} m", self.value / 1000.0)
                } else if self.value >= 10.0 {
                    format!("{:.2} mm", self.value)
                } else {
                    format!("{:.3} mm", self.value)
                }
            }
        }
    }
}

/// The measure tool state.
pub struct MeasureTool {
    /// Whether the tool is active.
    pub active: bool,
    /// Current measurement mode.
    pub mode: MeasureKind,
    /// All measurements (current session).
    pub measurements: Vec<Measurement>,
    /// Active (in-progress) measurement index.
    pub current: Option<usize>,
    /// Hovered measurement index.
    pub hovered: Option<usize>,
    /// Whether the results panel is expanded.
    pub panel_expanded: bool,
    /// Panel width.
    pub panel_width: f32,
    /// Scroll offset in results panel.
    pub scroll_offset: usize,
}

impl MeasureTool {
    pub fn new() -> Self {
        Self {
            active: false,
            mode: MeasureKind::PointToPoint,
            measurements: Vec::new(),
            current: None,
            hovered: None,
            panel_expanded: true,
            panel_width: 240.0,
            scroll_offset: 0,
        }
    }

    /// Toggle the measure tool on/off.
    pub fn toggle(&mut self) {
        self.active = !self.active;
        if !self.active {
            self.current = None;
        }
    }

    /// Set measurement mode.
    pub fn set_mode(&mut self, mode: MeasureKind) {
        self.mode = mode;
        self.current = None;
    }

    /// Add a completed measurement.
    pub fn add(&mut self, measurement: Measurement) -> usize {
        let idx = self.measurements.len();
        self.measurements.push(measurement);
        idx
    }

    /// Pin/unpin a measurement.
    pub fn toggle_pin(&mut self, idx: usize) {
        if let Some(m) = self.measurements.get_mut(idx) {
            m.pinned = !m.pinned;
        }
    }

    /// Clear all non-pinned measurements.
    pub fn clear_unpinned(&mut self) {
        self.measurements.retain(|m| m.pinned);
        self.current = None;
    }

    /// Clear all measurements.
    pub fn clear_all(&mut self) {
        self.measurements.clear();
        self.current = None;
    }

    /// Get pinned measurement count.
    pub fn pinned_count(&self) -> usize {
        self.measurements.iter().filter(|m| m.pinned).count()
    }

    /// Draw on-canvas measurement readouts.
    pub fn draw_readouts(
        &self,
        dl: &mut DrawList,
        text_color: [f32; 4],
        accent_color: [f32; 4],
    ) {
        if !self.active && self.pinned_count() == 0 { return; }

        for (i, m) in self.measurements.iter().enumerate() {
            if !self.active && !m.pinned { continue; }

            let [sx, sy] = m.screen_pos;
            if sx == 0.0 && sy == 0.0 { continue; }

            let is_hovered = self.hovered == Some(i);
            let text = m.format_value();
            let tw = font::measure_text(&text, 11.0, None);

            // Background
            let bg = if m.pinned {
                [0.0, 0.0, 0.0, 0.75]
            } else {
                [0.0, 0.0, 0.0, 0.6]
            };
            dl.push_quad(sx - 4.0, sy - 2.0, tw + 8.0, 16.0, bg);

            // Pin indicator
            if m.pinned {
                dl.push_quad(sx - 4.0, sy - 2.0, 2.0, 16.0, accent_color);
            }

            let color = if is_hovered { accent_color } else { text_color };
            emit_text(dl, &text, sx, sy, 11.0, color);

            // Delta readout for point-to-point
            if let Some([dx, dy, dz]) = m.delta {
                if is_hovered {
                    let delta_str = format!("dx:{:.2} dy:{:.2} dz:{:.2}", dx, dy, dz);
                    let dw = font::measure_text(&delta_str, 8.0, None);
                    dl.push_quad(sx - 4.0, sy + 14.0, dw + 8.0, 12.0, [0.0, 0.0, 0.0, 0.6]);
                    emit_text(dl, &delta_str, sx, sy + 15.0, 8.0,
                        [text_color[0] * 0.7, text_color[1] * 0.7, text_color[2] * 0.7, text_color[3]]);
                }
            }
        }
    }

    /// Draw the results panel.
    pub fn draw_panel(
        &self,
        dl: &mut DrawList,
        panel_x: f32,
        panel_y: f32,
        bg_color: [f32; 4],
        text_color: [f32; 4],
        accent_color: [f32; 4],
    ) {
        if !self.active || !self.panel_expanded { return; }

        let row_h = 28.0;
        let header_h = 48.0;
        let rows = self.measurements.len().min(10);
        let panel_h = header_h + rows as f32 * row_h + 8.0;

        // Background
        dl.push_quad(panel_x, panel_y, self.panel_width, panel_h, bg_color);

        // Border
        let border = [bg_color[0] + 0.1, bg_color[1] + 0.1, bg_color[2] + 0.1, 0.8];
        dl.push_quad(panel_x, panel_y, self.panel_width, 1.0, border);

        // Title
        emit_text(dl, "Measure", panel_x + 8.0, panel_y + 5.0, 11.0, text_color);

        // Mode label
        let muted = [text_color[0] * 0.5, text_color[1] * 0.5, text_color[2] * 0.5, text_color[3]];
        emit_text(dl, self.mode.label(), panel_x + 8.0, panel_y + 22.0, 9.0, muted);

        // Count
        let count = format!("{}", self.measurements.len());
        let cw = font::measure_text(&count, 9.0, None);
        emit_text(dl, &count, panel_x + self.panel_width - cw - 8.0, panel_y + 7.0, 9.0, muted);

        // Mode buttons
        let btn_y = panel_y + 34.0;
        let modes = [MeasureKind::PointToPoint, MeasureKind::EdgeLength, MeasureKind::Angle,
                     MeasureKind::Radius, MeasureKind::MinDistance];
        let mut bx = panel_x + 8.0;
        for mode in modes {
            let is_active = self.mode == mode;
            let btn_bg = if is_active { accent_color } else { [0.3, 0.3, 0.3, 0.4] };
            let icon = mode.icon();
            let iw = font::measure_text(icon, 9.0, None);
            dl.push_quad(bx, btn_y, iw + 8.0, 14.0, btn_bg);
            let tc = if is_active { [1.0, 1.0, 1.0, 1.0] } else { text_color };
            emit_text(dl, icon, bx + 4.0, btn_y + 2.0, 9.0, tc);
            bx += iw + 12.0;
        }

        // Results rows
        let start_y = panel_y + header_h;
        let end = (self.scroll_offset + rows).min(self.measurements.len());
        for i in self.scroll_offset..end {
            let m = &self.measurements[i];
            let vis_i = (i - self.scroll_offset) as f32;
            let ry = start_y + vis_i * row_h;

            let is_hov = self.hovered == Some(i);
            if is_hov {
                dl.push_quad(panel_x, ry, self.panel_width, row_h, [1.0, 1.0, 1.0, 0.05]);
            }

            // Kind icon
            emit_text(dl, m.kind.icon(), panel_x + 8.0, ry + 4.0, 9.0, muted);

            // Value
            let val = m.format_value();
            let color = if is_hov { accent_color } else { text_color };
            emit_text(dl, &val, panel_x + 40.0, ry + 4.0, 10.0, color);

            // Pin indicator
            if m.pinned {
                emit_text(dl, "*", panel_x + self.panel_width - 16.0, ry + 3.0, 10.0, accent_color);
            }

            // Label (if any)
            if !m.label.is_empty() {
                emit_text(dl, &m.label, panel_x + 40.0, ry + 16.0, 7.0, muted);
            }
        }
    }
}

impl Default for MeasureTool {
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
    fn distance_calculation() {
        let m = Measurement::distance([0.0, 0.0, 0.0], [3.0, 4.0, 0.0]);
        assert!((m.value - 5.0).abs() < 1e-9);
        assert_eq!(m.kind, MeasureKind::PointToPoint);
    }

    #[test]
    fn format_value_mm() {
        let m = Measurement::distance([0.0, 0.0, 0.0], [12.5, 0.0, 0.0]);
        assert!(m.format_value().contains("12.50"));
        assert!(m.format_value().contains("mm"));
    }

    #[test]
    fn format_angle() {
        let m = Measurement::angle(45.0);
        assert!(m.format_value().contains("45.00"));
        assert!(m.format_value().contains("°"));
    }

    #[test]
    fn pin_and_clear_unpinned() {
        let mut mt = MeasureTool::new();
        mt.add(Measurement::distance([0.0; 3], [1.0, 0.0, 0.0]));
        mt.add(Measurement::distance([0.0; 3], [2.0, 0.0, 0.0]));
        mt.toggle_pin(0);
        assert_eq!(mt.pinned_count(), 1);
        mt.clear_unpinned();
        assert_eq!(mt.measurements.len(), 1);
        assert!(mt.measurements[0].pinned);
    }

    #[test]
    fn mode_labels() {
        let modes = [
            MeasureKind::PointToPoint, MeasureKind::EdgeLength, MeasureKind::Angle,
            MeasureKind::MinDistance, MeasureKind::Radius, MeasureKind::Diameter,
            MeasureKind::Area,
        ];
        for m in modes {
            assert!(!m.label().is_empty());
            assert!(!m.icon().is_empty());
        }
    }
}
