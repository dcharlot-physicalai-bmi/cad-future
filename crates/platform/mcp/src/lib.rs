//! `physical-mcp` — MCP tool server for AI agent integration.
//!
//! Design principles (AI ergonomics — "path of least resistance"):
//!
//! 1. **Flat params**: No nested objects. Every param is a scalar, string, or flat array.
//!    AI models generate flat JSON trivially; nested schemas cause hallucination.
//!
//! 2. **Intent-matching names**: Tool names match how an AI phrases the goal.
//!    "create_box" not "instantiate_brep_with_planar_topology".
//!    "check_manufacturability" not "run_dfm_validation_pipeline".
//!
//! 3. **Generous defaults**: Minimize required params. A box needs width/height/depth.
//!    Material, position, name — all optional with sensible defaults.
//!
//! 4. **One tool per intent**: Don't split "analyze part" into 5 calls.
//!    `analyze_part` returns mass, centroid, bounding box, surface area in ONE response.
//!    If the AI has to chain 3 calls, it will fake the answer instead.
//!
//! 5. **Immediately useful returns**: Return human-readable summaries alongside raw data.
//!    The AI can relay the summary directly without parsing.
//!
//! 6. **Impossible to bypass**: For operations the AI literally cannot fake (booleans,
//!    FEA, export), the tool must be the ONLY path. For operations it COULD fake
//!    (stress formulas), the tool must be EASIER than computing.
//!
//! 7. **Error messages are instructions**: When a tool fails, the error tells the AI
//!    exactly what to fix. "width must be > 0" not "InvalidArgument".

pub mod registry;
pub mod tools;
pub mod types;
pub mod session;

pub use registry::{ToolRegistry, ToolDef, ToolParam, ParamType};
pub use session::McpSession;
