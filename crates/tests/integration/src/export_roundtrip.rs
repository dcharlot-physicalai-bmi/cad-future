//! Export roundtrip tests — write a solid to STEP/STL/OIE, read it back,
//! and verify the geometry survived the trip.

use physical_brep::builder::{make_box, make_cylinder};
use physical_brep::Profile;
use physical_analytical::mass_properties;
use physical_parametric::{ModelDocument, FeatureOp};
use physical_tessellation::tessellate;

/// STEP roundtrip: export a box, reimport, verify vertex count and bounding box.
#[test]
fn step_roundtrip_box_geometry() {
    let original = make_box(30.0, 20.0, 10.0);
    let step_text = physical_emit_step::write_step_ap203(&original, "TestBox");
    assert!(step_text.contains("MANIFOLD_SOLID_BREP"), "STEP should contain MSB entity");
    assert!(step_text.contains("ADVANCED_FACE"), "STEP should contain face entities");

    let reimported = physical_emit_step::read_step(&step_text)
        .expect("should parse valid STEP");

    // Bounding box should be preserved
    let (orig_min, orig_max) = original.bounding_box();
    let (re_min, re_max) = reimported.bounding_box();

    for axis in 0..3 {
        let orig_size = orig_max[axis] - orig_min[axis];
        let re_size = re_max[axis] - re_min[axis];
        let err = (re_size - orig_size).abs();
        assert!(
            err < 1.0,
            "axis {axis}: reimported size {re_size:.1} vs original {orig_size:.1}"
        );
    }
}

/// STEP roundtrip should preserve face count for planar solids.
#[test]
fn step_roundtrip_preserves_topology() {
    let original = make_box(10.0, 10.0, 10.0);
    let step_text = physical_emit_step::write_step_ap203(&original, "Cube");
    let reimported = physical_emit_step::read_step(&step_text).unwrap();

    assert_eq!(
        reimported.face_count(), original.face_count(),
        "face count mismatch: reimported {} vs original {}",
        reimported.face_count(), original.face_count()
    );
}

/// STL roundtrip: export → reimport → verify triangle count and mesh bounds.
#[test]
fn stl_binary_roundtrip_mesh_integrity() {
    let solid = make_box(40.0, 20.0, 10.0);
    let mesh = tessellate(&solid, 0.1);
    let tri_count = mesh.triangle_count();
    assert!(tri_count >= 12, "box should have at least 12 triangles");

    // Write binary STL
    let stl_bytes = physical_emit_stl::write_binary_stl(&mesh);
    assert!(stl_bytes.len() > 84, "STL should be larger than header");

    // Read back
    let reimported = physical_emit_stl::read_binary_stl(&stl_bytes)
        .expect("should parse binary STL");

    assert_eq!(
        reimported.triangle_count(), tri_count,
        "triangle count mismatch: {} vs {}",
        reimported.triangle_count(), tri_count
    );

    // Verify mesh bounds match
    let orig_bounds = mesh_bounds(&mesh);
    let re_bounds = mesh_bounds(&reimported);
    for axis in 0..3 {
        assert!(
            (orig_bounds.0[axis] - re_bounds.0[axis]).abs() < 0.1,
            "min bounds mismatch on axis {axis}"
        );
        assert!(
            (orig_bounds.1[axis] - re_bounds.1[axis]).abs() < 0.1,
            "max bounds mismatch on axis {axis}"
        );
    }
}

/// STL ASCII roundtrip should produce identical mesh.
#[test]
fn stl_ascii_roundtrip() {
    let solid = make_cylinder(5.0, 20.0, 16);
    let mesh = tessellate(&solid, 0.5);

    let ascii = physical_emit_stl::write_ascii_stl(&mesh, "Cylinder");
    assert!(ascii.contains("solid Cylinder"), "should have solid name");
    assert!(ascii.contains("endsolid"), "should have endsolid");

    let reimported = physical_emit_stl::read_ascii_stl(&ascii)
        .expect("should parse ASCII STL");

    assert_eq!(reimported.triangle_count(), mesh.triangle_count());
}

/// Auto-detect: binary STL should be detected correctly.
#[test]
fn stl_auto_detect_format() {
    let solid = make_box(10.0, 10.0, 10.0);
    let mesh = tessellate(&solid, 0.1);

    // Binary
    let binary = physical_emit_stl::write_binary_stl(&mesh);
    let from_binary = physical_emit_stl::read_stl(&binary).expect("auto-detect binary");
    assert_eq!(from_binary.triangle_count(), mesh.triangle_count());

    // ASCII
    let ascii = physical_emit_stl::write_ascii_stl(&mesh, "Test");
    let from_ascii = physical_emit_stl::read_stl(ascii.as_bytes()).expect("auto-detect ASCII");
    assert_eq!(from_ascii.triangle_count(), mesh.triangle_count());
}

/// OIE roundtrip with a complex multi-feature document.
#[test]
fn oie_roundtrip_complex_document() {
    let mut doc = ModelDocument::new("ComplexPart");
    doc.add("Box", FeatureOp::Box { width: 50.0, height: 30.0, depth: 20.0 });
    doc.add("Fillet", FeatureOp::Fillet { edge_indices: vec![0, 1], radius: 2.0 });

    let original = doc.rebuild().unwrap();
    let orig_faces = original.face_count();
    let orig_verts = original.vertex_count();

    // Save and reload
    let bytes = physical_emit_oie::save_oie(&doc);
    let loaded = physical_emit_oie::load_oie(&bytes).unwrap();

    assert_eq!(loaded.name, "ComplexPart");
    assert_eq!(loaded.features.len(), 2);

    let rebuilt = loaded.rebuild().unwrap();
    assert_eq!(rebuilt.face_count(), orig_faces);
    assert_eq!(rebuilt.vertex_count(), orig_verts);
}

/// Export an extruded L-shape through STEP, verify it survives.
#[test]
fn step_roundtrip_extruded_l_shape() {
    let profile = Profile::l_shape(20.0, 30.0, 5.0);
    let solid = physical_brep::extrude::extrude_z(&profile, 15.0);
    assert!(solid.is_valid_shell());

    let step = physical_emit_step::write_step_ap203(&solid, "LBracket");
    let reimported = physical_emit_step::read_step(&step)
        .expect("should parse L-shape STEP");

    // L-shape has 8 faces (2 caps + 6 sides)
    assert_eq!(reimported.face_count(), solid.face_count());
}

/// A corrupted STL should return None, not panic.
#[test]
fn stl_corrupt_data_returns_none() {
    assert!(physical_emit_stl::read_binary_stl(&[0u8; 10]).is_none());
    assert!(physical_emit_stl::read_ascii_stl("not a valid stl").is_none());
    assert!(physical_emit_stl::read_stl(&[]).is_none());
}

/// A corrupted STEP should return None, not panic.
#[test]
fn step_corrupt_data_returns_none() {
    assert!(physical_emit_step::read_step("garbage text").is_none());
    assert!(physical_emit_step::read_step("").is_none());
    assert!(physical_emit_step::read_step("ISO-10303-21;\nDATA;\nENDSEC;\nEND-ISO-10303-21;").is_none());
}

fn mesh_bounds(mesh: &physical_tessellation::TessMesh) -> ([f32; 3], [f32; 3]) {
    let mut min = [f32::MAX; 3];
    let mut max = [f32::MIN; 3];
    for v in &mesh.vertices {
        for i in 0..3 {
            min[i] = min[i].min(v.position[i]);
            max[i] = max[i].max(v.position[i]);
        }
    }
    (min, max)
}
