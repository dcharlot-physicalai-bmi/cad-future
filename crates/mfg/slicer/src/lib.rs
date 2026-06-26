//! FDM 3D printing slicer — mesh to G-code pipeline.
//!
//! Takes a tessellated mesh and produces G-code for FDM 3D printers.
//!
//! Pipeline:
//! 1. Slice mesh into layers (horizontal plane intersections)
//! 2. Generate perimeter walls (contour offset)
//! 3. Generate infill (parallel lines, grid, etc.)
//! 4. Emit G-code (Marlin, Klipper, etc.)

pub mod config;
pub mod emit;
pub mod infill;
pub mod perimeter;
pub mod slice;
pub mod adaptive_layer;
pub mod support;
pub mod estimation;

pub use config::{InfillPattern, SeamPosition, SlicerConfig};
pub use physical_mfg_toolpath::gcode::GCodeProgram;
pub use adaptive_layer::{adaptive_layer_heights, AdaptiveLayer, AdaptiveLayerConfig};
pub use support::{generate_supports, SupportConfig, SupportPattern, SupportRegion, SupportStructure};
pub use estimation::{
    estimate_print_time, estimate_material, PrintEstimate, MaterialEstimate, SliceLayer,
    PrintEstimation, FilamentInfo, estimate_print,
};

use physical_tessellation::TessMesh;

/// Slice a mesh and generate complete G-code.
///
/// This is the main entry point for the slicer pipeline.
pub fn slice_to_gcode(mesh: &TessMesh, config: &SlicerConfig) -> GCodeProgram {
    let mut prog = GCodeProgram::new();
    prog.header_comments.push("OpenIE CAD Slicer".into());
    prog.header_comments.push(format!(
        "Layer height: {:.2}mm, Nozzle: {:.2}mm",
        config.layer_height, config.nozzle_diameter
    ));

    // Preamble
    emit::emit_preamble(&mut prog, config);

    // Slice mesh into layers
    let layers = slice::slice_mesh(mesh, config.layer_height, config.first_layer_height);

    let e_per_mm = config.e_per_mm();
    let extrusion_width = config.extrusion_width();
    let mut e_state = 0.0;

    for (layer_idx, (_z, contours)) in layers.iter().enumerate() {
        if contours.is_empty() {
            continue;
        }

        // Layer Z position (top of layer, not mid-plane)
        let layer_z = if layer_idx == 0 {
            config.first_layer_height
        } else {
            config.first_layer_height + config.layer_height * layer_idx as f64
        };

        emit::emit_layer_change(&mut prog, layer_z, layer_idx, config);

        let speed = if layer_idx == 0 {
            config.first_layer_speed
        } else {
            config.print_speed
        };

        // Process each contour in the layer
        for contour in contours {
            // Generate perimeter walls
            let walls = perimeter::generate_perimeters(contour, config.wall_count, extrusion_width);

            // Emit perimeters (outer first)
            for wall in &walls {
                e_state = emit::emit_contour(
                    &mut prog,
                    &wall.points,
                    layer_z,
                    speed,
                    e_per_mm,
                    e_state,
                    config.travel_speed,
                );
            }

            // Generate and emit infill
            if config.infill_density > 0.0 {
                if let Some(infill_boundary) =
                    perimeter::infill_boundary(contour, config.wall_count, extrusion_width)
                {
                    let infill_lines = infill::generate_infill(
                        &infill_boundary,
                        config.infill_pattern,
                        config.infill_density,
                        layer_idx,
                        extrusion_width,
                    );

                    let infill_speed = if layer_idx == 0 {
                        config.first_layer_speed
                    } else {
                        config.infill_speed
                    };

                    e_state = emit::emit_infill(
                        &mut prog,
                        &infill_lines,
                        layer_z,
                        infill_speed,
                        e_per_mm,
                        e_state,
                    );
                }
            }
        }

        // Reset extruder periodically to avoid float precision issues
        if layer_idx % 50 == 49 {
            prog.push(physical_mfg_toolpath::GCommand::ResetExtruder);
            e_state = 0.0;
        }
    }

    // Postamble
    emit::emit_postamble(&mut prog, config);

    prog
}

#[cfg(test)]
mod tests {
    use super::*;
    use physical_mfg_toolpath::post;
    use physical_tessellation::{TessMesh, TessVertex};

    fn test_box_mesh() -> TessMesh {
        let vertices = vec![
            TessVertex { position: [0.0, 0.0, 0.0], normal: [0.0, 0.0, -1.0], uv: [0.0, 0.0] },
            TessVertex { position: [20.0, 0.0, 0.0], normal: [0.0, 0.0, -1.0], uv: [1.0, 0.0] },
            TessVertex { position: [20.0, 20.0, 0.0], normal: [0.0, 0.0, -1.0], uv: [1.0, 1.0] },
            TessVertex { position: [0.0, 20.0, 0.0], normal: [0.0, 0.0, -1.0], uv: [0.0, 1.0] },
            TessVertex { position: [0.0, 0.0, 10.0], normal: [0.0, 0.0, 1.0], uv: [0.0, 0.0] },
            TessVertex { position: [20.0, 0.0, 10.0], normal: [0.0, 0.0, 1.0], uv: [1.0, 0.0] },
            TessVertex { position: [20.0, 20.0, 10.0], normal: [0.0, 0.0, 1.0], uv: [1.0, 1.0] },
            TessVertex { position: [0.0, 20.0, 10.0], normal: [0.0, 0.0, 1.0], uv: [0.0, 1.0] },
        ];
        #[rustfmt::skip]
        let indices = vec![
            0, 2, 1,  0, 3, 2,
            4, 5, 6,  4, 6, 7,
            0, 1, 5,  0, 5, 4,
            2, 3, 7,  2, 7, 6,
            0, 4, 7,  0, 7, 3,
            1, 2, 6,  1, 6, 5,
        ];
        TessMesh { vertices, indices }
    }

    #[test]
    fn slice_box_produces_gcode() {
        let mesh = test_box_mesh();
        let config = SlicerConfig::default();
        let gcode = slice_to_gcode(&mesh, &config);
        let output = gcode.to_string(&post::marlin());

        assert!(output.contains("G1"), "Should contain linear moves");
        assert!(output.contains("M109"), "Should set extruder temp");
        assert!(output.contains("M190"), "Should set bed temp");
        assert!(output.contains("G21"), "Should set metric units");
        assert!(output.contains("M30"), "Should end program");
        assert!(gcode.estimated_time_s(6000.0) > 0.0, "Should have non-zero time estimate");
        assert!(gcode.total_extrusion_mm() > 0.0, "Should extrude filament");
    }

    #[test]
    fn slice_box_layer_count() {
        let mesh = test_box_mesh();
        let mut config = SlicerConfig::default();
        config.layer_height = 0.2;
        config.first_layer_height = 0.3;
        let gcode = slice_to_gcode(&mesh, &config);
        let layers = gcode.layer_count();
        // 10mm tall box: 0.3mm first layer + ~48 layers at 0.2mm ≈ 49 layers
        assert!(layers > 10, "Should have multiple layers, got {layers}");
    }
}
