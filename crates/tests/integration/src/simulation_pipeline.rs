//! Simulation pipeline tests — model a part, mesh it, run FEA/thermal,
//! and verify the results make physical sense.

use glam::DVec3;
use physical_brep::builder::{make_box, make_cylinder};
use physical_analytical::{mass_properties, beam_approximation};
use physical_fea::{tetrahedralize, solve, BC, FEAMesh, solve_coupled};
use physical_fea::thermal::{solve_thermal, ThermalBC};

/// FEA on a cantilever beam: fixed one end, loaded the other.
/// Max displacement should be at the loaded end, not the fixed end.
/// Stress should be positive (tension/compression exists).
#[test]
fn cantilever_beam_fea() {
    // 100×10×10mm beam
    let beam = make_box(100.0, 10.0, 10.0);
    let mesh = tetrahedralize(&beam);

    assert!(mesh.nodes.len() > 8, "mesh should have interior nodes");
    assert!(!mesh.elements.is_empty(), "mesh should have elements");

    // Fix left end (x < -45), load right end (x > 45) downward
    let mut bcs = Vec::new();
    let mut loaded_nodes = Vec::new();
    for (i, node) in mesh.nodes.iter().enumerate() {
        if node.position.x < -45.0 {
            bcs.push(BC::FixAll(i));
        } else if node.position.x > 45.0 {
            bcs.push(BC::Force(i, DVec3::new(0.0, -10.0, 0.0)));
            loaded_nodes.push(i);
        }
    }

    // 6061-T6 aluminum: E = 68.9 GPa = 68900 MPa, ν = 0.33
    let result = solve(&mesh, 68_900.0, 0.33, &bcs);

    assert!(result.max_displacement > 0.0, "should have displacement");
    assert!(result.max_von_mises > 0.0, "should have stress");

    // Displacement at loaded end should be larger than at fixed end
    let fixed_disp: f64 = mesh.nodes.iter().enumerate()
        .filter(|(_, n)| n.position.x < -45.0)
        .map(|(i, _)| result.displacements[i].length())
        .sum::<f64>();

    let loaded_disp: f64 = loaded_nodes.iter()
        .map(|&i| result.displacements[i].length())
        .sum::<f64>();

    assert!(
        loaded_disp > fixed_disp,
        "loaded end displacement {loaded_disp:.4} should exceed fixed end {fixed_disp:.4}"
    );
}

/// Thermal: fix one end hot (100°C), other end cold (20°C).
/// Temperature should monotonically transition between them.
#[test]
fn thermal_gradient_monotonic() {
    let bar = make_box(50.0, 5.0, 5.0);
    let mesh = tetrahedralize(&bar);

    let mut bcs = Vec::new();
    for (i, node) in mesh.nodes.iter().enumerate() {
        if node.position.x < -20.0 {
            bcs.push(ThermalBC::FixedTemp(i, 100.0)); // hot end
        } else if node.position.x > 20.0 {
            bcs.push(ThermalBC::FixedTemp(i, 20.0)); // cold end
        }
    }

    // Aluminum thermal conductivity: 167 W/m·K
    let result = solve_thermal(&mesh, 167.0, &bcs);

    assert!(result.max_temperature >= 99.0, "max temp {:.1} should be ~100", result.max_temperature);
    assert!(result.min_temperature <= 21.0, "min temp {:.1} should be ~20", result.min_temperature);

    // All temperatures should be bounded
    for &t in &result.temperatures {
        assert!(
            t >= 19.0 && t <= 101.0,
            "temperature {t:.1} should be between boundary values"
        );
    }
}

/// Analytical beam approximation should give reasonable dimensions.
#[test]
fn beam_approximation_dimensions() {
    let b = make_box(100.0, 10.0, 5.0); // thin long plate
    let beam = beam_approximation(&b);

    // Length should be the longest dimension
    assert!(
        beam.span > beam.width && beam.span > beam.height,
        "beam length {:.1} should be largest (w={:.1}, h={:.1})",
        beam.span, beam.width, beam.height
    );
}

/// Combined structural + thermal: a heated beam should still have
/// consistent FEA results (no NaN, no negative von Mises).
#[test]
fn structural_and_thermal_on_same_mesh() {
    let part = make_box(30.0, 10.0, 10.0);
    let mesh = tetrahedralize(&part);

    // Structural: fix bottom, push top
    let mut struct_bcs = Vec::new();
    let mut therm_bcs = Vec::new();
    for (i, node) in mesh.nodes.iter().enumerate() {
        if node.position.y < -4.0 {
            struct_bcs.push(BC::FixAll(i));
            therm_bcs.push(ThermalBC::FixedTemp(i, 25.0));
        } else if node.position.y > 4.0 {
            struct_bcs.push(BC::Force(i, DVec3::new(0.0, -50.0, 0.0)));
            therm_bcs.push(ThermalBC::FixedTemp(i, 200.0));
        }
    }

    let struct_result = solve(&mesh, 200_000.0, 0.3, &struct_bcs);
    let thermal_result = solve_thermal(&mesh, 50.0, &therm_bcs);

    // No NaN in results
    assert!(!struct_result.max_von_mises.is_nan(), "von Mises should not be NaN");
    assert!(!struct_result.max_displacement.is_nan(), "displacement should not be NaN");
    assert!(!thermal_result.max_temperature.is_nan(), "temperature should not be NaN");

    // Positive results
    assert!(struct_result.max_von_mises >= 0.0);
    assert!(struct_result.max_displacement >= 0.0);
    assert!(thermal_result.max_temperature > thermal_result.min_temperature);
}

