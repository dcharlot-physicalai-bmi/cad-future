//! G-code post-processor — converts toolpath data into machine-ready G-code
//! with controller-specific dialects, canned cycles, and validation.

use glam::DVec3;
use physical_mfg_toolpath::gcode::{GCodeProgram, GCommand};
use physical_mfg_toolpath::path::{MoveType, ToolpathSegment};
use physical_mfg_toolpath::tool::Tool;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// MachineConfig
// ---------------------------------------------------------------------------

/// CNC controller type / dialect.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ControllerType {
    Fanuc,
    LinuxCNC,
    Haas,
    Grbl,
    Mach3,
}

/// Coolant delivery type.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CoolantType {
    None,
    Flood,
    Mist,
    Through,
}

/// Work coordinate offset (G54-G59).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum WorkOffset {
    G54,
    G55,
    G56,
    G57,
    G58,
    G59,
}

impl WorkOffset {
    /// G-code index: G54=1, G55=2, ... G59=6.
    pub fn index(self) -> u8 {
        match self {
            Self::G54 => 1,
            Self::G55 => 2,
            Self::G56 => 3,
            Self::G57 => 4,
            Self::G58 => 5,
            Self::G59 => 6,
        }
    }
}

/// Complete machine configuration for post-processing.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MachineConfig {
    /// Human-readable machine name.
    pub machine_name: String,
    /// Controller dialect.
    pub controller_type: ControllerType,
    /// Maximum spindle RPM.
    pub max_spindle_rpm: f64,
    /// Maximum programmed feed rate (mm/min).
    pub max_feed_mm_min: f64,
    /// Active work offset.
    pub work_offset: WorkOffset,
    /// Coolant delivery type.
    pub coolant_type: CoolantType,
    /// Safe retract position for tool changes.
    pub tool_change_position: DVec3,
    /// Use incremental mode (G91) instead of absolute (G90).
    pub use_incremental: bool,
    /// Decimal places for coordinate output (3-6).
    pub decimal_places: usize,
    /// Program number (O-number, used by Fanuc/Haas).
    pub program_number: u32,
}

impl Default for MachineConfig {
    fn default() -> Self {
        Self {
            machine_name: "Generic 3-Axis Mill".into(),
            controller_type: ControllerType::Fanuc,
            max_spindle_rpm: 24000.0,
            max_feed_mm_min: 10000.0,
            work_offset: WorkOffset::G54,
            coolant_type: CoolantType::Flood,
            tool_change_position: DVec3::new(0.0, 0.0, 50.0),
            use_incremental: false,
            decimal_places: 3,
            program_number: 1,
        }
    }
}

// ---------------------------------------------------------------------------
// GcodeProgram (post-processed output)
// ---------------------------------------------------------------------------

/// A fully post-processed G-code program with structured sections.
#[derive(Clone, Debug)]
pub struct PostProcessedProgram {
    /// The underlying GCodeProgram.
    pub program: GCodeProgram,
    /// Machine config used for this program.
    pub config: MachineConfig,
}

impl PostProcessedProgram {
    /// Render the program to a G-code string.
    pub fn to_string(&self) -> String {
        let dialect = dialect_from_config(&self.config);
        self.program.to_string(&dialect)
    }

    /// Estimate cycle time in seconds, given rapid traverse speed in mm/min.
    pub fn estimate_cycle_time(&self, rapid_speed_mm_min: f64) -> f64 {
        estimate_cycle_time(&self.program, rapid_speed_mm_min)
    }

    /// Validate the program and return warnings.
    pub fn validate(&self) -> Vec<GcodeWarning> {
        validate_gcode(&self.program)
    }
}

// ---------------------------------------------------------------------------
// PostProcessor
// ---------------------------------------------------------------------------

