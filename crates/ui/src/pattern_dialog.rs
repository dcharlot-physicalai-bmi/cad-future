//! Pattern dialog — linear, circular, and sketch-driven pattern configuration.
//!
//! Inspired by SolidWorks Linear/Circular Pattern, Fusion 360 Pattern,
//! and CATIA Rectangular/Circular Pattern. Creates arrays of features
//! with configurable spacing, count, and skip instances.

use crate::draw::DrawList;
use crate::font;

/// Pattern type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PatternType {
    Linear,
    Circular,
    SketchDriven,
    CurveDriven,
    FillPattern,
}

impl PatternType {
    pub fn label(self) -> &'static str {
        match self {
            Self::Linear => "Linear",
            Self::Circular => "Circular",
            Self::SketchDriven => "Sketch Driven",
            Self::CurveDriven => "Curve Driven",
            Self::FillPattern => "Fill Pattern",
        }
    }

    pub fn icon(self) -> &'static str {
        match self {
            Self::Linear => "|||",
            Self::Circular => "((()",
            Self::SketchDriven => ".*.",
            Self::CurveDriven => "~|~",
            Self::FillPattern => "###",
        }
    }
}

/// The pattern dialog.
pub struct PatternDialog {
    /// Whether the dialog is visible.
    pub visible: bool,
    /// Pattern type.
    pub pattern_type: PatternType,
    /// Direction 1 count.
    pub count_1: u32,
    /// Direction 1 spacing (mm or degrees).
    pub spacing_1: f64,
    /// Direction 2 enabled (for linear).
    pub dir2_enabled: bool,
    /// Direction 2 count.
    pub count_2: u32,
    /// Direction 2 spacing (mm).
    pub spacing_2: f64,
    /// For circular: full angle (degrees, 360 = full circle).
    pub full_angle: f64,
    /// Equal spacing (auto-compute spacing from angle and count).
    pub equal_spacing: bool,
    /// Instances to skip (indices).
    pub skip_instances: Vec<usize>,
    /// Whether to seed the geometry or just the feature.
    pub geometry_pattern: bool,
    /// Panel width.
    pub width: f32,
    /// Feature name being patterned.
    pub feature_name: String,
}

impl PatternDialog {
    pub fn new() -> Self {
        Self {
            visible: false,
            pattern_type: PatternType::Linear,
            count_1: 4,
            spacing_1: 20.0,
            dir2_enabled: false,
            count_2: 3,
            spacing_2: 20.0,
            full_angle: 360.0,
            equal_spacing: true,
            skip_instances: Vec::new(),
            geometry_pattern: false,
            width: 260.0,
            feature_name: String::new(),
        }
    }

    /// Open the dialog for a specific pattern type.
    pub fn open(&mut self, pattern_type: PatternType) {
        self.visible = true;
        self.pattern_type = pattern_type;
        self.skip_instances.clear();
    }

    /// Close the dialog.
    pub fn close(&mut self) {
        self.visible = false;
    }

    /// Total instance count (including skipped).
    pub fn total_instances(&self) -> u32 {
        match self.pattern_type {
            PatternType::Linear => {
                let d1 = self.count_1;
                let d2 = if self.dir2_enabled { self.count_2 } else { 1 };
                d1 * d2
            }
            PatternType::Circular => self.count_1,
            _ => self.count_1,
        }
    }

    /// Effective instance count (minus skipped).
    pub fn effective_instances(&self) -> u32 {
        let total = self.total_instances();
        total.saturating_sub(self.skip_instances.len() as u32)
    }

    /// Computed spacing for circular equal-spacing.
    pub fn circular_spacing(&self) -> f64 {
        if self.count_1 <= 1 { return 0.0; }
        self.full_angle / self.count_1 as f64
    }

    /// Draw the pattern dialog.
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

        let panel_h = 300.0;
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
        let title = format!("{} Pattern", self.pattern_type.label());
        emit_text(dl, &title, px + 8.0, py + 6.0, 12.0, text_color);

        // Pattern type tabs
        let tab_y = py + 28.0;
        let types = [PatternType::Linear, PatternType::Circular, PatternType::SketchDriven];
        let mut tx = px + 8.0;
        for t in types {
            let is_active = self.pattern_type == t;
            let btn_bg = if is_active { accent_color } else { [0.3, 0.3, 0.3, 0.4] };
            let lbl = t.label();
            let lw = font::measure_text(lbl, 9.0, None);
            dl.push_quad(tx, tab_y, lw + 8.0, 18.0, btn_bg);
            let tc = if is_active { [1.0, 1.0, 1.0, 1.0] } else { text_color };
            emit_text(dl, lbl, tx + 4.0, tab_y + 3.0, 9.0, tc);
            tx += lw + 12.0;
        }

        // Feature being patterned
        if !self.feature_name.is_empty() {
            emit_text(dl, "Feature:", px + 8.0, py + 58.0, 8.0, muted);
            emit_text(dl, &self.feature_name, px + 56.0, py + 58.0, 9.0, text_color);
        }

        let params_y = py + 78.0;

