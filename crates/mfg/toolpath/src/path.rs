//! Toolpath types — segments with feed rates and move types.

use glam::DVec3;
use serde::{Deserialize, Serialize};

/// Type of motion for a toolpath segment.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MoveType {
    /// G0: rapid traverse (no cutting).
    Rapid,
    /// G1: linear cutting move.
    Cut,
    /// G1 Z-only downward: plunge into material.
    Plunge,
    /// G0 Z-only upward: retract from material.
    Retract,
    /// G2/G3: arc interpolation.
    Arc { cw: bool },
}

/// A single toolpath segment — a polyline with associated feed rate and move type.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ToolpathSegment {
    /// 3D points along this segment.
    pub path: Vec<DVec3>,
    /// Feed rate in mm/min.
    pub feed_rate: f64,
    /// Type of motion.
    pub move_type: MoveType,
}

impl ToolpathSegment {
    /// Create a rapid move between two points.
    pub fn rapid(from: DVec3, to: DVec3) -> Self {
        Self {
            path: vec![from, to],
            feed_rate: 0.0, // Rapids have no programmed feed
            move_type: MoveType::Rapid,
        }
    }

    /// Create a linear cutting move.
    pub fn cut(points: Vec<DVec3>, feed_rate: f64) -> Self {
        Self {
            path: points,
            feed_rate,
            move_type: MoveType::Cut,
        }
    }

    /// Total length of this segment.
    pub fn length(&self) -> f64 {
        let mut len = 0.0;
        for i in 1..self.path.len() {
            len += (self.path[i] - self.path[i - 1]).length();
        }
        len
    }

    /// Estimated time in seconds (assumes instant acceleration).
    pub fn estimated_time_s(&self, rapid_feed: f64) -> f64 {
        let feed = if self.move_type == MoveType::Rapid {
            rapid_feed
        } else {
            self.feed_rate
        };
        if feed <= 0.0 {
            return 0.0;
        }
        self.length() / (feed / 60.0) // feed is mm/min, convert to mm/s
    }
}

/// Reference to a tool by index in a tool library.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct ToolRef(pub usize);

/// A complete toolpath — ordered sequence of segments with a tool reference.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Toolpath {
    /// Ordered segments.
    pub segments: Vec<ToolpathSegment>,
    /// Tool used for this toolpath.
    pub tool: ToolRef,
    /// Human-readable label.
    pub label: String,
}

impl Toolpath {
    /// Total cutting length (excludes rapids).
    pub fn cutting_length(&self) -> f64 {
        self.segments
            .iter()
            .filter(|s| s.move_type == MoveType::Cut)
            .map(|s| s.length())
            .sum()
    }

    /// Estimated total time in seconds.
    pub fn estimated_time_s(&self, rapid_feed: f64) -> f64 {
        self.segments.iter().map(|s| s.estimated_time_s(rapid_feed)).sum()
    }
}
