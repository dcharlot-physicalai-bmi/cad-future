//! Preferences dialog — application settings.
//!
//! Inspired by SolidWorks System Options, Fusion 360 Preferences,
//! and Blender Preferences. Provides categorized settings for
//! display, units, performance, and input configuration.

use crate::draw::DrawList;
use crate::font;

/// Settings category.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrefCategory {
    General,
    Display,
    Units,
    Performance,
    Input,
    FileLocations,
}

impl PrefCategory {
    pub fn label(self) -> &'static str {
        match self {
            Self::General => "General",
            Self::Display => "Display",
            Self::Units => "Units",
            Self::Performance => "Performance",
            Self::Input => "Input",
            Self::FileLocations => "File Locations",
        }
    }

    pub fn all() -> &'static [Self] {
        &[Self::General, Self::Display, Self::Units, Self::Performance,
          Self::Input, Self::FileLocations]
    }
}

/// Unit system selection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnitPreset {
    MillimetersMetric,
    CentimetersMetric,
    MetersMetric,
    InchesImperial,
    FeetImperial,
}

impl UnitPreset {
    pub fn label(self) -> &'static str {
        match self {
            Self::MillimetersMetric => "Millimeters (Metric)",
            Self::CentimetersMetric => "Centimeters (Metric)",
            Self::MetersMetric => "Meters (Metric)",
            Self::InchesImperial => "Inches (Imperial)",
            Self::FeetImperial => "Feet (Imperial)",
        }
    }
}

/// The preferences dialog.
pub struct Preferences {
    /// Whether the dialog is visible.
    pub visible: bool,
    /// Active category.
    pub category: PrefCategory,
    /// Hovered category index.
    pub hovered_category: Option<usize>,

    // General
    /// Auto-save interval (seconds, 0 = disabled).
    pub auto_save_interval: u32,
    /// Start with last opened file.
    pub restore_last_file: bool,
    /// Show welcome screen on start.
    pub show_welcome: bool,

    // Display
    /// Anti-aliasing samples (1, 2, 4, 8).
    pub msaa_samples: u32,
    /// Show grid.
    pub show_grid: bool,
    /// Show origin.
    pub show_origin: bool,
    /// Ambient occlusion.
    pub ambient_occlusion: bool,
    /// Edge display.
    pub show_edges: bool,
    /// Background gradient.
    pub gradient_bg: bool,

    // Units
    /// Unit preset.
    pub units: UnitPreset,
    /// Decimal places.
    pub decimal_places: u32,
    /// Angular units (degrees/radians).
    pub angular_degrees: bool,

    // Performance
    /// GPU acceleration enabled.
    pub gpu_acceleration: bool,
    /// Max triangle count before LOD.
    pub lod_threshold: u32,
    /// Texture resolution limit.
    pub max_texture_size: u32,

    // Input
    /// Orbit button (0=left, 1=middle, 2=right).
    pub orbit_button: u8,
    /// Invert zoom direction.
    pub invert_zoom: bool,
    /// Mouse sensitivity.
    pub mouse_sensitivity: f32,

    /// Dialog width.
    pub width: f32,
}

impl Preferences {
    pub fn new() -> Self {
        Self {
            visible: false,
            category: PrefCategory::General,
            hovered_category: None,
            auto_save_interval: 300,
            restore_last_file: true,
            show_welcome: true,
            msaa_samples: 4,
            show_grid: true,
            show_origin: true,
            ambient_occlusion: true,
            show_edges: true,
            gradient_bg: true,
            units: UnitPreset::MillimetersMetric,
            decimal_places: 3,
            angular_degrees: true,
            gpu_acceleration: true,
            lod_threshold: 500_000,
            max_texture_size: 2048,
            orbit_button: 1, // middle
            invert_zoom: false,
            mouse_sensitivity: 1.0,
            width: 480.0,
        }
    }

    pub fn toggle(&mut self) {
        self.visible = !self.visible;
    }

    /// Draw the preferences dialog.
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

        let panel_h = 400.0;
        let px = (screen_w - self.width) * 0.5;
        let py = (screen_h - panel_h) * 0.5;
        let sidebar_w = 120.0;

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
        emit_text(dl, "Preferences", px + 8.0, py + 8.0, 12.0, text_color);

