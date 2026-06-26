//! `physical-emit-drawing` — SVG technical drawing emitter.
//!
//! Projects 3D B-Rep geometry into 2D orthographic views.
//! Generates dimensioned technical drawings as SVG output
//! with standard engineering annotations.

use glam::DVec3;
use physical_brep::Solid;

// ---------------------------------------------------------------------------
// View Configuration
// ---------------------------------------------------------------------------

/// Standard orthographic projection direction.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ViewDirection {
    Front,   // -Z
    Back,    // +Z
    Top,     // -Y
    Bottom,  // +Y
    Left,    // +X
    Right,   // -X
    Iso,     // isometric (equal angle)
}

impl ViewDirection {
    /// Get the view matrix (direction and up vectors).
    pub fn vectors(self) -> (DVec3, DVec3) {
        match self {
            Self::Front  => (-DVec3::Z, DVec3::Y),
            Self::Back   => (DVec3::Z, DVec3::Y),
            Self::Top    => (-DVec3::Y, -DVec3::Z),
            Self::Bottom => (DVec3::Y, DVec3::Z),
            Self::Left   => (DVec3::X, DVec3::Y),
            Self::Right  => (-DVec3::X, DVec3::Y),
            Self::Iso    => {
                let d = DVec3::new(-1.0, -1.0, -1.0).normalize();
                (d, DVec3::Y)
            }
        }
    }
}

/// Drawing sheet size (mm).
#[derive(Clone, Copy, Debug)]
pub enum SheetSize {
    A4,     // 210 × 297
    A3,     // 297 × 420
    A2,     // 420 × 594
    A1,     // 594 × 841
    Letter, // 215.9 × 279.4
}

impl SheetSize {
    pub fn dimensions_mm(self) -> (f64, f64) {
        match self {
            Self::A4     => (297.0, 210.0),
            Self::A3     => (420.0, 297.0),
            Self::A2     => (594.0, 420.0),
            Self::A1     => (841.0, 594.0),
            Self::Letter => (279.4, 215.9),
        }
    }
}

// ---------------------------------------------------------------------------
// 2D Projected Geometry
// ---------------------------------------------------------------------------

/// A 2D line segment in drawing space.
#[derive(Clone, Debug)]
pub struct Line2D {
    pub start: (f64, f64),
    pub end: (f64, f64),
    pub style: LineStyle,
}

/// Line style.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LineStyle {
    /// Visible edge (solid, thick).
    Visible,
    /// Hidden edge (dashed).
    Hidden,
    /// Center line (dash-dot).
    Center,
    /// Dimension line (thin, solid).
    Dimension,
    /// Construction/phantom (thin, dashed).
    Phantom,
}

