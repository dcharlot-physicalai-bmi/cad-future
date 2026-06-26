//! Geometry accuracy tests — verify that B-Rep operations produce
//! geometrically correct solids with known analytical properties.

use glam::DVec3;
use physical_brep::builder::{make_box, make_cylinder};
use physical_brep::{Profile, Solid};
use physical_analytical::mass_properties;

const TAU: f64 = std::f64::consts::TAU;
const PI: f64 = std::f64::consts::PI;

/// Box volume should equal w × h × d to within tessellation tolerance.
#[test]
fn box_volume_exact() {
    for (w, h, d) in [(10.0, 20.0, 30.0), (1.0, 1.0, 1.0), (100.0, 0.5, 50.0)] {
        let b = make_box(w, h, d);
        let props = mass_properties(&b);
        let expected = w * h * d;
        let err = (props.volume - expected).abs() / expected;
        assert!(
            err < 0.01,
            "box {w}×{h}×{d}: volume {:.2} expected {:.2} (err {:.1}%)",
            props.volume, expected, err * 100.0
        );
    }
}

/// Box surface area = 2(wh + wd + hd).
#[test]
fn box_surface_area() {
    let (w, h, d) = (20.0, 30.0, 10.0);
    let b = make_box(w, h, d);
    let props = mass_properties(&b);
    let expected = 2.0 * (w * h + w * d + h * d);
    let err = (props.surface_area - expected).abs() / expected;
    assert!(
        err < 0.01,
        "box surface area {:.2} expected {:.2} (err {:.1}%)",
        props.surface_area, expected, err * 100.0
    );
}

/// Cylinder volume = π r² h. Tolerance depends on segment count.
#[test]
fn cylinder_volume_convergence() {
    let r = 10.0;
    let h = 50.0;
    let expected = PI * r * r * h;

    // Low segment count — coarse approximation
    let c16 = make_cylinder(r, h, 16);
    let v16 = mass_properties(&c16).volume;

    // High segment count — better approximation
    let c64 = make_cylinder(r, h, 64);
    let v64 = mass_properties(&c64).volume;

    let err16 = (v16 - expected).abs() / expected;
    let err64 = (v64 - expected).abs() / expected;

    // More segments should converge toward true volume
    assert!(
        err64 < err16,
        "64-segment cylinder (err {:.2}%) should be more accurate than 16-segment ({:.2}%)",
        err64 * 100.0, err16 * 100.0
    );
    assert!(
        err64 < 0.05,
        "64-segment cylinder volume {:.1} should be within 5% of {:.1} (err {:.2}%)",
        v64, expected, err64 * 100.0
    );
}

/// Boolean subtraction: volume(A - B) = volume(A) - volume(intersection).
/// For a box with a smaller concentric box subtracted, result should be
/// the difference of volumes.
#[test]
fn boolean_subtract_volume() {
    let outer = make_box(20.0, 20.0, 20.0);
    let inner = make_box(10.0, 10.0, 10.0);
    let result = physical_brep::boolean::subtract(&outer, &inner);

    let outer_vol = mass_properties(&outer).volume;
    let inner_vol = mass_properties(&inner).volume;
    let result_vol = mass_properties(&result).volume;

    let expected = outer_vol - inner_vol;
    let err = (result_vol - expected).abs() / expected;
    assert!(
        err < 0.15,
        "subtract volume {:.1} expected ~{:.1} (err {:.1}%)",
        result_vol, expected, err * 100.0
    );
}

/// Union of two non-overlapping boxes should have combined volume.
#[test]
fn boolean_union_volume_additive() {
    let a = make_box(10.0, 10.0, 10.0);
    // B is offset so they don't overlap
    let mut b = make_box(10.0, 10.0, 10.0);
    for (_, v) in &mut b.vertices {
        v.point.x += 20.0;
    }

    let combined = physical_brep::boolean::union(&a, &b);
    let va = mass_properties(&a).volume;
    let vb = mass_properties(&b).volume;
    let vc = mass_properties(&combined).volume;

    let expected = va + vb;
    let err = (vc - expected).abs() / expected;
    assert!(
        err < 0.10,
        "union volume {:.1} expected ~{:.1} (err {:.1}%)",
        vc, expected, err * 100.0
    );
}

/// Extrusion of a rectangle should produce a box with known volume.
#[test]
fn extrusion_volume_matches_analytical() {
    let profile = Profile::rectangle(15.0, 8.0);
    let solid = physical_brep::extrude::extrude_z(&profile, 25.0);
    let props = mass_properties(&solid);

    let expected = 15.0 * 8.0 * 25.0;
    let err = (props.volume - expected).abs() / expected;
    assert!(
        err < 0.01,
        "extrusion volume {:.1} expected {:.1}",
        props.volume, expected
    );

    assert!(solid.is_valid_shell(), "extrusion should be watertight");
}