        // Sidebar
        let sidebar_bg = [bg_color[0] - 0.02, bg_color[1] - 0.02, bg_color[2] - 0.02, bg_color[3]];
        dl.push_quad(px, py + 30.0, sidebar_w, panel_h - 30.0, sidebar_bg);
        dl.push_quad(px + sidebar_w, py + 30.0, 1.0, panel_h - 30.0, border);

        for (i, cat) in PrefCategory::all().iter().enumerate() {
            let cy = py + 34.0 + i as f32 * 28.0;
            let is_active = self.category == *cat;
            let is_hov = self.hovered_category == Some(i);

            if is_active {
                dl.push_quad(px, cy, sidebar_w, 28.0,
                    [accent_color[0] * 0.2, accent_color[1] * 0.2, accent_color[2] * 0.2, 0.4]);
                dl.push_quad(px, cy, 3.0, 28.0, accent_color);
            } else if is_hov {
                dl.push_quad(px, cy, sidebar_w, 28.0, [1.0, 1.0, 1.0, 0.05]);
            }

            let tc = if is_active { accent_color } else { text_color };
            emit_text(dl, cat.label(), px + 12.0, cy + 8.0, 9.0, tc);
        }

        // Content area
        let content_x = px + sidebar_w + 12.0;
        let content_w = self.width - sidebar_w - 24.0;
        let row_h = 24.0;

        match self.category {
            PrefCategory::General => {
                let mut cy = py + 38.0;
                emit_text(dl, "General Settings", content_x, cy, 10.0, text_color);
                cy += 24.0;

                emit_text(dl, "Auto-save interval:", content_x, cy + 2.0, 8.0, muted);
                let val = if self.auto_save_interval == 0 { "Disabled".to_string() }
                    else { format!("{} sec", self.auto_save_interval) };
                emit_text(dl, &val, content_x + 120.0, cy + 2.0, 9.0, text_color);
                cy += row_h;

                draw_toggle(dl, content_x, cy, self.restore_last_file, "Restore last file on startup", text_color, accent_color);
                cy += row_h;
                draw_toggle(dl, content_x, cy, self.show_welcome, "Show welcome screen", text_color, accent_color);
            }
            PrefCategory::Display => {
                let mut cy = py + 38.0;
                emit_text(dl, "Display Settings", content_x, cy, 10.0, text_color);
                cy += 24.0;

                emit_text(dl, "Anti-aliasing:", content_x, cy + 2.0, 8.0, muted);
                let aa = format!("{}x MSAA", self.msaa_samples);
                emit_text(dl, &aa, content_x + 120.0, cy + 2.0, 9.0, text_color);
                cy += row_h;

                draw_toggle(dl, content_x, cy, self.show_grid, "Show grid", text_color, accent_color);
                cy += row_h;
                draw_toggle(dl, content_x, cy, self.show_origin, "Show origin", text_color, accent_color);
                cy += row_h;
                draw_toggle(dl, content_x, cy, self.ambient_occlusion, "Ambient occlusion", text_color, accent_color);
                cy += row_h;
                draw_toggle(dl, content_x, cy, self.show_edges, "Show edges", text_color, accent_color);
                cy += row_h;
                draw_toggle(dl, content_x, cy, self.gradient_bg, "Gradient background", text_color, accent_color);
            }
            PrefCategory::Units => {
                let mut cy = py + 38.0;
                emit_text(dl, "Unit Settings", content_x, cy, 10.0, text_color);
                cy += 24.0;

                emit_text(dl, "Unit system:", content_x, cy + 2.0, 8.0, muted);
                emit_text(dl, self.units.label(), content_x + 80.0, cy + 2.0, 9.0, text_color);
                cy += row_h;

                emit_text(dl, "Decimals:", content_x, cy + 2.0, 8.0, muted);
                let dec = format!("{}", self.decimal_places);
                emit_text(dl, &dec, content_x + 80.0, cy + 2.0, 9.0, text_color);
                cy += row_h;

                draw_toggle(dl, content_x, cy, self.angular_degrees, "Degrees (vs Radians)", text_color, accent_color);
            }
            PrefCategory::Performance => {
                let mut cy = py + 38.0;
                emit_text(dl, "Performance Settings", content_x, cy, 10.0, text_color);
                cy += 24.0;

                draw_toggle(dl, content_x, cy, self.gpu_acceleration, "GPU acceleration", text_color, accent_color);
                cy += row_h;

                emit_text(dl, "LOD threshold:", content_x, cy + 2.0, 8.0, muted);
                let lod = format!("{} tris", self.lod_threshold);
                emit_text(dl, &lod, content_x + 120.0, cy + 2.0, 9.0, text_color);
                cy += row_h;

                emit_text(dl, "Max texture:", content_x, cy + 2.0, 8.0, muted);
                let tex = format!("{}px", self.max_texture_size);
                emit_text(dl, &tex, content_x + 120.0, cy + 2.0, 9.0, text_color);
            }
            PrefCategory::Input => {
                let mut cy = py + 38.0;
                emit_text(dl, "Input Settings", content_x, cy, 10.0, text_color);
                cy += 24.0;

                emit_text(dl, "Orbit button:", content_x, cy + 2.0, 8.0, muted);
                let btn = match self.orbit_button {
                    0 => "Left",
                    1 => "Middle",
                    _ => "Right",
                };
                emit_text(dl, btn, content_x + 120.0, cy + 2.0, 9.0, text_color);
                cy += row_h;

                draw_toggle(dl, content_x, cy, self.invert_zoom, "Invert zoom direction", text_color, accent_color);
                cy += row_h;

                emit_text(dl, "Sensitivity:", content_x, cy + 2.0, 8.0, muted);
                let sens = format!("{:.1}x", self.mouse_sensitivity);
                emit_text(dl, &sens, content_x + 120.0, cy + 2.0, 9.0, text_color);

                // Sensitivity slider
                let slider_x = content_x + 160.0;
                let slider_w = content_w - 168.0;
                dl.push_quad(slider_x, cy + 6.0, slider_w, 4.0, [0.3, 0.3, 0.3, 0.5]);
                let t = (self.mouse_sensitivity / 3.0).min(1.0);
                dl.push_quad(slider_x, cy + 6.0, slider_w * t, 4.0, accent_color);
            }
            PrefCategory::FileLocations => {
                let cy = py + 38.0;
                emit_text(dl, "File Locations", content_x, cy, 10.0, text_color);
                emit_text(dl, "(Cloud-native — files stored on server)", content_x, cy + 24.0, 8.0, muted);
            }
        }

