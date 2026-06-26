//! G-code intermediate representation and serialization.

use serde::{Deserialize, Serialize};

use crate::post::GCodeDialect;

/// A single G-code command (typed IR — not serialized until output).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum GCommand {
    // Motion
    /// G0: Rapid positioning.
    RapidMove {
        x: Option<f64>,
        y: Option<f64>,
        z: Option<f64>,
    },
    /// G1: Linear interpolation (cutting move).
    LinearMove {
        x: Option<f64>,
        y: Option<f64>,
        z: Option<f64>,
        f: f64,
    },
    /// G2: Clockwise arc.
    ArcCW {
        x: f64,
        y: f64,
        i: f64,
        j: f64,
        f: f64,
    },
    /// G3: Counter-clockwise arc.
    ArcCCW {
        x: f64,
        y: f64,
        i: f64,
        j: f64,
        f: f64,
    },
    /// G4: Dwell.
    Dwell { seconds: f64 },

    // Setup
    /// G21: Set units to millimeters.
    SetUnitsMetric,
    /// G90: Absolute positioning mode.
    AbsoluteMode,
    /// G91: Relative positioning mode.
    RelativeMode,
    /// G28: Home all axes.
    HomeAll,

    // Tool
    /// Tool change (T + M6).
    ToolChange { index: usize },
    /// M3/M4: Spindle on.
    SpindleOn { rpm: f64, cw: bool },
    /// M5: Spindle off.
    SpindleOff,
    /// M8: Coolant on.
    CoolantOn,
    /// M9: Coolant off.
    CoolantOff,

    // Program control
    /// M30: Program end.
    ProgramEnd,
    /// Comment line.
    Comment(String),

    // 3D printing specific
    /// M104/M109: Set extruder temperature.
    SetExtruderTemp { temp: f64, wait: bool },
    /// M140/M190: Set bed temperature.
    SetBedTemp { temp: f64, wait: bool },
    /// M106/M107: Set fan speed.
    SetFanSpeed { percent: f64 },
    /// G1 with extrusion: linear move with E parameter.
    ExtrudeMove {
        x: Option<f64>,
        y: Option<f64>,
        z: Option<f64>,
        e: f64,
        f: f64,
    },
    /// G92 E0: Reset extruder position.
    ResetExtruder,

    // Laser specific
    /// Set laser power (S value, 0-1000 or 0-255 depending on dialect).
    LaserPower { percent: f64 },
    /// Laser off (M5 or S0).
    LaserOff,

    // CNC-specific
    /// G43 H_: Tool length compensation.
    ToolLengthComp { index: usize },
    /// G49: Cancel tool length compensation.
    CancelToolLengthComp,
    /// G54-G59: Work offset.
    WorkOffset { index: u8 },
    /// G81: Simple drilling canned cycle.
    DrillCycle {
        x: f64,
        y: f64,
        z: f64,
        r: f64,
        f: f64,
    },
    /// G83: Peck drilling canned cycle.
    PeckDrillCycle {
        x: f64,
        y: f64,
        z: f64,
        r: f64,
        q: f64,
        f: f64,
    },
    /// G84: Tapping canned cycle.
    TapCycle {
        x: f64,
        y: f64,
        z: f64,
        r: f64,
        f: f64,
    },
    /// G80: Cancel canned cycle.
    CancelCannedCycle,
    /// G53 Z0: Machine coordinate safe retract.
    SafeRetractG53,
    /// M7: Mist coolant on.
    MistCoolantOn,

    // Passthrough
    /// Raw G-code string.
    Raw(String),
}

/// A complete G-code program.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GCodeProgram {
    pub commands: Vec<GCommand>,
    pub header_comments: Vec<String>,
}

impl GCodeProgram {
    pub fn new() -> Self {
        Self {
            commands: Vec::new(),
            header_comments: Vec::new(),
        }
    }

    pub fn push(&mut self, cmd: GCommand) {
        self.commands.push(cmd);
    }

    pub fn comment(&mut self, text: impl Into<String>) {
        self.commands.push(GCommand::Comment(text.into()));
    }

    /// Serialize to G-code string using the given dialect.
    pub fn to_string(&self, dialect: &GCodeDialect) -> String {
        let mut out = String::new();
        let d = dialect.decimal_places;

        // Header comments
        for c in &self.header_comments {
            out.push_str(&format_comment(c, dialect));
            out.push('\n');
        }

        // Preamble
        for line in &dialect.preamble {
            out.push_str(line);
            out.push('\n');
        }

        // Commands
        let mut line_num: u32 = 10;
        for cmd in &self.commands {
            let line = format_command(cmd, d, dialect);
            if dialect.line_numbers {
                out.push_str(&format!("N{line_num} {line}\n"));
                line_num += 10;
            } else {
                out.push_str(&line);
                out.push('\n');
            }
        }

        // Postamble
        for line in &dialect.postamble {
            out.push_str(line);
            out.push('\n');
        }

        out
    }

