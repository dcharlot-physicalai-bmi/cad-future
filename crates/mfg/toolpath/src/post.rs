//! G-code dialect / post-processor configuration.

use serde::{Deserialize, Serialize};

/// Comment formatting style.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum CommentStyle {
    /// (comment) — Fanuc, LinuxCNC, Haas
    Parentheses,
    /// ; comment — GRBL, Marlin, Klipper
    Semicolon,
}

/// G-code dialect configuration controlling output formatting.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GCodeDialect {
    pub name: String,
    /// Include line numbers (N10, N20, ...).
    pub line_numbers: bool,
    /// Decimal places for coordinates.
    pub decimal_places: usize,
    /// Whether the controller supports G2/G3 arc commands.
    pub arc_support: bool,
    /// Lines emitted before the program body.
    pub preamble: Vec<String>,
    /// Lines emitted after the program body.
    pub postamble: Vec<String>,
    /// Comment formatting style.
    pub comment_style: CommentStyle,
    /// Maximum S value for laser power (e.g., 255, 1000).
    pub max_laser_s: f64,
}

/// Marlin firmware (most FDM 3D printers).
pub fn marlin() -> GCodeDialect {
    GCodeDialect {
        name: "Marlin".into(),
        line_numbers: false,
        decimal_places: 3,
        arc_support: true,
        preamble: vec![],
        postamble: vec![],
        comment_style: CommentStyle::Semicolon,
        max_laser_s: 255.0,
    }
}

/// Klipper firmware (advanced 3D printers).
pub fn klipper() -> GCodeDialect {
    GCodeDialect {
        name: "Klipper".into(),
        line_numbers: false,
        decimal_places: 3,
        arc_support: false, // Klipper linearizes arcs
        preamble: vec![],
        postamble: vec![],
        comment_style: CommentStyle::Semicolon,
        max_laser_s: 255.0,
    }
}

/// GRBL (CNC controllers, laser engravers).
pub fn grbl() -> GCodeDialect {
    GCodeDialect {
        name: "GRBL".into(),
        line_numbers: false,
        decimal_places: 3,
        arc_support: true,
        preamble: vec![],
        postamble: vec![],
        comment_style: CommentStyle::Semicolon,
        max_laser_s: 1000.0,
    }
}

/// Fanuc (industrial CNC mills and lathes).
pub fn fanuc() -> GCodeDialect {
    GCodeDialect {
        name: "Fanuc".into(),
        line_numbers: true,
        decimal_places: 3,
        arc_support: true,
        preamble: vec![
            "%".into(),
            "O0001".into(),
        ],
        postamble: vec![
            "M30".into(),
            "%".into(),
        ],
        comment_style: CommentStyle::Parentheses,
        max_laser_s: 0.0,
    }
}

/// LinuxCNC.
pub fn linuxcnc() -> GCodeDialect {
    GCodeDialect {
        name: "LinuxCNC".into(),
        line_numbers: false,
        decimal_places: 4,
        arc_support: true,
        preamble: vec![],
        postamble: vec![],
        comment_style: CommentStyle::Parentheses,
        max_laser_s: 0.0,
    }
}

/// Haas (CNC mills).
pub fn haas() -> GCodeDialect {
    GCodeDialect {
        name: "Haas".into(),
        line_numbers: true,
        decimal_places: 4,
        arc_support: true,
        preamble: vec![
            "%".into(),
            "O00001".into(),
        ],
        postamble: vec![
            "M30".into(),
            "%".into(),
        ],
        comment_style: CommentStyle::Parentheses,
        max_laser_s: 0.0,
    }
}
