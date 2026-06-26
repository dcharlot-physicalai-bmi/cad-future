//! Export pipeline tests — Design → Export to multiple formats.
//!
//! Verifies that a single designed part can be exported to STEP AP203,
//! STEP AP214, 3MF, GLB, DXF, SVG, and PDF with correct format markers.

use physical_brep::builder::make_box;
use physical_tessellation::tessellate;

/// Create a box → export to STEP AP203 → verify contains CARTESIAN_POINT.
#[test]
fn step_ap203_contains_cartesian_point() {
    let solid = make_box(30.0, 20.0, 10.0);
    let step = physical_emit_step::write_step_ap203(&solid, "TestBox");

    assert!(
        step.contains("CARTESIAN_POINT"),
        "AP203 STEP should contain CARTESIAN_POINT entities"
    );
    assert!(
        step.contains("MANIFOLD_SOLID_BREP"),
        "AP203 STEP should contain MANIFOLD_SOLID_BREP"
    );
    assert!(
        step.contains("ADVANCED_FACE"),
        "AP203 STEP should contain ADVANCED_FACE"
    );
}

/// Same box → export to STEP AP214 → verify contains COLOUR_RGB.
#[test]
fn step_ap214_contains_colour_rgb() {
    let solid = make_box(30.0, 20.0, 10.0);

    let face_colors = vec![physical_emit_step::FaceColor {
        face_index: 0,
        color: physical_emit_step::Color::silver(),
    }];

    let step = physical_emit_step::write_step_ap214(&solid, "ColorBox", &face_colors);

    assert!(
        step.contains("COLOUR_RGB"),
        "AP214 STEP with face colors should contain COLOUR_RGB"
    );
    assert!(
        step.contains("CARTESIAN_POINT"),
        "AP214 STEP should also contain CARTESIAN_POINT"
    );
    assert!(
        step.contains("STYLED_ITEM") || step.contains("SURFACE_STYLE"),
        "AP214 STEP should contain styling entities"
    );
}

/// Same box → tessellate → export to 3MF → verify ZIP structure.
#[test]
fn box_to_3mf_zip_structure() {
    let solid = make_box(30.0, 20.0, 10.0);
    let mesh = tessellate(&solid, 0.1);
    let bytes = physical_emit_threemf::write_3mf(&mesh, "TestBox");

    // ZIP files start with PK\x03\x04
    assert_eq!(
        &bytes[0..4],
        &[0x50, 0x4b, 0x03, 0x04],
        "3MF should start with ZIP local header signature (PK\\x03\\x04)"
    );

    // Should contain end-of-central-directory signature
    let eocd_sig = [0x50_u8, 0x4b, 0x05, 0x06];
    assert!(
        bytes.windows(4).any(|w| w == eocd_sig),
        "3MF should contain ZIP end-of-central-directory record"
    );

    // File names should be present in the archive
    let content = String::from_utf8_lossy(&bytes);
    assert!(content.contains("3D/3dmodel.model"), "3MF should contain model file");
    assert!(content.contains("[Content_Types].xml"), "3MF should contain content types");
}

/// Same box → tessellate → export to GLB → verify GLB magic bytes.
#[test]
fn box_to_glb_magic_bytes() {
    let solid = make_box(30.0, 20.0, 10.0);
    let mesh = tessellate(&solid, 0.1);
    let glb = physical_emit_gltf::write_glb(&mesh, "TestBox");

    // GLB starts with "glTF" magic (0x46546C67 little-endian)
    assert!(glb.len() >= 12, "GLB should have at least 12 bytes header");
    let magic = u32::from_le_bytes(glb[0..4].try_into().unwrap());
    assert_eq!(magic, 0x46546C67, "GLB should start with 'glTF' magic");

    // Version should be 2
    let version = u32::from_le_bytes(glb[4..8].try_into().unwrap());
    assert_eq!(version, 2, "GLB version should be 2");

    // Total length should match actual data
    let total_len = u32::from_le_bytes(glb[8..12].try_into().unwrap());
    assert_eq!(total_len as usize, glb.len(), "GLB length field should match data length");
}

