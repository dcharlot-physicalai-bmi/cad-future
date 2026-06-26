//! Hole wizard — standardized hole creation dialog.
//!
//! Inspired by SolidWorks Hole Wizard, Fusion 360 Hole command,
//! and CATIA Hole Feature. Supports counterbore, countersink,
//! through-hole, blind, and tapped holes with standard sizes.

use crate::draw::DrawList;
use crate::font;

/// Hole type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HoleType {
    Simple,
    Counterbore,
    Countersink,
    SlotHole,
}

impl HoleType {
    pub fn label(self) -> &'static str {
        match self {
            Self::Simple => "Simple",
            Self::Counterbore => "Counterbore",
            Self::Countersink => "Countersink",
            Self::SlotHole => "Slot",
        }
    }
}

/// Hole end condition.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HoleEnd {
    Blind,
    ThroughAll,
    UpToNext,
    UpToSurface,
}

impl HoleEnd {
    pub fn label(self) -> &'static str {
        match self {
            Self::Blind => "Blind",
            Self::ThroughAll => "Through All",
            Self::UpToNext => "Up to Next",
            Self::UpToSurface => "Up to Surface",
        }
    }
}

/// Thread standard.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThreadStandard {
    None,
    MetricCoarse,
    MetricFine,
    UNCCoarse,
    UNFFine,
}

impl ThreadStandard {
    pub fn label(self) -> &'static str {
        match self {
            Self::None => "None",
            Self::MetricCoarse => "Metric Coarse",
            Self::MetricFine => "Metric Fine",
            Self::UNCCoarse => "UNC",
            Self::UNFFine => "UNF",
        }
    }
}

/// Standard metric thread sizes.
pub struct MetricSize {
    pub label: &'static str,
    pub diameter: f64,
    pub pitch: f64,
    pub tap_drill: f64,
}

impl MetricSize {
    pub fn standard_sizes() -> &'static [MetricSize] {
        &[
            MetricSize { label: "M2", diameter: 2.0, pitch: 0.4, tap_drill: 1.6 },
            MetricSize { label: "M2.5", diameter: 2.5, pitch: 0.45, tap_drill: 2.05 },
            MetricSize { label: "M3", diameter: 3.0, pitch: 0.5, tap_drill: 2.5 },
            MetricSize { label: "M4", diameter: 4.0, pitch: 0.7, tap_drill: 3.3 },
            MetricSize { label: "M5", diameter: 5.0, pitch: 0.8, tap_drill: 4.2 },
            MetricSize { label: "M6", diameter: 6.0, pitch: 1.0, tap_drill: 5.0 },
            MetricSize { label: "M8", diameter: 8.0, pitch: 1.25, tap_drill: 6.75 },
            MetricSize { label: "M10", diameter: 10.0, pitch: 1.5, tap_drill: 8.5 },
            MetricSize { label: "M12", diameter: 12.0, pitch: 1.75, tap_drill: 10.25 },
            MetricSize { label: "M16", diameter: 16.0, pitch: 2.0, tap_drill: 14.0 },
            MetricSize { label: "M20", diameter: 20.0, pitch: 2.5, tap_drill: 17.5 },
            MetricSize { label: "M24", diameter: 24.0, pitch: 3.0, tap_drill: 21.0 },
        ]
    }
}

/// The hole wizard dialog.
pub struct HoleWizard {
    /// Whether the dialog is visible.
    pub visible: bool,
    /// Hole type.
    pub hole_type: HoleType,
    /// End condition.
    pub end_condition: HoleEnd,
    /// Thread standard.
    pub thread: ThreadStandard,
    /// Selected size index.
    pub size_index: usize,
    /// Hole diameter (mm).
    pub diameter: f64,
    /// Hole depth (mm, for blind holes).
    pub depth: f64,
    /// Counterbore diameter.
    pub cb_diameter: f64,
    /// Counterbore depth.
    pub cb_depth: f64,
    /// Countersink angle (degrees).
    pub cs_angle: f64,
    /// Countersink diameter.
    pub cs_diameter: f64,
    /// Whether to add cosmetic thread.
    pub cosmetic_thread: bool,
    /// Panel width.
    pub width: f32,
    /// Hovered size row.
    pub hovered_size: Option<usize>,
}

