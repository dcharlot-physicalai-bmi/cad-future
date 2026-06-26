//! Job lifecycle — first-class state machine with structured transitions.
//!
//! Every job follows a deterministic state machine. No guessing from
//! string states, no protocol-specific quirks.

use alloc::string::String;
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

/// Job state machine.
///
/// ```text
///                  ┌──────────┐
///         ┌───────►│ Queued   │
///         │        └────┬─────┘
///         │             │ start
///         │             ▼
///         │        ┌──────────┐     pause    ┌──────────┐
///         │        │ Running  │─────────────►│ Paused   │
///         │        └──┬───┬───┘              └────┬─────┘
///         │           │   │  ▲                    │ resume
///         │           │   │  └────────────────────┘
///         │    cancel │   │ complete
///         │           ▼   ▼
///         │    ┌──────────┐  ┌──────────┐
///         │    │Cancelled │  │ Complete │
///         │    └──────────┘  └──────────┘
///         │
///    error│    ┌──────────┐
///    (any)└───►│ Failed   │
///              └──────────┘
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobState {
    /// Job received, waiting to start.
    Queued,
    /// Job is actively executing.
    Running,
    /// Job paused by user or machine.
    Paused,
    /// Job completed successfully.
    Complete,
    /// Job cancelled by user.
    Cancelled,
    /// Job failed due to error.
    Failed,
}

impl JobState {
    /// Whether the job is in a terminal state.
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Complete | Self::Cancelled | Self::Failed)
    }

    /// Whether a transition from self to target is valid.
    pub fn can_transition_to(&self, target: JobState) -> bool {
        match (self, target) {
            // From queued: can start, cancel, or fail
            (Self::Queued, Self::Running) => true,
            (Self::Queued, Self::Cancelled) => true,
            (Self::Queued, Self::Failed) => true,
            // From running: can pause, complete, cancel, or fail
            (Self::Running, Self::Paused) => true,
            (Self::Running, Self::Complete) => true,
            (Self::Running, Self::Cancelled) => true,
            (Self::Running, Self::Failed) => true,
            // From paused: can resume, cancel, or fail
            (Self::Paused, Self::Running) => true,
            (Self::Paused, Self::Cancelled) => true,
            (Self::Paused, Self::Failed) => true,
            // Terminal states: no transitions
            _ => false,
        }
    }
}

/// Request to submit a job.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct JobSubmitRequest {
    /// Display name for the job.
    pub name: String,
    /// File format MIME type.
    pub mime_type: String,
    /// File extension.
    pub extension: String,
    /// Total file size in bytes.
    pub size_bytes: u64,
    /// Start printing immediately after upload completes.
    pub auto_start: bool,
    /// Optional metadata (material, layer height, etc.).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<JobMetadata>,
}

/// Response to job submission.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct JobSubmitResponse {
    /// Assigned job ID.
    pub job_id: String,
    /// Upload ID for binary frame transfer.
    pub upload_id: u64,
    /// Maximum chunk size for binary frames.
    pub max_chunk_bytes: u32,
}

/// Optional job metadata.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct JobMetadata {
    /// Estimated print time in seconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub estimated_time_s: Option<f64>,
    /// Estimated material usage (grams for filament, ml for resin).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub material_amount: Option<f64>,
    /// Material name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub material_name: Option<String>,
    /// Layer height in mm.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layer_height_mm: Option<f64>,
    /// Total layer count.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total_layers: Option<u32>,
    /// Nozzle diameter in mm.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nozzle_diameter_mm: Option<f64>,
}

/// Full job status (returned by job.status).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct JobStatus {
    /// Job ID.
    pub job_id: String,
    /// Current state.
    pub state: JobState,
    /// Filename.
    pub filename: String,
    /// Progress percentage (0.0 - 100.0).
    pub progress_pct: f64,
    /// Elapsed time in seconds.
    pub elapsed_s: f64,
    /// Estimated remaining time in seconds.
    pub remaining_s: Option<f64>,
    /// Current layer info.
    pub layer: Option<super::status::LayerInfo>,
    /// Metadata provided at submission.
    pub metadata: Option<JobMetadata>,
    /// Error details if state is Failed.
    pub error: Option<JobError>,
    /// History of state transitions.
    pub transitions: Vec<StateTransition>,
}

/// Job error details.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct JobError {
    /// Error code.
    pub code: u32,
    /// Error category.
    pub category: super::status::ErrorCategory,
    /// Human-readable message.
    pub message: String,
    /// Line number in G-code where error occurred.
    pub gcode_line: Option<u64>,
}

/// Record of a state transition.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StateTransition {
    /// State transitioned from.
    pub from: JobState,
    /// State transitioned to.
    pub to: JobState,
    /// Timestamp (seconds since job creation).
    pub at_s: f64,
    /// Reason for transition (e.g., "user_request", "thermal_runaway").
    pub reason: String,
}

/// Job queue entry.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct JobQueueEntry {
    pub job_id: String,
    pub state: JobState,
    pub filename: String,
    pub progress_pct: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_transitions() {
        assert!(JobState::Queued.can_transition_to(JobState::Running));
        assert!(JobState::Running.can_transition_to(JobState::Paused));
        assert!(JobState::Running.can_transition_to(JobState::Complete));
        assert!(JobState::Paused.can_transition_to(JobState::Running));
        assert!(JobState::Paused.can_transition_to(JobState::Cancelled));
    }

    #[test]
    fn invalid_transitions() {
        assert!(!JobState::Complete.can_transition_to(JobState::Running));
        assert!(!JobState::Cancelled.can_transition_to(JobState::Running));
        assert!(!JobState::Failed.can_transition_to(JobState::Running));
        assert!(!JobState::Queued.can_transition_to(JobState::Complete));
    }

    #[test]
    fn terminal_states() {
        assert!(JobState::Complete.is_terminal());
        assert!(JobState::Cancelled.is_terminal());
        assert!(JobState::Failed.is_terminal());
        assert!(!JobState::Running.is_terminal());
        assert!(!JobState::Paused.is_terminal());
        assert!(!JobState::Queued.is_terminal());
    }

    #[test]
    fn submit_request_roundtrip() {
        let req = JobSubmitRequest {
            name: "benchy.gcode".into(),
            mime_type: "application/x-gcode".into(),
            extension: "gcode".into(),
            size_bytes: 1024 * 1024,
            auto_start: true,
            metadata: Some(JobMetadata {
                estimated_time_s: Some(3600.0),
                material_amount: Some(15.0),
                material_name: Some("PLA".into()),
                layer_height_mm: Some(0.2),
                total_layers: Some(200),
                nozzle_diameter_mm: Some(0.4),
            }),
        };
        let json = serde_json::to_string(&req).unwrap();
        let parsed: JobSubmitRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.name, "benchy.gcode");
        assert!(parsed.auto_start);
        assert_eq!(parsed.metadata.unwrap().total_layers, Some(200));
    }

    #[test]
    fn any_state_can_fail() {
        for state in [JobState::Queued, JobState::Running, JobState::Paused] {
            assert!(state.can_transition_to(JobState::Failed), "{state:?} should be able to fail");
        }
    }
}
