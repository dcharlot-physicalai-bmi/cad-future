//! Drawing view — 2D orthographic projection sheet with title block.
//!
//! Inspired by SolidWorks Drawing, Fusion 360 Drawing, and AutoCAD Layout.
//! Provides a 2D sheet with standard view projections (front, top, right, iso),
//! title block, scale indicator, and view alignment.

use crate::draw::DrawList;
use crate::font;

/// Standard drawing sheet sizes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SheetSize {
    A4Landscape,
    A3Landscape,
    A2Landscape,
    A1Landscape,
    LetterLandscape,
    TabloidLandscape,
}

impl SheetSize {
    pub fn label(self) -> &'static str {
        match self {
            Self::A4Landscape => "A4 Landscape",
            Self::A3Landscape => "A3 Landscape",
            Self::A2Landscape => "A2 Landscape",
            Self::A1Landscape => "A1 Landscape",
            Self::LetterLandscape => "Letter Landscape",
            Self::TabloidLandscape => "Tabloid Landscape",
        }
    }

    /// Sheet dimensions in mm [width, height].
    pub fn mm(self) -> [f32; 2] {
        match self {
            Self::A4Landscape => [297.0, 210.0],
            Self::A3Landscape => [420.0, 297.0],
            Self::A2Landscape => [594.0, 420.0],
            Self::A1Landscape => [841.0, 594.0],
            Self::LetterLandscape => [279.4, 215.9],
            Self::TabloidLandscape => [431.8, 279.4],
        }
    }
}

/// Projection type for a drawing view.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProjectionType {
    Front,
    Back,
    Right,
    Left,
    Top,
    Bottom,
    Isometric,
    Section,
    Detail,
}

impl ProjectionType {
    pub fn label(self) -> &'static str {
        match self {
            Self::Front => "FRONT",
            Self::Back => "BACK",
            Self::Right => "RIGHT",
            Self::Left => "LEFT",
            Self::Top => "TOP",
            Self::Bottom => "BOTTOM",
            Self::Isometric => "ISOMETRIC",
            Self::Section => "SECTION",
            Self::Detail => "DETAIL",
        }
    }
}

/// A single view on the drawing sheet.
#[derive(Clone, Debug)]
pub struct DrawingViewEntry {
    /// View projection type.
    pub projection: ProjectionType,
    /// Position on sheet (mm from bottom-left).
    pub position: [f32; 2],
    /// View bounding box size (mm).
    pub size: [f32; 2],
    /// Scale as ratio (e.g., 0.5 = 1:2).
    pub scale: f32,
    /// Label override (empty = auto from projection).
    pub label: String,
    /// Whether this view is selected.
    pub selected: bool,
    /// Whether this view has been placed.
    pub placed: bool,
}

impl DrawingViewEntry {
    pub fn new(projection: ProjectionType, x: f32, y: f32, w: f32, h: f32, scale: f32) -> Self {
        Self {
            projection,
            position: [x, y],
            size: [w, h],
            scale,
            label: String::new(),
            selected: false,
            placed: true,
        }
    }

    pub fn scale_label(&self) -> String {
        if self.scale >= 1.0 {
            format!("{}:1", self.scale as u32)
        } else {
            let denom = (1.0 / self.scale).round() as u32;
            format!("1:{}", denom)
        }
    }

    pub fn display_label(&self) -> String {
        if !self.label.is_empty() {
            self.label.clone()
        } else {
            self.projection.label().to_string()
        }
    }
}

/// Title block information.
#[derive(Clone, Debug)]
pub struct TitleBlock {
    pub title: String,
    pub part_number: String,
    pub revision: String,
    pub material: String,
    pub drawn_by: String,
    pub checked_by: String,
    pub date: String,
    pub company: String,
    pub scale: String,
    pub sheet: String,
}

impl TitleBlock {
    pub fn new() -> Self {
        Self {
            title: String::new(),
            part_number: String::new(),
            revision: "A".to_string(),
            material: String::new(),
            drawn_by: String::new(),
            checked_by: String::new(),
            date: String::new(),
            company: String::new(),
            scale: "1:1".to_string(),
            sheet: "1 of 1".to_string(),
        }
    }
}

impl Default for TitleBlock {
    fn default() -> Self {
        Self::new()
    }
}