/// A swept rectangle along a straight line should match extrusion volume.
#[test]
fn sweep_straight_matches_extrusion() {
    let profile = Profile::rectangle(6.0, 4.0);
    let path = physical_brep::Curve::line(DVec3::ZERO, DVec3::new(0.0, 0.0, 20.0));
    let swept = physical_brep::sweep::sweep(&profile, &path, 4);

    let extruded = physical_brep::extrude::extrude_z(&profile, 20.0);

    let v_sweep = mass_properties(&swept).volume;
    let v_extrude = mass_properties(&extruded).volume;

    let err = (v_sweep - v_extrude).abs() / v_extrude;
    assert!(
        err < 0.05,
        "sweep volume {:.1} should match extrusion {:.1} (err {:.1}%)",
        v_sweep, v_extrude, err * 100.0
    );
}

/// Mirror should produce same volume, reflected bounding box.
#[test]
fn mirror_preserves_volume_reflects_position() {
    let b = make_box(10.0, 10.0, 10.0);
    // Translate box to +X side
    let mut shifted = b.clone();
    for (_, v) in &mut shifted.vertices {
        v.point.x += 20.0;
    }

    let mirrored = physical_brep::pattern::mirror(
        &shifted,
        DVec3::ZERO,
        DVec3::X,
    );

    let v_orig = mass_properties(&shifted).volume;
    let v_mirror = mass_properties(&mirrored).volume;

    let err = (v_mirror - v_orig).abs() / v_orig;
    assert!(
        err < 0.05,
        "mirror volume {:.1} should match original {:.1}",
        v_mirror, v_orig
    );

    // Mirrored bounding box should be on -X side
    let (min, max) = mirrored.bounding_box();
    assert!(
        max.x < 0.0,
        "mirrored box should be on -X side, but max.x={:.1}",
        max.x
    );
}

/// Bounding box of operations should be geometrically consistent.
#[test]
fn bounding_box_consistency() {
    let b = make_box(10.0, 20.0, 30.0);
    let (min, max) = b.bounding_box();

    // Box centered at origin
    assert!((min.x - (-5.0)).abs() < 1e-6);
    assert!((max.x - 5.0).abs() < 1e-6);
    assert!((min.y - (-10.0)).abs() < 1e-6);
    assert!((max.y - 10.0).abs() < 1e-6);
    assert!((min.z - (-15.0)).abs() < 1e-6);
    assert!((max.z - 15.0).abs() < 1e-6);

    // Linear pattern should extend bounding box
    let patterned = physical_brep::pattern::linear_pattern(
        &b, DVec3::X, 20.0, 3,
    );
    let (pmin, pmax) = patterned.bounding_box();
    assert!(
        pmax.x > max.x,
        "pattern should extend past original bbox: {:.1} vs {:.1}",
        pmax.x, max.x
    );
}

/// Topology invariants: every closed solid should satisfy Euler's formula.
#[test]
fn euler_formula_all_primitives() {
    let solids: Vec<(&str, Solid)> = vec![
        ("box", make_box(10.0, 10.0, 10.0)),
        ("cylinder_16", make_cylinder(5.0, 10.0, 16)),
        ("cylinder_32", make_cylinder(5.0, 10.0, 32)),
        ("extruded_L", physical_brep::extrude::extrude_z(
            &Profile::l_shape(20.0, 30.0, 5.0), 10.0
        )),
    ];

    for (name, solid) in &solids {
        assert!(
            solid.is_valid_shell(),
            "{name}: Euler characteristic = {} (expected 2)",
            solid.euler_characteristic()
        );
    }
}

/// Loft between identical cross-sections should approximate extrusion.
#[test]
fn loft_identical_sections_matches_extrusion() {
    let w = 12.0;
    let h = 8.0;
    let d = 25.0;
    let hw = w / 2.0;
    let hh = h / 2.0;

    let bottom = vec![
        [-hw, -hh, 0.0], [hw, -hh, 0.0],
        [hw, hh, 0.0], [-hw, hh, 0.0],
    ];
    let top = vec![
        [-hw, -hh, d], [hw, -hh, d],
        [hw, hh, d], [-hw, hh, d],
    ];

    let lofted = physical_brep::loft::loft(&[
        bottom.iter().map(|p| DVec3::from_array(*p)).collect(),
        top.iter().map(|p| DVec3::from_array(*p)).collect(),
    ]);

    let extruded = physical_brep::extrude::extrude(
        &Profile::rectangle(w, h),
        DVec3::ZERO, DVec3::X, DVec3::Y, DVec3::Z, d,
    );

    let v_loft = mass_properties(&lofted).volume;
    let v_ext = mass_properties(&extruded).volume;

    let err = (v_loft - v_ext).abs() / v_ext;
    assert!(
        err < 0.05,
        "loft volume {:.1} should match extrusion {:.1} (err {:.1}%)",
        v_loft, v_ext, err * 100.0
    );
}