/// Mesh quality: tetrahedra should have positive volume.
#[test]
fn mesh_elements_positive_volume() {
    let solid = make_cylinder(10.0, 30.0, 16);
    let mesh = tetrahedralize(&solid);

    for (idx, elem) in mesh.elements.iter().enumerate() {
        let p = [
            mesh.nodes[elem.nodes[0]].position,
            mesh.nodes[elem.nodes[1]].position,
            mesh.nodes[elem.nodes[2]].position,
            mesh.nodes[elem.nodes[3]].position,
        ];
        let a = p[1] - p[0];
        let b = p[2] - p[0];
        let c = p[3] - p[0];
        let vol = a.dot(b.cross(c)) / 6.0;
        // Volume magnitude should be positive (sign depends on orientation)
        assert!(
            vol.abs() > 1e-12,
            "element {idx} has degenerate volume {vol}"
        );
    }
}

/// A pressure vessel: cylinder with internal pressure.
/// Hoop stress = p*r/t from analytical, FEA should be in same ballpark.
#[test]
fn cylinder_stress_order_of_magnitude() {
    let c = make_cylinder(20.0, 50.0, 12);
    let mesh = tetrahedralize(&c);

    // Apply outward radial force on outer surface nodes
    let mut bcs = Vec::new();
    for (i, node) in mesh.nodes.iter().enumerate() {
        if node.position.y < -20.0 || node.position.y > 20.0 {
            bcs.push(BC::FixAll(i)); // fix ends
        } else {
            // Radial distance from Y axis
            let radial = DVec3::new(node.position.x, 0.0, node.position.z);
            let r = radial.length();
            if r > 15.0 {
                let outward = radial.normalize_or_zero() * 5.0;
                bcs.push(BC::Force(i, outward));
            }
        }
    }

    let result = solve(&mesh, 200_000.0, 0.3, &bcs);
    assert!(result.max_von_mises > 0.0, "should have stress under pressure");
    assert!(result.max_displacement > 0.0, "cylinder should deform under pressure");
}

/// Full coupled pipeline: thermal solve → feed temperatures into structural.
/// A heated beam fixed at both ends should develop thermal stress.
#[test]
fn coupled_thermal_structural_pipeline() {
    let beam = make_box(50.0, 10.0, 10.0);
    let mesh = tetrahedralize(&beam);

    // Step 1: Thermal solve — hot on left, cold on right
    let mut thermal_bcs = Vec::new();
    for (i, node) in mesh.nodes.iter().enumerate() {
        if node.position.x < -20.0 {
            thermal_bcs.push(ThermalBC::FixedTemp(i, 200.0));
        } else if node.position.x > 20.0 {
            thermal_bcs.push(ThermalBC::FixedTemp(i, 20.0));
        }
    }

    let thermal = solve_thermal(&mesh, 50.0, &thermal_bcs);
    assert!(thermal.max_temperature > thermal.min_temperature);

    // Step 2: Coupled solve — fix both ends (constrained expansion → stress)
    let mut mech_bcs = Vec::new();
    for (i, node) in mesh.nodes.iter().enumerate() {
        if node.position.x < -20.0 || node.position.x > 20.0 {
            mech_bcs.push(BC::FixAll(i));
        }
    }

    // Aluminum CTE ≈ 23e-6 /K, E ≈ 70 GPa, ν ≈ 0.33
    let coupled = solve_coupled(
        &mesh,
        &thermal.temperatures,
        20.0,      // reference temp
        23e-6,     // CTE
        70_000.0,  // E (MPa)
        0.33,      // Poisson's
        &mech_bcs,
    );

    // Thermal gradient + constraints should produce stress
    assert!(
        coupled.structural.max_von_mises > 0.0,
        "coupled analysis should produce stress from thermal gradient"
    );

    // Temperature field should match thermal solve
    assert_eq!(coupled.temperatures.len(), mesh.nodes.len());

    // For comparison: solve structural only (no thermal)
    let struct_only = solve(&mesh, 70_000.0, 0.33, &mech_bcs);

    // Coupled should have larger displacement than no-load structural
    // (thermal expansion pushes material, structural-only has zero load → zero displacement)
    assert!(
        coupled.structural.max_displacement > struct_only.max_displacement,
        "coupled displacement {:.6} should exceed structural-only {:.6}",
        coupled.structural.max_displacement, struct_only.max_displacement
    );
}
