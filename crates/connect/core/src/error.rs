//! Error types for machine connectivity.

use thiserror::Error;

/// Errors that can occur when communicating with a machine.
#[derive(Debug, Error)]
pub enum ConnectError {
    /// Machine is not reachable on the network.
    #[error("connection refused: {0}")]
    ConnectionRefused(String),

    /// Authentication failed (wrong API key, password, etc.).
    #[error("authentication failed: {0}")]
    AuthFailed(String),

    /// Operation timed out.
    #[error("timeout: {0}")]
    Timeout(String),

    /// Protocol-level error (unexpected response, malformed data).
    #[error("protocol error: {0}")]
    Protocol(String),

    /// The requested operation is not supported by this machine/protocol.
    #[error("unsupported: {0}")]
    Unsupported(String),

    /// The requested file format is not accepted by this machine.
    #[error("format not accepted: {0}")]
    FormatNotAccepted(String),

    /// Machine is in an error state and cannot accept commands.
    #[error("machine error: {0}")]
    MachineError(String),

    /// Job not found.
    #[error("job not found: {0}")]
    JobNotFound(String),

    /// I/O error (serial port, file system).
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON serialization/deserialization error.
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
}
