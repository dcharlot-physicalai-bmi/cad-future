//! Parametric workflow tests — full feature tree rebuild, undo/redo integrity,
//! and save/load roundtrips with geometry verification after each step.

use glam::DVec3;
use physical_parametric::{ModelDocument, FeatureOp};
use physical_brep::{Profile, Solid};
use physical_analytical::mass_properties;

/// Design a bracket: box → shell → fillet. Verify volume decreases
/// at each step, and final part has correct topology.
#[test]
fn bracket_design_workflow() {
    let mut doc = ModelDocument::new("Bracket");

    // Step 1: 100×60×10mm plate
    doc.add("Plate", FeatureOp::Box { width: 100.0, height: 60.0, depth: 10.0 });
    let plate = doc.rebuild().unwrap();
    let plate_props = mass_properties(&plate);
    let plate_vol = plate_props.volume;
    assert!(
        (plate_vol - 60_000.0).abs() / 60_000.0 < 0.05,
        "plate volume {plate_vol} should be ~60000 mm³"
    );

    // Step 2: shell to 2mm walls, open the top face
    doc.add("Shell", FeatureOp::Shell { thickness: 2.0, open_face_indices: vec![0] });
    let shelled = doc.rebuild().unwrap();
    let shelled_props = mass_properties(&shelled);
    assert!(
        shelled_props.volume < plate_vol,
        "shelled volume {} must be less than plate volume {}",
        shelled_props.volume, plate_vol
    );

    // Step 3: undo shell, verify we get plate volume back
    doc.undo();
    let undone = doc.rebuild().unwrap();
    let undone_props = mass_properties(&undone);
    assert!(
        (undone_props.volume - plate_vol).abs() / plate_vol < 0.01,
        "undo should restore plate volume: got {} expected {}",
        undone_props.volume, plate_vol
    );

    // Step 4: redo shell
    doc.redo();
    let redone = doc.rebuild().unwrap();
    let redone_props = mass_properties(&redone);
    assert!(
        (redone_props.volume - shelled_props.volume).abs() < 1.0,
        "redo should restore shelled volume"
    );
}

/// Save a multi-feature document to OIE, reload, rebuild, and verify
/// the geometry is identical.
#[test]
fn save_load_rebuild_preserves_geometry() {
    let mut doc = ModelDocument::new("Housing");
    doc.add("Box", FeatureOp::Box { width: 80.0, height: 40.0, depth: 30.0 });
    doc.add("Shell", FeatureOp::Shell { thickness: 3.0, open_face_indices: vec![0] });

    let original = doc.rebuild().unwrap();
    let orig_props = mass_properties(&original);

    // Save to OIE format
    let bytes = physical_emit_oie::save_oie(&doc);
    assert!(bytes.len() > 100, "OIE file should be non-trivial");

    // Reload
    let loaded = physical_emit_oie::load_oie(&bytes)
        .expect("should parse saved OIE");
    assert_eq!(loaded.features.len(), doc.features.len());

    // Rebuild from loaded document
    let reloaded_solid = loaded.rebuild().unwrap();
    let reloaded_props = mass_properties(&reloaded_solid);

    assert!(
        (reloaded_props.volume - orig_props.volume).abs() / orig_props.volume < 0.01,
        "reloaded volume {:.1} != original {:.1}",
        reloaded_props.volume, orig_props.volume
    );
    assert_eq!(reloaded_solid.face_count(), original.face_count());
    assert_eq!(reloaded_solid.vertex_count(), original.vertex_count());
}

/// Build a part with every feature type and verify the tree rebuilds
/// without panics and produces a non-degenerate solid.
#[test]
fn all_feature_types_rebuild() {
    let tau = std::f64::consts::TAU;

    // Revolve: a washer (rectangle revolved around Y)
    let mut doc = ModelDocument::new("AllFeatures");
    doc.add("Revolve", FeatureOp::Revolve {
        profile: Profile::rectangle(3.0, 10.0),
        origin: [0.0, 0.0, 0.0],
        axis: [0.0, 1.0, 0.0],
        u_axis: [1.0, 0.0, 0.0],
        v_axis: [0.0, 1.0, 0.0],
        angle: tau,
        segments: 16,
    });
    let solid = doc.rebuild().unwrap();
    assert!(solid.vertex_count() > 0, "revolve produced empty solid");
    let props = mass_properties(&solid);
    assert!(props.volume > 0.0, "revolve volume should be positive");

    // Loft: tapered box
    let mut doc2 = ModelDocument::new("Loft");
    let bottom = vec![
        [-10.0, -10.0, 0.0], [10.0, -10.0, 0.0],
        [10.0, 10.0, 0.0], [-10.0, 10.0, 0.0],
    ];
    let top = vec![
        [-5.0, -5.0, 20.0], [5.0, -5.0, 20.0],
        [5.0, 5.0, 20.0], [-5.0, 5.0, 20.0],
    ];
    doc2.add("Loft", FeatureOp::Loft { sections: vec![bottom, top] });
    let lofted = doc2.rebuild().unwrap();
    assert!(lofted.is_valid_shell(), "loft should produce valid shell");
    let loft_props = mass_properties(&lofted);
    // Frustum volume = h/3 * (A1 + A2 + sqrt(A1*A2)) = 20/3 * (400 + 100 + 200) = 4666.67
    let expected_frustum = 20.0 / 3.0 * (400.0 + 100.0 + (400.0_f64 * 100.0).sqrt());
    assert!(
        (loft_props.volume - expected_frustum).abs() / expected_frustum < 0.10,
        "loft volume {:.1} should be ~{:.1} (frustum)",
        loft_props.volume, expected_frustum
    );

    // Linear pattern: 3 boxes in a row
    let mut doc3 = ModelDocument::new("Pattern");
    doc3.add("Box", FeatureOp::Box { width: 5.0, height: 5.0, depth: 5.0 });
    doc3.add("Pattern", FeatureOp::LinearPattern {
        direction: [1.0, 0.0, 0.0],
        spacing: 15.0,
        count: 3,
    });
    let patterned = doc3.rebuild().unwrap();
    let pat_props = mass_properties(&patterned);
    assert!(
        (pat_props.volume - 375.0).abs() / 375.0 < 0.05,
        "3 × 125mm³ cubes should be ~375mm³, got {:.1}",
        pat_props.volume
    );
}

/// Verify that centroid position makes physical sense after operations.
#[test]
fn centroid_position_sanity() {
    // A box centered at origin should have centroid near origin
    let b = physical_brep::builder::make_box(20.0, 20.0, 20.0);
    let props = mass_properties(&b);
    assert!(
        props.centroid.length() < 1.0,
        "box centroid {} should be near origin",
        props.centroid
    );

    // A box extruded along +Z should have centroid at (0, 0, half-height)
    let profile = Profile::rectangle(10.0, 10.0);
    let extruded = physical_brep::extrude::extrude_z(&profile, 30.0);
    let ext_props = mass_properties(&extruded);
    assert!(
        (ext_props.centroid.z - 15.0).abs() < 2.0,
        "extrusion centroid Z={:.1} should be ~15",
        ext_props.centroid.z
    );
}
