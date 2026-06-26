//! Enhanced status bar — cursor coordinates, unit system, selection filters.
//!
//! Inspired by SolidWorks status bar with coordinates, unit toggle,
//! selection filter buttons, and contextual hints.

use crate::draw::DrawList;
use crate::font;

/// Unit system for display.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnitSystem {
    /// Millimeters (SI).
    Millimeters,
    /// Meters.
    Meters,
    /// Inches (Imperial).
    Inches,
}

impl UnitSystem {
    pub fn label(self) -> &'static str {
        match self {
            Self::Millimeters => "mm",
            Self::Meters => "m",
            Self::Inches => "in",
        }
    }

    pub fn cycle(self) -> Self {
        match self {
            Self::Millimeters => Self::Meters,
            Self::Meters => Self::Inches,
            Self::Inches => Self::Millimeters,
        }
    }

    pub fn all() -> &'static [UnitSystem] {
        &[Self::Millimeters, Self::Meters, Self::Inches]
    }
}

/// Selection filter flags.
#[derive(Clone, Copy, Debug)]
pub struct SelectionFilter {
    pub vertices: bool,
    pub edges: bool,
    pub faces: bool,
    pub bodies: bool,
}

impl SelectionFilter {
    pub fn all_enabled() -> Self {
        Self { vertices: true, edges: true, faces: true, bodies: true }
    }

    pub fn is_active(&self) -> bool {
        // Active means at least one filter is disabled
        !self.vertices || !self.edges || !self.faces || !self.bodies
    }
}

impl Default for SelectionFilter {
    fn default() -> Self {
        Self::all_enabled()
    }
}

/// Enhanced status bar with coordinates, units, filters, and context info.
pub struct EnhancedStatusBar {
    /// Current tool name.
    pub tool: String,
    /// Object count.
    pub object_count: usize,
    /// Mode string (Object/Scene + shading + ortho + snap).
    pub mode: String,
    /// Contextual hints.
    pub hints: String,
    /// Cursor world coordinates (if available).
    pub cursor_coords: Option<[f32; 3]>,
    /// Current unit system.
    pub unit_system: UnitSystem,
    /// Selection filter state.
    pub selection_filter: SelectionFilter,
    /// Whether selection filter bar is visible.
    pub filter_visible: bool,
    /// Hovered filter button index (for highlight).
    pub hovered_filter: Option<usize>,
    /// Hovered unit button.
    pub hovered_unit: bool,
    /// Status bar height.
    pub height: f32,
}

impl EnhancedStatusBar {
    pub fn new() -> Self {
        Self {
            tool: "Select".to_string(),
            object_count: 0,
            mode: String::new(),
            hints: String::new(),
            cursor_coords: None,
            unit_system: UnitSystem::Meters,
            selection_filter: SelectionFilter::default(),
            filter_visible: false,
            hovered_filter: None,
            hovered_unit: false,
            height: 22.0,
        }
    }

    /// Format coordinates using the current unit system.
    pub fn format_coord(&self, val: f32) -> String {
        match self.unit_system {
            UnitSystem::Meters => format!("{:.3}", val),
            UnitSystem::Millimeters => format!("{:.1}", val * 1000.0),
            UnitSystem::Inches => format!("{:.3}", val * 39.3701),
        }
    }

    /// Hit test: which element was clicked?
    /// Returns: "unit" if unit button, "filter_V/E/F/B" if filter buttons,
    /// or None.
    pub fn hit_test(&self, mx: f32, my: f32, screen_w: f32, screen_h: f32) -> Option<&'static str> {
        let bar_y = screen_h - self.height;
        if my < bar_y || my > screen_h { return None; }

        // Unit system button (far right)
        let unit_x = screen_w - 40.0;
        if mx >= unit_x && mx < screen_w {
            return Some("unit");
        }

        // Filter buttons (right of coordinates)
        if self.filter_visible {
            let filter_start = screen_w - 180.0;
            let btn_w = 24.0;
            let labels = ["V", "E", "F", "B"];
            for (i, _) in labels.iter().enumerate() {
                let bx = filter_start + i as f32 * (btn_w + 2.0);
                if mx >= bx && mx < bx + btn_w {
                    return match i {
                        0 => Some("filter_V"),
                        1 => Some("filter_E"),
                        2 => Some("filter_F"),
                        3 => Some("filter_B"),
                        _ => None,
                    };
                }
            }
        }

