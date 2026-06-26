//! Export/Import dialog — file format selection with options.
//!
//! Inspired by SolidWorks Save As, Fusion 360 Export, and FreeCAD Export.
//! Supports STEP, IGES, STL, 3MF, OBJ, glTF, and DXF with
//! format-specific options (tessellation, units, coordinate system).

use crate::draw::DrawList;
use crate::font;

/// Export file format.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExportFormat {
    STEP,
    IGES,
    STL,
    ThreeMF,
    OBJ,
    GLTF,
    DXF,
    PDF,
    PNG,
}

impl ExportFormat {
    pub fn label(self) -> &'static str {
        match self {
            Self::STEP => "STEP (.step)",
            Self::IGES => "IGES (.igs)",
            Self::STL => "STL (.stl)",
            Self::ThreeMF => "3MF (.3mf)",
            Self::OBJ => "OBJ (.obj)",
            Self::GLTF => "glTF (.gltf)",
            Self::DXF => "DXF (.dxf)",
            Self::PDF => "PDF (.pdf)",
            Self::PNG => "PNG (.png)",
        }
    }

    pub fn extension(self) -> &'static str {
        match self {
            Self::STEP => ".step",
            Self::IGES => ".igs",
            Self::STL => ".stl",
            Self::ThreeMF => ".3mf",
            Self::OBJ => ".obj",
            Self::GLTF => ".gltf",
            Self::DXF => ".dxf",
            Self::PDF => ".pdf",
            Self::PNG => ".png",
        }
    }

    pub fn is_mesh(self) -> bool {
        matches!(self, Self::STL | Self::ThreeMF | Self::OBJ | Self::GLTF)
    }

    pub fn is_brep(self) -> bool {
        matches!(self, Self::STEP | Self::IGES)
    }

    pub fn all() -> &'static [Self] {
        &[Self::STEP, Self::IGES, Self::STL, Self::ThreeMF, Self::OBJ,
          Self::GLTF, Self::DXF, Self::PDF, Self::PNG]
    }
}

/// STL export options.
#[derive(Clone, Debug)]
pub struct StlOptions {
    /// Binary format (vs ASCII).
    pub binary: bool,
    /// Angular tolerance (degrees).
    pub angular_tolerance: f64,
    /// Chord tolerance (mm).
    pub chord_tolerance: f64,
}

impl Default for StlOptions {
    fn default() -> Self {
        Self { binary: true, angular_tolerance: 10.0, chord_tolerance: 0.1 }
    }
}

/// STEP export options.
#[derive(Clone, Debug)]
pub struct StepOptions {
    /// STEP AP version.
    pub ap_version: &'static str,
    /// Include colors.
    pub include_colors: bool,
    /// Include PMI (product manufacturing info).
    pub include_pmi: bool,
}

impl Default for StepOptions {
    fn default() -> Self {
        Self { ap_version: "AP214", include_colors: true, include_pmi: false }
    }
}

/// The export dialog.
pub struct ExportDialog {
    /// Whether the dialog is visible.
    pub visible: bool,
    /// Whether this is export (true) or import (false).
    pub is_export: bool,
    /// Selected format.
    pub format: ExportFormat,
    /// File name.
    pub filename: String,
    /// STL options.
    pub stl_options: StlOptions,
    /// STEP options.
    pub step_options: StepOptions,
    /// Selected format index (in format list).
    pub selected_format_idx: usize,
    /// Hovered format index.
    pub hovered_format_idx: Option<usize>,
    /// Panel width.
    pub width: f32,
    /// Whether export is in progress.
    pub exporting: bool,
    /// Export progress (0.0..1.0).
    pub progress: f32,
}

impl ExportDialog {
    pub fn new() -> Self {
        Self {
            visible: false,
            is_export: true,
            format: ExportFormat::STEP,
            filename: String::new(),
            stl_options: StlOptions::default(),
            step_options: StepOptions::default(),
            selected_format_idx: 0,
            hovered_format_idx: None,
            width: 340.0,
            exporting: false,
            progress: 0.0,
        }
    }

    /// Open as export dialog.
    pub fn open_export(&mut self, default_name: &str) {
        self.visible = true;
        self.is_export = true;
        self.filename = default_name.to_string();
        self.exporting = false;
        self.progress = 0.0;
    }

    /// Open as import dialog.
    pub fn open_import(&mut self) {
        self.visible = true;
        self.is_export = false;
        self.filename.clear();
        self.exporting = false;
    }

    /// Close the dialog.
    pub fn close(&mut self) {
        self.visible = false;
    }

    /// Get the full filename with extension.
    pub fn full_filename(&self) -> String {
        if self.filename.ends_with(self.format.extension()) {
            self.filename.clone()
        } else {
            format!("{}{}", self.filename, self.format.extension())
        }
    }

    /// Select format by index.
    pub fn select_format(&mut self, idx: usize) {
        let formats = ExportFormat::all();
        if let Some(&f) = formats.get(idx) {
            self.format = f;
            self.selected_format_idx = idx;
        }
    }

    /// Draw the export dialog.
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

        let panel_h = 360.0;
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
        let title = if self.is_export { "Export" } else { "Import" };
        emit_text(dl, title, px + 8.0, py + 6.0, 12.0, text_color);

