//! Shared toolpath types, contour operations, and G-code emitter.
//!
//! This crate provides the foundation used by all manufacturing pipelines:
//! - `contour`: 2D closed-loop geometry with offset, area, containment
//! - `contour_tree`: Nested contour hierarchy (outer boundaries + holes)
//! - `path`: Toolpath segments with feed rates and move types
//! - `tool`: Tool definitions (end mills, nozzles, lasers, drills)
//! - `material`: Work material properties and feeds/speeds calculations
//! - `gcode`: Typed G-code intermediate representation
//! - `post`: G-code dialect presets (Marlin, GRBL, Fanuc, etc.)

pub mod contour;
pub mod contour_tree;
pub mod gcode;
pub mod material;
pub mod path;
pub mod post;
pub mod tool;
pub mod adaptive;
pub mod simulation;

pub use contour::{chain_segments, Contour};
pub use contour_tree::{build_contour_tree, ContourTree};
pub use gcode::{GCodeProgram, GCommand};
pub use material::{WorkMaterial, calc_feed, calc_rpm, recommended_feeds_speeds};
pub use path::{MoveType, ToolRef, Toolpath, ToolpathSegment};
pub use post::GCodeDialect;
pub use tool::{Tool, ToolGeometry, ToolLibrary, ToolMaterial};
pub use adaptive::{
    AdaptiveStrategy, adaptive_clearing, rest_machining, pencil_finishing,
    adaptive_clear, rest_machine, pencil_finish,
    FeedSpeed, CncOperation, feeds_speeds_lut,
};
pub use simulation::{MachineSimulation, CollisionResult};
