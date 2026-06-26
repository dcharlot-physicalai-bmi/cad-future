//! Convert CNC toolpath segments into G-code.

use physical_mfg_toolpath::gcode::{GCodeProgram, GCommand};
use physical_mfg_toolpath::material::recommended_feeds_speeds;
use physical_mfg_toolpath::path::{MoveType, ToolpathSegment};

use crate::config::CncConfig;

/// Emit a complete CNC G-code program from toolpath segments.
pub fn emit_cnc_program(
    segments: &[ToolpathSegment],
    config: &CncConfig,
) -> GCodeProgram {
    let mut prog = GCodeProgram::new();
    prog.header_comments.push("OpenIE CAD CNC CAM".into());
    prog.header_comments.push(format!("Tool: {}", config.tool.name));
    prog.header_comments.push(format!("Material: {}", config.material.name));

    emit_preamble(&mut prog, config);

    for segment in segments {
        emit_segment(&mut prog, segment, &config.dialect);
    }

    emit_postamble(&mut prog, config);
    prog
}

/// CNC preamble: units, absolute mode, spindle on, coolant.
fn emit_preamble(prog: &mut GCodeProgram, config: &CncConfig) {
    prog.push(GCommand::SetUnitsMetric);
    prog.push(GCommand::AbsoluteMode);

    // Tool change
    prog.push(GCommand::ToolChange { index: 1 });

    // Spindle speed from material + tool
    let (rpm, _feed) = recommended_feeds_speeds(
        &config.material,
        config.tool_diameter(),
        config.tool.geometry.flute_count(),
        config.tool.max_rpm,
    );
    prog.push(GCommand::SpindleOn { rpm, cw: true });

    if config.coolant {
        prog.push(GCommand::CoolantOn);
    }

    // Dwell to let spindle reach speed
    prog.push(GCommand::Dwell { seconds: 2.0 });
}

/// CNC postamble: spindle off, coolant off, home.
fn emit_postamble(prog: &mut GCodeProgram, config: &CncConfig) {
    prog.push(GCommand::SpindleOff);
    if config.coolant {
        prog.push(GCommand::CoolantOff);
    }
    prog.push(GCommand::RapidMove {
        x: Some(0.0),
        y: Some(0.0),
        z: Some(config.safe_height + 20.0),
    });
    prog.push(GCommand::ProgramEnd);
}

/// Emit a single toolpath segment as G-code commands.
fn emit_segment(prog: &mut GCodeProgram, segment: &ToolpathSegment, dialect: &physical_mfg_toolpath::GCodeDialect) {
    let _ = dialect; // Available for dialect-specific emission in future

    if segment.path.len() < 2 {
        return;
    }

    match segment.move_type {
        MoveType::Rapid | MoveType::Retract => {
            for point in &segment.path[1..] {
                prog.push(GCommand::RapidMove {
                    x: Some(point.x),
                    y: Some(point.y),
                    z: Some(point.z),
                });
            }
        }
        MoveType::Cut => {
            for point in &segment.path[1..] {
                prog.push(GCommand::LinearMove {
                    x: Some(point.x),
                    y: Some(point.y),
                    z: Some(point.z),
                    f: segment.feed_rate,
                });
            }
        }
        MoveType::Plunge => {
            for point in &segment.path[1..] {
                prog.push(GCommand::LinearMove {
                    x: Some(point.x),
                    y: Some(point.y),
                    z: Some(point.z),
                    f: segment.feed_rate,
                });
            }
        }
        MoveType::Arc { .. } => {
            // Arc segments — emit as linear moves for now
            for point in &segment.path[1..] {
                prog.push(GCommand::LinearMove {
                    x: Some(point.x),
                    y: Some(point.y),
                    z: Some(point.z),
                    f: segment.feed_rate,
                });
            }
        }
    }
}