/// Post-process toolpath segments into a machine-ready G-code program.
pub fn post_process(
    segments: &[ToolpathSegment],
    config: &MachineConfig,
    tool: &Tool,
    tool_index: usize,
    spindle_rpm: f64,
    feed_rate: f64,
) -> PostProcessedProgram {
    let mut prog = GCodeProgram::new();

    // Header comments
    emit_header_comments(&mut prog, config, tool);

    // Body: preamble + segments + footer
    emit_preamble(&mut prog, config, tool_index, spindle_rpm);
    emit_segments(&mut prog, segments, config, feed_rate);
    emit_footer(&mut prog, config);

    PostProcessedProgram {
        program: prog,
        config: config.clone(),
    }
}

// ---------------------------------------------------------------------------
// Header comments
// ---------------------------------------------------------------------------

fn emit_header_comments(prog: &mut GCodeProgram, config: &MachineConfig, tool: &Tool) {
    prog.header_comments
        .push(format!("Program O{:04}", config.program_number));
    prog.header_comments
        .push(format!("Machine: {}", config.machine_name));
    prog.header_comments.push(format!("Tool: {}", tool.name));
    prog.header_comments
        .push(format!("Controller: {:?}", config.controller_type));
}

// ---------------------------------------------------------------------------
// Preamble (controller-specific)
// ---------------------------------------------------------------------------

fn emit_preamble(
    prog: &mut GCodeProgram,
    config: &MachineConfig,
    tool_index: usize,
    spindle_rpm: f64,
) {
    let rpm = spindle_rpm.min(config.max_spindle_rpm);

    match config.controller_type {
        ControllerType::Fanuc => {
            // Safety line
            prog.push(GCommand::SetUnitsMetric);
            if config.use_incremental {
                prog.push(GCommand::RelativeMode);
            } else {
                prog.push(GCommand::AbsoluteMode);
            }
            prog.push(GCommand::WorkOffset {
                index: config.work_offset.index(),
            });
            prog.push(GCommand::ToolChange { index: tool_index });
            prog.push(GCommand::ToolLengthComp {
                index: tool_index,
            });
            prog.push(GCommand::SpindleOn { rpm, cw: true });
            emit_coolant_on(prog, config);
            prog.push(GCommand::Dwell { seconds: 2.0 });
            // Safe position via G28
            prog.push(GCommand::HomeAll);
        }
        ControllerType::Haas => {
            prog.push(GCommand::SetUnitsMetric);
            if config.use_incremental {
                prog.push(GCommand::RelativeMode);
            } else {
                prog.push(GCommand::AbsoluteMode);
            }
            prog.push(GCommand::WorkOffset {
                index: config.work_offset.index(),
            });
            prog.push(GCommand::ToolChange { index: tool_index });
            prog.push(GCommand::ToolLengthComp {
                index: tool_index,
            });
            prog.push(GCommand::SpindleOn { rpm, cw: true });
            emit_coolant_on(prog, config);
            prog.push(GCommand::Dwell { seconds: 2.0 });
        }
        ControllerType::LinuxCNC => {
            prog.push(GCommand::SetUnitsMetric);
            if config.use_incremental {
                prog.push(GCommand::RelativeMode);
            } else {
                prog.push(GCommand::AbsoluteMode);
            }
            prog.push(GCommand::WorkOffset {
                index: config.work_offset.index(),
            });
            prog.push(GCommand::ToolChange { index: tool_index });
            prog.push(GCommand::ToolLengthComp {
                index: tool_index,
            });
            prog.push(GCommand::SpindleOn { rpm, cw: true });
            emit_coolant_on(prog, config);
            prog.push(GCommand::Dwell { seconds: 2.0 });
            // LinuxCNC uses G53 for safe position
            prog.push(GCommand::SafeRetractG53);
        }
        ControllerType::Grbl => {
            // Grbl: no O-number, simple header
            prog.push(GCommand::SetUnitsMetric);
            if config.use_incremental {
                prog.push(GCommand::RelativeMode);
            } else {
                prog.push(GCommand::AbsoluteMode);
            }
            // Grbl doesn't use work offsets explicitly in the same way,
            // but G54 is the default. Include for compatibility.
            prog.push(GCommand::ToolChange { index: tool_index });
            prog.push(GCommand::SpindleOn { rpm, cw: true });
            emit_coolant_on(prog, config);
            prog.push(GCommand::Dwell { seconds: 2.0 });
        }
        ControllerType::Mach3 => {
            prog.push(GCommand::SetUnitsMetric);
            if config.use_incremental {
                prog.push(GCommand::RelativeMode);
            } else {
                prog.push(GCommand::AbsoluteMode);
            }
            prog.push(GCommand::WorkOffset {
                index: config.work_offset.index(),
            });
            prog.push(GCommand::ToolChange { index: tool_index });
            prog.push(GCommand::ToolLengthComp {
                index: tool_index,
            });
            prog.push(GCommand::SpindleOn { rpm, cw: true });
            emit_coolant_on(prog, config);
            prog.push(GCommand::Dwell { seconds: 2.0 });
        }
    }
}

