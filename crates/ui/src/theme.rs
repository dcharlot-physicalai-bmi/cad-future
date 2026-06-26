//! Color themes for the OpenIE UI.
//!
//! Light, dark, and everything in between. Auto mode picks based on
//! time of day. The default is Auto — never assume dark.

/// Which theme to use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeMode {
    Light,
    Dark,
    /// Warm light theme — easy on the eyes in bright environments.
    Warm,
    /// High contrast — accessibility-first.
    HighContrast,
    /// Blue engineering — inspired by technical drawing paper.
    Blueprint,
    /// Auto — picks Light (6am–6pm) or Dark (6pm–6am) based on hour.
    Auto,
}

/// Resolved color palette for the UI.
#[derive(Debug, Clone, Copy)]
pub struct ThemeColors {
    /// Main viewport / canvas background.
    pub viewport_bg: [f32; 4],
    /// Panel / widget background.
    pub panel_bg: [f32; 4],
    /// Panel header background.
    pub header_bg: [f32; 4],
    /// Panel border color.
    pub border: [f32; 4],
    /// Primary text color.
    pub text: [f32; 4],
    /// Secondary / muted text.
    pub text_muted: [f32; 4],
    /// Accent color (buttons, sliders, active states).
    pub accent: [f32; 4],
    /// Hover highlight.
    pub hover: [f32; 4],
    /// Active / pressed highlight.
    pub active: [f32; 4],
    /// Success / pass indicator.
    pub success: [f32; 4],
    /// Error / fail indicator.
    pub error: [f32; 4],
    /// Grid line color.
    pub grid_line: f32,
    /// Viewport clear color (RGB, no alpha).
    pub clear_r: f64,
    pub clear_g: f64,
    pub clear_b: f64,
}

impl ThemeMode {
    /// Resolve this mode into concrete colors.
    /// `hour` is 0..24 (only used by Auto).
    pub fn resolve(self, hour: u32) -> ThemeColors {
        match self {
            ThemeMode::Light => light(),
            ThemeMode::Dark => dark(),
            ThemeMode::Warm => warm(),
            ThemeMode::HighContrast => high_contrast(),
            ThemeMode::Blueprint => blueprint(),
            ThemeMode::Auto => {
                if (6..18).contains(&hour) {
                    light()
                } else {
                    dark()
                }
            }
        }
    }

    /// All available themes for UI cycling.
    pub fn all() -> &'static [ThemeMode] {
        &[
            ThemeMode::Auto,
            ThemeMode::Light,
            ThemeMode::Dark,
            ThemeMode::Warm,
            ThemeMode::HighContrast,
            ThemeMode::Blueprint,
        ]
    }

    pub fn name(self) -> &'static str {
        match self {
            ThemeMode::Light => "Light",
            ThemeMode::Dark => "Dark",
            ThemeMode::Warm => "Warm",
            ThemeMode::HighContrast => "High Contrast",
            ThemeMode::Blueprint => "Blueprint",
            ThemeMode::Auto => "Auto",
        }
    }
}

fn light() -> ThemeColors {
    ThemeColors {
        viewport_bg: [0.92, 0.92, 0.93, 1.0],
        panel_bg: [0.96, 0.96, 0.97, 1.0],
        header_bg: [0.88, 0.88, 0.90, 1.0],
        border: [0.78, 0.78, 0.80, 1.0],
        text: [0.12, 0.12, 0.14, 1.0],
        text_muted: [0.45, 0.45, 0.48, 1.0],
        accent: [0.20, 0.47, 0.85, 1.0],
        hover: [0.85, 0.88, 0.95, 1.0],
        active: [0.75, 0.80, 0.90, 1.0],
        success: [0.18, 0.65, 0.30, 1.0],
        error: [0.82, 0.18, 0.18, 1.0],
        grid_line: 0.7,
        clear_r: 0.92,
        clear_g: 0.92,
        clear_b: 0.93,
    }
}

fn dark() -> ThemeColors {
    ThemeColors {
        viewport_bg: [0.10, 0.10, 0.12, 1.0],
        panel_bg: [0.15, 0.15, 0.15, 1.0],
        header_bg: [0.12, 0.12, 0.14, 1.0],
        border: [0.25, 0.25, 0.25, 1.0],
        text: [0.90, 0.90, 0.90, 1.0],
        text_muted: [0.55, 0.55, 0.55, 1.0],
        accent: [0.26, 0.59, 0.98, 1.0],
        hover: [0.30, 0.30, 0.30, 1.0],
        active: [0.20, 0.20, 0.20, 1.0],
        success: [0.15, 0.80, 0.25, 1.0],
        error: [0.90, 0.15, 0.15, 1.0],
        grid_line: 0.3,
        clear_r: 0.08,
        clear_g: 0.08,
        clear_b: 0.10,
    }
}