    /// Estimated print/machining time in seconds (assumes instant acceleration).
    pub fn estimated_time_s(&self, rapid_feed_mm_min: f64) -> f64 {
        let mut time = 0.0;
        let mut pos = (0.0_f64, 0.0_f64, 0.0_f64);

        for cmd in &self.commands {
            match cmd {
                GCommand::RapidMove { x, y, z } => {
                    let nx = x.unwrap_or(pos.0);
                    let ny = y.unwrap_or(pos.1);
                    let nz = z.unwrap_or(pos.2);
                    let dist = ((nx - pos.0).powi(2) + (ny - pos.1).powi(2) + (nz - pos.2).powi(2)).sqrt();
                    if rapid_feed_mm_min > 0.0 {
                        time += dist / (rapid_feed_mm_min / 60.0);
                    }
                    pos = (nx, ny, nz);
                }
                GCommand::LinearMove { x, y, z, f } | GCommand::ExtrudeMove { x, y, z, f, .. } => {
                    let nx = x.unwrap_or(pos.0);
                    let ny = y.unwrap_or(pos.1);
                    let nz = z.unwrap_or(pos.2);
                    let dist = ((nx - pos.0).powi(2) + (ny - pos.1).powi(2) + (nz - pos.2).powi(2)).sqrt();
                    if *f > 0.0 {
                        time += dist / (*f / 60.0);
                    }
                    pos = (nx, ny, nz);
                }
                GCommand::Dwell { seconds } => {
                    time += seconds;
                }
                _ => {}
            }
        }

        time
    }

    /// Total extrusion in mm (for 3D printing).
    pub fn total_extrusion_mm(&self) -> f64 {
        let mut total = 0.0;
        let mut last_e = 0.0;
        for cmd in &self.commands {
            match cmd {
                GCommand::ExtrudeMove { e, .. } => {
                    if *e > last_e {
                        total += e - last_e;
                    }
                    last_e = *e;
                }
                GCommand::ResetExtruder => {
                    last_e = 0.0;
                }
                _ => {}
            }
        }
        total
    }

    /// Number of layers (counted by Z-change moves).
    pub fn layer_count(&self) -> usize {
        let mut count = 0;
        let mut last_z = f64::NEG_INFINITY;
        for cmd in &self.commands {
            let z = match cmd {
                GCommand::RapidMove { z: Some(z), .. } => Some(*z),
                GCommand::LinearMove { z: Some(z), .. } => Some(*z),
                GCommand::ExtrudeMove { z: Some(z), .. } => Some(*z),
                _ => None,
            };
            if let Some(z) = z {
                if (z - last_z).abs() > 1e-6 && z > last_z {
                    count += 1;
                    last_z = z;
                }
            }
        }
        count
    }
}

impl Default for GCodeProgram {
    fn default() -> Self {
        Self::new()
    }
}

fn format_comment(text: &str, dialect: &GCodeDialect) -> String {
    match dialect.comment_style {
        crate::post::CommentStyle::Parentheses => format!("({text})"),
        crate::post::CommentStyle::Semicolon => format!("; {text}"),
    }
}