/// The drawing view workspace.
pub struct DrawingView {
    /// Whether drawing mode is active.
    pub active: bool,
    /// Sheet size.
    pub sheet_size: SheetSize,
    /// Views placed on the sheet.
    pub views: Vec<DrawingViewEntry>,
    /// Title block data.
    pub title_block: TitleBlock,
    /// Zoom level (pixels per mm).
    pub zoom: f32,
    /// Pan offset [x, y].
    pub pan: [f32; 2],
    /// Selected view index.
    pub selected_view: Option<usize>,
    /// Hovered view index.
    pub hovered_view: Option<usize>,
    /// Whether the sheet border is visible.
    pub show_border: bool,
    /// Whether view labels are visible.
    pub show_labels: bool,
    /// Whether center marks are visible.
    pub show_center_marks: bool,
}

impl DrawingView {
    pub fn new() -> Self {
        Self {
            active: false,
            sheet_size: SheetSize::A3Landscape,
            views: Vec::new(),
            title_block: TitleBlock::new(),
            zoom: 2.0,
            pan: [40.0, 40.0],
            selected_view: None,
            hovered_view: None,
            show_border: true,
            show_labels: true,
            show_center_marks: true,
        }
    }

    /// Toggle drawing mode.
    pub fn toggle(&mut self) {
        self.active = !self.active;
    }

    /// Add a standard three-view layout (Front, Top, Right) plus Isometric.
    pub fn add_standard_views(&mut self) {
        let [sw, sh] = self.sheet_size.mm();
        let margin = 30.0;
        let view_w = (sw - margin * 3.0) / 2.5;
        let view_h = (sh - margin * 3.0) / 2.5;

        // Front view (bottom-left)
        self.views.push(DrawingViewEntry::new(
            ProjectionType::Front,
            margin, margin + view_h + 20.0,
            view_w, view_h, 1.0,
        ));
        // Top view (top-left)
        self.views.push(DrawingViewEntry::new(
            ProjectionType::Top,
            margin, sh - margin - view_h,
            view_w, view_h, 1.0,
        ));
        // Right view (bottom-right)
        self.views.push(DrawingViewEntry::new(
            ProjectionType::Right,
            margin + view_w + 20.0, margin + view_h + 20.0,
            view_w, view_h, 1.0,
        ));
        // Isometric (top-right)
        self.views.push(DrawingViewEntry::new(
            ProjectionType::Isometric,
            margin + view_w + 20.0, sh - margin - view_h,
            view_w, view_h, 0.5,
        ));
    }

    /// Remove a view by index.
    pub fn remove_view(&mut self, idx: usize) -> Option<DrawingViewEntry> {
        if idx < self.views.len() {
            Some(self.views.remove(idx))
        } else {
            None
        }
    }

    /// Convert sheet coordinates (mm) to screen coordinates (pixels).
    fn sheet_to_screen(&self, x: f32, y: f32) -> (f32, f32) {
        (self.pan[0] + x * self.zoom, self.pan[1] + y * self.zoom)
    }

    /// Hit test which view was clicked (screen coords).
    pub fn hit_test_view(&self, mx: f32, my: f32) -> Option<usize> {
        if !self.active { return None; }

        for (i, view) in self.views.iter().enumerate().rev() {
            let (sx, sy) = self.sheet_to_screen(view.position[0], view.position[1]);
            let sw = view.size[0] * self.zoom;
            let sh = view.size[1] * self.zoom;
            if mx >= sx && mx <= sx + sw && my >= sy && my <= sy + sh {
                return Some(i);
            }
        }
        None
    }