fn warm() -> ThemeColors {
    ThemeColors {
        viewport_bg: [0.95, 0.93, 0.88, 1.0],
        panel_bg: [0.97, 0.95, 0.91, 1.0],
        header_bg: [0.90, 0.87, 0.82, 1.0],
        border: [0.80, 0.76, 0.70, 1.0],
        text: [0.20, 0.18, 0.15, 1.0],
        text_muted: [0.50, 0.47, 0.42, 1.0],
        accent: [0.72, 0.45, 0.12, 1.0],
        hover: [0.90, 0.86, 0.78, 1.0],
        active: [0.85, 0.80, 0.70, 1.0],
        success: [0.30, 0.60, 0.20, 1.0],
        error: [0.78, 0.22, 0.15, 1.0],
        grid_line: 0.65,
        clear_r: 0.95,
        clear_g: 0.93,
        clear_b: 0.88,
    }
}

fn high_contrast() -> ThemeColors {
    ThemeColors {
        viewport_bg: [1.0, 1.0, 1.0, 1.0],
        panel_bg: [0.0, 0.0, 0.0, 1.0],
        header_bg: [0.0, 0.0, 0.4, 1.0],
        border: [1.0, 1.0, 0.0, 1.0],
        text: [1.0, 1.0, 1.0, 1.0],
        text_muted: [0.8, 0.8, 0.0, 1.0],
        accent: [0.0, 1.0, 1.0, 1.0],
        hover: [0.2, 0.2, 0.6, 1.0],
        active: [0.3, 0.3, 0.8, 1.0],
        success: [0.0, 1.0, 0.0, 1.0],
        error: [1.0, 0.0, 0.0, 1.0],
        grid_line: 0.5,
        clear_r: 1.0,
        clear_g: 1.0,
        clear_b: 1.0,
    }
}

fn blueprint() -> ThemeColors {
    ThemeColors {
        viewport_bg: [0.12, 0.18, 0.35, 1.0],
        panel_bg: [0.10, 0.15, 0.28, 1.0],
        header_bg: [0.08, 0.12, 0.24, 1.0],
        border: [0.25, 0.35, 0.55, 1.0],
        text: [0.85, 0.90, 0.95, 1.0],
        text_muted: [0.50, 0.60, 0.75, 1.0],
        accent: [0.40, 0.70, 1.0, 1.0],
        hover: [0.18, 0.25, 0.45, 1.0],
        active: [0.15, 0.22, 0.40, 1.0],
        success: [0.30, 0.85, 0.50, 1.0],
        error: [1.0, 0.35, 0.25, 1.0],
        grid_line: 0.35,
        clear_r: 0.12,
        clear_g: 0.18,
        clear_b: 0.35,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_themes_resolve() {
        for mode in ThemeMode::all() {
            let colors = mode.resolve(12);
            assert!(colors.text[3] > 0.0, "{}: text alpha must be > 0", mode.name());
            assert!(colors.accent[3] > 0.0, "{}: accent alpha must be > 0", mode.name());
        }
    }

    #[test]
    fn auto_picks_light_during_day() {
        let colors = ThemeMode::Auto.resolve(12);
        let light_colors = ThemeMode::Light.resolve(12);
        assert_eq!(colors.clear_r, light_colors.clear_r);
    }

    #[test]
    fn auto_picks_dark_at_night() {
        let colors = ThemeMode::Auto.resolve(22);
        let dark_colors = ThemeMode::Dark.resolve(22);
        assert_eq!(colors.clear_r, dark_colors.clear_r);
    }

    #[test]
    fn theme_names_non_empty() {
        for mode in ThemeMode::all() {
            assert!(!mode.name().is_empty());
        }
    }

    #[test]
    fn all_themes_have_valid_clear_colors() {
        for mode in ThemeMode::all() {
            let c = mode.resolve(12);
            assert!(c.clear_r >= 0.0 && c.clear_r <= 1.0);
            assert!(c.clear_g >= 0.0 && c.clear_g <= 1.0);
            assert!(c.clear_b >= 0.0 && c.clear_b <= 1.0);
        }
    }

    #[test]
    fn light_text_is_dark() {
        let c = ThemeMode::Light.resolve(12);
        assert!(c.text[0] < 0.3, "light theme text should be dark");
    }

    #[test]
    fn dark_text_is_light() {
        let c = ThemeMode::Dark.resolve(0);
        assert!(c.text[0] > 0.7, "dark theme text should be light");
    }
}