        match self.pattern_type {
            PatternType::Linear => {
                // Direction 1
                emit_text(dl, "Direction 1", px + 8.0, params_y, 9.0, text_color);
                let c1 = format!("Count: {}", self.count_1);
                emit_text(dl, &c1, px + 20.0, params_y + 18.0, 8.0, muted);
                let s1 = format!("Spacing: {:.2} mm", self.spacing_1);
                emit_text(dl, &s1, px + 20.0, params_y + 34.0, 8.0, muted);

                // Direction 2
                let d2_y = params_y + 56.0;
                let d2_bg = if self.dir2_enabled { accent_color } else { [0.3, 0.3, 0.3, 0.5] };
                dl.push_quad(px + 8.0, d2_y, 12.0, 12.0, d2_bg);
                emit_text(dl, "Direction 2", px + 26.0, d2_y + 1.0, 9.0, text_color);

                if self.dir2_enabled {
                    let c2 = format!("Count: {}", self.count_2);
                    emit_text(dl, &c2, px + 20.0, d2_y + 18.0, 8.0, muted);
                    let s2 = format!("Spacing: {:.2} mm", self.spacing_2);
                    emit_text(dl, &s2, px + 20.0, d2_y + 34.0, 8.0, muted);
                }
            }
            PatternType::Circular => {
                emit_text(dl, "Circular Pattern", px + 8.0, params_y, 9.0, text_color);
                let c1 = format!("Count: {}", self.count_1);
                emit_text(dl, &c1, px + 20.0, params_y + 18.0, 8.0, muted);
                let angle = format!("Angle: {:.1}°", self.full_angle);
                emit_text(dl, &angle, px + 20.0, params_y + 34.0, 8.0, muted);

                let eq_bg = if self.equal_spacing { accent_color } else { [0.3, 0.3, 0.3, 0.5] };
                dl.push_quad(px + 20.0, params_y + 52.0, 12.0, 12.0, eq_bg);
                emit_text(dl, "Equal spacing", px + 38.0, params_y + 53.0, 8.0, text_color);

                if self.equal_spacing {
                    let sp = format!("Spacing: {:.1}°", self.circular_spacing());
                    emit_text(dl, &sp, px + 20.0, params_y + 70.0, 8.0, muted);
                }
            }
            _ => {
                emit_text(dl, "Select sketch points for pattern...", px + 8.0, params_y, 8.0, accent_color);
            }
        }

        // Summary
        let summary_y = py + panel_h - 68.0;
        dl.push_quad(px + 8.0, summary_y, self.width - 16.0, 1.0, border);
        let total = format!("Total: {} instances", self.total_instances());
        emit_text(dl, &total, px + 8.0, summary_y + 6.0, 9.0, text_color);
        if !self.skip_instances.is_empty() {
            let skip = format!("({} skipped)", self.skip_instances.len());
            emit_text(dl, &skip, px + 8.0, summary_y + 20.0, 8.0, muted);
        }

        // Geometry pattern toggle
        let gp_bg = if self.geometry_pattern { accent_color } else { [0.3, 0.3, 0.3, 0.5] };
        dl.push_quad(px + 140.0, summary_y + 4.0, 12.0, 12.0, gp_bg);
        emit_text(dl, "Geometry pattern", px + 158.0, summary_y + 5.0, 8.0, text_color);

        // OK / Cancel
        let btn_y = py + panel_h - 34.0;
        dl.push_quad(px + 8.0, btn_y, 60.0, 24.0, accent_color);
        emit_text(dl, "OK", px + 30.0, btn_y + 5.0, 11.0, [1.0, 1.0, 1.0, 1.0]);
        dl.push_quad(px + 76.0, btn_y, 60.0, 24.0, [0.4, 0.4, 0.4, 0.6]);
        emit_text(dl, "Cancel", px + 86.0, btn_y + 5.0, 11.0, text_color);
    }
}

impl Default for PatternDialog {
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
    fn linear_total_instances() {
        let mut pd = PatternDialog::new();
        pd.count_1 = 4;
        pd.dir2_enabled = true;
        pd.count_2 = 3;
        assert_eq!(pd.total_instances(), 12);
    }

    #[test]
    fn circular_spacing() {
        let mut pd = PatternDialog::new();
        pd.pattern_type = PatternType::Circular;
        pd.count_1 = 6;
        pd.full_angle = 360.0;
        assert!((pd.circular_spacing() - 60.0).abs() < 0.01);
    }

    #[test]
    fn skip_instances() {
        let mut pd = PatternDialog::new();
        pd.count_1 = 8;
        pd.skip_instances.push(2);
        pd.skip_instances.push(5);
        assert_eq!(pd.effective_instances(), 6);
    }

    #[test]
    fn pattern_types() {
        let types = [PatternType::Linear, PatternType::Circular, PatternType::SketchDriven,
                    PatternType::CurveDriven, PatternType::FillPattern];
        for t in types {
            assert!(!t.label().is_empty());
            assert!(!t.icon().is_empty());
        }
    }

    #[test]
    fn open_and_close() {
        let mut pd = PatternDialog::new();
        pd.open(PatternType::Circular);
        assert!(pd.visible);
        assert_eq!(pd.pattern_type, PatternType::Circular);
        pd.close();
        assert!(!pd.visible);
    }
}
