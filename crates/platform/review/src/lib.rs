//! Design review with geometry commenting.
//!
//! Point at a face, edge, or point in 3D space and leave feedback.
//! Tracks review lifecycle from open through approval or rejection.

use serde::{Deserialize, Serialize};

/// Overall status of a design review.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReviewStatus {
    Open,
    InProgress,
    Approved,
    Rejected,
}

/// Severity of a review comment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Severity {
    Info,
    Warning,
    Critical,
}

/// Resolution status of a comment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CommentStatus {
    Open,
    Resolved,
}

/// Permission level for a review participant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ReviewPermission {
    Viewer,
    Commenter,
    Reviewer,
    Approver,
    Owner,
}

/// A location in 3D geometry — anchors a comment to a specific feature.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeometryLocation {
    pub face_id: Option<u64>,
    pub edge_id: Option<u64>,
    pub point: [f64; 3],
    pub view_direction: [f64; 3],
}

/// A single review comment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewComment {
    pub id: u64,
    pub author: String,
    pub text: String,
    pub location: Option<GeometryLocation>,
    pub severity: Severity,
    pub status: CommentStatus,
}

/// A reviewer participant.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Reviewer {
    pub name: String,
    pub permission: ReviewPermission,
}

/// A full design review session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DesignReview {
    pub id: u64,
    pub design_id: String,
    pub status: ReviewStatus,
    pub reviewers: Vec<Reviewer>,
    pub comments: Vec<ReviewComment>,
    pub created_at: u64,
    next_comment_id: u64,
}

/// Errors from review operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReviewError {
    ReviewNotFound(u64),
    InsufficientPermission,
    ReviewAlreadyClosed,
    CommentNotFound(u64),
    UnresolvedCriticalComments,
}

impl std::fmt::Display for ReviewError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ReviewNotFound(id) => write!(f, "review {id} not found"),
            Self::InsufficientPermission => write!(f, "insufficient permission"),
            Self::ReviewAlreadyClosed => write!(f, "review already closed"),
            Self::CommentNotFound(id) => write!(f, "comment {id} not found"),
            Self::UnresolvedCriticalComments => {
                write!(f, "unresolved critical comments remain")
            }
        }
    }
}

impl std::error::Error for ReviewError {}

/// Manages design reviews.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ReviewManager {
    reviews: Vec<DesignReview>,
    next_id: u64,
}

impl ReviewManager {
    pub fn new() -> Self {
        Self {
            reviews: Vec::new(),
            next_id: 1,
        }
    }