impl HoleWizard {
    pub fn new() -> Self {
        Self {
            visible: false,
            hole_type: HoleType::Simple,
            end_condition: HoleEnd::ThroughAll,
            thread: ThreadStandard::None,
            size_index: 5, // M6
            diameter: 6.0,
            depth: 20.0,
            cb_diameter: 11.0,
            cb_depth: 6.0,
            cs_angle: 82.0,
            cs_diameter: 12.0,
            cosmetic_thread: false,
            width: 280.0,
            hovered_size: None,
        }
    }

    pub fn toggle(&mut self) {
        self.visible = !self.visible;
    }

    /// Select a standard size and auto-populate dimensions.
    pub fn select_size(&mut self, idx: usize) {
        let sizes = MetricSize::standard_sizes();
        if let Some(size) = sizes.get(idx) {
            self.size_index = idx;
            self.diameter = size.diameter;
            if self.thread != ThreadStandard::None {
                self.diameter = size.tap_drill;
            }
            // Auto-populate CB/CS based on size
            self.cb_diameter = size.diameter * 1.8;
            self.cb_depth = size.diameter;
            self.cs_diameter = size.diameter * 2.0;
        }
    }

    /// Get current size label.
    pub fn size_label(&self) -> &'static str {
        MetricSize::standard_sizes().get(self.size_index)
            .map(|s| s.label)
            .unwrap_or("Custom")
    }

    /// Draw the hole wizard dialog.
    pub fn draw(
        &self,
        dl: &mut DrawList,
        screen_w: f32,
        screen_h: f32,
        bg_color: [f32; 4],
        text_color: [f32; 4],
        accent_color: [f32; 4],
    ) {
        if !self.visible { return; }

        let panel_h = 380.0;
        let px = (screen_w - self.width) * 0.5;
        let py = (screen_h - panel_h) * 0.5;

        // Shadow + background
        dl.push_quad(px + 3.0, py + 3.0, self.width, panel_h, [0.0, 0.0, 0.0, 0.25]);
        dl.push_quad(px, py, self.width, panel_h, bg_color);

        let border = [bg_color[0] + 0.1, bg_color[1] + 0.1, bg_color[2] + 0.1, 0.8];
        dl.push_quad(px, py, self.width, 1.0, border);
        dl.push_quad(px, py + panel_h - 1.0, self.width, 1.0, border);
        dl.push_quad(px, py, 1.0, panel_h, border);
        dl.push_quad(px + self.width - 1.0, py, 1.0, panel_h, border);

        let muted = [text_color[0] * 0.5, text_color[1] * 0.5, text_color[2] * 0.5, text_color[3]];

        // Title
        emit_text(dl, "Hole Wizard", px + 8.0, py + 6.0, 12.0, text_color);

        // Hole type tabs
        let tab_y = py + 28.0;
        let types = [HoleType::Simple, HoleType::Counterbore, HoleType::Countersink, HoleType::SlotHole];
        let mut tx = px + 8.0;
        for t in types {
            let is_active = self.hole_type == t;
            let btn_bg = if is_active { accent_color } else { [0.3, 0.3, 0.3, 0.4] };
            let lbl = t.label();
            let lw = font::measure_text(lbl, 9.0, None);
            dl.push_quad(tx, tab_y, lw + 8.0, 18.0, btn_bg);
            let tc = if is_active { [1.0, 1.0, 1.0, 1.0] } else { text_color };
            emit_text(dl, lbl, tx + 4.0, tab_y + 3.0, 9.0, tc);
            tx += lw + 12.0;
        }

        // Thread standard
        emit_text(dl, "Thread:", px + 8.0, py + 58.0, 8.0, muted);
        emit_text(dl, self.thread.label(), px + 52.0, py + 58.0, 9.0, text_color);

        // Size label
        emit_text(dl, "Size:", px + 8.0, py + 78.0, 8.0, muted);
        emit_text(dl, self.size_label(), px + 52.0, py + 78.0, 10.0, accent_color);

        // Diameter
        emit_text(dl, "Diameter:", px + 8.0, py + 100.0, 8.0, muted);
        let dia_str = format!("{:.2} mm", self.diameter);
        emit_text(dl, &dia_str, px + 80.0, py + 100.0, 9.0, text_color);

        // End condition
        emit_text(dl, "End:", px + 8.0, py + 120.0, 8.0, muted);
        emit_text(dl, self.end_condition.label(), px + 80.0, py + 120.0, 9.0, text_color);

        // Depth (if blind)
        if self.end_condition == HoleEnd::Blind {
            emit_text(dl, "Depth:", px + 8.0, py + 140.0, 8.0, muted);
            let dep_str = format!("{:.2} mm", self.depth);
            emit_text(dl, &dep_str, px + 80.0, py + 140.0, 9.0, text_color);
        }

        // CB/CS parameters
        match self.hole_type {
            HoleType::Counterbore => {
                let cb_y = py + 168.0;
                dl.push_quad(px + 8.0, cb_y, self.width - 16.0, 1.0, border);
                emit_text(dl, "Counterbore", px + 8.0, cb_y + 6.0, 9.0, text_color);
                let cbd_str = format!("D = {:.2} mm", self.cb_diameter);
                emit_text(dl, &cbd_str, px + 20.0, cb_y + 22.0, 8.0, muted);
                let cbdp_str = format!("Depth = {:.2} mm", self.cb_depth);
                emit_text(dl, &cbdp_str, px + 20.0, cb_y + 38.0, 8.0, muted);
            }
            HoleType::Countersink => {
                let cs_y = py + 168.0;
                dl.push_quad(px + 8.0, cs_y, self.width - 16.0, 1.0, border);
                emit_text(dl, "Countersink", px + 8.0, cs_y + 6.0, 9.0, text_color);
                let csd_str = format!("D = {:.2} mm", self.cs_diameter);
                emit_text(dl, &csd_str, px + 20.0, cs_y + 22.0, 8.0, muted);
                let csa_str = format!("Angle = {:.0}°", self.cs_angle);
                emit_text(dl, &csa_str, px + 20.0, cs_y + 38.0, 8.0, muted);
            }
            _ => {}
        }

        // Size table (bottom portion)
        let table_y = py + 230.0;
        emit_text(dl, "Standard Sizes", px + 8.0, table_y, 9.0, text_color);

        let sizes = MetricSize::standard_sizes();
        let row_h = 16.0;
        for (i, size) in sizes.iter().enumerate().take(8) {
            let ry = table_y + 16.0 + i as f32 * row_h;
            let is_sel = self.size_index == i;
            let is_hov = self.hovered_size == Some(i);

            if is_sel {
                dl.push_quad(px + 8.0, ry, self.width - 16.0, row_h, [accent_color[0] * 0.2, accent_color[1] * 0.2, accent_color[2] * 0.2, 0.4]);
            } else if is_hov {
                dl.push_quad(px + 8.0, ry, self.width - 16.0, row_h, [1.0, 1.0, 1.0, 0.04]);
            }

            let tc = if is_sel { accent_color } else { text_color };
            emit_text(dl, size.label, px + 12.0, ry + 2.0, 9.0, tc);
            let dim_str = format!("{:.1} x {:.1}", size.diameter, size.pitch);
            emit_text(dl, &dim_str, px + 60.0, ry + 2.0, 8.0, muted);
        }

        // OK / Cancel
        let btn_y = py + panel_h - 34.0;
        dl.push_quad(px + 8.0, btn_y, 60.0, 24.0, accent_color);
        emit_text(dl, "OK", px + 30.0, btn_y + 5.0, 11.0, [1.0, 1.0, 1.0, 1.0]);
        dl.push_quad(px + 76.0, btn_y, 60.0, 24.0, [0.4, 0.4, 0.4, 0.6]);
        emit_text(dl, "Cancel", px + 86.0, btn_y + 5.0, 11.0, text_color);
    }
}