        // OK / Cancel / Apply buttons
        let btn_y = py + panel_h - 36.0;
        dl.push_quad(px + self.width - 210.0, btn_y, 60.0, 24.0, accent_color);
        emit_text(dl, "OK", px + self.width - 190.0, btn_y + 5.0, 11.0, [1.0, 1.0, 1.0, 1.0]);

        dl.push_quad(px + self.width - 140.0, btn_y, 60.0, 24.0, [0.4, 0.4, 0.4, 0.6]);
        emit_text(dl, "Cancel", px + self.width - 132.0, btn_y + 5.0, 10.0, text_color);

        dl.push_quad(px + self.width - 70.0, btn_y, 60.0, 24.0, [0.4, 0.4, 0.4, 0.6]);
        emit_text(dl, "Apply", px + self.width - 60.0, btn_y + 5.0, 10.0, text_color);
    }
}

fn draw_toggle(dl: &mut DrawList, x: f32, y: f32, value: bool, label: &str, text_color: [f32; 4], accent_color: [f32; 4]) {
    let bg = if value { accent_color } else { [0.3, 0.3, 0.3, 0.5] };
    dl.push_quad(x, y + 2.0, 12.0, 12.0, bg);
    emit_text(dl, label, x + 18.0, y + 3.0, 8.0, text_color);
}

impl Default for Preferences {
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
    fn toggle_dialog() {
        let mut prefs = Preferences::new();
        assert!(!prefs.visible);
        prefs.toggle();
        assert!(prefs.visible);
    }

    #[test]
    fn default_units() {
        let prefs = Preferences::new();
        assert_eq!(prefs.units, UnitPreset::MillimetersMetric);
        assert_eq!(prefs.decimal_places, 3);
    }

    #[test]
    fn categories() {
        assert_eq!(PrefCategory::all().len(), 6);
        for c in PrefCategory::all() {
            assert!(!c.label().is_empty());
        }
    }

    #[test]
    fn unit_presets() {
        let presets = [UnitPreset::MillimetersMetric, UnitPreset::InchesImperial, UnitPreset::MetersMetric];
        for p in presets {
            assert!(!p.label().is_empty());
        }
    }

    #[test]
    fn default_performance() {
        let prefs = Preferences::new();
        assert!(prefs.gpu_acceleration);
        assert_eq!(prefs.msaa_samples, 4);
    }
}