    /// Draw the drawing sheet and views.
    pub fn draw(
        &self,
        dl: &mut DrawList,
        screen_w: f32,
        screen_h: f32,
        _bg_color: [f32; 4],
        text_color: [f32; 4],
        accent_color: [f32; 4],
    ) {
        if !self.active { return; }

        // Canvas background (dark)
        dl.push_quad(0.0, 0.0, screen_w, screen_h, [0.15, 0.15, 0.17, 1.0]);

        let [sw_mm, sh_mm] = self.sheet_size.mm();
        let (sheet_sx, sheet_sy) = self.sheet_to_screen(0.0, 0.0);
        let sheet_sw = sw_mm * self.zoom;
        let sheet_sh = sh_mm * self.zoom;

        // Sheet shadow
        dl.push_quad(sheet_sx + 4.0, sheet_sy + 4.0, sheet_sw, sheet_sh, [0.0, 0.0, 0.0, 0.3]);

        // Sheet paper
        dl.push_quad(sheet_sx, sheet_sy, sheet_sw, sheet_sh, [1.0, 1.0, 1.0, 1.0]);

        // Sheet border
        if self.show_border {
            let bw = 1.0;
            let bcolor = [0.0, 0.0, 0.0, 1.0];
            dl.push_quad(sheet_sx, sheet_sy, sheet_sw, bw, bcolor);
            dl.push_quad(sheet_sx, sheet_sy + sheet_sh - bw, sheet_sw, bw, bcolor);
            dl.push_quad(sheet_sx, sheet_sy, bw, sheet_sh, bcolor);
            dl.push_quad(sheet_sx + sheet_sw - bw, sheet_sy, bw, sheet_sh, bcolor);

            // Inner margin line
            let m = 10.0 * self.zoom;
            dl.push_quad(sheet_sx + m, sheet_sy + m, sheet_sw - 2.0 * m, 0.5, [0.0, 0.0, 0.0, 0.5]);
            dl.push_quad(sheet_sx + m, sheet_sy + sheet_sh - m, sheet_sw - 2.0 * m, 0.5, [0.0, 0.0, 0.0, 0.5]);
            dl.push_quad(sheet_sx + m, sheet_sy + m, 0.5, sheet_sh - 2.0 * m, [0.0, 0.0, 0.0, 0.5]);
            dl.push_quad(sheet_sx + sheet_sw - m, sheet_sy + m, 0.5, sheet_sh - 2.0 * m, [0.0, 0.0, 0.0, 0.5]);
        }

        // Title block (bottom-right corner)
        {
            let tb_w = 180.0 * self.zoom;
            let tb_h = 56.0 * self.zoom;
            let tb_x = sheet_sx + sheet_sw - tb_w - 10.0 * self.zoom;
            let tb_y = sheet_sy + sheet_sh - tb_h - 10.0 * self.zoom;

            dl.push_quad(tb_x, tb_y, tb_w, tb_h, [0.98, 0.98, 0.98, 1.0]);
            // Outer border
            dl.push_quad(tb_x, tb_y, tb_w, 1.0, [0.0, 0.0, 0.0, 1.0]);
            dl.push_quad(tb_x, tb_y + tb_h, tb_w, 1.0, [0.0, 0.0, 0.0, 1.0]);
            dl.push_quad(tb_x, tb_y, 1.0, tb_h, [0.0, 0.0, 0.0, 1.0]);
            dl.push_quad(tb_x + tb_w, tb_y, 1.0, tb_h, [0.0, 0.0, 0.0, 1.0]);

            // Title block text (on white paper, use black)
            let black = [0.0, 0.0, 0.0, 1.0];
            let gray = [0.3, 0.3, 0.3, 1.0];
            let ts = 8.0 * self.zoom.min(3.0);

            if !self.title_block.title.is_empty() {
                emit_text(dl, &self.title_block.title, tb_x + 4.0, tb_y + 4.0, ts * 1.2, black);
            }
            if !self.title_block.part_number.is_empty() {
                emit_text(dl, &self.title_block.part_number, tb_x + 4.0, tb_y + ts * 1.8, ts * 0.9, gray);
            }
            if !self.title_block.material.is_empty() {
                emit_text(dl, &self.title_block.material, tb_x + 4.0, tb_y + ts * 3.0, ts * 0.9, gray);
            }
            // Rev in top-right
            if !self.title_block.revision.is_empty() {
                let rev = format!("REV {}", self.title_block.revision);
                let rw = font::measure_text(&rev, ts * 0.9, None);
                emit_text(dl, &rev, tb_x + tb_w - rw - 4.0, tb_y + 4.0, ts * 0.9, black);
            }
            // Scale and sheet in bottom
            emit_text(dl, &self.title_block.scale, tb_x + 4.0, tb_y + tb_h - ts * 1.5, ts * 0.8, gray);
            let sw_text = font::measure_text(&self.title_block.sheet, ts * 0.8, None);
            emit_text(dl, &self.title_block.sheet, tb_x + tb_w - sw_text - 4.0,
                tb_y + tb_h - ts * 1.5, ts * 0.8, gray);
        }

        // Draw views
        for (i, view) in self.views.iter().enumerate() {
            if !view.placed { continue; }

            let (vx, vy) = self.sheet_to_screen(view.position[0], view.position[1]);
            let vw = view.size[0] * self.zoom;
            let vh = view.size[1] * self.zoom;

            let is_sel = self.selected_view == Some(i);
            let is_hov = self.hovered_view == Some(i);

            // View border
            let vborder = if is_sel {
                accent_color
            } else if is_hov {
                [0.3, 0.5, 0.8, 0.6]
            } else {
                [0.0, 0.0, 0.0, 0.15]
            };
            dl.push_quad(vx, vy, vw, 0.5, vborder);
            dl.push_quad(vx, vy + vh, vw, 0.5, vborder);
            dl.push_quad(vx, vy, 0.5, vh, vborder);
            dl.push_quad(vx + vw, vy, 0.5, vh, vborder);

            // Center marks
            if self.show_center_marks {
                let cx = vx + vw * 0.5;
                let cy = vy + vh * 0.5;
                let cm = [0.0, 0.0, 0.0, 0.2];
                dl.push_quad(cx - 8.0, cy, 16.0, 0.5, cm);
                dl.push_quad(cx, cy - 8.0, 0.5, 16.0, cm);
            }

            // View label
            if self.show_labels {
                let label = view.display_label();
                let scale_label = view.scale_label();
                let full_label = format!("{} ({})", label, scale_label);
                let lw = font::measure_text(&full_label, 8.0, None);
                let lx = vx + (vw - lw) * 0.5;
                let ly = vy + vh + 4.0;
                emit_text(dl, &full_label, lx, ly, 8.0, [0.0, 0.0, 0.0, 0.7]);
            }
        }

        // Sheet size indicator (top-left corner of canvas)
        {
            emit_text(dl, self.sheet_size.label(), 8.0, 8.0, 10.0, text_color);
            let zoom_str = format!("Zoom: {:.0}%", self.zoom * 100.0 / 2.0);
            emit_text(dl, &zoom_str, 8.0, 22.0, 8.0,
                [text_color[0] * 0.6, text_color[1] * 0.6, text_color[2] * 0.6, text_color[3]]);
        }
    }
}

