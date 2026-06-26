//! Sheet metal panel — bend table, K-factor, flat pattern controls.
//!
//! Inspired by SolidWorks Sheet Metal, Fusion 360 Sheet Metal,
//! and Onshape Sheet Metal. Provides bend table management,
//! K-factor/bend allowance controls, and flat pattern toggle.

use crate::draw::DrawList;
use crate::font;

/// Bend relief type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BendRelief {
    Rectangular,
    Obround,
    Tear,
    None,
}

impl BendRelief {
    pub fn label(self) -> &'static str {
        match self {
            Self::Rectangular => "Rectangular",
            Self::Obround => "Obround",
            Self::Tear => "Tear",
            Self::None => "None",
        }
    }
}

/// A bend in the sheet metal part.
#[derive(Clone, Debug)]
pub struct BendEntry {
    /// Bend ID/name.
    pub name: String,
    /// Bend angle (degrees).
    pub angle: f64,
    /// Bend radius (mm).
    pub radius: f64,
    /// Bend direction (up/down).
    pub direction_up: bool,
    /// K-factor for this bend (overrides default if set).
    pub k_factor: Option<f64>,
    /// Bend status.
    pub status: BendStatus,
}

/// Status of a bend.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BendStatus {
    Ok,
    Warning,
    Error,
}

impl BendEntry {
    pub fn new(name: &str, angle: f64, radius: f64) -> Self {
        Self {
            name: name.to_string(),
            angle,
            radius,
            direction_up: true,
            k_factor: None,
            status: BendStatus::Ok,
        }
    }

    /// Compute bend allowance for this bend.
    pub fn bend_allowance(&self, thickness: f64, default_k: f64) -> f64 {
        let k = self.k_factor.unwrap_or(default_k);
        let angle_rad = self.angle.to_radians();
        angle_rad * (self.radius + k * thickness)
    }
}

/// The sheet metal panel.
pub struct SheetMetal {
    /// Whether sheet metal mode is active.
    pub active: bool,
    /// Material thickness (mm).
    pub thickness: f64,
    /// Default K-factor.
    pub k_factor: f64,
    /// Default bend radius (mm).
    pub default_radius: f64,
    /// Bend relief type.
    pub bend_relief: BendRelief,
    /// Bend relief ratio (relative to thickness).
    pub relief_ratio: f64,
    /// Whether flat pattern is shown.
    pub flat_pattern: bool,
    /// Bend table entries.
    pub bends: Vec<BendEntry>,
    /// Selected bend index.
    pub selected_bend: Option<usize>,
    /// Hovered bend index.
    pub hovered_bend: Option<usize>,
    /// Panel width.
    pub width: f32,
    /// Panel visible.
    pub panel_visible: bool,
}

impl SheetMetal {
    pub fn new() -> Self {
        Self {
            active: false,
            thickness: 1.5,
            k_factor: 0.44,
            default_radius: 1.5,
            bend_relief: BendRelief::Rectangular,
            relief_ratio: 0.5,
            flat_pattern: false,
            bends: Vec::new(),
            selected_bend: None,
            hovered_bend: None,
            width: 260.0,
            panel_visible: true,
        }
    }

    /// Toggle sheet metal mode.
    pub fn toggle(&mut self) {
        self.active = !self.active;
    }

    /// Toggle flat pattern view.
    pub fn toggle_flat_pattern(&mut self) {
        self.flat_pattern = !self.flat_pattern;
    }

    /// Add a bend.
    pub fn add_bend(&mut self, bend: BendEntry) {
        self.bends.push(bend);
    }

    /// Total flat length (sum of bend allowances + flat segments).
    pub fn total_bend_allowance(&self) -> f64 {
        self.bends.iter()
            .map(|b| b.bend_allowance(self.thickness, self.k_factor))
            .sum()
    }

    /// Count bends by status.
    pub fn error_count(&self) -> usize {
        self.bends.iter().filter(|b| b.status == BendStatus::Error).count()
    }

    /// Draw the sheet metal panel.
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

        let header_h = 80.0;
        let row_h = 22.0;
        let rows = self.bends.len().min(10);
        let panel_h = header_h + rows as f32 * row_h + 8.0;

        // Background
        dl.push_quad(panel_x, panel_y, self.width, panel_h, bg_color);
        let border = [bg_color[0] + 0.1, bg_color[1] + 0.1, bg_color[2] + 0.1, 0.8];
        dl.push_quad(panel_x + self.width - 1.0, panel_y, 1.0, panel_h, border);

        let muted = [text_color[0] * 0.5, text_color[1] * 0.5, text_color[2] * 0.5, text_color[3]];

        // Title
        emit_text(dl, "Sheet Metal", panel_x + 8.0, panel_y + 5.0, 11.0, text_color);

        // Flat pattern toggle
        let fp_bg = if self.flat_pattern { accent_color } else { [0.3, 0.3, 0.3, 0.5] };
        dl.push_quad(panel_x + self.width - 80.0, panel_y + 4.0, 72.0, 16.0, fp_bg);
        emit_text(dl, "Flat Pattern", panel_x + self.width - 76.0, panel_y + 6.0, 8.0,
            if self.flat_pattern { [1.0, 1.0, 1.0, 1.0] } else { text_color });

        // Parameters
        let thick_str = format!("Thickness: {:.2} mm", self.thickness);
        emit_text(dl, &thick_str, panel_x + 8.0, panel_y + 26.0, 8.0, muted);