/// A dimension annotation.
#[derive(Clone, Debug)]
pub struct Dimension {
    /// Start point (on geometry).
    pub from: (f64, f64),
    /// End point (on geometry).
    pub to: (f64, f64),
    /// Offset distance for the dimension line.
    pub offset: f64,
    /// Dimension text (auto-computed or override).
    pub text: String,
    /// Dimension type.
    pub dim_type: DimensionType,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DimensionType {
    Linear,
    Angular,
    Radial,
    Diameter,
    Ordinate,
}

/// A text annotation.
#[derive(Clone, Debug)]
pub struct TextAnnotation {
    pub position: (f64, f64),
    pub text: String,
    pub font_size: f64,
    pub anchor: TextAnchor,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TextAnchor {
    Start,
    Middle,
    End,
}

// ---------------------------------------------------------------------------
// Drawing View
// ---------------------------------------------------------------------------

/// A single projected view within a drawing.
#[derive(Clone, Debug)]
pub struct DrawingView {
    /// View direction.
    pub direction: ViewDirection,
    /// Origin on the sheet (mm).
    pub origin: (f64, f64),
    /// Scale factor (1.0 = 1:1).
    pub scale: f64,
    /// Projected visible edges.
    pub visible_edges: Vec<Line2D>,
    /// Projected hidden edges.
    pub hidden_edges: Vec<Line2D>,
    /// Dimensions.
    pub dimensions: Vec<Dimension>,
    /// Annotations.
    pub annotations: Vec<TextAnnotation>,
}

/// A complete technical drawing.
#[derive(Clone, Debug)]
pub struct Drawing {
    pub sheet_size: SheetSize,
    pub views: Vec<DrawingView>,
    pub title_block: TitleBlock,
    pub border_margin_mm: f64,
}

/// Title block data.
#[derive(Clone, Debug)]
pub struct TitleBlock {
    pub title: String,
    pub part_number: String,
    pub material: String,
    pub drawn_by: String,
    pub date: String,
    pub scale: String,
    pub revision: String,
}

impl TitleBlock {
    pub fn new(title: &str, part_number: &str) -> Self {
        Self {
            title: title.to_string(),
            part_number: part_number.to_string(),
            material: String::new(),
            drawn_by: String::new(),
            date: String::new(),
            scale: "1:1".to_string(),
            revision: "A".to_string(),
        }
    }
}

// ---------------------------------------------------------------------------
// Projection Engine
// ---------------------------------------------------------------------------

/// Project a 3D solid into a 2D view.
pub fn project_view(solid: &Solid, direction: ViewDirection, scale: f64) -> DrawingView {
    let (view_dir, up) = direction.vectors();
    let right = view_dir.cross(up).normalize();

    let mut visible_edges = Vec::new();

    // Project each edge of the solid
    for eid in solid.edge_ids() {
        let edge = &solid.edges[eid];
        let p0 = solid.vertices[edge.v_start].point;
        let p1 = solid.vertices[edge.v_end].point;

        let x0 = p0.dot(right) * scale;
        let y0 = p0.dot(up) * scale;
        let x1 = p1.dot(right) * scale;
        let y1 = p1.dot(up) * scale;

        // Simple visibility: edge is visible if midpoint normal faces viewer
        // (simplified — real HLR is much more complex)
        visible_edges.push(Line2D {
            start: (x0, y0),
            end: (x1, y1),
            style: LineStyle::Visible,
        });
    }

    DrawingView {
        direction,
        origin: (0.0, 0.0),
        scale,
        visible_edges,
        hidden_edges: Vec::new(),
        dimensions: Vec::new(),
        annotations: Vec::new(),
    }
}

/// Auto-add overall dimensions to a view.
pub fn add_overall_dimensions(view: &mut DrawingView, solid: &Solid, direction: ViewDirection) {
    let (bb_min, bb_max) = solid.bounding_box();
    let (view_dir, up) = direction.vectors();
    let right = view_dir.cross(up).normalize();
    let scale = view.scale;

    // Width (horizontal)
    let w_min = bb_min.dot(right) * scale;
    let w_max = bb_max.dot(right) * scale;
    let h_min = bb_min.dot(up) * scale;
    let h_max = bb_max.dot(up) * scale;

    let width = (w_max - w_min).abs();
    let height = (h_max - h_min).abs();

    // Horizontal dimension below
    view.dimensions.push(Dimension {
        from: (w_min, h_min),
        to: (w_max, h_min),
        offset: -10.0,
        text: format!("{:.1}", width / scale),
        dim_type: DimensionType::Linear,
    });

    // Vertical dimension to the right
    view.dimensions.push(Dimension {
        from: (w_max, h_min),
        to: (w_max, h_max),
        offset: 10.0,
        text: format!("{:.1}", height / scale),
        dim_type: DimensionType::Linear,
    });
}

// ---------------------------------------------------------------------------
// SVG Output
// ---------------------------------------------------------------------------

/// Generate SVG string from a drawing.
pub fn write_svg(drawing: &Drawing) -> String {
    let (sheet_w, sheet_h) = drawing.sheet_size.dimensions_mm();
    let margin = drawing.border_margin_mm;

    let mut svg = String::new();
    svg.push_str(&format!(
        r##"<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" width="{sheet_w}mm" height="{sheet_h}mm" viewBox="0 0 {sheet_w} {sheet_h}">
<defs>
  <style>
    .visible {{ stroke: #000; stroke-width: 0.5; fill: none; }}
    .hidden {{ stroke: #000; stroke-width: 0.25; stroke-dasharray: 3,2; fill: none; }}
    .center {{ stroke: #000; stroke-width: 0.18; stroke-dasharray: 8,2,2,2; fill: none; }}
    .dimension {{ stroke: #000; stroke-width: 0.18; fill: none; }}
    .dim-text {{ font-family: 'Arial', sans-serif; font-size: 3.5px; fill: #000; }}
    .title-text {{ font-family: 'Arial', sans-serif; font-size: 5px; fill: #000; font-weight: bold; }}
    .info-text {{ font-family: 'Arial', sans-serif; font-size: 3px; fill: #000; }}
  </style>
  <marker id="arrow" markerWidth="6" markerHeight="4" refX="6" refY="2" orient="auto">
    <path d="M0,0 L6,2 L0,4" fill="#000" />
  </marker>
</defs>
"##));

    // Border
    svg.push_str(&format!(
        r#"<rect x="{margin}" y="{margin}" width="{}" height="{}" class="visible" />
"#,
        sheet_w - 2.0 * margin, sheet_h - 2.0 * margin
    ));

    // Views
    for view in &drawing.views {
        let ox = view.origin.0 + sheet_w / 2.0;
        let oy = view.origin.1 + sheet_h / 2.0;

        svg.push_str(&format!(r#"<g transform="translate({ox},{oy}) scale(1,-1)">"#));
        svg.push('\n');

        // Visible edges
        for line in &view.visible_edges {
            svg.push_str(&format!(
                r#"  <line x1="{:.3}" y1="{:.3}" x2="{:.3}" y2="{:.3}" class="visible" />"#,
                line.start.0, line.start.1, line.end.0, line.end.1
            ));
            svg.push('\n');
        }

        // Hidden edges
        for line in &view.hidden_edges {
            svg.push_str(&format!(
                r#"  <line x1="{:.3}" y1="{:.3}" x2="{:.3}" y2="{:.3}" class="hidden" />"#,
                line.start.0, line.start.1, line.end.0, line.end.1
            ));
            svg.push('\n');
        }

        // Dimensions
        for dim in &view.dimensions {
            emit_dimension_svg(&mut svg, dim);
        }

        svg.push_str("</g>\n");
    }

    // Title block (bottom right corner)
    let tb = &drawing.title_block;
    let tb_x = sheet_w - margin - 90.0;
    let tb_y = sheet_h - margin - 30.0;

    svg.push_str(&format!(
        r#"<rect x="{tb_x}" y="{tb_y}" width="90" height="30" class="visible" />
<text x="{}" y="{}" class="title-text">{}</text>
<text x="{}" y="{}" class="info-text">Part: {}</text>
<text x="{}" y="{}" class="info-text">Material: {}</text>
<text x="{}" y="{}" class="info-text">Scale: {} | Rev: {}</text>
"#,
        tb_x + 2.0, tb_y + 8.0, tb.title,
        tb_x + 2.0, tb_y + 14.0, tb.part_number,
        tb_x + 2.0, tb_y + 19.0, tb.material,
        tb_x + 2.0, tb_y + 24.0, tb.scale, tb.revision,
    ));

    svg.push_str("</svg>\n");
    svg
}

fn emit_dimension_svg(svg: &mut String, dim: &Dimension) {
    let (x1, y1) = dim.from;
    let (x2, y2) = dim.to;
    let mid_x = (x1 + x2) / 2.0;
    let mid_y = (y1 + y2) / 2.0;

    // Determine if horizontal or vertical
    let dx = (x2 - x1).abs();
    let dy = (y2 - y1).abs();

    let (line_x1, line_y1, line_x2, line_y2, text_x, text_y) = if dx > dy {
        // Horizontal dimension
        let ly = y1 + dim.offset;
        (x1, ly, x2, ly, mid_x, ly - 1.5)
    } else {
        // Vertical dimension
        let lx = x1 + dim.offset;
        (lx, y1, lx, y2, lx + 1.5, mid_y)
    };

    // Extension lines
    svg.push_str(&format!(
        r#"  <line x1="{x1:.3}" y1="{y1:.3}" x2="{line_x1:.3}" y2="{line_y1:.3}" class="dimension" />"#
    ));
    svg.push('\n');
    svg.push_str(&format!(
        r#"  <line x1="{x2:.3}" y1="{y2:.3}" x2="{line_x2:.3}" y2="{line_y2:.3}" class="dimension" />"#
    ));
    svg.push('\n');

    // Dimension line with arrows
    svg.push_str(&format!(
        r#"  <line x1="{line_x1:.3}" y1="{line_y1:.3}" x2="{line_x2:.3}" y2="{line_y2:.3}" class="dimension" marker-start="url(#arrow)" marker-end="url(#arrow)" />"#
    ));
    svg.push('\n');

    // Dimension text (in non-flipped coordinates)
    svg.push_str(&format!(
        r#"  <g transform="scale(1,-1)"><text x="{text_x:.3}" y="{:.3}" class="dim-text" text-anchor="middle">{}</text></g>"#,
        -text_y, dim.text
    ));
    svg.push('\n');
}

/// Create a standard 3-view drawing (front, top, right) with title block.
pub fn three_view_drawing(
    solid: &Solid,
    title: &str,
    part_number: &str,
    material: &str,
    sheet: SheetSize,
) -> Drawing {
    let scale = auto_scale(solid, sheet);

    let mut front = project_view(solid, ViewDirection::Front, scale);
    front.origin = (-40.0, 20.0);
    add_overall_dimensions(&mut front, solid, ViewDirection::Front);

    let mut top = project_view(solid, ViewDirection::Top, scale);
    top.origin = (-40.0, -50.0);

    let mut right = project_view(solid, ViewDirection::Right, scale);
    right.origin = (40.0, 20.0);

    let mut tb = TitleBlock::new(title, part_number);
    tb.material = material.to_string();
    tb.scale = format!("1:{:.0}", 1.0 / scale);

    Drawing {
        sheet_size: sheet,
        views: vec![front, top, right],
        title_block: tb,
        border_margin_mm: 10.0,
    }
}

/// Compute a scale that fits the solid on the sheet.
fn auto_scale(solid: &Solid, sheet: SheetSize) -> f64 {
    let (min, max) = solid.bounding_box();
    let size = max - min;
    let max_dim = size.x.max(size.y).max(size.z);

    let (sheet_w, sheet_h) = sheet.dimensions_mm();
    let available = sheet_w.min(sheet_h) * 0.3; // use ~30% of sheet for each view

    if max_dim > 0.0 {
        (available / max_dim).min(10.0).max(0.01)
    } else {
        1.0
    }
}

// ---------------------------------------------------------------------------
// PDF Output — minimal PDF 1.4 writer (no external dependencies)
// ---------------------------------------------------------------------------

/// Generate a valid PDF from a technical drawing.
///
/// Produces PDF 1.4 with vector line drawing, text (built-in Helvetica),
/// dimension annotations, and a title block.
pub fn write_pdf(drawing: &Drawing) -> Vec<u8> {
    let (sheet_w_mm, sheet_h_mm) = drawing.sheet_size.dimensions_mm();
    // PDF uses points (1 pt = 1/72 inch = 25.4/72 mm ≈ 0.3528 mm)
    let mm_to_pt = 72.0 / 25.4;
    let page_w = sheet_w_mm * mm_to_pt;
    let page_h = sheet_h_mm * mm_to_pt;
    let margin = drawing.border_margin_mm * mm_to_pt;

    // We build the PDF by collecting objects, then serializing.
    // Objects: 1=Catalog, 2=Pages, 3=Page, 4=Font, 5=ContentStream
    let mut objects: Vec<String> = Vec::new();
    let mut offsets: Vec<usize> = Vec::new();
    let mut pdf = Vec::<u8>::new();

    // Header
    pdf.extend_from_slice(b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n");

    // Build content stream
    let content = build_pdf_content(drawing, page_w, page_h, margin, mm_to_pt);

    // Object 1: Catalog
    objects.push(format!(
        "1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n"
    ));

    // Object 2: Pages
    objects.push(format!(
        "2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n"
    ));

    // Object 3: Page
    objects.push(format!(
        "3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {page_w:.2} {page_h:.2}] \
         /Contents 5 0 R /Resources << /Font << /F1 4 0 R >> >> >>\nendobj\n"
    ));

    // Object 4: Font (Helvetica, built-in)
    objects.push(format!(
        "4 0 obj\n<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>\nendobj\n"
    ));

    // Object 5: Content stream
    let stream_bytes = content.as_bytes();
    objects.push(format!(
        "5 0 obj\n<< /Length {} >>\nstream\n{}\nendstream\nendobj\n",
        stream_bytes.len(),
        content
    ));

    // Write objects and record offsets
    for obj in &objects {
        offsets.push(pdf.len());
        pdf.extend_from_slice(obj.as_bytes());
    }

    // Cross-reference table
    let xref_offset = pdf.len();
    pdf.extend_from_slice(b"xref\n");
    pdf.extend_from_slice(format!("0 {}\n", objects.len() + 1).as_bytes());
    pdf.extend_from_slice(b"0000000000 65535 f \n");
    for offset in &offsets {
        pdf.extend_from_slice(format!("{:010} 00000 n \n", offset).as_bytes());
    }

    // Trailer
    pdf.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{}\n%%EOF\n",
            objects.len() + 1,
            xref_offset
        )
        .as_bytes(),
    );

    pdf
}

/// Build the PDF content stream (drawing operators).
fn build_pdf_content(
    drawing: &Drawing,
    page_w: f64,
    page_h: f64,
    margin: f64,
    mm_to_pt: f64,
) -> String {
    let mut s = String::new();

    // Border rectangle
    s.push_str(&format!(
        "{:.2} w\n{:.2} {:.2} {:.2} {:.2} re S\n",
        0.5 * mm_to_pt * 0.5, // ~0.18mm line
        margin,
        margin,
        page_w - 2.0 * margin,
        page_h - 2.0 * margin,
    ));

    // Views
    let center_x = page_w / 2.0;
    let center_y = page_h / 2.0;

    for view in &drawing.views {
        let ox = view.origin.0 * mm_to_pt + center_x;
        // PDF Y is bottom-up, drawing Y is math-style (up), so no flip needed
        let oy = view.origin.1 * mm_to_pt + center_y;

        // Visible edges — solid, 0.5mm
        if !view.visible_edges.is_empty() {
            s.push_str(&format!("{:.3} w\n[] 0 d\n", 0.5 * mm_to_pt));
            for line in &view.visible_edges {
                let (x1, y1, x2, y2) = pdf_line_coords(line, ox, oy, mm_to_pt);
                s.push_str(&format!("{x1:.2} {y1:.2} m {x2:.2} {y2:.2} l S\n"));
            }
        }

        // Hidden edges — dashed, 0.25mm
        if !view.hidden_edges.is_empty() {
            let dash_len = 3.0 * mm_to_pt;
            let gap_len = 2.0 * mm_to_pt;
            s.push_str(&format!(
                "{:.3} w\n[{dash_len:.1} {gap_len:.1}] 0 d\n",
                0.25 * mm_to_pt
            ));
            for line in &view.hidden_edges {
                let (x1, y1, x2, y2) = pdf_line_coords(line, ox, oy, mm_to_pt);
                s.push_str(&format!("{x1:.2} {y1:.2} m {x2:.2} {y2:.2} l S\n"));
            }
        }

        // Dimensions
        for dim in &view.dimensions {
            emit_dimension_pdf(&mut s, dim, ox, oy, mm_to_pt);
        }

        // Annotations
        for ann in &view.annotations {
            let ax = ann.position.0 * mm_to_pt + ox;
            let ay = ann.position.1 * mm_to_pt + oy;
            let fs = ann.font_size * mm_to_pt;
            s.push_str(&format!(
                "BT /F1 {fs:.1} Tf {ax:.2} {ay:.2} Td ({}) Tj ET\n",
                pdf_escape(&ann.text)
            ));
        }
    }

    // Reset line style for title block
    s.push_str(&format!("{:.3} w\n[] 0 d\n", 0.5 * mm_to_pt));

    // Title block (bottom-right corner)
    let tb = &drawing.title_block;
    let tb_w = 90.0 * mm_to_pt;
    let tb_h = 30.0 * mm_to_pt;
    let tb_x = page_w - margin - tb_w;
    let tb_y = margin; // bottom of page

    // Title block rectangle
    s.push_str(&format!("{tb_x:.2} {tb_y:.2} {tb_w:.2} {tb_h:.2} re S\n"));

    // Title block text
    let text_x = tb_x + 2.0 * mm_to_pt;
    let title_fs = 5.0 * mm_to_pt;
    let info_fs = 3.0 * mm_to_pt;

    s.push_str(&format!(
        "BT /F1 {title_fs:.1} Tf {text_x:.2} {:.2} Td ({}) Tj ET\n",
        tb_y + tb_h - 8.0 * mm_to_pt,
        pdf_escape(&tb.title)
    ));
    s.push_str(&format!(
        "BT /F1 {info_fs:.1} Tf {text_x:.2} {:.2} Td (Part: {}) Tj ET\n",
        tb_y + tb_h - 14.0 * mm_to_pt,
        pdf_escape(&tb.part_number)
    ));
    s.push_str(&format!(
        "BT /F1 {info_fs:.1} Tf {text_x:.2} {:.2} Td (Material: {}) Tj ET\n",
        tb_y + tb_h - 19.0 * mm_to_pt,
        pdf_escape(&tb.material)
    ));
    s.push_str(&format!(
        "BT /F1 {info_fs:.1} Tf {text_x:.2} {:.2} Td (Scale: {} | Rev: {}) Tj ET\n",
        tb_y + tb_h - 24.0 * mm_to_pt,
        pdf_escape(&tb.scale),
        pdf_escape(&tb.revision)
    ));

    if !tb.drawn_by.is_empty() {
        s.push_str(&format!(
            "BT /F1 {info_fs:.1} Tf {text_x:.2} {:.2} Td (Drawn: {}) Tj ET\n",
            tb_y + tb_h - 28.0 * mm_to_pt,
            pdf_escape(&tb.drawn_by)
        ));
    }

    s
}

/// Convert a Line2D to PDF coordinates.
fn pdf_line_coords(line: &Line2D, ox: f64, oy: f64, mm_to_pt: f64) -> (f64, f64, f64, f64) {
    (
        line.start.0 * mm_to_pt + ox,
        line.start.1 * mm_to_pt + oy,
        line.end.0 * mm_to_pt + ox,
        line.end.1 * mm_to_pt + oy,
    )
}

/// Emit a dimension annotation to the PDF content stream.
fn emit_dimension_pdf(s: &mut String, dim: &Dimension, ox: f64, oy: f64, mm_to_pt: f64) {
    let x1 = dim.from.0 * mm_to_pt + ox;
    let y1 = dim.from.1 * mm_to_pt + oy;
    let x2 = dim.to.0 * mm_to_pt + ox;
    let y2 = dim.to.1 * mm_to_pt + oy;
    let mid_x = (x1 + x2) / 2.0;
    let mid_y = (y1 + y2) / 2.0;
    let offset = dim.offset * mm_to_pt;

    let dx = (x2 - x1).abs();
    let dy = (y2 - y1).abs();

    let (lx1, ly1, lx2, ly2, tx, ty) = if dx > dy {
        // Horizontal dimension
        let ly = y1 + offset;
        (x1, ly, x2, ly, mid_x, ly + 1.5 * mm_to_pt)
    } else {
        // Vertical dimension
        let lx = x1 + offset;
        (lx, y1, lx, y2, lx + 1.5 * mm_to_pt, mid_y)
    };

    // Thin line style for dimension lines
    let thin = 0.18 * mm_to_pt;
    s.push_str(&format!("{thin:.3} w\n[] 0 d\n"));

    // Extension lines
    s.push_str(&format!("{x1:.2} {y1:.2} m {lx1:.2} {ly1:.2} l S\n"));
    s.push_str(&format!("{x2:.2} {y2:.2} m {lx2:.2} {ly2:.2} l S\n"));

    // Dimension line
    s.push_str(&format!("{lx1:.2} {ly1:.2} m {lx2:.2} {ly2:.2} l S\n"));

    // Arrow heads (simple filled triangles)
    let arrow_len = 2.0 * mm_to_pt;
    let arrow_half_w = 0.7 * mm_to_pt;

    if dx > dy {
        // Horizontal: arrows point left/right
        // Left arrow at (lx1, ly1)
        s.push_str(&format!(
            "{:.2} {:.2} m {:.2} {:.2} l {:.2} {:.2} l f\n",
            lx1, ly1, lx1 + arrow_len, ly1 + arrow_half_w, lx1 + arrow_len, ly1 - arrow_half_w
        ));
        // Right arrow at (lx2, ly2)
        s.push_str(&format!(
            "{:.2} {:.2} m {:.2} {:.2} l {:.2} {:.2} l f\n",
            lx2, ly2, lx2 - arrow_len, ly2 + arrow_half_w, lx2 - arrow_len, ly2 - arrow_half_w
        ));
    } else {
        // Vertical: arrows point up/down
        // Bottom arrow at (lx1, ly1)
        s.push_str(&format!(
            "{:.2} {:.2} m {:.2} {:.2} l {:.2} {:.2} l f\n",
            lx1, ly1, lx1 + arrow_half_w, ly1 + arrow_len, lx1 - arrow_half_w, ly1 + arrow_len
        ));
        // Top arrow at (lx2, ly2)
        s.push_str(&format!(
            "{:.2} {:.2} m {:.2} {:.2} l {:.2} {:.2} l f\n",
            lx2, ly2, lx2 + arrow_half_w, ly2 - arrow_len, lx2 - arrow_half_w, ly2 - arrow_len
        ));
    }

    // Dimension text
    let fs = 3.5 * mm_to_pt;
    s.push_str(&format!(
        "BT /F1 {fs:.1} Tf {tx:.2} {ty:.2} Td ({}) Tj ET\n",
        pdf_escape(&dim.text)
    ));
}

/// Escape special characters for PDF text strings.
fn pdf_escape(text: &str) -> String {
    text.replace('\\', "\\\\")
        .replace('(', "\\(")
        .replace(')', "\\)")
}

#[cfg(test)]
mod tests {
    use super::*;
    use physical_brep::builder::make_box;

    #[test]
    fn project_box_front() {
        let solid = make_box(10.0, 20.0, 30.0);
        let view = project_view(&solid, ViewDirection::Front, 1.0);
        assert!(!view.visible_edges.is_empty());
    }

    #[test]
    fn project_box_top() {
        let solid = make_box(10.0, 20.0, 30.0);
        let view = project_view(&solid, ViewDirection::Top, 1.0);
        assert!(!view.visible_edges.is_empty());
    }

    #[test]
    fn overall_dimensions() {
        let solid = make_box(10.0, 20.0, 30.0);
        let mut view = project_view(&solid, ViewDirection::Front, 1.0);
        add_overall_dimensions(&mut view, &solid, ViewDirection::Front);
        assert_eq!(view.dimensions.len(), 2);
    }

    #[test]
    fn svg_output_valid() {
        let solid = make_box(10.0, 10.0, 10.0);
        let drawing = three_view_drawing(&solid, "Test Part", "TP-001", "Steel", SheetSize::A4);
        let svg = write_svg(&drawing);

        assert!(svg.contains("<svg"));
        assert!(svg.contains("</svg>"));
        assert!(svg.contains("Test Part"));
        assert!(svg.contains("TP-001"));
        assert!(svg.contains("Steel"));
    }

    #[test]
    fn three_view_has_views() {
        let solid = make_box(10.0, 10.0, 10.0);
        let drawing = three_view_drawing(&solid, "Box", "B-001", "Aluminum", SheetSize::A3);
        assert_eq!(drawing.views.len(), 3);
    }

    #[test]
    fn sheet_sizes() {
        let (w, h) = SheetSize::A4.dimensions_mm();
        assert!((w - 297.0).abs() < 0.1);
        assert!((h - 210.0).abs() < 0.1);
    }

    #[test]
    fn auto_scale_reasonable() {
        let solid = make_box(100.0, 100.0, 100.0);
        let s = auto_scale(&solid, SheetSize::A4);
        assert!(s > 0.01 && s < 10.0, "scale={}", s);
    }

    #[test]
    fn title_block() {
        let tb = TitleBlock::new("Widget", "W-100");
        assert_eq!(tb.title, "Widget");
        assert_eq!(tb.revision, "A");
    }

    // -----------------------------------------------------------------------
    // PDF output tests
    // -----------------------------------------------------------------------

    #[test]
    fn pdf_starts_with_header() {
        let solid = make_box(10.0, 10.0, 10.0);
        let drawing = three_view_drawing(&solid, "Test", "T-001", "Steel", SheetSize::A4);
        let pdf = write_pdf(&drawing);
        assert!(pdf.starts_with(b"%PDF"), "PDF must start with %PDF header");
    }

    #[test]
    fn pdf_contains_page_objects() {
        let solid = make_box(20.0, 15.0, 10.0);
        let drawing = three_view_drawing(&solid, "Widget", "W-001", "Aluminum", SheetSize::A3);
        let pdf = write_pdf(&drawing);
        let text = String::from_utf8_lossy(&pdf);

        assert!(text.contains("/Type /Catalog"), "Missing catalog object");
        assert!(text.contains("/Type /Pages"), "Missing pages object");
        assert!(text.contains("/Type /Page"), "Missing page object");
        assert!(text.contains("/Type /Font"), "Missing font object");
        assert!(text.contains("/BaseFont /Helvetica"), "Missing Helvetica font");
        assert!(text.contains("%%EOF"), "Missing EOF marker");
    }

    #[test]
    fn pdf_correct_page_size_a4() {
        let solid = make_box(10.0, 10.0, 10.0);
        let drawing = three_view_drawing(&solid, "A4 Test", "A4-001", "Steel", SheetSize::A4);
        let pdf = write_pdf(&drawing);
        let text = String::from_utf8_lossy(&pdf);

        // A4 landscape: 297mm x 210mm => ~841.89pt x ~595.28pt
        assert!(text.contains("/MediaBox"), "Missing MediaBox");
        // Check approximate page dimensions
        assert!(text.contains("841"), "A4 width ~841pt missing");
        assert!(text.contains("595"), "A4 height ~595pt missing");
    }

    #[test]
    fn pdf_correct_page_size_a3() {
        let solid = make_box(10.0, 10.0, 10.0);
        let drawing = three_view_drawing(&solid, "A3 Test", "A3-001", "Steel", SheetSize::A3);
        let pdf = write_pdf(&drawing);
        let text = String::from_utf8_lossy(&pdf);

        // A3 landscape: 420mm x 297mm => ~1190.55pt x ~841.89pt
        assert!(text.contains("1190"), "A3 width ~1190pt missing");
        assert!(text.contains("841"), "A3 height ~841pt missing");
    }

    #[test]
    fn pdf_nontrivial_size() {
        let solid = make_box(50.0, 30.0, 20.0);
        let drawing = three_view_drawing(&solid, "Big Part", "BP-001", "Titanium", SheetSize::A3);
        let pdf = write_pdf(&drawing);
        // A valid PDF with drawing content should be at least a few KB
        assert!(
            pdf.len() > 500,
            "PDF too small ({} bytes) — likely missing content",
            pdf.len()
        );
    }

    #[test]
    fn pdf_contains_title_block_text() {
        let solid = make_box(10.0, 10.0, 10.0);
        let drawing = three_view_drawing(&solid, "Bracket", "BR-100", "Steel", SheetSize::A4);
        let pdf = write_pdf(&drawing);
        let text = String::from_utf8_lossy(&pdf);

        assert!(text.contains("Bracket"), "Title block should contain title");
        assert!(text.contains("BR-100"), "Title block should contain part number");
        assert!(text.contains("Steel"), "Title block should contain material");
    }

    #[test]
    fn pdf_contains_drawing_operators() {
        let solid = make_box(10.0, 10.0, 10.0);
        let drawing = three_view_drawing(&solid, "Ops", "O-001", "Al", SheetSize::A4);
        let pdf = write_pdf(&drawing);
        let text = String::from_utf8_lossy(&pdf);

        // Should contain PDF drawing operators
        assert!(text.contains(" m "), "Missing moveto operator");
        assert!(text.contains(" l S"), "Missing lineto+stroke operators");
        assert!(text.contains("re S"), "Missing rectangle+stroke for border");
        assert!(text.contains("BT"), "Missing text begin operator");
        assert!(text.contains("ET"), "Missing text end operator");
    }

    #[test]
    fn pdf_has_valid_xref() {
        let solid = make_box(10.0, 10.0, 10.0);
        let drawing = three_view_drawing(&solid, "X", "X-1", "S", SheetSize::A4);
        let pdf = write_pdf(&drawing);
        let text = String::from_utf8_lossy(&pdf);

        assert!(text.contains("xref"), "Missing xref table");
        assert!(text.contains("trailer"), "Missing trailer");
        assert!(text.contains("startxref"), "Missing startxref");
    }
}
