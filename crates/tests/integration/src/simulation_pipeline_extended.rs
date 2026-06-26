//! Extended simulation pipeline tests — Design → Mesh → FEA/Topology/CFD → Results.
//!
//! Exercises cross-crate pipelines: parametric modeling → tessellation/FEA →
//! topology optimization → iso-surface extraction, and analytical CFD pipe flow.

use glam::DVec3;
use physical_brep::builder::{make_box, make_cylinder};
use physical_fea::{tetrahedralize, solve, BC};
use physical_topology::{TopologyProblem, Load, Support, optimize, extract_iso_surface};
use physical_cfd::{pipe_flow, FlowRegime};

/// Create a cantilever beam (box) → tessellate → run FEA with load → verify non-zero displacement.
#[test]
fn cantilever_beam_nonzero_displacement() {
    let beam = make_box(80.0, 8.0, 8.0);
    let mesh = tetrahedralize(&beam);

    assert!(mesh.nodes.len() > 8, "mesh should have interior nodes");
    assert!(!mesh.elements.is_empty(), "mesh should have tet elements");

    // Fix left end (x < -35), load right end (x > 35) downward
    let mut bcs = Vec::new();
    for (i, node) in mesh.nodes.iter().enumerate() {
        if node.position.x < -35.0 {
            bcs.push(BC::FixAll(i));
        } else if node.position.x > 35.0 {
            bcs.push(BC::Force(i, DVec3::new(0.0, -20.0, 0.0)));
        }
    }

    // Steel: E = 200 GPa, nu = 0.3
    let result = solve(&mesh, 200_000.0, 0.3, &bcs);

    assert!(
        result.max_displacement > 0.0,
        "cantilever should have non-zero displacement"
    );
    assert!(
        result.max_von_mises > 0.0,
        "cantilever should have non-zero stress"
    );

    // Displacement should be physically small for steel
    assert!(
        result.max_displacement < 10.0,
        "displacement {:.4}mm should be bounded for steel beam",
        result.max_displacement
    );
}

/// Create a 2D topology optimization problem → run SIMP → extract iso-surface → verify mesh is non-empty.
#[test]
fn topology_optimization_produces_mesh() {
    // 20×10 grid, 1mm elements
    let mut problem = TopologyProblem::new_2d(20, 10, 1.0);
    problem.volume_fraction = 0.5;
    problem.e_modulus = 1.0;
    problem.poisson = 0.3;

    // Cantilever: fix left edge, load bottom-right corner
    for iy in 0..=problem.ny {
        for iz in 0..=problem.nz {
            let node_idx = problem.node_index(0, iy, iz);
            problem.supports.push(Support {
                node: node_idx,
                fix_x: true,
                fix_y: true,
                fix_z: true,
            });
        }
    }

    // Apply downward load at bottom-right corner
    let load_node = problem.node_index(problem.nx, 0, 0);
    problem.loads.push(Load {
        node: load_node,
        force: DVec3::new(0.0, -1.0, 0.0),
    });

    // Run optimization (small number of iterations for test speed)
    let result = optimize(&problem, 30, 1.5);

    assert!(
        result.converged || result.iterations >= 5,
        "optimizer should run at least a few iterations"
    );
    assert!(
        !result.densities.is_empty(),
        "density field should not be empty"
    );

    // Volume fraction should be near target
    let vf = result.volume_fraction();
    assert!(
        vf > 0.1 && vf < 0.9,
        "volume fraction {vf:.3} should be between 0.1 and 0.9"
    );

    // Extract iso-surface
    let iso_mesh = extract_iso_surface(&problem, &result, 0.5);

    assert!(
        !iso_mesh.vertices.is_empty(),
        "iso-surface should have vertices"
    );
    assert!(
        !iso_mesh.indices.is_empty(),
        "iso-surface should have triangle indices"
    );
    assert!(
        iso_mesh.indices.len() % 3 == 0,
        "indices should be a multiple of 3 (triangles)"
    );
}

/// Create a pipe geometry → run CFD pipe flow → verify Reynolds number and pressure drop.
#[test]
fn pipe_flow_reynolds_and_pressure_drop() {
    // 25mm diameter pipe, 1m long, water at 1 m/s
    let diameter = 0.025; // m
    let length = 1.0; // m
    let velocity = 1.0; // m/s
    let water_density = 998.0; // kg/m³
    let water_viscosity = 0.001; // Pa·s (dynamic viscosity at 20°C)
    let roughness = 0.000045; // m (commercial steel)

    let result = pipe_flow(diameter, length, velocity, water_density, water_viscosity, roughness);

    // Re = ρvD/μ = 998 × 1.0 × 0.025 / 0.001 = 24950
    assert!(
        (result.reynolds - 24_950.0).abs() / 24_950.0 < 0.01,
        "Reynolds number {:.0} should be ~24950",
        result.reynolds
    );

    // Should be turbulent (Re > 4000)
    assert_eq!(
        result.regime,
        FlowRegime::Turbulent,
        "Re={:.0} should be turbulent",
        result.reynolds
    );

    // Pressure drop should be positive
    assert!(
        result.pressure_drop_pa > 0.0,
        "pressure drop should be positive"
    );

    // Friction factor should be reasonable for turbulent flow
    assert!(
        result.friction_factor > 0.01 && result.friction_factor < 0.1,
        "friction factor {:.4} should be in typical range",
        result.friction_factor
    );

    // Flow rate should match velocity × area
    let area = std::f64::consts::PI * diameter * diameter / 4.0;
    let expected_flow = velocity * area;
    assert!(
        (result.flow_rate_m3_s - expected_flow).abs() / expected_flow < 0.01,
        "flow rate {:.6} m³/s should be ~{:.6}",
        result.flow_rate_m3_s, expected_flow
    );
}

/// Laminar pipe flow: low velocity → verify laminar regime and f = 64/Re.
#[test]
fn laminar_pipe_flow() {
    // Very low velocity to ensure laminar flow
    let diameter = 0.01; // 10mm
    let length = 0.5;
    let velocity = 0.1; // m/s
    let density_fluid = 998.0;
    let viscosity = 0.001;
    let roughness = 0.0; // smooth

    let result = pipe_flow(diameter, length, velocity, density_fluid, viscosity, roughness);

    // Re = 998 × 0.1 × 0.01 / 0.001 = 998 (laminar)
    assert!(
        result.reynolds < 2300.0,
        "Re={:.0} should be laminar (< 2300)",
        result.reynolds
    );
    assert_eq!(result.regime, FlowRegime::Laminar);

    // For laminar flow, f = 64/Re
    let expected_f = 64.0 / result.reynolds;
    assert!(
        (result.friction_factor - expected_f).abs() / expected_f < 0.01,
        "laminar friction factor {:.4} should be 64/Re = {:.4}",
        result.friction_factor, expected_f
    );
}