        // Filename
        emit_text(dl, "Filename:", px + 8.0, py + 30.0, 8.0, muted);
        let fn_display = if self.filename.is_empty() { "(untitled)" } else { &self.filename };
        dl.push_quad(px + 8.0, py + 42.0, self.width - 16.0, 20.0, [0.15, 0.15, 0.15, 0.5]);
        emit_text(dl, fn_display, px + 12.0, py + 46.0, 9.0, text_color);

        // Format list
        emit_text(dl, "Format:", px + 8.0, py + 72.0, 8.0, muted);

        let formats = ExportFormat::all();
        let row_h = 20.0;
        for (i, fmt) in formats.iter().enumerate() {
            let ry = py + 84.0 + i as f32 * row_h;

            let is_sel = self.selected_format_idx == i;
            let is_hov = self.hovered_format_idx == Some(i);

            if is_sel {
                dl.push_quad(px + 8.0, ry, self.width - 16.0, row_h,
                    [accent_color[0] * 0.2, accent_color[1] * 0.2, accent_color[2] * 0.2, 0.4]);
            } else if is_hov {
                dl.push_quad(px + 8.0, ry, self.width - 16.0, row_h, [1.0, 1.0, 1.0, 0.04]);
            }

            let tc = if is_sel { accent_color } else { text_color };
            emit_text(dl, fmt.label(), px + 16.0, ry + 4.0, 9.0, tc);

            // Type badge
            let badge = if fmt.is_brep() { "BREP" } else if fmt.is_mesh() { "MESH" } else { "2D" };
            let bw = font::measure_text(badge, 7.0, None);
            emit_text(dl, badge, px + self.width - bw - 16.0, ry + 5.0, 7.0, muted);
        }

        // Format-specific options
        let opts_y = py + 84.0 + formats.len() as f32 * row_h + 8.0;
        dl.push_quad(px + 8.0, opts_y, self.width - 16.0, 1.0, border);

        match self.format {
            ExportFormat::STL => {
                let bin_bg = if self.stl_options.binary { accent_color } else { [0.3, 0.3, 0.3, 0.5] };
                dl.push_quad(px + 8.0, opts_y + 6.0, 12.0, 12.0, bin_bg);
                emit_text(dl, "Binary format", px + 26.0, opts_y + 7.0, 8.0, text_color);

                let chord = format!("Chord: {:.3} mm", self.stl_options.chord_tolerance);
                emit_text(dl, &chord, px + 8.0, opts_y + 24.0, 8.0, muted);
                let angular = format!("Angular: {:.1}°", self.stl_options.angular_tolerance);
                emit_text(dl, &angular, px + 140.0, opts_y + 24.0, 8.0, muted);
            }
            ExportFormat::STEP => {
                emit_text(dl, "Version:", px + 8.0, opts_y + 8.0, 8.0, muted);
                emit_text(dl, self.step_options.ap_version, px + 56.0, opts_y + 8.0, 9.0, text_color);

                let col_bg = if self.step_options.include_colors { accent_color } else { [0.3, 0.3, 0.3, 0.5] };
                dl.push_quad(px + 8.0, opts_y + 24.0, 12.0, 12.0, col_bg);
                emit_text(dl, "Include colors", px + 26.0, opts_y + 25.0, 8.0, text_color);
            }
            _ => {}
        }

        // Progress bar (if exporting)
        if self.exporting {
            let bar_y = py + panel_h - 60.0;
            dl.push_quad(px + 8.0, bar_y, self.width - 16.0, 8.0, [0.2, 0.2, 0.2, 0.5]);
            dl.push_quad(px + 8.0, bar_y, (self.width - 16.0) * self.progress, 8.0, accent_color);
        }

        // OK / Cancel
        let btn_y = py + panel_h - 34.0;
        let ok_label = if self.is_export { "Export" } else { "Import" };
        dl.push_quad(px + 8.0, btn_y, 70.0, 24.0, accent_color);
        emit_text(dl, ok_label, px + 20.0, btn_y + 5.0, 11.0, [1.0, 1.0, 1.0, 1.0]);
        dl.push_quad(px + 86.0, btn_y, 60.0, 24.0, [0.4, 0.4, 0.4, 0.6]);
        emit_text(dl, "Cancel", px + 96.0, btn_y + 5.0, 11.0, text_color);
    }
}

impl Default for ExportDialog {
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
    fn full_filename() {
        let mut ed = ExportDialog::new();
        ed.filename = "part1".to_string();
        ed.format = ExportFormat::STEP;
        assert_eq!(ed.full_filename(), "part1.step");
    }

    #[test]
    fn format_categories() {
        assert!(ExportFormat::STEP.is_brep());
        assert!(ExportFormat::STL.is_mesh());
        assert!(!ExportFormat::DXF.is_mesh());
        assert!(!ExportFormat::DXF.is_brep());
    }

    #[test]
    fn all_formats_have_labels() {
        for f in ExportFormat::all() {
            assert!(!f.label().is_empty());
            assert!(!f.extension().is_empty());
        }
    }

    #[test]
    fn open_export_sets_name() {
        let mut ed = ExportDialog::new();
        ed.open_export("bracket");
        assert!(ed.visible);
        assert!(ed.is_export);
        assert_eq!(ed.filename, "bracket");
    }

    #[test]
    fn select_format() {
        let mut ed = ExportDialog::new();
        ed.select_format(2); // STL
        assert_eq!(ed.format, ExportFormat::STL);
    }
}
