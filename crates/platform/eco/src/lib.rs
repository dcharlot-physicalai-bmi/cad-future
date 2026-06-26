//! Engineering Change Order workflow — ECR/ECO/ECN process management.
//!
//! Models the full lifecycle: a Change Request (ECR) proposes a change,
//! a Change Order (ECO) tracks approval and implementation, and a
//! Change Notice (ECN) announces the completed change.

use serde::{Deserialize, Serialize};

/// Current state of an ECO workflow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EcoState {
    Draft,
    Submitted,
    InReview,
    Approved,
    Implemented,
    Closed,
    Rejected,
}

/// Decision made by an approver.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ApprovalDecision {
    Approve,
    Reject,
    NeedsInfo,
}

/// An approval record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Approval {
    pub approver: String,
    pub role: String,
    pub decision: ApprovalDecision,
    pub timestamp: u64,
    pub comments: String,
}

/// Engineering Change Request — proposes a change.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeRequest {
    pub id: u64,
    pub title: String,
    pub reason: String,
    pub impact: String,
    pub requestor: String,
    pub created_at: u64,
}

/// Engineering Change Order — approved change with implementation details.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeOrder {
    pub id: u64,
    pub ecr_id: u64,
    pub title: String,
    pub state: EcoState,
    pub affected_parts: Vec<String>,
    pub instructions: String,
    pub approvals: Vec<Approval>,
    pub created_at: u64,
}

/// Engineering Change Notice — notification that a change is complete.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeNotice {
    pub id: u64,
    pub eco_id: u64,
    pub title: String,
    pub summary: String,
    pub notified_at: u64,
}

/// Error type for workflow transitions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EcoError {
    InvalidTransition { from: EcoState, to: EcoState },
    EcoNotFound(u64),
    EcrNotFound(u64),
    AlreadyExists(u64),
}

impl std::fmt::Display for EcoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidTransition { from, to } => {
                write!(f, "invalid transition from {from:?} to {to:?}")
            }
            Self::EcoNotFound(id) => write!(f, "ECO {id} not found"),
            Self::EcrNotFound(id) => write!(f, "ECR {id} not found"),
            Self::AlreadyExists(id) => write!(f, "ID {id} already exists"),
        }
    }
}

impl std::error::Error for EcoError {}

/// Manages the full ECR -> ECO -> ECN workflow.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EcoWorkflow {
    ecrs: Vec<ChangeRequest>,
    ecos: Vec<ChangeOrder>,
    ecns: Vec<ChangeNotice>,
    next_id: u64,
}

impl EcoWorkflow {
    pub fn new() -> Self {
        Self {
            ecrs: Vec::new(),
            ecos: Vec::new(),
            ecns: Vec::new(),
            next_id: 1,
        }
    }

    fn alloc_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    /// Create a new change request.
    pub fn create_ecr(
        &mut self,
        title: String,
        reason: String,
        impact: String,
        requestor: String,
        timestamp: u64,
    ) -> u64 {
        let id = self.alloc_id();
        self.ecrs.push(ChangeRequest {
            id,
            title,
            reason,
            impact,
            requestor,
            created_at: timestamp,
        });
        id
    }

    /// Create an ECO from an existing ECR.
    pub fn create_eco(
        &mut self,
        ecr_id: u64,
        affected_parts: Vec<String>,
        instructions: String,
        timestamp: u64,
    ) -> Result<u64, EcoError> {
        let ecr_title = self
            .ecrs
            .iter()
            .find(|r| r.id == ecr_id)
            .map(|r| r.title.clone())
            .ok_or(EcoError::EcrNotFound(ecr_id))?;
        let id = self.alloc_id();
        self.ecos.push(ChangeOrder {
            id,
            ecr_id,
            title: ecr_title,
            state: EcoState::Draft,
            affected_parts,
            instructions,
            approvals: Vec::new(),
            created_at: timestamp,
        });
        Ok(id)
    }