/// Same box → export to DXF 3D → verify LINE entities.
#[test]
fn box_to_dxf_3d_line_entities() {
    let solid = make_box(30.0, 20.0, 10.0);
    let dxf = physical_emit_dxf::write_dxf_3d(&solid);

    assert!(dxf.contains("LINE"), "DXF 3D should contain LINE entities");
    assert!(dxf.contains("AC1009"), "DXF should contain ACADVER header");
    assert!(dxf.contains("EOF"), "DXF should end with EOF marker");

    // A box has 12 edges → 12 LINE entities
    let line_count = dxf.lines().filter(|l| l.trim() == "LINE").count();
    assert_eq!(
        line_count, 12,
        "box DXF should have 12 LINE entities (12 edges), got {line_count}"
    );
}

/// Create drawing → export SVG → verify SVG tags.
#[test]
fn drawing_to_svg_tags() {
    let solid = make_box(50.0, 30.0, 20.0);

    let view = physical_emit_drawing::project_view(
        &solid,
        physical_emit_drawing::ViewDirection::Front,
        1.0,
    );

    let drawing = physical_emit_drawing::Drawing {
        sheet_size: physical_emit_drawing::SheetSize::A4,
        views: vec![view],
        title_block: physical_emit_drawing::TitleBlock::new("Test Part", "TP-001"),
        border_margin_mm: 10.0,
    };

    let svg = physical_emit_drawing::write_svg(&drawing);

    assert!(svg.contains("<svg"), "SVG output should contain <svg tag");
    assert!(svg.contains("</svg>"), "SVG output should contain closing </svg> tag");
    assert!(
        svg.contains("xmlns"),
        "SVG should declare xmlns namespace"
    );
}

/// Create drawing → export PDF → verify %PDF header.
#[test]
fn drawing_to_pdf_header() {
    let solid = make_box(50.0, 30.0, 20.0);

    let view = physical_emit_drawing::project_view(
        &solid,
        physical_emit_drawing::ViewDirection::Top,
        1.0,
    );

    let drawing = physical_emit_drawing::Drawing {
        sheet_size: physical_emit_drawing::SheetSize::A4,
        views: vec![view],
        title_block: physical_emit_drawing::TitleBlock::new("Test Part", "TP-002"),
        border_margin_mm: 10.0,
    };

    let pdf_bytes = physical_emit_drawing::write_pdf(&drawing);

    assert!(pdf_bytes.len() > 20, "PDF should be non-trivial");
    let header = String::from_utf8_lossy(&pdf_bytes[..5]);
    assert_eq!(
        header, "%PDF-",
        "PDF should start with %PDF- header"
    );
}

/// Multiple exports from a single box should all succeed.
#[test]
fn single_box_all_formats() {
    let solid = make_box(25.0, 15.0, 10.0);
    let mesh = tessellate(&solid, 0.5);

    // STEP AP203
    let step203 = physical_emit_step::write_step_ap203(&solid, "MultiExport");
    assert!(step203.contains("CARTESIAN_POINT"));

    // STEP AP214
    let step214 = physical_emit_step::write_step_ap214(&solid, "MultiExport", &[]);
    assert!(step214.contains("CARTESIAN_POINT"));

    // 3MF
    let threemf = physical_emit_threemf::write_3mf(&mesh, "MultiExport");
    assert_eq!(&threemf[0..4], &[0x50, 0x4b, 0x03, 0x04]);

    // GLB
    let glb = physical_emit_gltf::write_glb(&mesh, "MultiExport");
    let magic = u32::from_le_bytes(glb[0..4].try_into().unwrap());
    assert_eq!(magic, 0x46546C67);

    // DXF 3D
    let dxf = physical_emit_dxf::write_dxf_3d(&solid);
    assert!(dxf.contains("LINE"));
}