        None
    }

    /// Draw the enhanced status bar.
    pub fn draw(
        &self,
        dl: &mut DrawList,
        screen_w: f32,
        screen_h: f32,
        bg_color: [f32; 4],
        text_color: [f32; 4],
        accent_color: [f32; 4],
    ) {
        let bar_y = screen_h - self.height;

        // Background
        dl.push_quad(0.0, bar_y, screen_w, self.height, bg_color);

        // Top border
        let border = [bg_color[0] + 0.1, bg_color[1] + 0.1, bg_color[2] + 0.1, 1.0];
        dl.push_quad(0.0, bar_y, screen_w, 1.0, border);

        let text_y = bar_y + 5.0;
        let font_size = 11.0;
        let muted = [text_color[0] * 0.6, text_color[1] * 0.6, text_color[2] * 0.6, text_color[3]];
        let mut cx = 8.0;

        // Tool name
        emit_text(dl, &self.tool, cx, text_y, font_size, accent_color);
        cx += font::measure_text(&self.tool, font_size, None) + 12.0;

        // Separator
        dl.push_quad(cx, bar_y + 4.0, 1.0, self.height - 8.0, muted);
        cx += 8.0;

        // Cursor coordinates
        if let Some(coords) = &self.cursor_coords {
            let coord_str = format!(
                "X:{}  Y:{}  Z:{}",
                self.format_coord(coords[0]),
                self.format_coord(coords[1]),
                self.format_coord(coords[2]),
            );
            emit_text(dl, &coord_str, cx, text_y, font_size, text_color);
            cx += font::measure_text(&coord_str, font_size, None) + 12.0;

            // Separator
            dl.push_quad(cx, bar_y + 4.0, 1.0, self.height - 8.0, muted);
            cx += 8.0;
        }

        // Mode info
        if !self.mode.is_empty() {
            emit_text(dl, &self.mode, cx, text_y, font_size, muted);
            cx += font::measure_text(&self.mode, font_size, None) + 12.0;
        }

        // Hints (center-ish)
        if !self.hints.is_empty() {
            dl.push_quad(cx, bar_y + 4.0, 1.0, self.height - 8.0, muted);
            cx += 8.0;
            emit_text(dl, &self.hints, cx, text_y, font_size, muted);
        }

        // Right side: selection filter buttons
        if self.filter_visible {
            let filter_start = screen_w - 180.0;
            let btn_w = 24.0;
            let btn_h = self.height - 4.0;
            let filters = [
                ("V", self.selection_filter.vertices),
                ("E", self.selection_filter.edges),
                ("F", self.selection_filter.faces),
                ("B", self.selection_filter.bodies),
            ];
            for (i, (label, active)) in filters.iter().enumerate() {
                let bx = filter_start + i as f32 * (btn_w + 2.0);
                let by = bar_y + 2.0;
                let bg = if *active {
                    [accent_color[0], accent_color[1], accent_color[2], 0.3]
                } else {
                    [bg_color[0] - 0.02, bg_color[1] - 0.02, bg_color[2] - 0.02, 0.5]
                };
                dl.push_quad(bx, by, btn_w, btn_h, bg);

                let label_color = if *active { accent_color } else { muted };
                let lx = bx + (btn_w - font::measure_text(label, 10.0, None)) * 0.5;
                emit_text(dl, label, lx, text_y, 10.0, label_color);
            }

            // Filter active indicator
            if self.selection_filter.is_active() {
                let indicator_x = filter_start - 16.0;
                emit_text(dl, "F", indicator_x, text_y, 10.0, accent_color);
            }
        }

        // Unit system button (far right)
        let unit_label = self.unit_system.label();
        let unit_x = screen_w - 36.0;
        let unit_bg = if self.hovered_unit {
            [accent_color[0], accent_color[1], accent_color[2], 0.2]
        } else {
            [0.0; 4]
        };
        dl.push_quad(unit_x - 4.0, bar_y + 2.0, 36.0, self.height - 4.0, unit_bg);
        emit_text(dl, unit_label, unit_x, text_y, font_size, accent_color);

        // Object count (left of unit)
        let count_str = format!("{} obj", self.object_count);
        let count_w = font::measure_text(&count_str, 10.0, None);
        emit_text(dl, &count_str, unit_x - count_w - 12.0, text_y, 10.0, muted);
    }
}

impl Default for EnhancedStatusBar {
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
    fn unit_cycle() {
        let u = UnitSystem::Meters;
        assert_eq!(u.cycle(), UnitSystem::Inches);
        assert_eq!(u.cycle().cycle(), UnitSystem::Millimeters);
        assert_eq!(u.cycle().cycle().cycle(), UnitSystem::Meters);
    }

    #[test]
    fn format_coords() {
        let sb = EnhancedStatusBar::new();
        // Default is meters
        assert_eq!(sb.format_coord(1.5), "1.500");

        let mut sb_mm = EnhancedStatusBar::new();
        sb_mm.unit_system = UnitSystem::Millimeters;
        assert_eq!(sb_mm.format_coord(1.5), "1500.0");
    }

    #[test]
    fn selection_filter_active() {
        let f = SelectionFilter::all_enabled();
        assert!(!f.is_active()); // no filtering
        let mut f2 = f;
        f2.vertices = false;
        assert!(f2.is_active()); // filtering active
    }

    #[test]
    fn hit_test_unit() {
        let sb = EnhancedStatusBar::new();
        let result = sb.hit_test(780.0, 580.0, 800.0, 600.0);
        assert_eq!(result, Some("unit"));
    }
}