fn emit_coolant_on(prog: &mut GCodeProgram, config: &MachineConfig) {
    match config.coolant_type {
        CoolantType::None => {}
        CoolantType::Flood | CoolantType::Through => {
            prog.push(GCommand::CoolantOn);
        }
        CoolantType::Mist => {
            prog.push(GCommand::MistCoolantOn);
        }
    }
}

// ---------------------------------------------------------------------------
// Segment emission
// ---------------------------------------------------------------------------

fn emit_segments(
    prog: &mut GCodeProgram,
    segments: &[ToolpathSegment],
    config: &MachineConfig,
    _feed_rate: f64,
) {
    for segment in segments {
        emit_segment(prog, segment, config);
    }
}

fn emit_segment(prog: &mut GCodeProgram, segment: &ToolpathSegment, config: &MachineConfig) {
    if segment.path.len() < 2 {
        return;
    }

    let feed = segment.feed_rate.min(config.max_feed_mm_min);

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
                    f: feed,
                });
            }
        }
        MoveType::Plunge => {
            for point in &segment.path[1..] {
                prog.push(GCommand::LinearMove {
                    x: Some(point.x),
                    y: Some(point.y),
                    z: Some(point.z),
                    f: feed,
                });
            }
        }
        MoveType::Arc { cw } => {
            // For arc segments with exactly 2 points, emit G2/G3.
            // With more points, linearize.
            if segment.path.len() == 2 {
                let start = segment.path[0];
                let end = segment.path[1];
                // Compute center offset (I, J) as midpoint offset heuristic
                let mid_x = (end.x - start.x) / 2.0;
                let mid_y = (end.y - start.y) / 2.0;
                if cw {
                    prog.push(GCommand::ArcCW {
                        x: end.x,
                        y: end.y,
                        i: mid_x,
                        j: mid_y,
                        f: feed,
                    });
                } else {
                    prog.push(GCommand::ArcCCW {
                        x: end.x,
                        y: end.y,
                        i: mid_x,
                        j: mid_y,
                        f: feed,
                    });
                }
            } else {
                for point in &segment.path[1..] {
                    prog.push(GCommand::LinearMove {
                        x: Some(point.x),
                        y: Some(point.y),
                        z: Some(point.z),
                        f: feed,
                    });
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Footer (controller-specific)
// ---------------------------------------------------------------------------

fn emit_footer(prog: &mut GCodeProgram, config: &MachineConfig) {
    prog.push(GCommand::SpindleOff);

    if config.coolant_type != CoolantType::None {
        prog.push(GCommand::CoolantOff);
    }

    // Safe retract
    match config.controller_type {
        ControllerType::Fanuc | ControllerType::Haas | ControllerType::Mach3 => {
            prog.push(GCommand::CancelToolLengthComp);
            prog.push(GCommand::HomeAll);
        }
        ControllerType::LinuxCNC => {
            prog.push(GCommand::CancelToolLengthComp);
            prog.push(GCommand::SafeRetractG53);
        }
        ControllerType::Grbl => {
            prog.push(GCommand::RapidMove {
                x: Some(config.tool_change_position.x),
                y: Some(config.tool_change_position.y),
                z: Some(config.tool_change_position.z),
            });
        }
    }

    prog.push(GCommand::ProgramEnd);
}

// ---------------------------------------------------------------------------
// Canned cycle helpers
// ---------------------------------------------------------------------------

/// Emit G81 simple drilling canned cycles for a set of hole positions.
pub fn emit_drill_canned(
    prog: &mut GCodeProgram,
    holes: &[(f64, f64)],
    z_depth: f64,
    r_plane: f64,
    feed: f64,
) {
    for &(x, y) in holes {
        prog.push(GCommand::DrillCycle {
            x,
            y,
            z: z_depth,
            r: r_plane,
            f: feed,
        });
    }
    prog.push(GCommand::CancelCannedCycle);
}

/// Emit G83 peck drilling canned cycles.
pub fn emit_peck_drill_canned(
    prog: &mut GCodeProgram,
    holes: &[(f64, f64)],
    z_depth: f64,
    r_plane: f64,
    peck_depth: f64,
    feed: f64,
) {
    for &(x, y) in holes {
        prog.push(GCommand::PeckDrillCycle {
            x,
            y,
            z: z_depth,
            r: r_plane,
            q: peck_depth,
            f: feed,
        });
    }
    prog.push(GCommand::CancelCannedCycle);
}

/// Emit G84 tapping canned cycles.
pub fn emit_tap_canned(
    prog: &mut GCodeProgram,
    holes: &[(f64, f64)],
    z_depth: f64,
    r_plane: f64,
    feed: f64,
) {
    for &(x, y) in holes {
        prog.push(GCommand::TapCycle {
            x,
            y,
            z: z_depth,
            r: r_plane,
            f: feed,
        });
    }
    prog.push(GCommand::CancelCannedCycle);
}

// ---------------------------------------------------------------------------
// Dialect helper
// ---------------------------------------------------------------------------

/// Build a `GCodeDialect` from a `MachineConfig`.
pub fn dialect_from_config(config: &MachineConfig) -> physical_mfg_toolpath::post::GCodeDialect {
    use physical_mfg_toolpath::post::CommentStyle;

    match config.controller_type {
        ControllerType::Fanuc => physical_mfg_toolpath::post::GCodeDialect {
            name: "Fanuc".into(),
            line_numbers: true,
            decimal_places: config.decimal_places,
            arc_support: true,
            preamble: vec!["%".into(), format!("O{:04}", config.program_number)],
            postamble: vec!["M30".into(), "%".into()],
            comment_style: CommentStyle::Parentheses,
            max_laser_s: 0.0,
        },
        ControllerType::Haas => physical_mfg_toolpath::post::GCodeDialect {
            name: "Haas".into(),
            line_numbers: true,
            decimal_places: config.decimal_places,
            arc_support: true,
            preamble: vec!["%".into(), format!("O{:05}", config.program_number)],
            postamble: vec!["M30".into(), "%".into()],
            comment_style: CommentStyle::Parentheses,
            max_laser_s: 0.0,
        },
        ControllerType::LinuxCNC => physical_mfg_toolpath::post::GCodeDialect {
            name: "LinuxCNC".into(),
            line_numbers: false,
            decimal_places: config.decimal_places,
            arc_support: true,
            preamble: vec!["%".into()],
            postamble: vec!["%".into()],
            comment_style: CommentStyle::Parentheses,
            max_laser_s: 0.0,
        },
        ControllerType::Grbl => physical_mfg_toolpath::post::GCodeDialect {
            name: "GRBL".into(),
            line_numbers: false,
            decimal_places: config.decimal_places,
            arc_support: true,
            preamble: vec![],
            postamble: vec![],
            comment_style: CommentStyle::Semicolon,
            max_laser_s: 1000.0,
        },
        ControllerType::Mach3 => physical_mfg_toolpath::post::GCodeDialect {
            name: "Mach3".into(),
            line_numbers: true,
            decimal_places: config.decimal_places,
            arc_support: true,
            preamble: vec![format!("O{:04}", config.program_number)],
            postamble: vec!["M30".into()],
            comment_style: CommentStyle::Parentheses,
            max_laser_s: 0.0,
        },
    }
}

// ---------------------------------------------------------------------------
// Simulation / verification
// ---------------------------------------------------------------------------

/// Estimate cycle time in seconds for a G-code program.
///
/// `rapid_speed` is the machine's rapid traverse speed in mm/min.
pub fn estimate_cycle_time(program: &GCodeProgram, rapid_speed: f64) -> f64 {
    program.estimated_time_s(rapid_speed)
}

/// A warning found during G-code validation.
#[derive(Clone, Debug, PartialEq)]
pub enum GcodeWarning {
    /// No spindle-on command found before cutting moves.
    MissingSpindleOn,
    /// No tool change command found.
    MissingToolChange,
    /// Spindle started without a tool change first.
    SpindleBeforeToolChange,
    /// Cutting moves detected after spindle off.
    CuttingAfterSpindleOff,
    /// No program end (M30) found.
    MissingProgramEnd,
    /// Feed rate is zero on a cutting move.
    ZeroFeedRate,
    /// Feed rate exceeds machine maximum.
    FeedRateExceeded { feed: f64, max: f64 },
    /// No coolant command (may be intentional).
    NoCoolant,
}

/// Validate a G-code program for common errors and omissions.
pub fn validate_gcode(program: &GCodeProgram) -> Vec<GcodeWarning> {
    let mut warnings = Vec::new();

    let mut has_spindle_on = false;
    let mut has_tool_change = false;
    let mut spindle_off = false;
    let mut has_program_end = false;
    let mut has_coolant = false;
    let mut has_cutting_moves = false;

    for cmd in &program.commands {
        match cmd {
            GCommand::ToolChange { .. } => {
                has_tool_change = true;
            }
            GCommand::SpindleOn { .. } => {
                if !has_tool_change {
                    warnings.push(GcodeWarning::SpindleBeforeToolChange);
                }
                has_spindle_on = true;
                spindle_off = false;
            }
            GCommand::SpindleOff => {
                spindle_off = true;
            }
            GCommand::CoolantOn | GCommand::MistCoolantOn => {
                has_coolant = true;
            }
            GCommand::LinearMove { f, .. } => {
                has_cutting_moves = true;
                if !has_spindle_on {
                    // Will be caught as MissingSpindleOn
                }
                if spindle_off {
                    warnings.push(GcodeWarning::CuttingAfterSpindleOff);
                }
                if *f <= 0.0 {
                    warnings.push(GcodeWarning::ZeroFeedRate);
                }
            }
            GCommand::ArcCW { .. } | GCommand::ArcCCW { .. } => {
                has_cutting_moves = true;
                if spindle_off {
                    warnings.push(GcodeWarning::CuttingAfterSpindleOff);
                }
            }
            GCommand::DrillCycle { .. }
            | GCommand::PeckDrillCycle { .. }
            | GCommand::TapCycle { .. } => {
                has_cutting_moves = true;
                if spindle_off {
                    warnings.push(GcodeWarning::CuttingAfterSpindleOff);
                }
            }
            GCommand::ProgramEnd => {
                has_program_end = true;
            }
            _ => {}
        }
    }

    if has_cutting_moves && !has_spindle_on {
        warnings.push(GcodeWarning::MissingSpindleOn);
    }
    if !has_tool_change {
        warnings.push(GcodeWarning::MissingToolChange);
    }
    if !has_program_end {
        warnings.push(GcodeWarning::MissingProgramEnd);
    }
    if has_cutting_moves && !has_coolant {
        warnings.push(GcodeWarning::NoCoolant);
    }

    warnings
}

/// Validate a G-code program against a machine config (includes feed rate checks).
pub fn validate_gcode_with_config(
    program: &GCodeProgram,
    config: &MachineConfig,
) -> Vec<GcodeWarning> {
    let mut warnings = validate_gcode(program);

    for cmd in &program.commands {
        match cmd {
            GCommand::LinearMove { f, .. } => {
                if *f > config.max_feed_mm_min {
                    warnings.push(GcodeWarning::FeedRateExceeded {
                        feed: *f,
                        max: config.max_feed_mm_min,
                    });
                }
            }
            GCommand::ArcCW { f, .. } | GCommand::ArcCCW { f, .. } => {
                if *f > config.max_feed_mm_min {
                    warnings.push(GcodeWarning::FeedRateExceeded {
                        feed: *f,
                        max: config.max_feed_mm_min,
                    });
                }
            }
            _ => {}
        }
    }

    warnings
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::DVec3;
    use physical_mfg_toolpath::path::{MoveType, ToolpathSegment};
    use physical_mfg_toolpath::tool::{Tool, ToolGeometry, ToolMaterial};

    fn test_tool() -> Tool {
        Tool {
            name: "6mm End Mill".into(),
            geometry: ToolGeometry::EndMill {
                diameter: 6.0,
                flute_length: 20.0,
                flute_count: 2,
            },
            max_rpm: 24000.0,
            material: ToolMaterial::Carbide,
        }
    }

    fn rect_pocket_segments() -> Vec<ToolpathSegment> {
        let safe_z = 25.0;
        let cut_z = 18.0;
        let feed = 800.0;

        vec![
            // Rapid to start position
            ToolpathSegment::rapid(
                DVec3::new(0.0, 0.0, safe_z),
                DVec3::new(5.0, 5.0, safe_z),
            ),
            // Plunge
            ToolpathSegment {
                path: vec![DVec3::new(5.0, 5.0, safe_z), DVec3::new(5.0, 5.0, cut_z)],
                feed_rate: feed * 0.5,
                move_type: MoveType::Plunge,
            },
            // Cut rectangle
            ToolpathSegment::cut(
                vec![
                    DVec3::new(5.0, 5.0, cut_z),
                    DVec3::new(45.0, 5.0, cut_z),
                    DVec3::new(45.0, 25.0, cut_z),
                    DVec3::new(5.0, 25.0, cut_z),
                    DVec3::new(5.0, 5.0, cut_z),
                ],
                feed,
            ),
            // Retract
            ToolpathSegment {
                path: vec![DVec3::new(5.0, 5.0, cut_z), DVec3::new(5.0, 5.0, safe_z)],
                feed_rate: 0.0,
                move_type: MoveType::Retract,
            },
        ]
    }

    #[test]
    fn rectangular_pocket_gcode() {
        let config = MachineConfig::default();
        let tool = test_tool();
        let segments = rect_pocket_segments();

        let result = post_process(&segments, &config, &tool, 1, 12000.0, 800.0);
        let output = result.to_string();

        assert!(output.contains("G21"), "Should set metric units");
        assert!(output.contains("G90"), "Should set absolute mode");
        assert!(output.contains("M3"), "Should turn spindle on");
        assert!(output.contains("G1"), "Should have cutting moves");
        assert!(output.contains("G0"), "Should have rapid moves");
        assert!(output.contains("M5"), "Should turn spindle off");
        assert!(output.contains("M30"), "Should end program");
    }

    #[test]
    fn fanuc_dialect() {
        let config = MachineConfig {
            controller_type: ControllerType::Fanuc,
            program_number: 42,
            ..Default::default()
        };
        let tool = test_tool();
        let segments = rect_pocket_segments();

        let result = post_process(&segments, &config, &tool, 1, 12000.0, 800.0);
        let output = result.to_string();

        // Fanuc: %-delimited, O-number header, line numbers, G28 for home
        assert!(output.contains('%'), "Fanuc should have % delimiters");
        assert!(output.contains("O0042"), "Fanuc should have O-number");
        assert!(output.contains("N10"), "Fanuc should have line numbers");
        assert!(output.contains("G28"), "Fanuc should use G28 for home");
    }

    #[test]
    fn linuxcnc_dialect() {
        let config = MachineConfig {
            controller_type: ControllerType::LinuxCNC,
            decimal_places: 4,
            ..Default::default()
        };
        let tool = test_tool();
        let segments = rect_pocket_segments();

        let result = post_process(&segments, &config, &tool, 1, 12000.0, 800.0);
        let output = result.to_string();

        // LinuxCNC: %-delimited, no line numbers, G53 for safe position
        assert!(output.contains('%'), "LinuxCNC should have % delimiters");
        assert!(
            !output.contains("N10"),
            "LinuxCNC should not have line numbers"
        );
        assert!(
            output.contains("G53"),
            "LinuxCNC should use G53 for safe retract"
        );
    }

    #[test]
    fn grbl_dialect() {
        let config = MachineConfig {
            controller_type: ControllerType::Grbl,
            ..Default::default()
        };
        let tool = test_tool();
        let segments = rect_pocket_segments();

        let result = post_process(&segments, &config, &tool, 1, 12000.0, 800.0);
        let output = result.to_string();

        // Grbl: no %, no line numbers, semicolon comments
        assert!(!output.contains('%'), "Grbl should not have % delimiters");
        assert!(
            !output.contains("N10"),
            "Grbl should not have line numbers"
        );
        // Semicolon comments
        assert!(output.contains(';'), "Grbl should use semicolon comments");
    }

    #[test]
    fn drilling_canned_cycle() {
        let mut prog = GCodeProgram::new();
        prog.push(GCommand::SetUnitsMetric);
        prog.push(GCommand::AbsoluteMode);
        prog.push(GCommand::ToolChange { index: 1 });
        prog.push(GCommand::SpindleOn {
            rpm: 3000.0,
            cw: true,
        });
        prog.push(GCommand::CoolantOn);

        emit_drill_canned(
            &mut prog,
            &[(10.0, 10.0), (30.0, 10.0), (50.0, 10.0)],
            -15.0,
            2.0,
            200.0,
        );

        prog.push(GCommand::SpindleOff);
        prog.push(GCommand::CoolantOff);
        prog.push(GCommand::ProgramEnd);

        let dialect = physical_mfg_toolpath::post::fanuc();
        let output = prog.to_string(&dialect);

        assert!(output.contains("G81"), "Should have G81 drill cycle");
        assert!(output.contains("G80"), "Should cancel canned cycle");
        assert!(
            output.contains("Z-15.000"),
            "Should have correct Z depth"
        );
        assert!(output.contains("R2.000"), "Should have R plane");
        assert!(output.contains("F200"), "Should have feed rate");
    }

    #[test]
    fn peck_drill_canned_cycle() {
        let mut prog = GCodeProgram::new();
        prog.push(GCommand::ToolChange { index: 2 });
        prog.push(GCommand::SpindleOn {
            rpm: 2000.0,
            cw: true,
        });
        prog.push(GCommand::CoolantOn);

        emit_peck_drill_canned(
            &mut prog,
            &[(10.0, 10.0)],
            -20.0,
            2.0,
            3.0,
            150.0,
        );

        prog.push(GCommand::SpindleOff);
        prog.push(GCommand::CoolantOff);
        prog.push(GCommand::ProgramEnd);

        let dialect = physical_mfg_toolpath::post::haas();
        let output = prog.to_string(&dialect);

        assert!(output.contains("G83"), "Should have G83 peck drill");
        assert!(output.contains("Q3.0000"), "Haas peck drill should have Q peck depth");
        assert!(output.contains("G80"), "Should cancel canned cycle");
    }

    #[test]
    fn cycle_time_estimation() {
        let config = MachineConfig::default();
        let tool = test_tool();
        let segments = rect_pocket_segments();

        let result = post_process(&segments, &config, &tool, 1, 12000.0, 800.0);
        let time = result.estimate_cycle_time(6000.0);

        assert!(time > 0.0, "Cycle time should be positive");
        // A rectangular pocket 40x20mm at 800mm/min feed should take at least a few seconds
        assert!(
            time > 1.0,
            "Cycle time should be at least 1 second for this toolpath"
        );
        assert!(
            time < 600.0,
            "Cycle time should be less than 10 minutes"
        );
    }

    #[test]
    fn validation_catches_missing_spindle() {
        let mut prog = GCodeProgram::new();
        prog.push(GCommand::SetUnitsMetric);
        // No tool change, no spindle on
        prog.push(GCommand::LinearMove {
            x: Some(10.0),
            y: Some(10.0),
            z: Some(0.0),
            f: 500.0,
        });
        prog.push(GCommand::ProgramEnd);

        let warnings = validate_gcode(&prog);
        assert!(
            warnings.contains(&GcodeWarning::MissingSpindleOn),
            "Should warn about missing spindle on"
        );
        assert!(
            warnings.contains(&GcodeWarning::MissingToolChange),
            "Should warn about missing tool change"
        );
    }

    #[test]
    fn validation_passes_for_valid_program() {
        let config = MachineConfig::default();
        let tool = test_tool();
        let segments = rect_pocket_segments();

        let result = post_process(&segments, &config, &tool, 1, 12000.0, 800.0);
        let warnings = result.validate();

        // Valid program should have no critical warnings
        assert!(
            !warnings.contains(&GcodeWarning::MissingSpindleOn),
            "Should not warn about spindle"
        );
        assert!(
            !warnings.contains(&GcodeWarning::MissingToolChange),
            "Should not warn about tool change"
        );
        assert!(
            !warnings.contains(&GcodeWarning::MissingProgramEnd),
            "Should not warn about program end"
        );
    }

    #[test]
    fn validation_catches_cutting_after_spindle_off() {
        let mut prog = GCodeProgram::new();
        prog.push(GCommand::ToolChange { index: 1 });
        prog.push(GCommand::SpindleOn {
            rpm: 10000.0,
            cw: true,
        });
        prog.push(GCommand::CoolantOn);
        prog.push(GCommand::SpindleOff);
        // Cutting after spindle is off — dangerous!
        prog.push(GCommand::LinearMove {
            x: Some(10.0),
            y: Some(10.0),
            z: Some(0.0),
            f: 500.0,
        });
        prog.push(GCommand::ProgramEnd);

        let warnings = validate_gcode(&prog);
        assert!(
            warnings.contains(&GcodeWarning::CuttingAfterSpindleOff),
            "Should warn about cutting after spindle off"
        );
    }

    #[test]
    fn tapping_cycle() {
        let mut prog = GCodeProgram::new();
        prog.push(GCommand::ToolChange { index: 3 });
        prog.push(GCommand::SpindleOn {
            rpm: 500.0,
            cw: true,
        });
        prog.push(GCommand::CoolantOn);

        emit_tap_canned(
            &mut prog,
            &[(10.0, 10.0), (30.0, 10.0)],
            -12.0,
            2.0,
            100.0,
        );

        prog.push(GCommand::SpindleOff);
        prog.push(GCommand::CoolantOff);
        prog.push(GCommand::ProgramEnd);

        let dialect = physical_mfg_toolpath::post::fanuc();
        let output = prog.to_string(&dialect);

        assert!(output.contains("G84"), "Should have G84 tapping cycle");
        assert!(output.contains("G80"), "Should cancel canned cycle");
    }

    #[test]
    fn mist_coolant() {
        let config = MachineConfig {
            coolant_type: CoolantType::Mist,
            ..Default::default()
        };
        let tool = test_tool();
        let segments = rect_pocket_segments();

        let result = post_process(&segments, &config, &tool, 1, 12000.0, 800.0);
        let output = result.to_string();

        assert!(output.contains("M7"), "Mist coolant should use M7");
    }

    #[test]
    fn feed_rate_clamped_to_machine_max() {
        let config = MachineConfig {
            max_feed_mm_min: 500.0,
            ..Default::default()
        };
        let tool = test_tool();

        // Create segment with feed rate exceeding machine max
        let segments = vec![ToolpathSegment::cut(
            vec![
                DVec3::new(0.0, 0.0, 10.0),
                DVec3::new(50.0, 0.0, 10.0),
            ],
            2000.0, // exceeds 500 max
        )];

        let result = post_process(&segments, &config, &tool, 1, 12000.0, 2000.0);
        let output = result.to_string();

        // The feed in the output should be clamped to 500
        assert!(output.contains("F500"), "Feed should be clamped to machine max");
    }
}