fn format_command(cmd: &GCommand, d: usize, dialect: &GCodeDialect) -> String {
    match cmd {
        GCommand::RapidMove { x, y, z } => {
            let mut s = "G0".to_string();
            if let Some(v) = x { s.push_str(&format!(" X{v:.d$}")); }
            if let Some(v) = y { s.push_str(&format!(" Y{v:.d$}")); }
            if let Some(v) = z { s.push_str(&format!(" Z{v:.d$}")); }
            s
        }
        GCommand::LinearMove { x, y, z, f } => {
            let mut s = "G1".to_string();
            if let Some(v) = x { s.push_str(&format!(" X{v:.d$}")); }
            if let Some(v) = y { s.push_str(&format!(" Y{v:.d$}")); }
            if let Some(v) = z { s.push_str(&format!(" Z{v:.d$}")); }
            s.push_str(&format!(" F{f:.0}"));
            s
        }
        GCommand::ArcCW { x, y, i, j, f } => {
            format!("G2 X{x:.d$} Y{y:.d$} I{i:.d$} J{j:.d$} F{f:.0}")
        }
        GCommand::ArcCCW { x, y, i, j, f } => {
            format!("G3 X{x:.d$} Y{y:.d$} I{i:.d$} J{j:.d$} F{f:.0}")
        }
        GCommand::Dwell { seconds } => format!("G4 P{:.1}", seconds * 1000.0),
        GCommand::SetUnitsMetric => "G21".to_string(),
        GCommand::AbsoluteMode => "G90".to_string(),
        GCommand::RelativeMode => "G91".to_string(),
        GCommand::HomeAll => "G28".to_string(),
        GCommand::ToolChange { index } => format!("T{index}\nM6"),
        GCommand::SpindleOn { rpm, cw } => {
            format!("{} S{rpm:.0}", if *cw { "M3" } else { "M4" })
        }
        GCommand::SpindleOff => "M5".to_string(),
        GCommand::CoolantOn => "M8".to_string(),
        GCommand::CoolantOff => "M9".to_string(),
        GCommand::ProgramEnd => "M30".to_string(),
        GCommand::Comment(text) => format_comment(text, dialect),
        GCommand::SetExtruderTemp { temp, wait } => {
            if *wait { format!("M109 S{temp:.0}") } else { format!("M104 S{temp:.0}") }
        }
        GCommand::SetBedTemp { temp, wait } => {
            if *wait { format!("M190 S{temp:.0}") } else { format!("M140 S{temp:.0}") }
        }
        GCommand::SetFanSpeed { percent } => {
            let pwm = (percent / 100.0 * 255.0) as u8;
            if pwm == 0 { "M107".to_string() } else { format!("M106 S{pwm}") }
        }
        GCommand::ExtrudeMove { x, y, z, e, f } => {
            let mut s = "G1".to_string();
            if let Some(v) = x { s.push_str(&format!(" X{v:.d$}")); }
            if let Some(v) = y { s.push_str(&format!(" Y{v:.d$}")); }
            if let Some(v) = z { s.push_str(&format!(" Z{v:.d$}")); }
            s.push_str(&format!(" E{e:.5}"));
            s.push_str(&format!(" F{f:.0}"));
            s
        }
        GCommand::ResetExtruder => "G92 E0".to_string(),
        GCommand::LaserPower { percent } => {
            let s_value = (percent / 100.0 * 1000.0) as u16;
            format!("M3 S{s_value}")
        }
        GCommand::LaserOff => "M5".to_string(),
        GCommand::ToolLengthComp { index } => format!("G43 H{index}"),
        GCommand::CancelToolLengthComp => "G49".to_string(),
        GCommand::WorkOffset { index } => format!("G{}", 53 + index),
        GCommand::DrillCycle { x, y, z, r, f } => {
            format!("G81 X{x:.d$} Y{y:.d$} Z{z:.d$} R{r:.d$} F{f:.0}")
        }
        GCommand::PeckDrillCycle { x, y, z, r, q, f } => {
            format!("G83 X{x:.d$} Y{y:.d$} Z{z:.d$} R{r:.d$} Q{q:.d$} F{f:.0}")
        }
        GCommand::TapCycle { x, y, z, r, f } => {
            format!("G84 X{x:.d$} Y{y:.d$} Z{z:.d$} R{r:.d$} F{f:.0}")
        }
        GCommand::CancelCannedCycle => "G80".to_string(),
        GCommand::SafeRetractG53 => "G53 Z0".to_string(),
        GCommand::MistCoolantOn => "M7".to_string(),
        GCommand::Raw(s) => s.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::post;

    #[test]
    fn basic_gcode_output() {
        let mut prog = GCodeProgram::new();
        prog.push(GCommand::SetUnitsMetric);
        prog.push(GCommand::AbsoluteMode);
        prog.push(GCommand::RapidMove { x: Some(10.0), y: Some(20.0), z: None });
        prog.push(GCommand::LinearMove { x: Some(30.0), y: Some(40.0), z: None, f: 1000.0 });
        prog.push(GCommand::ProgramEnd);

        let out = prog.to_string(&post::marlin());
        assert!(out.contains("G21"));
        assert!(out.contains("G90"));
        assert!(out.contains("G0 X10.000 Y20.000"));
        assert!(out.contains("G1 X30.000 Y40.000 F1000"));
        assert!(out.contains("M30"));
    }

    #[test]
    fn estimated_time() {
        let mut prog = GCodeProgram::new();
        // Move 60mm at 3600mm/min = 1 second
        prog.push(GCommand::LinearMove {
            x: Some(60.0), y: Some(0.0), z: Some(0.0), f: 3600.0,
        });
        let time = prog.estimated_time_s(6000.0);
        assert!((time - 1.0).abs() < 0.01);
    }

    #[test]
    fn extrusion_tracking() {
        let mut prog = GCodeProgram::new();
        prog.push(GCommand::ExtrudeMove { x: Some(10.0), y: None, z: None, e: 1.5, f: 1000.0 });
        prog.push(GCommand::ExtrudeMove { x: Some(20.0), y: None, z: None, e: 3.0, f: 1000.0 });
        prog.push(GCommand::ResetExtruder);
        prog.push(GCommand::ExtrudeMove { x: Some(30.0), y: None, z: None, e: 0.5, f: 1000.0 });
        assert!((prog.total_extrusion_mm() - 3.5).abs() < 1e-8);
    }
}
