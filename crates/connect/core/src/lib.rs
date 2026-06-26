//! Core traits and types for manufacturing machine connectivity.
//!
//! Defines the `MachineConnection` trait that all protocol drivers implement,
//! plus shared types for machine identity, status, job management, and discovery.

mod types;
mod error;
mod registry;

pub use types::*;
pub use error::ConnectError;
pub use registry::MachineRegistry;

use async_trait::async_trait;
use std::time::Duration;

/// Primary trait for communicating with a manufacturing machine.
///
/// Each protocol driver (OctoPrint, Moonraker, Bambu, etc.) implements this
/// trait to provide a uniform interface for job submission, status monitoring,
/// and manual control.
#[async_trait]
pub trait MachineConnection: Send + Sync {
    /// Static information about this machine.
    fn info(&self) -> &MachineInfo;

    /// Check if the machine is reachable.
    async fn ping(&self) -> Result<(), ConnectError>;

    /// Query current machine status (temperatures, position, state).
    async fn status(&self) -> Result<MachineStatus, ConnectError>;

    /// Submit a job (G-code, 3MF, etc.) to the machine.
    async fn submit_job(&self, job: JobSubmission) -> Result<JobHandle, ConnectError>;

    /// Cancel a running or queued job.
    async fn cancel_job(&self, handle: &JobHandle) -> Result<(), ConnectError>;

    /// Pause a running job.
    async fn pause_job(&self, handle: &JobHandle) -> Result<(), ConnectError>;

    /// Resume a paused job.
    async fn resume_job(&self, handle: &JobHandle) -> Result<(), ConnectError>;

    /// Query the status of a specific job.
    async fn job_status(&self, handle: &JobHandle) -> Result<JobStatus, ConnectError>;

    /// Send a raw command (G-code line) and return the response.
    async fn send_command(&self, cmd: &str) -> Result<String, ConnectError>;

    /// Gracefully disconnect from the machine.
    async fn disconnect(&mut self) -> Result<(), ConnectError>;
}

/// Trait for discovering machines on the network.
///
/// Implementations scan for specific protocols (mDNS, UDP broadcast, serial enumeration).
#[async_trait]
pub trait MachineDiscovery: Send + Sync {
    /// Human-readable protocol name (e.g., "OctoPrint", "Moonraker").
    fn protocol_name(&self) -> &str;

    /// Scan the network for machines, returning discovered machine info.
    async fn discover(&self, timeout: Duration) -> Result<Vec<DiscoveredMachine>, ConnectError>;
}
