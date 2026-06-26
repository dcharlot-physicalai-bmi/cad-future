//! Protocol error types.

use alloc::string::String;
use serde::{Deserialize, Serialize};

/// Protocol-level error.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProtocolError {
    pub code: i32,
    pub message: String,
}

impl core::fmt::Display for ProtocolError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "OMP error {}: {}", self.code, self.message)
    }
}