    /// Valid state transitions.
    fn is_valid_transition(from: EcoState, to: EcoState) -> bool {
        matches!(
            (from, to),
            (EcoState::Draft, EcoState::Submitted)
                | (EcoState::Submitted, EcoState::InReview)
                | (EcoState::InReview, EcoState::Approved)
                | (EcoState::InReview, EcoState::Rejected)
                | (EcoState::Approved, EcoState::Implemented)
                | (EcoState::Implemented, EcoState::Closed)
        )
    }

    /// Transition an ECO to a new state.
    pub fn transition(
        &mut self,
        eco_id: u64,
        new_state: EcoState,
    ) -> Result<(), EcoError> {
        let eco = self
            .ecos
            .iter_mut()
            .find(|e| e.id == eco_id)
            .ok_or(EcoError::EcoNotFound(eco_id))?;
        if !Self::is_valid_transition(eco.state, new_state) {
            return Err(EcoError::InvalidTransition {
                from: eco.state,
                to: new_state,
            });
        }
        eco.state = new_state;
        Ok(())
    }

    /// Add an approval to an ECO.
    pub fn add_approval(
        &mut self,
        eco_id: u64,
        approver: String,
        role: String,
        decision: ApprovalDecision,
        timestamp: u64,
        comments: String,
    ) -> Result<(), EcoError> {
        let eco = self
            .ecos
            .iter_mut()
            .find(|e| e.id == eco_id)
            .ok_or(EcoError::EcoNotFound(eco_id))?;
        eco.approvals.push(Approval {
            approver,
            role,
            decision,
            timestamp,
            comments,
        });
        Ok(())
    }

    /// Create an ECN from an implemented ECO.
    pub fn create_ecn(
        &mut self,
        eco_id: u64,
        summary: String,
        timestamp: u64,
    ) -> Result<u64, EcoError> {
        let eco_title = self
            .ecos
            .iter()
            .find(|e| e.id == eco_id)
            .map(|e| e.title.clone())
            .ok_or(EcoError::EcoNotFound(eco_id))?;
        let id = self.alloc_id();
        self.ecns.push(ChangeNotice {
            id,
            eco_id,
            title: eco_title,
            summary,
            notified_at: timestamp,
        });
        Ok(id)
    }

    /// Produce a summary of what an ECO affects.
    pub fn impact_summary(&self, eco_id: u64) -> Result<String, EcoError> {
        let eco = self
            .ecos
            .iter()
            .find(|e| e.id == eco_id)
            .ok_or(EcoError::EcoNotFound(eco_id))?;
        let ecr = self.ecrs.iter().find(|r| r.id == eco.ecr_id);
        let mut out = format!("ECO-{}: {}\n", eco.id, eco.title);
        out.push_str(&format!("State: {:?}\n", eco.state));
        if let Some(ecr) = ecr {
            out.push_str(&format!("Reason: {}\n", ecr.reason));
            out.push_str(&format!("Impact: {}\n", ecr.impact));
        }
        out.push_str(&format!(
            "Affected parts: {}\n",
            if eco.affected_parts.is_empty() {
                "none".to_string()
            } else {
                eco.affected_parts.join(", ")
            }
        ));
        out.push_str(&format!("Approvals: {}\n", eco.approvals.len()));
        Ok(out)
    }

    pub fn ecrs(&self) -> &[ChangeRequest] {
        &self.ecrs
    }

    pub fn ecos(&self) -> &[ChangeOrder] {
        &self.ecos
    }

    pub fn ecns(&self) -> &[ChangeNotice] {
        &self.ecns
    }

