//! Appearance browser — grid material/appearance browser with thumbnail swatches.
//!
//! Inspired by SolidWorks Appearance Manager, Fusion 360 Material Library,
//! and KeyShot material browser. Provides a searchable grid of material
//! appearances with color swatches, category filtering, and apply-on-click.

use crate::draw::DrawList;
use crate::font;

/// A material/appearance category.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AppearanceCategory {
    Metal,
    Plastic,
    Composite,
    Ceramic,
    Glass,
    Wood,
    Rubber,
    Custom,
}

impl AppearanceCategory {
    pub fn label(self) -> &'static str {
        match self {
            Self::Metal => "Metal",
            Self::Plastic => "Plastic",
            Self::Composite => "Composite",
            Self::Ceramic => "Ceramic",
            Self::Glass => "Glass",
            Self::Wood => "Wood",
            Self::Rubber => "Rubber",
            Self::Custom => "Custom",
        }
    }

    pub fn all() -> &'static [AppearanceCategory] {
        &[
            Self::Metal, Self::Plastic, Self::Composite,
            Self::Ceramic, Self::Glass, Self::Wood,
            Self::Rubber, Self::Custom,
        ]
    }
}

/// A single appearance/material entry.
#[derive(Clone, Debug)]
pub struct AppearanceEntry {
    /// Display name.
    pub name: String,
    /// Material ID (e.g., "6061-T6").
    pub material_id: String,
    /// Category.
    pub category: AppearanceCategory,
    /// Preview color swatch (RGBA).
    pub color: [f32; 4],
    /// Roughness (0.0 = mirror, 1.0 = matte).
    pub roughness: f32,
    /// Metallic (0.0 = dielectric, 1.0 = metal).
    pub metallic: f32,
}

impl AppearanceEntry {
    pub fn new(name: &str, material_id: &str, category: AppearanceCategory, color: [f32; 4]) -> Self {
        Self {
            name: name.to_string(),
            material_id: material_id.to_string(),
            category,
            color,
            roughness: 0.5,
            metallic: 0.0,
        }
    }

    pub fn metal(name: &str, material_id: &str, color: [f32; 4]) -> Self {
        Self {
            name: name.to_string(),
            material_id: material_id.to_string(),
            category: AppearanceCategory::Metal,
            color,
            roughness: 0.3,
            metallic: 1.0,
        }
    }
}

/// The appearance browser panel.
pub struct AppearanceBrowser {
    /// All available appearances.
    pub entries: Vec<AppearanceEntry>,
    /// Whether the browser is visible.
    pub visible: bool,
    /// Active category filter (None = show all).
    pub filter: Option<AppearanceCategory>,
    /// Search query.
    pub search: String,
    /// Hovered entry index (in filtered list).
    pub hovered: Option<usize>,
    /// Selected entry index (in entries list).
    pub selected: Option<usize>,
    /// Panel width.
    pub width: f32,
    /// Scroll offset.
    pub scroll_offset: usize,
    /// Swatch size.
    pub swatch_size: f32,
    /// Columns in the grid.
    pub columns: usize,
}

