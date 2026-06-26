//! Serializable types for MCP tool inputs and outputs.
//!
//! These are the wire types — flat, JSON-friendly, no glam/slotmap.
//! AI models produce and consume these directly.

use serde::{Deserialize, Serialize};

/// Unique handle for a solid in the current session.
/// Opaque string — AI doesn't need to know the internal representation.
pub type SolidHandle = String;

/// Unique handle for a sketch in the current session.
pub type SketchHandle = String;

/// Unique handle for a parametric document.
pub type DocHandle = String;

// ---------------------------------------------------------------------------
// Tool Results — always include a human-readable `summary` field
// ---------------------------------------------------------------------------

/// Standard tool response wrapper.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    /// Human-readable summary the AI can relay directly to the user.
    pub summary: String,
    /// Structured data (tool-specific).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
    /// Handle to a created/modified solid (if applicable).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub solid: Option<SolidHandle>,
    /// Handle to a created/modified sketch (if applicable).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sketch: Option<SketchHandle>,
}

/// Error response — the message tells the AI exactly what to fix.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolError {
    /// What went wrong, in terms the AI can act on.
    pub message: String,
    /// Which parameter caused the issue (if applicable).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub param: Option<String>,
    /// Suggested fix.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggestion: Option<String>,
}

impl std::fmt::Display for ToolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)?;
        if let Some(ref param) = self.param {
            write!(f, " (param: {})", param)?;
        }
        if let Some(ref suggestion) = self.suggestion {
            write!(f, " — try: {}", suggestion)?;
        }
        Ok(())
    }
}