    pub fn get_eco(&self, id: u64) -> Option<&ChangeOrder> {
        self.ecos.iter().find(|e| e.id == id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_workflow() -> (EcoWorkflow, u64, u64) {
        let mut wf = EcoWorkflow::new();
        let ecr_id = wf.create_ecr(
            "Fix bracket".into(),
            "Stress crack in field".into(),
            "Affects main assembly".into(),
            "jdoe".into(),
            1000,
        );
        let eco_id = wf
            .create_eco(
                ecr_id,
                vec!["BRK-001".into(), "ASM-100".into()],
                "Increase fillet radius to R5mm".into(),
                1001,
            )
            .unwrap();
        (wf, ecr_id, eco_id)
    }

    #[test]
    fn test_create_ecr() {
        let mut wf = EcoWorkflow::new();
        let id = wf.create_ecr(
            "Title".into(),
            "Reason".into(),
            "Impact".into(),
            "req".into(),
            100,
        );
        assert_eq!(wf.ecrs().len(), 1);
        assert_eq!(wf.ecrs()[0].id, id);
    }

    #[test]
    fn test_create_eco_from_ecr() {
        let (wf, _, eco_id) = setup_workflow();
        let eco = wf.get_eco(eco_id).unwrap();
        assert_eq!(eco.state, EcoState::Draft);
        assert_eq!(eco.affected_parts.len(), 2);
    }

    #[test]
    fn test_create_eco_invalid_ecr() {
        let mut wf = EcoWorkflow::new();
        let result = wf.create_eco(999, vec![], "".into(), 0);
        assert_eq!(result, Err(EcoError::EcrNotFound(999)));
    }

    #[test]
    fn test_valid_state_transitions() {
        let (mut wf, _, eco_id) = setup_workflow();
        wf.transition(eco_id, EcoState::Submitted).unwrap();
        wf.transition(eco_id, EcoState::InReview).unwrap();
        wf.transition(eco_id, EcoState::Approved).unwrap();
        wf.transition(eco_id, EcoState::Implemented).unwrap();
        wf.transition(eco_id, EcoState::Closed).unwrap();
        assert_eq!(wf.get_eco(eco_id).unwrap().state, EcoState::Closed);
    }

    #[test]
    fn test_invalid_state_transition() {
        let (mut wf, _, eco_id) = setup_workflow();
        let result = wf.transition(eco_id, EcoState::Approved);
        assert!(result.is_err());
    }

    #[test]
    fn test_rejection_path() {
        let (mut wf, _, eco_id) = setup_workflow();
        wf.transition(eco_id, EcoState::Submitted).unwrap();
        wf.transition(eco_id, EcoState::InReview).unwrap();
        wf.transition(eco_id, EcoState::Rejected).unwrap();
        assert_eq!(wf.get_eco(eco_id).unwrap().state, EcoState::Rejected);
    }

    #[test]
    fn test_add_approval() {
        let (mut wf, _, eco_id) = setup_workflow();
        wf.add_approval(
            eco_id,
            "mgr".into(),
            "Engineering Manager".into(),
            ApprovalDecision::Approve,
            1010,
            "Looks good".into(),
        )
        .unwrap();
        assert_eq!(wf.get_eco(eco_id).unwrap().approvals.len(), 1);
    }

    #[test]
    fn test_impact_summary() {
        let (wf, _, eco_id) = setup_workflow();
        let summary = wf.impact_summary(eco_id).unwrap();
        assert!(summary.contains("BRK-001"));
        assert!(summary.contains("ASM-100"));
        assert!(summary.contains("Fix bracket"));
    }

    #[test]
    fn test_create_ecn() {
        let (mut wf, _, eco_id) = setup_workflow();
        let ecn_id = wf
            .create_ecn(eco_id, "Bracket redesigned".into(), 2000)
            .unwrap();
        assert_eq!(wf.ecns().len(), 1);
        assert_eq!(wf.ecns()[0].id, ecn_id);
        assert_eq!(wf.ecns()[0].eco_id, eco_id);
    }

    #[test]
    fn test_serialization_roundtrip() {
        let (wf, _, _) = setup_workflow();
        let json = serde_json::to_string(&wf).unwrap();
        let wf2: EcoWorkflow = serde_json::from_str(&json).unwrap();
        assert_eq!(wf2.ecos().len(), 1);
    }
}
