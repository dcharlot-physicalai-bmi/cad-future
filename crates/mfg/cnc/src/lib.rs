//! CNC milling CAM — solid to G-code pipeline (2.5D and 3D).
//!
//! Pipeline:
//! 1. Define stock (bounding box, block, or cylinder)
//! 2. Plan operations (facing, pocketing, profiling, drilling)
//! 3. Generate toolpaths for each operation
//! 4. Post-process into G-code (Fanuc, LinuxCNC, GRBL, Haas)

pub mod config;
pub mod drill;
pub mod emit;
pub mod facing;
pub mod pocket;
pub mod postprocess;
pub mod profile;
pub mod stock;

pub use config::{CncConfig, StockDefinition};
pub use physical_mfg_toolpath::gcode::GCodeProgram;
pub use physical_mfg_toolpath::Contour;
pub use postprocess::{
    ControllerType, CoolantType, GcodeWarning, MachineConfig, PostProcessedProgram,
    WorkOffset, dialect_from_config, emit_drill_canned, emit_peck_drill_canned,
    emit_tap_canned, estimate_cycle_time, post_process, validate_gcode,
    validate_gcode_with_config,
};

use physical_brep::Solid;
use physical_mfg_toolpath::material::recommended_feeds_speeds;
use physical_mfg_toolpath::path::ToolpathSegment;

/// A CNC operation to perform.
#[derive(Clone, Debug)]
pub enum CncOperation {
    /// Face the top of the stock to a target Z.
    Face { target_z: f64 },
    /// Clear a 2D pocket.
    Pocket {
        contour: Contour,
        top_z: f64,
        bottom_z: f64,
        strategy: pocket::PocketStrategy,
    },
    /// Profile cut along a contour.
    Profile {
        contour: Contour,
        cut_side: profile::CutSide,
        top_z: f64,
        bottom_z: f64,
    },
    /// Drill holes.
    Drill {
        holes: Vec<drill::DrillHole>,
        cycle: drill::DrillCycle,
        top_z: f64,
    },
}

/// Generate G-code from a solid and a list of CNC operations.
pub fn cnc_operations(
    solid: &Solid,
    operations: &[CncOperation],
    config: &CncConfig,
) -> GCodeProgram {
    let stk = stock::Stock::from_config(config, solid);
    let (rpm, feed) = recommended_feeds_speeds(
        &config.material,
        config.tool_diameter(),
        config.tool.geometry.flute_count(),
        config.tool.max_rpm,
    );
    let _ = rpm; // Used in preamble emission

    let mut all_segments: Vec<ToolpathSegment> = Vec::new();

    for op in operations {
        match op {
            CncOperation::Face { target_z } => {
                let segs = facing::generate_facing(
                    &stk,
                    *target_z,
                    config.step_over_mm(),
                    feed,
                    stk.top_z() + config.safe_height,
                );
                all_segments.extend(segs);
            }
            CncOperation::Pocket {
                contour,
                top_z,
                bottom_z,
                strategy,
            } => {
                let segs = pocket::generate_pocket(
                    contour,
                    *top_z,
                    *bottom_z,
                    config.step_down,
                    config.step_over_mm(),
                    config.tool_diameter(),
                    feed,
                    *top_z + config.safe_height,
                    *strategy,
                );
                all_segments.extend(segs);
            }
            CncOperation::Profile {
                contour,
                cut_side,
                top_z,
                bottom_z,
            } => {
                let segs = profile::generate_profile(
                    contour,
                    *cut_side,
                    *top_z,
                    *bottom_z,
                    config.step_down,
                    config.tool_diameter(),
                    feed,
                    *top_z + config.safe_height,
                );
                all_segments.extend(segs);
            }
            CncOperation::Drill {
                holes,
                cycle,
                top_z,
            } => {
                let segs = drill::generate_drill_cycle(
                    holes,
                    *cycle,
                    *top_z,
                    feed * 0.3, // Drill feed = 30% of milling feed
                    *top_z + config.safe_height,
                );
                all_segments.extend(segs);
            }
        }
    }

    emit::emit_cnc_program(&all_segments, config)
}

/// Quick facing operation: face the top of the stock down to part top.
pub fn face_stock(solid: &Solid, config: &CncConfig) -> GCodeProgram {
    let (_part_min, _part_max) = solid.bounding_box();
    let stk = stock::Stock::from_config(config, solid);
    let target_z = stk.top_z() - 0.5; // Face 0.5mm off stock top

    cnc_operations(
        solid,
        &[CncOperation::Face { target_z }],
        config,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::DVec2;
    use physical_brep::builder::make_box;
    use physical_mfg_toolpath::post;

    #[test]
    fn face_stock_produces_gcode() {
        let solid = make_box(50.0, 30.0, 20.0);
        let config = CncConfig::default();
        let gcode = face_stock(&solid, &config);
        let output = gcode.to_string(&post::grbl());

        assert!(output.contains("G1"), "Should have cutting moves");
        assert!(output.contains("M3"), "Should turn spindle on");
        assert!(output.contains("M5"), "Should turn spindle off");
    }

    #[test]
    fn pocket_and_profile() {
        let solid = make_box(50.0, 30.0, 20.0);
        let config = CncConfig::default();

        let pocket_contour = Contour::closed(vec![
            DVec2::new(5.0, 5.0),
            DVec2::new(25.0, 5.0),
            DVec2::new(25.0, 15.0),
            DVec2::new(5.0, 15.0),
        ]);

        let profile_contour = Contour::closed(vec![
            DVec2::new(0.0, 0.0),
            DVec2::new(50.0, 0.0),
            DVec2::new(50.0, 30.0),
            DVec2::new(0.0, 30.0),
        ]);

        let ops = vec![
            CncOperation::Face { target_z: 19.5 },
            CncOperation::Pocket {
                contour: pocket_contour,
                top_z: 19.5,
                bottom_z: 10.0,
                strategy: pocket::PocketStrategy::ContourParallel,
            },
            CncOperation::Profile {
                contour: profile_contour,
                cut_side: profile::CutSide::Outside,
                top_z: 20.0,
                bottom_z: 0.0,
            },
        ];

        let gcode = cnc_operations(&solid, &ops, &config);
        let output = gcode.to_string(&post::grbl());
        assert!(output.contains("G1"));
        assert!(gcode.estimated_time_s(6000.0) > 0.0);
    }

    #[test]
    fn drill_holes() {
        let solid = make_box(50.0, 30.0, 20.0);
        let config = CncConfig::default();

        let holes = vec![
            drill::DrillHole { position: DVec2::new(10.0, 15.0), depth: 15.0 },
            drill::DrillHole { position: DVec2::new(25.0, 15.0), depth: 15.0 },
            drill::DrillHole { position: DVec2::new(40.0, 15.0), depth: 15.0 },
        ];

        let ops = vec![CncOperation::Drill {
            holes,
            cycle: drill::DrillCycle::Peck { peck_depth: 3.0 },
            top_z: 20.0,
        }];

        let gcode = cnc_operations(&solid, &ops, &config);
        let output = gcode.to_string(&post::grbl());
        assert!(output.contains("G1"), "Drill cycles should produce moves");
    }
}