impl AppearanceBrowser {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            visible: false,
            filter: None,
            search: String::new(),
            hovered: None,
            selected: None,
            width: 280.0,
            scroll_offset: 0,
            swatch_size: 48.0,
            columns: 5,
        }
    }

    /// Load default engineering materials.
    pub fn load_defaults(&mut self) {
        self.entries.clear();
        // Metals
        self.entries.push(AppearanceEntry::metal("Aluminum 6061", "6061-T6", [0.78, 0.78, 0.80, 1.0]));
        self.entries.push(AppearanceEntry::metal("Aluminum 7075", "7075-T6", [0.75, 0.75, 0.78, 1.0]));
        self.entries.push(AppearanceEntry::metal("Steel 1018", "1018", [0.65, 0.65, 0.65, 1.0]));
        self.entries.push(AppearanceEntry::metal("Steel 4140", "4140", [0.60, 0.60, 0.62, 1.0]));
        self.entries.push(AppearanceEntry::metal("Stainless 304", "304", [0.72, 0.72, 0.74, 1.0]));
        self.entries.push(AppearanceEntry::metal("Stainless 316", "316", [0.70, 0.70, 0.73, 1.0]));
        self.entries.push(AppearanceEntry::metal("Titanium", "Ti-6Al-4V", [0.68, 0.66, 0.65, 1.0]));
        self.entries.push(AppearanceEntry::metal("Brass", "C360", [0.72, 0.53, 0.04, 1.0]));
        self.entries.push(AppearanceEntry::metal("Copper", "C110", [0.85, 0.45, 0.20, 1.0]));
        self.entries.push(AppearanceEntry::metal("Bronze", "C932", [0.70, 0.50, 0.25, 1.0]));
        // Plastics
        self.entries.push(AppearanceEntry::new("ABS White", "ABS", AppearanceCategory::Plastic, [0.92, 0.92, 0.92, 1.0]));
        self.entries.push(AppearanceEntry::new("ABS Black", "ABS-BK", AppearanceCategory::Plastic, [0.12, 0.12, 0.12, 1.0]));
        self.entries.push(AppearanceEntry::new("Nylon", "PA6", AppearanceCategory::Plastic, [0.88, 0.85, 0.78, 1.0]));
        self.entries.push(AppearanceEntry::new("PEEK", "PEEK", AppearanceCategory::Plastic, [0.55, 0.50, 0.40, 1.0]));
        self.entries.push(AppearanceEntry::new("Polycarbonate", "PC", AppearanceCategory::Plastic, [0.85, 0.88, 0.90, 0.7]));
        // Composites
        self.entries.push(AppearanceEntry::new("Carbon Fiber", "CFRP", AppearanceCategory::Composite, [0.15, 0.15, 0.15, 1.0]));
        self.entries.push(AppearanceEntry::new("Fiberglass", "GFRP", AppearanceCategory::Composite, [0.80, 0.82, 0.70, 1.0]));
        // Ceramics
        self.entries.push(AppearanceEntry::new("Alumina", "Al2O3", AppearanceCategory::Ceramic, [0.90, 0.88, 0.85, 1.0]));
        self.entries.push(AppearanceEntry::new("Zirconia", "ZrO2", AppearanceCategory::Ceramic, [0.95, 0.95, 0.92, 1.0]));
        // Rubber
        self.entries.push(AppearanceEntry::new("Silicone", "SI-RUB", AppearanceCategory::Rubber, [0.70, 0.70, 0.72, 1.0]));
    }

    /// Get filtered entries based on category and search.
    pub fn filtered(&self) -> Vec<(usize, &AppearanceEntry)> {
        self.entries.iter().enumerate()
            .filter(|(_, e)| {
                if let Some(cat) = self.filter {
                    if e.category != cat { return false; }
                }
                if !self.search.is_empty() {
                    let search_lower = self.search.to_lowercase();
                    if !e.name.to_lowercase().contains(&search_lower)
                        && !e.material_id.to_lowercase().contains(&search_lower)
                    {
                        return false;
                    }
                }
                true
            })
            .collect()
    }

    /// Toggle visibility.
    pub fn toggle(&mut self) {
        self.visible = !self.visible;
    }

    /// Hit test: which swatch was clicked? Returns index in entries.
    pub fn hit_test(
        &self, mx: f32, my: f32,
        panel_x: f32, panel_y: f32,
    ) -> Option<usize> {
        if !self.visible { return None; }

        let header_h = 56.0; // title + category tabs
        let gap = 4.0;
        let filtered = self.filtered();

        for (vis_i, (real_i, _)) in filtered.iter().skip(self.scroll_offset).enumerate() {
            let col = vis_i % self.columns;
            let row = vis_i / self.columns;
            let sx = panel_x + 8.0 + col as f32 * (self.swatch_size + gap);
            let sy = panel_y + header_h + row as f32 * (self.swatch_size + gap + 14.0);

            if mx >= sx && mx < sx + self.swatch_size
                && my >= sy && my < sy + self.swatch_size
            {
                return Some(*real_i);
            }
        }
        None
    }

    /// Draw the appearance browser.
    pub fn draw(
        &self,
        dl: &mut DrawList,
        panel_x: f32,
        panel_y: f32,
        bg_color: [f32; 4],
        text_color: [f32; 4],
        accent_color: [f32; 4],
    ) {
        if !self.visible { return; }

        let header_h = 56.0;
        let gap = 4.0;
        let row_h = self.swatch_size + gap + 14.0;
        let filtered = self.filtered();
        let rows = (filtered.len() + self.columns - 1) / self.columns;
        let grid_h = rows as f32 * row_h;
        let panel_h = header_h + grid_h + 8.0;

        // Shadow
        dl.push_quad(panel_x + 2.0, panel_y + 2.0, self.width, panel_h, [0.0, 0.0, 0.0, 0.25]);

        // Background
        dl.push_quad(panel_x, panel_y, self.width, panel_h, bg_color);

        // Border
        let border = [bg_color[0] + 0.1, bg_color[1] + 0.1, bg_color[2] + 0.1, 0.8];
        dl.push_quad(panel_x, panel_y, self.width, 1.0, border);
        dl.push_quad(panel_x, panel_y + panel_h - 1.0, self.width, 1.0, border);
        dl.push_quad(panel_x, panel_y, 1.0, panel_h, border);
        dl.push_quad(panel_x + self.width - 1.0, panel_y, 1.0, panel_h, border);

        // Title
        emit_text(dl, "Appearances", panel_x + 8.0, panel_y + 6.0, 12.0, text_color);

        // Count label
        let count = format!("{}", filtered.len());
        let cw = font::measure_text(&count, 10.0, None);
        let muted = [text_color[0] * 0.5, text_color[1] * 0.5, text_color[2] * 0.5, text_color[3]];
        emit_text(dl, &count, panel_x + self.width - cw - 8.0, panel_y + 8.0, 10.0, muted);

        // Category filter tabs
        let tab_y = panel_y + 24.0;
        let mut tx = panel_x + 8.0;
        // "All" tab
        {
            let is_active = self.filter.is_none();
            let tab_color = if is_active { accent_color } else { muted };
            emit_text(dl, "All", tx, tab_y + 3.0, 9.0, tab_color);
            if is_active {
                let w = font::measure_text("All", 9.0, None);
                dl.push_quad(tx, tab_y + 14.0, w, 2.0, accent_color);
            }
            tx += 28.0;
        }
        for cat in &[AppearanceCategory::Metal, AppearanceCategory::Plastic, AppearanceCategory::Composite] {
            let is_active = self.filter == Some(*cat);
            let tab_color = if is_active { accent_color } else { muted };
            let label = cat.label();
            emit_text(dl, label, tx, tab_y + 3.0, 9.0, tab_color);
            if is_active {
                let w = font::measure_text(label, 9.0, None);
                dl.push_quad(tx, tab_y + 14.0, w, 2.0, accent_color);
            }
            tx += font::measure_text(label, 9.0, None) + 10.0;
        }

        // Grid of swatches
        for (vis_i, (real_i, entry)) in filtered.iter().skip(self.scroll_offset).enumerate() {
            let col = vis_i % self.columns;
            let row = vis_i / self.columns;
            let sx = panel_x + 8.0 + col as f32 * (self.swatch_size + gap);
            let sy = panel_y + header_h + row as f32 * row_h;

            let is_hovered = self.hovered == Some(vis_i);
            let is_selected = self.selected == Some(*real_i);

            // Swatch background
            dl.push_quad(sx, sy, self.swatch_size, self.swatch_size, entry.color);

            // Selection/hover border
            if is_selected {
                let bw = 2.0;
                dl.push_quad(sx - bw, sy - bw, self.swatch_size + bw * 2.0, bw, accent_color);
                dl.push_quad(sx - bw, sy + self.swatch_size, self.swatch_size + bw * 2.0, bw, accent_color);
                dl.push_quad(sx - bw, sy, bw, self.swatch_size, accent_color);
                dl.push_quad(sx + self.swatch_size, sy, bw, self.swatch_size, accent_color);
            } else if is_hovered {
                dl.push_quad(sx, sy, self.swatch_size, 1.0, [1.0, 1.0, 1.0, 0.6]);
                dl.push_quad(sx, sy + self.swatch_size - 1.0, self.swatch_size, 1.0, [1.0, 1.0, 1.0, 0.6]);
                dl.push_quad(sx, sy, 1.0, self.swatch_size, [1.0, 1.0, 1.0, 0.6]);
                dl.push_quad(sx + self.swatch_size - 1.0, sy, 1.0, self.swatch_size, [1.0, 1.0, 1.0, 0.6]);
            }

            // Metallic indicator (small "M" badge for metals)
            if entry.metallic > 0.5 {
                dl.push_quad(sx + self.swatch_size - 10.0, sy + 1.0, 9.0, 9.0, [0.0, 0.0, 0.0, 0.5]);
                emit_text(dl, "M", sx + self.swatch_size - 9.0, sy + 1.0, 7.0, [0.8, 0.8, 0.8, 0.7]);
            }

            // Name label below swatch (truncate)
            let label = if entry.name.len() > 8 {
                &entry.name[..8]
            } else {
                &entry.name
            };
            let label_color = if is_hovered { text_color } else { muted };
            emit_text(dl, label, sx, sy + self.swatch_size + 2.0, 8.0, label_color);
        }
    }
}

impl Default for AppearanceBrowser {
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
    fn load_defaults() {
        let mut ab = AppearanceBrowser::new();
        ab.load_defaults();
        assert!(ab.entries.len() >= 15);
    }

    #[test]
    fn filter_by_category() {
        let mut ab = AppearanceBrowser::new();
        ab.load_defaults();
        ab.filter = Some(AppearanceCategory::Metal);
        let filtered = ab.filtered();
        assert!(filtered.iter().all(|(_, e)| e.category == AppearanceCategory::Metal));
        assert!(filtered.len() >= 5);
    }

    #[test]
    fn search_filter() {
        let mut ab = AppearanceBrowser::new();
        ab.load_defaults();
        ab.search = "aluminum".to_string();
        let filtered = ab.filtered();
        assert!(filtered.len() >= 2);
    }

    #[test]
    fn toggle_visibility() {
        let mut ab = AppearanceBrowser::new();
        assert!(!ab.visible);
        ab.toggle();
        assert!(ab.visible);
    }

    #[test]
    fn category_labels() {
        for cat in AppearanceCategory::all() {
            assert!(!cat.label().is_empty());
        }
    }
}
