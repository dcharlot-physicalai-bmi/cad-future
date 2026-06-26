//! Laser cutting and engraving CAM — contours to G-code pipeline.
//!
//! Pipeline:
//! 1. Project 3D solid to 2D contours (or accept 2D contours directly)
//! 2. Apply kerf compensation (offset by half beam width)
//! 3. Order contours (holes first, then outers, minimize travel)
//! 4. Optionally add tabs (hold-downs)
//! 5. Emit G-code (GRBL dialect)

pub mod config;
pub mod emit;
pub mod kerf;
pub mod nesting;
pub mod ordering;
pub mod project;
pub mod tabs;

pub use config::LaserConfig;
pub use nesting::{
    CutPath, PartOutline, Point2D, SimpleNestResult,
    apply_kerf_compensation, nest_part_outlines,
    optimize_cut_order_paths, optimize_heat_distribution,
    total_travel_distance,
};
pub use physical_mfg_toolpath::gcode::GCodeProgram;
pub use physical_mfg_toolpath::Contour;

/// Generate laser cutting G-code from 2D contours.
///
/// Main entry point for the laser CAM pipeline.
pub fn laser_cut(contours: &[Contour], config: &LaserConfig) -> GCodeProgram {
    let mut prog = GCodeProgram::new();
    prog.header_comments.push("OpenIE CAD Laser CAM".into());
    prog.header_comments.push(format!(
        "Kerf: {:.2}mm, Power: {:.0}%, Speed: {:.0}mm/min",
        config.kerf_width, config.cut_power, config.cut_speed
    ));

    emit::emit_preamble(&mut prog, config);

    // Apply kerf compensation
    let compensated = kerf::compensate_kerf(contours, config.kerf_width);

    // Order contours (holes first)
    let order = ordering::order_contours(&compensated);

    // Cut each contour
    for pass in 0..config.passes {
        if config.passes > 1 {
            prog.comment(format!("Pass {}/{}", pass + 1, config.passes));
        }

        for &idx in &order {
            let contour = &compensated[idx];

            if config.tabs_enabled {
                let tab_locs =
                    tabs::generate_tabs(contour, config.tab_spacing, config.tab_width);
                let segments = tabs::apply_tabs(contour, &tab_locs);
                emit::emit_cut_segments(&mut prog, &segments, config);
            } else {
                emit::emit_cut_contour(&mut prog, &contour.points, config);
            }
        }
    }

    emit::emit_postamble(&mut prog);
    prog
}

/// Generate laser cutting G-code from a 3D solid (top-down projection).
pub fn laser_cut_solid(solid: &physical_brep::Solid, config: &LaserConfig) -> GCodeProgram {
    let contours = project::project_solid_top(solid);
    laser_cut(&contours, config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::DVec2;
    use physical_mfg_toolpath::post;

    #[test]
    fn laser_cut_square() {
        let contour = Contour::closed(vec![
            DVec2::new(0.0, 0.0),
            DVec2::new(50.0, 0.0),
            DVec2::new(50.0, 50.0),
            DVec2::new(0.0, 50.0),
        ]);

        let config = LaserConfig::default();
        let gcode = laser_cut(&[contour], &config);
        let output = gcode.to_string(&post::grbl());

        assert!(output.contains("G1"), "Should have cutting moves");
        assert!(output.contains("M3 S"), "Should enable laser");
        assert!(output.contains("M5"), "Should disable laser");
        assert!(output.contains("M30"), "Should end program");
    }

    #[test]
    fn laser_cut_with_tabs() {
        let contour = Contour::closed(vec![
            DVec2::new(0.0, 0.0),
            DVec2::new(100.0, 0.0),
            DVec2::new(100.0, 100.0),
            DVec2::new(0.0, 100.0),
        ]);

        let mut config = LaserConfig::default();
        config.tabs_enabled = true;
        config.tab_spacing = 50.0;
        config.tab_width = 3.0;

        let gcode = laser_cut(&[contour], &config);
        let output = gcode.to_string(&post::grbl());
        assert!(output.contains("G1"));
    }

    #[test]
    fn laser_cut_multipass() {
        let contour = Contour::closed(vec![
            DVec2::new(0.0, 0.0),
            DVec2::new(20.0, 0.0),
            DVec2::new(20.0, 20.0),
            DVec2::new(0.0, 20.0),
        ]);

        let mut config = LaserConfig::default();
        config.passes = 3;

        let gcode = laser_cut(&[contour], &config);
        let output = gcode.to_string(&post::grbl());
        assert!(output.contains("Pass 1/3"));
        assert!(output.contains("Pass 3/3"));
    }

    #[test]
    fn laser_cut_solid_box() {
        let solid = physical_brep::builder::make_box(30.0, 20.0, 10.0);
        let config = LaserConfig::default();
        let gcode = laser_cut_solid(&solid, &config);
        let output = gcode.to_string(&post::grbl());
        assert!(output.contains("G1"), "Should produce cutting moves from solid");
    }
}