    /// Start a new design review.
    pub fn start_review(
        &mut self,
        design_id: String,
        owner: String,
        reviewers: Vec<(String, ReviewPermission)>,
        timestamp: u64,
    ) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        let mut all_reviewers: Vec<Reviewer> = vec![Reviewer {
            name: owner,
            permission: ReviewPermission::Owner,
        }];
        all_reviewers.extend(reviewers.into_iter().map(|(name, perm)| Reviewer {
            name,
            permission: perm,
        }));
        self.reviews.push(DesignReview {
            id,
            design_id,
            status: ReviewStatus::Open,
            reviewers: all_reviewers,
            comments: Vec::new(),
            created_at: timestamp,
            next_comment_id: 1,
        });
        id
    }

    fn get_review_mut(&mut self, id: u64) -> Result<&mut DesignReview, ReviewError> {
        self.reviews
            .iter_mut()
            .find(|r| r.id == id)
            .ok_or(ReviewError::ReviewNotFound(id))
    }

    fn get_review(&self, id: u64) -> Result<&DesignReview, ReviewError> {
        self.reviews
            .iter()
            .find(|r| r.id == id)
            .ok_or(ReviewError::ReviewNotFound(id))
    }

    fn check_permission(
        review: &DesignReview,
        user: &str,
        min_perm: ReviewPermission,
    ) -> Result<(), ReviewError> {
        let reviewer = review
            .reviewers
            .iter()
            .find(|r| r.name == user)
            .ok_or(ReviewError::InsufficientPermission)?;
        if reviewer.permission >= min_perm {
            Ok(())
        } else {
            Err(ReviewError::InsufficientPermission)
        }
    }

    /// Add a comment to a review.
    pub fn add_comment(
        &mut self,
        review_id: u64,
        author: String,
        text: String,
        location: Option<GeometryLocation>,
        severity: Severity,
    ) -> Result<u64, ReviewError> {
        let review = self.get_review_mut(review_id)?;
        if matches!(review.status, ReviewStatus::Approved | ReviewStatus::Rejected) {
            return Err(ReviewError::ReviewAlreadyClosed);
        }
        Self::check_permission(review, &author, ReviewPermission::Commenter)?;
        if review.status == ReviewStatus::Open {
            review.status = ReviewStatus::InProgress;
        }
        let cid = review.next_comment_id;
        review.next_comment_id += 1;
        review.comments.push(ReviewComment {
            id: cid,
            author,
            text,
            location,
            severity,
            status: CommentStatus::Open,
        });
        Ok(cid)
    }

    /// Resolve a comment.
    pub fn resolve_comment(
        &mut self,
        review_id: u64,
        comment_id: u64,
        user: &str,
    ) -> Result<(), ReviewError> {
        let review = self.get_review_mut(review_id)?;
        Self::check_permission(review, user, ReviewPermission::Commenter)?;
        let comment = review
            .comments
            .iter_mut()
            .find(|c| c.id == comment_id)
            .ok_or(ReviewError::CommentNotFound(comment_id))?;
        comment.status = CommentStatus::Resolved;
        Ok(())
    }

    /// Approve a review (requires Approver or Owner permission, no unresolved critical comments).
    pub fn approve(
        &mut self,
        review_id: u64,
        user: &str,
    ) -> Result<(), ReviewError> {
        let review = self.get_review_mut(review_id)?;
        Self::check_permission(review, user, ReviewPermission::Approver)?;
        let has_unresolved_critical = review.comments.iter().any(|c| {
            c.severity == Severity::Critical && c.status == CommentStatus::Open
        });
        if has_unresolved_critical {
            return Err(ReviewError::UnresolvedCriticalComments);
        }
        review.status = ReviewStatus::Approved;
        Ok(())
    }

    /// Reject a review.
    pub fn reject(
        &mut self,
        review_id: u64,
        user: &str,
    ) -> Result<(), ReviewError> {
        let review = self.get_review_mut(review_id)?;
        Self::check_permission(review, user, ReviewPermission::Reviewer)?;
        review.status = ReviewStatus::Rejected;
        Ok(())
    }

    /// Generate a markdown summary of a review.
    pub fn review_summary(&self, review_id: u64) -> Result<String, ReviewError> {
        let review = self.get_review(review_id)?;
        let mut out = format!("# Design Review: {}\n\n", review.design_id);
        out.push_str(&format!("**Status:** {:?}\n\n", review.status));
        out.push_str("## Reviewers\n\n");
        for r in &review.reviewers {
            out.push_str(&format!("- {} ({:?})\n", r.name, r.permission));
        }
        out.push('\n');

        let total = review.comments.len();
        let open = review
            .comments
            .iter()
            .filter(|c| c.status == CommentStatus::Open)
            .count();
        let critical = review
            .comments
            .iter()
            .filter(|c| c.severity == Severity::Critical)
            .count();
        out.push_str(&format!(
            "## Comments ({total} total, {open} open, {critical} critical)\n\n"
        ));
        for c in &review.comments {
            let loc = if let Some(ref gl) = c.location {
                format!(
                    " @ [{:.1}, {:.1}, {:.1}]",
                    gl.point[0], gl.point[1], gl.point[2]
                )
            } else {
                String::new()
            };
            let status = match c.status {
                CommentStatus::Open => "OPEN",
                CommentStatus::Resolved => "RESOLVED",
            };
            out.push_str(&format!(
                "- [{status}] **{:?}** by {}{loc}: {}\n",
                c.severity, c.author, c.text
            ));
        }
        Ok(out)
    }

    pub fn reviews(&self) -> &[DesignReview] {
        &self.reviews
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> (ReviewManager, u64) {
        let mut mgr = ReviewManager::new();
        let id = mgr.start_review(
            "bracket-v2".into(),
            "alice".into(),
            vec![
                ("bob".into(), ReviewPermission::Reviewer),
                ("carol".into(), ReviewPermission::Approver),
                ("dave".into(), ReviewPermission::Commenter),
                ("eve".into(), ReviewPermission::Viewer),
            ],
            1000,
        );
        (mgr, id)
    }

    #[test]
    fn test_start_review() {
        let (mgr, id) = setup();
        let review = mgr.get_review(id).unwrap();
        assert_eq!(review.status, ReviewStatus::Open);
        assert_eq!(review.reviewers.len(), 5);
    }

    #[test]
    fn test_add_comment_transitions_to_in_progress() {
        let (mut mgr, id) = setup();
        mgr.add_comment(id, "bob".into(), "Check this fillet".into(), None, Severity::Warning)
            .unwrap();
        assert_eq!(mgr.get_review(id).unwrap().status, ReviewStatus::InProgress);
    }

    #[test]
    fn test_add_comment_with_geometry() {
        let (mut mgr, id) = setup();
        let loc = GeometryLocation {
            face_id: Some(42),
            edge_id: None,
            point: [10.0, 20.0, 30.0],
            view_direction: [0.0, 0.0, -1.0],
        };
        let cid = mgr
            .add_comment(id, "dave".into(), "Sharp edge here".into(), Some(loc), Severity::Critical)
            .unwrap();
        assert_eq!(cid, 1);
        let review = mgr.get_review(id).unwrap();
        assert!(review.comments[0].location.is_some());
    }

    #[test]
    fn test_viewer_cannot_comment() {
        let (mut mgr, id) = setup();
        let result = mgr.add_comment(id, "eve".into(), "hi".into(), None, Severity::Info);
        assert_eq!(result, Err(ReviewError::InsufficientPermission));
    }

    #[test]
    fn test_resolve_comment() {
        let (mut mgr, id) = setup();
        let cid = mgr
            .add_comment(id, "bob".into(), "Issue".into(), None, Severity::Warning)
            .unwrap();
        mgr.resolve_comment(id, cid, "bob").unwrap();
        let review = mgr.get_review(id).unwrap();
        assert_eq!(review.comments[0].status, CommentStatus::Resolved);
    }

    #[test]
    fn test_approve_blocked_by_critical() {
        let (mut mgr, id) = setup();
        mgr.add_comment(id, "bob".into(), "Bad".into(), None, Severity::Critical)
            .unwrap();
        let result = mgr.approve(id, "carol");
        assert_eq!(result, Err(ReviewError::UnresolvedCriticalComments));
    }

    #[test]
    fn test_approve_after_resolving_critical() {
        let (mut mgr, id) = setup();
        let cid = mgr
            .add_comment(id, "bob".into(), "Bad".into(), None, Severity::Critical)
            .unwrap();
        mgr.resolve_comment(id, cid, "bob").unwrap();
        mgr.approve(id, "carol").unwrap();
        assert_eq!(mgr.get_review(id).unwrap().status, ReviewStatus::Approved);
    }

    #[test]
    fn test_reject() {
        let (mut mgr, id) = setup();
        mgr.reject(id, "bob").unwrap();
        assert_eq!(mgr.get_review(id).unwrap().status, ReviewStatus::Rejected);
    }

    #[test]
    fn test_cannot_comment_on_closed_review() {
        let (mut mgr, id) = setup();
        mgr.reject(id, "bob").unwrap();
        let result = mgr.add_comment(id, "bob".into(), "late".into(), None, Severity::Info);
        assert_eq!(result, Err(ReviewError::ReviewAlreadyClosed));
    }

    #[test]
    fn test_review_summary() {
        let (mut mgr, id) = setup();
        mgr.add_comment(id, "bob".into(), "Check wall thickness".into(), None, Severity::Warning)
            .unwrap();
        let loc = GeometryLocation {
            face_id: Some(7),
            edge_id: None,
            point: [1.0, 2.0, 3.0],
            view_direction: [0.0, 1.0, 0.0],
        };
        mgr.add_comment(id, "dave".into(), "Sharp edge".into(), Some(loc), Severity::Critical)
            .unwrap();
        let summary = mgr.review_summary(id).unwrap();
        assert!(summary.contains("bracket-v2"));
        assert!(summary.contains("2 total"));
        assert!(summary.contains("1 critical"));
        assert!(summary.contains("Check wall thickness"));
        assert!(summary.contains("[1.0, 2.0, 3.0]"));
    }

    #[test]
    fn test_serialization_roundtrip() {
        let (mut mgr, id) = setup();
        mgr.add_comment(id, "bob".into(), "Test".into(), None, Severity::Info)
            .unwrap();
        let json = serde_json::to_string(&mgr).unwrap();
        let mgr2: ReviewManager = serde_json::from_str(&json).unwrap();
        assert_eq!(mgr2.reviews().len(), 1);
        assert_eq!(mgr2.reviews()[0].comments.len(), 1);
    }
}