        let k_str = format!("K-factor: {:.3}", self.k_factor);
        emit_text(dl, &k_str, panel_x + 8.0, panel_y + 40.0, 8.0, muted);

        let rad_str = format!("Default R: {:.2} mm", self.default_radius);
        emit_text(dl, &rad_str, panel_x + 8.0, panel_y + 54.0, 8.0, muted);

        let relief_str = format!("Relief: {}", self.bend_relief.label());
        emit_text(dl, &relief_str, panel_x + 140.0, panel_y + 26.0, 8.0, muted);

        // Total bend allowance
        let ba = self.total_bend_allowance();
        let ba_str = format!("Total BA: {:.2} mm", ba);
        emit_text(dl, &ba_str, panel_x + 140.0, panel_y + 40.0, 8.0, text_color);

        // Bend count
        let bc_str = format!("{} bends", self.bends.len());
        emit_text(dl, &bc_str, panel_x + 140.0, panel_y + 54.0, 8.0, muted);

        // Bend table
        let table_y = panel_y + header_h;
        // Column headers
        dl.push_quad(panel_x, table_y - 16.0, self.width, 16.0,
            [bg_color[0] + 0.02, bg_color[1] + 0.02, bg_color[2] + 0.02, 1.0]);
        emit_text(dl, "Bend", panel_x + 8.0, table_y - 13.0, 7.0, muted);
        emit_text(dl, "Angle", panel_x + 60.0, table_y - 13.0, 7.0, muted);
        emit_text(dl, "Radius", panel_x + 110.0, table_y - 13.0, 7.0, muted);
        emit_text(dl, "BA", panel_x + 170.0, table_y - 13.0, 7.0, muted);
        emit_text(dl, "Dir", panel_x + 220.0, table_y - 13.0, 7.0, muted);

        for (i, bend) in self.bends.iter().enumerate().take(rows) {
            let ry = table_y + i as f32 * row_h;

            let is_sel = self.selected_bend == Some(i);
            let is_hov = self.hovered_bend == Some(i);

            if is_sel {
                dl.push_quad(panel_x, ry, self.width, row_h,
                    [accent_color[0] * 0.2, accent_color[1] * 0.2, accent_color[2] * 0.2, 0.4]);
            } else if is_hov {
                dl.push_quad(panel_x, ry, self.width, row_h, [1.0, 1.0, 1.0, 0.04]);
            }

            // Status indicator
            let status_color = match bend.status {
                BendStatus::Ok => [0.3, 0.8, 0.3, 0.7],
                BendStatus::Warning => [0.9, 0.7, 0.1, 0.7],
                BendStatus::Error => [0.9, 0.2, 0.2, 0.7],
            };
            dl.push_quad(panel_x + 4.0, ry + 6.0, 4.0, 4.0, status_color);

            let tc = if is_sel { accent_color } else { text_color };
            emit_text(dl, &bend.name, panel_x + 12.0, ry + 4.0, 8.0, tc);

            let angle = format!("{:.1}°", bend.angle);
            emit_text(dl, &angle, panel_x + 60.0, ry + 4.0, 8.0, muted);

            let rad = format!("{:.2}", bend.radius);
            emit_text(dl, &rad, panel_x + 110.0, ry + 4.0, 8.0, muted);

            let ba_val = bend.bend_allowance(self.thickness, self.k_factor);
            let ba_str = format!("{:.2}", ba_val);
            emit_text(dl, &ba_str, panel_x + 170.0, ry + 4.0, 8.0, muted);

            let dir = if bend.direction_up { "Up" } else { "Dn" };
            emit_text(dl, dir, panel_x + 220.0, ry + 4.0, 8.0, muted);
        }
    }
}

impl Default for SheetMetal {
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
    fn bend_allowance_calculation() {
        let bend = BendEntry::new("Bend1", 90.0, 2.0);
        let ba = bend.bend_allowance(1.5, 0.44);
        // BA = angle_rad * (R + K*T) = pi/2 * (2.0 + 0.44*1.5)
        let expected = std::f64::consts::FRAC_PI_2 * (2.0 + 0.44 * 1.5);
        assert!((ba - expected).abs() < 0.001);
    }

    #[test]
    fn toggle_flat_pattern() {
        let mut sm = SheetMetal::new();
        assert!(!sm.flat_pattern);
        sm.toggle_flat_pattern();
        assert!(sm.flat_pattern);
    }

    #[test]
    fn total_bend_allowance() {
        let mut sm = SheetMetal::new();
        sm.add_bend(BendEntry::new("B1", 90.0, 1.5));
        sm.add_bend(BendEntry::new("B2", 90.0, 1.5));
        let total = sm.total_bend_allowance();
        assert!(total > 0.0);
    }

    #[test]
    fn bend_relief_labels() {
        let reliefs = [BendRelief::Rectangular, BendRelief::Obround, BendRelief::Tear, BendRelief::None];
        for r in reliefs {
            assert!(!r.label().is_empty());
        }
    }

    #[test]
    fn error_count() {
        let mut sm = SheetMetal::new();
        sm.add_bend(BendEntry::new("B1", 90.0, 1.5));
        let mut b = BendEntry::new("B2", 90.0, 0.5);
        b.status = BendStatus::Error;
        sm.add_bend(b);
        assert_eq!(sm.error_count(), 1);
    }
}