impl Default for HoleWizard {
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
    fn select_m6() {
        let mut hw = HoleWizard::new();
        hw.select_size(5); // M6
        assert_eq!(hw.size_label(), "M6");
        assert!((hw.diameter - 6.0).abs() < 0.01);
    }

    #[test]
    fn standard_sizes_populated() {
        let sizes = MetricSize::standard_sizes();
        assert!(sizes.len() >= 10);
        assert_eq!(sizes[0].label, "M2");
    }

    #[test]
    fn hole_types() {
        let types = [HoleType::Simple, HoleType::Counterbore, HoleType::Countersink, HoleType::SlotHole];
        for t in types {
            assert!(!t.label().is_empty());
        }
    }

    #[test]
    fn thread_standards() {
        let stds = [ThreadStandard::None, ThreadStandard::MetricCoarse, ThreadStandard::MetricFine,
                    ThreadStandard::UNCCoarse, ThreadStandard::UNFFine];
        for s in stds {
            assert!(!s.label().is_empty());
        }
    }

    #[test]
    fn tap_drill_size() {
        let mut hw = HoleWizard::new();
        hw.thread = ThreadStandard::MetricCoarse;
        hw.select_size(5); // M6
        // With threading, diameter should be tap drill = 5.0
        assert!((hw.diameter - 5.0).abs() < 0.01);
    }
}