impl Default for DrawingView {
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
    fn standard_views_layout() {
        let mut dv = DrawingView::new();
        dv.add_standard_views();
        assert_eq!(dv.views.len(), 4);
        assert_eq!(dv.views[0].projection, ProjectionType::Front);
        assert_eq!(dv.views[1].projection, ProjectionType::Top);
        assert_eq!(dv.views[2].projection, ProjectionType::Right);
        assert_eq!(dv.views[3].projection, ProjectionType::Isometric);
    }

    #[test]
    fn scale_label_format() {
        let v = DrawingViewEntry::new(ProjectionType::Front, 0.0, 0.0, 100.0, 100.0, 1.0);
        assert_eq!(v.scale_label(), "1:1");

        let v2 = DrawingViewEntry::new(ProjectionType::Front, 0.0, 0.0, 100.0, 100.0, 0.5);
        assert_eq!(v2.scale_label(), "1:2");
    }

    #[test]
    fn sheet_sizes() {
        let sizes = [
            SheetSize::A4Landscape, SheetSize::A3Landscape,
            SheetSize::A2Landscape, SheetSize::A1Landscape,
            SheetSize::LetterLandscape, SheetSize::TabloidLandscape,
        ];
        for s in sizes {
            let [w, h] = s.mm();
            assert!(w > h); // landscape
            assert!(!s.label().is_empty());
        }
    }

    #[test]
    fn title_block_defaults() {
        let tb = TitleBlock::new();
        assert_eq!(tb.revision, "A");
        assert_eq!(tb.sheet, "1 of 1");
    }

    #[test]
    fn hit_test_views() {
        let mut dv = DrawingView::new();
        dv.active = true;
        dv.zoom = 1.0;
        dv.pan = [0.0, 0.0];
        dv.views.push(DrawingViewEntry::new(
            ProjectionType::Front, 10.0, 10.0, 100.0, 100.0, 1.0));
        // Inside the view
        assert_eq!(dv.hit_test_view(50.0, 50.0), Some(0));
        // Outside
        assert_eq!(dv.hit_test_view(200.0, 200.0), None);
    }
}
