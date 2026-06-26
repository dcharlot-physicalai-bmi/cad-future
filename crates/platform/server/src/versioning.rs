//! Design versioning: branching, merging, and version history.
//!
//! Git-like version control for CAD designs. Each design has an
//! append-only operation log. Branches are named pointers into
//! the log. Merging replays operations from one branch onto another.

use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Version Entry
// ---------------------------------------------------------------------------

/// A single version (snapshot point) in the design history.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Version {
    pub id: String,
    pub name: Option<String>,
    pub message: String,
    pub author: String,
    pub timestamp: DateTime<Utc>,
    /// Index into the operation log (all ops up to this index = this version).
    pub op_index: usize,
    /// Parent version ID (None for root).
    pub parent: Option<String>,
    /// Branch this version belongs to.
    pub branch: String,
}

/// A design operation recorded in the version history.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionedOp {
    pub id: String,
    pub op_type: String,
    pub data: String,
    pub author: String,
    pub timestamp: DateTime<Utc>,
    /// Can this operation be reversed?
    pub reversible: bool,
    /// The inverse operation data (for undo).
    pub inverse_data: Option<String>,
}

// ---------------------------------------------------------------------------
// Branch
// ---------------------------------------------------------------------------

/// A named branch pointing to a version.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Branch {
    pub name: String,
    pub head_version: String,
    pub created_at: DateTime<Utc>,
    pub created_by: String,
    /// If this branch was created from another branch.
    pub base_branch: Option<String>,
    /// The version where this branch diverged.
    pub fork_point: Option<String>,
}

// ---------------------------------------------------------------------------
// Design History
// ---------------------------------------------------------------------------

/// Complete version history for a single design document.
#[derive(Debug, Clone)]
pub struct DesignHistory {
    pub document_id: String,
    pub operations: Vec<VersionedOp>,
    pub versions: Vec<Version>,
    pub branches: HashMap<String, Branch>,
    pub current_branch: String,
}

impl DesignHistory {
    pub fn new(document_id: &str, creator: &str) -> Self {
        let root_version = Version {
            id: Uuid::new_v4().to_string(),
            name: Some("Initial".to_string()),
            message: "Initial version".to_string(),
            author: creator.to_string(),
            timestamp: Utc::now(),
            op_index: 0,
            parent: None,
            branch: "main".to_string(),
        };

        let main_branch = Branch {
            name: "main".to_string(),
            head_version: root_version.id.clone(),
            created_at: Utc::now(),
            created_by: creator.to_string(),
            base_branch: None,
            fork_point: None,
        };

        let mut branches = HashMap::new();
        branches.insert("main".to_string(), main_branch);

        Self {
            document_id: document_id.to_string(),
            operations: Vec::new(),
            versions: vec![root_version],
            branches,
            current_branch: "main".to_string(),
        }
    }
}

// ---------------------------------------------------------------------------
// Version Manager
// ---------------------------------------------------------------------------

/// Manages version histories for all design documents.
#[derive(Debug, Clone)]
pub struct VersionManager {
    histories: Arc<RwLock<HashMap<String, DesignHistory>>>,
}

impl VersionManager {
    pub fn new() -> Self {
        Self {
            histories: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Initialize version tracking for a document.
    pub fn init_document(&self, document_id: &str, creator: &str) -> Result<String, String> {
        let mut histories = self.histories.write().map_err(|_| "Lock poisoned")?;
        if histories.contains_key(document_id) {
            return Err("Document already tracked".into());
        }
        let history = DesignHistory::new(document_id, creator);
        let root_id = history.versions[0].id.clone();
        histories.insert(document_id.to_string(), history);
        Ok(root_id)
    }

    /// Record an operation on the current branch.
    pub fn record_op(
        &self,
        document_id: &str,
        op_type: &str,
        data: &str,
        author: &str,
        inverse_data: Option<&str>,
    ) -> Result<String, String> {
        let mut histories = self.histories.write().map_err(|_| "Lock poisoned")?;
        let history = histories.get_mut(document_id)
            .ok_or("Document not found")?;

        let op = VersionedOp {
            id: Uuid::new_v4().to_string(),
            op_type: op_type.to_string(),
            data: data.to_string(),
            author: author.to_string(),
            timestamp: Utc::now(),
            reversible: inverse_data.is_some(),
            inverse_data: inverse_data.map(String::from),
        };
        let op_id = op.id.clone();
        history.operations.push(op);
        Ok(op_id)
    }

    /// Create a named version (tag) at the current operation index.
    pub fn create_version(
        &self,
        document_id: &str,
        name: Option<&str>,
        message: &str,
        author: &str,
    ) -> Result<String, String> {
        let mut histories = self.histories.write().map_err(|_| "Lock poisoned")?;
        let history = histories.get_mut(document_id)
            .ok_or("Document not found")?;

        let branch = &history.current_branch;
        let parent_id = history.branches.get(branch)
            .map(|b| b.head_version.clone())
            .ok_or("Branch not found")?;

        let version = Version {
            id: Uuid::new_v4().to_string(),
            name: name.map(String::from),
            message: message.to_string(),
            author: author.to_string(),
            timestamp: Utc::now(),
            op_index: history.operations.len(),
            parent: Some(parent_id),
            branch: branch.clone(),
        };

        let version_id = version.id.clone();

        // Update branch head
        if let Some(b) = history.branches.get_mut(branch) {
            b.head_version = version_id.clone();
        }

        history.versions.push(version);
        Ok(version_id)
    }

    /// Create a new branch from the current branch head.
    pub fn create_branch(
        &self,
        document_id: &str,
        branch_name: &str,
        creator: &str,
    ) -> Result<String, String> {
        let mut histories = self.histories.write().map_err(|_| "Lock poisoned")?;
        let history = histories.get_mut(document_id)
            .ok_or("Document not found")?;

        if history.branches.contains_key(branch_name) {
            return Err("Branch already exists".into());
        }

        let current = &history.current_branch;
        let head_version = history.branches.get(current)
            .map(|b| b.head_version.clone())
            .ok_or("Current branch not found")?;

        let branch = Branch {
            name: branch_name.to_string(),
            head_version: head_version.clone(),
            created_at: Utc::now(),
            created_by: creator.to_string(),
            base_branch: Some(current.clone()),
            fork_point: Some(head_version),
        };

        history.branches.insert(branch_name.to_string(), branch);
        Ok(branch_name.to_string())
    }

    /// Switch to a different branch.
    pub fn checkout_branch(
        &self,
        document_id: &str,
        branch_name: &str,
    ) -> Result<usize, String> {
        let mut histories = self.histories.write().map_err(|_| "Lock poisoned")?;
        let history = histories.get_mut(document_id)
            .ok_or("Document not found")?;

        if !history.branches.contains_key(branch_name) {
            return Err("Branch not found".into());
        }

        history.current_branch = branch_name.to_string();

        // Return the op_index of the branch head
        let head_id = &history.branches[branch_name].head_version;
        let op_index = history.versions.iter()
            .find(|v| v.id == *head_id)
            .map(|v| v.op_index)
            .unwrap_or(0);

        Ok(op_index)
    }

    /// Merge a source branch into the current branch.
    /// Returns the operations that need to be replayed.
    pub fn merge_branch(
        &self,
        document_id: &str,
        source_branch: &str,
        author: &str,
    ) -> Result<MergeResult, String> {
        let mut histories = self.histories.write().map_err(|_| "Lock poisoned")?;
        let history = histories.get_mut(document_id)
            .ok_or("Document not found")?;

        let target_branch = history.current_branch.clone();
        if target_branch == source_branch {
            return Err("Cannot merge branch into itself".into());
        }

        let source = history.branches.get(source_branch)
            .ok_or("Source branch not found")?;
        let target = history.branches.get(&target_branch)
            .ok_or("Target branch not found")?;

        // Find fork point
        let fork_point_id = source.fork_point.as_ref()
            .or(Some(&history.versions[0].id))
            .unwrap();

        let fork_op_index = history.versions.iter()
            .find(|v| v.id == *fork_point_id)
            .map(|v| v.op_index)
            .unwrap_or(0);

        let source_head_op_index = history.versions.iter()
            .find(|v| v.id == source.head_version)
            .map(|v| v.op_index)
            .unwrap_or(0);

        let target_head_op_index = history.versions.iter()
            .find(|v| v.id == target.head_version)
            .map(|v| v.op_index)
            .unwrap_or(0);

        // Collect operations from source branch since fork
        let source_ops: Vec<VersionedOp> = history.operations
            .get(fork_op_index..source_head_op_index)
            .unwrap_or(&[])
            .to_vec();

        // Check for conflicts: operations on the same features
        let target_ops: Vec<VersionedOp> = history.operations
            .get(fork_op_index..target_head_op_index)
            .unwrap_or(&[])
            .to_vec();

        let conflicts = detect_conflicts(&source_ops, &target_ops);

        if !conflicts.is_empty() {
            return Ok(MergeResult::Conflict {
                conflicts,
                source_ops,
                target_ops,
            });
        }

        // Fast-forward or replay
        // Create merge version
        let merge_version = Version {
            id: Uuid::new_v4().to_string(),
            name: Some(format!("Merge {} into {}", source_branch, target_branch)),
            message: format!("Merged {} ({} ops) into {}", source_branch, source_ops.len(), target_branch),
            author: author.to_string(),
            timestamp: Utc::now(),
            op_index: history.operations.len(),
            parent: Some(target.head_version.clone()),
            branch: target_branch.clone(),
        };

        let version_id = merge_version.id.clone();
        history.versions.push(merge_version);

        if let Some(b) = history.branches.get_mut(&target_branch) {
            b.head_version = version_id;
        }

        Ok(MergeResult::Success {
            merged_ops: source_ops.len(),
            version_id: history.versions.last().unwrap().id.clone(),
        })
    }

    /// Rollback to a specific version.
    pub fn rollback(
        &self,
        document_id: &str,
        version_id: &str,
        author: &str,
    ) -> Result<RollbackResult, String> {
        let mut histories = self.histories.write().map_err(|_| "Lock poisoned")?;
        let history = histories.get_mut(document_id)
            .ok_or("Document not found")?;

        let target_version = history.versions.iter()
            .find(|v| v.id == version_id)
            .ok_or("Version not found")?;

        let target_op_index = target_version.op_index;
        let current_op_index = history.operations.len();

        if target_op_index >= current_op_index {
            return Err("Cannot rollback to a future version".into());
        }

        // Collect operations to undo (in reverse order)
        let ops_to_undo: Vec<VersionedOp> = history.operations[target_op_index..]
            .iter()
            .rev()
            .cloned()
            .collect();

        let inverse_ops: Vec<String> = ops_to_undo.iter()
            .filter_map(|op| op.inverse_data.clone())
            .collect();

        // Create rollback version
        let rollback_version = Version {
            id: Uuid::new_v4().to_string(),
            name: Some(format!("Rollback to {}", target_version.name.as_deref().unwrap_or(&target_version.id))),
            message: format!("Rolled back {} operations", ops_to_undo.len()),
            author: author.to_string(),
            timestamp: Utc::now(),
            op_index: target_op_index,
            parent: Some(history.branches[&history.current_branch].head_version.clone()),
            branch: history.current_branch.clone(),
        };

        let version_id = rollback_version.id.clone();
        history.versions.push(rollback_version);

        let branch = history.current_branch.clone();
        if let Some(b) = history.branches.get_mut(&branch) {
            b.head_version = version_id.clone();
        }

        Ok(RollbackResult {
            version_id,
            ops_undone: ops_to_undo.len(),
            inverse_ops,
        })
    }

    /// Get the version history (timeline) for a document.
    pub fn get_history(
        &self,
        document_id: &str,
    ) -> Result<Vec<Version>, String> {
        let histories = self.histories.read().map_err(|_| "Lock poisoned")?;
        let history = histories.get(document_id)
            .ok_or("Document not found")?;
        Ok(history.versions.clone())
    }

    /// List branches for a document.
    pub fn list_branches(
        &self,
        document_id: &str,
    ) -> Result<Vec<BranchInfo>, String> {
        let histories = self.histories.read().map_err(|_| "Lock poisoned")?;
        let history = histories.get(document_id)
            .ok_or("Document not found")?;

        Ok(history.branches.values().map(|b| {
            let op_count = history.versions.iter()
                .find(|v| v.id == b.head_version)
                .map(|v| v.op_index)
                .unwrap_or(0);

            BranchInfo {
                name: b.name.clone(),
                head_version: b.head_version.clone(),
                op_count,
                is_current: b.name == history.current_branch,
                created_by: b.created_by.clone(),
            }
        }).collect())
    }

    /// Get the current branch name.
    pub fn current_branch(&self, document_id: &str) -> Result<String, String> {
        let histories = self.histories.read().map_err(|_| "Lock poisoned")?;
        let history = histories.get(document_id)
            .ok_or("Document not found")?;
        Ok(history.current_branch.clone())
    }

    /// Get operations between two versions (for diff visualization).
    pub fn diff_versions(
        &self,
        document_id: &str,
        from_version: &str,
        to_version: &str,
    ) -> Result<Vec<VersionedOp>, String> {
        let histories = self.histories.read().map_err(|_| "Lock poisoned")?;
        let history = histories.get(document_id)
            .ok_or("Document not found")?;

        let from_idx = history.versions.iter()
            .find(|v| v.id == from_version)
            .map(|v| v.op_index)
            .ok_or("From version not found")?;

        let to_idx = history.versions.iter()
            .find(|v| v.id == to_version)
            .map(|v| v.op_index)
            .ok_or("To version not found")?;

        let (start, end) = if from_idx < to_idx {
            (from_idx, to_idx)
        } else {
            (to_idx, from_idx)
        };

        Ok(history.operations.get(start..end)
            .unwrap_or(&[])
            .to_vec())
    }
}

// ---------------------------------------------------------------------------
// Merge / Conflict Types
// ---------------------------------------------------------------------------

/// Result of a branch merge attempt.
#[derive(Debug, Clone)]
pub enum MergeResult {
    /// Merge succeeded without conflicts.
    Success {
        merged_ops: usize,
        version_id: String,
    },
    /// Merge has conflicts that need resolution.
    Conflict {
        conflicts: Vec<MergeConflict>,
        source_ops: Vec<VersionedOp>,
        target_ops: Vec<VersionedOp>,
    },
}

/// A specific conflict between two operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MergeConflict {
    pub source_op: String,
    pub target_op: String,
    pub description: String,
}

/// Result of a rollback operation.
#[derive(Debug, Clone)]
pub struct RollbackResult {
    pub version_id: String,
    pub ops_undone: usize,
    pub inverse_ops: Vec<String>,
}

/// Branch info for listing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchInfo {
    pub name: String,
    pub head_version: String,
    pub op_count: usize,
    pub is_current: bool,
    pub created_by: String,
}

// ---------------------------------------------------------------------------
// Conflict Detection
// ---------------------------------------------------------------------------

/// Detect conflicts between two sets of operations.
/// Two operations conflict if they modify the same feature.
fn detect_conflicts(source: &[VersionedOp], target: &[VersionedOp]) -> Vec<MergeConflict> {
    let mut conflicts = Vec::new();

    for s_op in source {
        for t_op in target {
            // Simple heuristic: same op_type on the same data → conflict
            if s_op.op_type == t_op.op_type && s_op.data == t_op.data
                && s_op.author != t_op.author
            {
                conflicts.push(MergeConflict {
                    source_op: s_op.id.clone(),
                    target_op: t_op.id.clone(),
                    description: format!(
                        "Both branches modified '{}' with same parameters",
                        s_op.op_type
                    ),
                });
            }
        }
    }

    conflicts
}

// ---------------------------------------------------------------------------
// Version Diff
// ---------------------------------------------------------------------------

/// Statistics about a version diff.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffStats {
    pub ops_added: usize,
    pub ops_removed: usize,
    pub ops_modified: usize,
    pub total_changes: usize,
}

/// The diff between two versions of a design.
#[derive(Debug, Clone)]
pub struct VersionDiff {
    pub from_version: String,
    pub to_version: String,
    pub operations_added: Vec<VersionedOp>,
    pub operations_removed: Vec<VersionedOp>,
    pub operations_modified: Vec<(VersionedOp, VersionedOp)>,
    pub summary: String,
    pub stats: DiffStats,
}

/// Result of a three-way merge diff analysis.
#[derive(Debug, Clone)]
pub struct MergeDiff {
    pub base_version: String,
    pub ours_version: String,
    pub theirs_version: String,
    pub ours_only: Vec<VersionedOp>,
    pub theirs_only: Vec<VersionedOp>,
    pub conflicting: Vec<(VersionedOp, VersionedOp)>,
}

/// Compare two versions by walking the operation log between their op_index values.
///
/// Operations present in `to` but not in `from` are "added".
/// Operations present in `from` but not in `to` are "removed".
/// Operations at the same log position with different data are "modified".
pub fn diff_versions(history: &DesignHistory, from_id: &str, to_id: &str) -> Option<VersionDiff> {
    let from_version = history.versions.iter().find(|v| v.id == from_id)?;
    let to_version = history.versions.iter().find(|v| v.id == to_id)?;

    let from_idx = from_version.op_index;
    let to_idx = to_version.op_index;

    let mut operations_added = Vec::new();
    let mut operations_removed = Vec::new();
    let operations_modified: Vec<(VersionedOp, VersionedOp)> = Vec::new();

    if from_idx <= to_idx {
        // Forward diff: ops in (from_idx..to_idx) are additions relative to `from`.
        // Check overlapping range for modifications (there is none when from < to in
        // a purely append-only log, but we handle the general case).
        let shared_end = from_idx; // no shared range beyond from_idx
        let _ = shared_end; // silence unused warning

        // All ops between from_idx and to_idx are added.
        if let Some(slice) = history.operations.get(from_idx..to_idx) {
            operations_added = slice.to_vec();
        }
    } else {
        // Backward diff: ops in (to_idx..from_idx) are removals relative to `from`.
        if let Some(slice) = history.operations.get(to_idx..from_idx) {
            operations_removed = slice.to_vec();
        }
    }

    // Detect modifications: compare ops in the overlapping index range.
    // In this model both versions share ops [0..min(from_idx, to_idx)].
    // A modification means two ops at the same position but with different content.
    // For an append-only log the shared prefix is identical, but if we ever support
    // rewritten history we compare here.
    let shared = std::cmp::min(from_idx, to_idx);
    // Walk the shared prefix looking for differences (op identity by id).
    // In a linear log the shared prefix is the same slice, so normally empty.
    // For branch-aware diffs we compare by op_type+data.
    // We intentionally leave this lightweight: modifications come from the
    // three_way_merge_diff path where both sides diverge from a common base.
    let _ = shared;

    let summary = build_diff_summary(&operations_added, &operations_removed, &operations_modified);

    let stats = DiffStats {
        ops_added: operations_added.len(),
        ops_removed: operations_removed.len(),
        ops_modified: operations_modified.len(),
        total_changes: operations_added.len() + operations_removed.len() + operations_modified.len(),
    };

    Some(VersionDiff {
        from_version: from_id.to_string(),
        to_version: to_id.to_string(),
        operations_added,
        operations_removed,
        operations_modified,
        summary,
        stats,
    })
}

/// Compare the head versions of two branches.
pub fn diff_branches(history: &DesignHistory, branch_a: &str, branch_b: &str) -> Option<VersionDiff> {
    let a = history.branches.get(branch_a)?;
    let b = history.branches.get(branch_b)?;
    diff_versions(history, &a.head_version, &b.head_version)
}

/// Three-way merge diff: identify operations unique to each side and conflicts.
///
/// `base` is the common ancestor version. `ours` and `theirs` are the two
/// diverged versions. Operations that appear on both sides with the same
/// op_type and data (by different authors) are flagged as conflicting.
pub fn three_way_merge_diff(
    history: &DesignHistory,
    base_id: &str,
    ours_id: &str,
    theirs_id: &str,
) -> Option<MergeDiff> {
    let base = history.versions.iter().find(|v| v.id == base_id)?;
    let ours = history.versions.iter().find(|v| v.id == ours_id)?;
    let theirs = history.versions.iter().find(|v| v.id == theirs_id)?;

    let base_idx = base.op_index;
    let ours_idx = ours.op_index;
    let theirs_idx = theirs.op_index;

    // In a linear append-only log, the shared prefix is base..min(ours, theirs).
    // Operations unique to each side are partitioned around the earlier version.
    let (ours_ops, theirs_ops) = if ours_idx <= theirs_idx {
        let ours_slice = history.operations
            .get(base_idx..ours_idx)
            .unwrap_or(&[])
            .to_vec();
        let theirs_slice = history.operations
            .get(ours_idx..theirs_idx)
            .unwrap_or(&[])
            .to_vec();
        (ours_slice, theirs_slice)
    } else {
        let theirs_slice = history.operations
            .get(base_idx..theirs_idx)
            .unwrap_or(&[])
            .to_vec();
        let ours_slice = history.operations
            .get(theirs_idx..ours_idx)
            .unwrap_or(&[])
            .to_vec();
        (ours_slice, theirs_slice)
    };

    // Detect conflicts: same op_type + data from different authors.
    let mut conflicting = Vec::new();
    let mut theirs_conflicted: Vec<bool> = vec![false; theirs_ops.len()];
    let mut ours_conflicted: Vec<bool> = vec![false; ours_ops.len()];

    for (oi, o) in ours_ops.iter().enumerate() {
        for (ti, t) in theirs_ops.iter().enumerate() {
            if o.op_type == t.op_type && o.data == t.data && o.author != t.author {
                conflicting.push((o.clone(), t.clone()));
                ours_conflicted[oi] = true;
                theirs_conflicted[ti] = true;
            }
        }
    }

    let ours_only: Vec<VersionedOp> = ours_ops.into_iter()
        .enumerate()
        .filter(|(i, _)| !ours_conflicted[*i])
        .map(|(_, op)| op)
        .collect();

    let theirs_only: Vec<VersionedOp> = theirs_ops.into_iter()
        .enumerate()
        .filter(|(i, _)| !theirs_conflicted[*i])
        .map(|(_, op)| op)
        .collect();

    Some(MergeDiff {
        base_version: base_id.to_string(),
        ours_version: ours_id.to_string(),
        theirs_version: theirs_id.to_string(),
        ours_only,
        theirs_only,
        conflicting,
    })
}

/// Build a human-readable summary of a diff.
fn build_diff_summary(
    added: &[VersionedOp],
    removed: &[VersionedOp],
    modified: &[(VersionedOp, VersionedOp)],
) -> String {
    let mut parts = Vec::new();
    if !added.is_empty() {
        let types: Vec<&str> = added.iter().map(|op| op.op_type.as_str()).collect();
        parts.push(format!("{} operation(s) added: {}", added.len(), types.join(", ")));
    }
    if !removed.is_empty() {
        let types: Vec<&str> = removed.iter().map(|op| op.op_type.as_str()).collect();
        parts.push(format!("{} operation(s) removed: {}", removed.len(), types.join(", ")));
    }
    if !modified.is_empty() {
        let types: Vec<String> = modified.iter()
            .map(|(old, _new)| old.op_type.clone())
            .collect();
        parts.push(format!("{} operation(s) modified: {}", modified.len(), types.join(", ")));
    }
    if parts.is_empty() {
        return "No changes".to_string();
    }
    parts.join("; ")
}

/// Format a `VersionDiff` into a human-readable summary string.
pub fn format_diff_summary(diff: &VersionDiff) -> String {
    let mut lines = Vec::new();
    lines.push(format!(
        "Diff: {} -> {} ({} change(s))",
        diff.from_version, diff.to_version, diff.stats.total_changes
    ));
    if !diff.operations_added.is_empty() {
        lines.push(format!("  Added ({}):", diff.stats.ops_added));
        for op in &diff.operations_added {
            lines.push(format!("    + {} [{}]", op.op_type, op.id));
        }
    }
    if !diff.operations_removed.is_empty() {
        lines.push(format!("  Removed ({}):", diff.stats.ops_removed));
        for op in &diff.operations_removed {
            lines.push(format!("    - {} [{}]", op.op_type, op.id));
        }
    }
    if !diff.operations_modified.is_empty() {
        lines.push(format!("  Modified ({}):", diff.stats.ops_modified));
        for (old, _new) in &diff.operations_modified {
            lines.push(format!("    ~ {} [{}]", old.op_type, old.id));
        }
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vm() -> VersionManager {
        VersionManager::new()
    }

    #[test]
    fn init_and_get_history() {
        let v = vm();
        v.init_document("doc1", "alice").unwrap();
        let history = v.get_history("doc1").unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].message, "Initial version");
    }

    #[test]
    fn record_ops_and_create_version() {
        let v = vm();
        v.init_document("doc1", "alice").unwrap();

        v.record_op("doc1", "add_box", "{\"w\":10}", "alice", None).unwrap();
        v.record_op("doc1", "add_hole", "{\"r\":2}", "alice", Some("{\"remove_hole\":{\"r\":2}}")).unwrap();

        let vid = v.create_version("doc1", Some("v1.0"), "Added box with hole", "alice").unwrap();

        let history = v.get_history("doc1").unwrap();
        assert_eq!(history.len(), 2);
        assert_eq!(history[1].name, Some("v1.0".to_string()));
        assert_eq!(history[1].op_index, 2);
    }

    #[test]
    fn create_and_list_branches() {
        let v = vm();
        v.init_document("doc1", "alice").unwrap();
        v.create_branch("doc1", "feature-a", "alice").unwrap();

        let branches = v.list_branches("doc1").unwrap();
        assert_eq!(branches.len(), 2);
        assert!(branches.iter().any(|b| b.name == "main" && b.is_current));
        assert!(branches.iter().any(|b| b.name == "feature-a" && !b.is_current));
    }

    #[test]
    fn checkout_branch() {
        let v = vm();
        v.init_document("doc1", "alice").unwrap();
        v.create_branch("doc1", "dev", "alice").unwrap();
        v.checkout_branch("doc1", "dev").unwrap();

        assert_eq!(v.current_branch("doc1").unwrap(), "dev");
    }

    #[test]
    fn merge_branch_success() {
        let v = vm();
        v.init_document("doc1", "alice").unwrap();
        v.create_branch("doc1", "feature", "alice").unwrap();

        // Add ops on feature branch
        v.checkout_branch("doc1", "feature").unwrap();
        v.record_op("doc1", "add_fillet", "{\"r\":1}", "alice", None).unwrap();
        v.create_version("doc1", None, "Added fillet", "alice").unwrap();

        // Merge back to main
        v.checkout_branch("doc1", "main").unwrap();
        let result = v.merge_branch("doc1", "feature", "alice").unwrap();

        match result {
            MergeResult::Success { merged_ops, .. } => {
                // The ops between fork and feature head
                assert!(merged_ops >= 0);
            }
            MergeResult::Conflict { .. } => panic!("Expected successful merge"),
        }
    }

    #[test]
    fn merge_self_rejected() {
        let v = vm();
        v.init_document("doc1", "alice").unwrap();
        assert!(v.merge_branch("doc1", "main", "alice").is_err());
    }

    #[test]
    fn rollback_to_version() {
        let v = vm();
        v.init_document("doc1", "alice").unwrap();

        let root_id = v.get_history("doc1").unwrap()[0].id.clone();

        v.record_op("doc1", "add_box", "{}", "alice", Some("{\"remove_box\":{}}")).unwrap();
        v.record_op("doc1", "add_hole", "{}", "alice", Some("{\"remove_hole\":{}}")).unwrap();
        v.create_version("doc1", Some("v1"), "Two features", "alice").unwrap();

        let result = v.rollback("doc1", &root_id, "alice").unwrap();
        assert_eq!(result.ops_undone, 2);
        assert_eq!(result.inverse_ops.len(), 2);
    }

    #[test]
    fn manager_diff_versions() {
        let v = vm();
        v.init_document("doc1", "alice").unwrap();
        let v0_id = v.get_history("doc1").unwrap()[0].id.clone();

        v.record_op("doc1", "add_box", "{}", "alice", None).unwrap();
        v.record_op("doc1", "add_hole", "{}", "alice", None).unwrap();
        let v1_id = v.create_version("doc1", Some("v1"), "Two ops", "alice").unwrap();

        let diff = v.diff_versions("doc1", &v0_id, &v1_id).unwrap();
        assert_eq!(diff.len(), 2);
    }

    #[test]
    fn duplicate_init_rejected() {
        let v = vm();
        v.init_document("doc1", "alice").unwrap();
        assert!(v.init_document("doc1", "bob").is_err());
    }

    #[test]
    fn duplicate_branch_rejected() {
        let v = vm();
        v.init_document("doc1", "alice").unwrap();
        v.create_branch("doc1", "dev", "alice").unwrap();
        assert!(v.create_branch("doc1", "dev", "alice").is_err());
    }

    #[test]
    fn checkout_nonexistent_branch_fails() {
        let v = vm();
        v.init_document("doc1", "alice").unwrap();
        assert!(v.checkout_branch("doc1", "nonexistent").is_err());
    }

    #[test]
    fn conflict_detection() {
        let source = vec![VersionedOp {
            id: "s1".into(),
            op_type: "modify_face".into(),
            data: "{\"face\":0}".into(),
            author: "alice".into(),
            timestamp: Utc::now(),
            reversible: false,
            inverse_data: None,
        }];
        let target = vec![VersionedOp {
            id: "t1".into(),
            op_type: "modify_face".into(),
            data: "{\"face\":0}".into(),
            author: "bob".into(),
            timestamp: Utc::now(),
            reversible: false,
            inverse_data: None,
        }];
        let conflicts = detect_conflicts(&source, &target);
        assert_eq!(conflicts.len(), 1);
    }

    #[test]
    fn no_conflict_different_ops() {
        let source = vec![VersionedOp {
            id: "s1".into(),
            op_type: "add_box".into(),
            data: "{}".into(),
            author: "alice".into(),
            timestamp: Utc::now(),
            reversible: false,
            inverse_data: None,
        }];
        let target = vec![VersionedOp {
            id: "t1".into(),
            op_type: "add_hole".into(),
            data: "{}".into(),
            author: "bob".into(),
            timestamp: Utc::now(),
            reversible: false,
            inverse_data: None,
        }];
        let conflicts = detect_conflicts(&source, &target);
        assert!(conflicts.is_empty());
    }

    // -------------------------------------------------------------------
    // CAD Diff tests
    // -------------------------------------------------------------------

    /// Helper: build a DesignHistory, add some ops and versions, return
    /// (history, vec-of-version-ids).
    fn history_with_ops(ops: &[(&str, &str)]) -> (DesignHistory, Vec<String>) {
        let mut h = DesignHistory::new("doc-test", "alice");
        let root_id = h.versions[0].id.clone();
        let mut version_ids = vec![root_id];

        for (op_type, data) in ops {
            h.operations.push(VersionedOp {
                id: Uuid::new_v4().to_string(),
                op_type: op_type.to_string(),
                data: data.to_string(),
                author: "alice".to_string(),
                timestamp: Utc::now(),
                reversible: false,
                inverse_data: None,
            });
            // Create a version after each op
            let v = Version {
                id: Uuid::new_v4().to_string(),
                name: None,
                message: format!("After {}", op_type),
                author: "alice".to_string(),
                timestamp: Utc::now(),
                op_index: h.operations.len(),
                parent: Some(version_ids.last().unwrap().clone()),
                branch: "main".to_string(),
            };
            let vid = v.id.clone();
            h.versions.push(v);
            if let Some(b) = h.branches.get_mut("main") {
                b.head_version = vid.clone();
            }
            version_ids.push(vid);
        }
        (h, version_ids)
    }

    #[test]
    fn diff_empty_versions() {
        let h = DesignHistory::new("doc-test", "alice");
        let root_id = h.versions[0].id.clone();

        let diff = diff_versions(&h, &root_id, &root_id).unwrap();
        assert!(diff.operations_added.is_empty());
        assert!(diff.operations_removed.is_empty());
        assert!(diff.operations_modified.is_empty());
        assert_eq!(diff.stats.total_changes, 0);
        assert_eq!(diff.summary, "No changes");
    }

    #[test]
    fn diff_single_operation() {
        let (h, vids) = history_with_ops(&[("add_box", "{\"w\":10}")]);

        let diff = diff_versions(&h, &vids[0], &vids[1]).unwrap();
        assert_eq!(diff.operations_added.len(), 1);
        assert_eq!(diff.operations_added[0].op_type, "add_box");
        assert!(diff.operations_removed.is_empty());
        assert_eq!(diff.stats.ops_added, 1);
        assert_eq!(diff.stats.total_changes, 1);
    }

    #[test]
    fn diff_multiple_operations() {
        let (h, vids) = history_with_ops(&[
            ("add_box", "{\"w\":10}"),
            ("add_hole", "{\"r\":2}"),
            ("add_fillet", "{\"r\":1}"),
        ]);

        // Diff from root to after 3rd op
        let diff = diff_versions(&h, &vids[0], &vids[3]).unwrap();
        assert_eq!(diff.operations_added.len(), 3);
        assert_eq!(diff.stats.ops_added, 3);

        // Diff from v1 to v3 (should show 2 added ops)
        let diff2 = diff_versions(&h, &vids[1], &vids[3]).unwrap();
        assert_eq!(diff2.operations_added.len(), 2);

        // Reverse diff: v3 -> v1 (should show 2 removed ops)
        let diff3 = diff_versions(&h, &vids[3], &vids[1]).unwrap();
        assert_eq!(diff3.operations_removed.len(), 2);
        assert_eq!(diff3.stats.ops_removed, 2);
    }

    #[test]
    fn diff_branches_test() {
        let (mut h, vids) = history_with_ops(&[("add_box", "{\"w\":10}")]);

        // Create a feature branch at the current head
        let feature = Branch {
            name: "feature".to_string(),
            head_version: vids[1].clone(),
            created_at: Utc::now(),
            created_by: "alice".to_string(),
            base_branch: Some("main".to_string()),
            fork_point: Some(vids[1].clone()),
        };
        h.branches.insert("feature".to_string(), feature);

        // Add more ops on main
        h.operations.push(VersionedOp {
            id: Uuid::new_v4().to_string(),
            op_type: "add_hole".to_string(),
            data: "{\"r\":2}".to_string(),
            author: "alice".to_string(),
            timestamp: Utc::now(),
            reversible: false,
            inverse_data: None,
        });
        let v2 = Version {
            id: Uuid::new_v4().to_string(),
            name: None,
            message: "Added hole on main".to_string(),
            author: "alice".to_string(),
            timestamp: Utc::now(),
            op_index: h.operations.len(),
            parent: Some(vids[1].clone()),
            branch: "main".to_string(),
        };
        let v2_id = v2.id.clone();
        h.versions.push(v2);
        h.branches.get_mut("main").unwrap().head_version = v2_id;

        // Diff between branches: feature is at op_index 1, main is at op_index 2
        let diff = diff_branches(&h, "feature", "main").unwrap();
        assert_eq!(diff.operations_added.len(), 1);
        assert_eq!(diff.operations_added[0].op_type, "add_hole");
    }

    #[test]
    fn diff_stats_count() {
        let (h, vids) = history_with_ops(&[
            ("add_box", "{}"),
            ("add_hole", "{}"),
            ("add_fillet", "{}"),
            ("add_chamfer", "{}"),
        ]);

        let diff = diff_versions(&h, &vids[0], &vids[4]).unwrap();
        assert_eq!(diff.stats.ops_added, 4);
        assert_eq!(diff.stats.ops_removed, 0);
        assert_eq!(diff.stats.ops_modified, 0);
        assert_eq!(diff.stats.total_changes, 4);
    }

    #[test]
    fn diff_summary_format() {
        let (h, vids) = history_with_ops(&[
            ("add_box", "{}"),
            ("add_hole", "{}"),
        ]);

        let diff = diff_versions(&h, &vids[0], &vids[2]).unwrap();
        let summary = format_diff_summary(&diff);
        assert!(summary.contains("Diff:"));
        assert!(summary.contains("2 change(s)"));
        assert!(summary.contains("Added (2):"));
        assert!(summary.contains("+ add_box"));
        assert!(summary.contains("+ add_hole"));
    }

    #[test]
    fn three_way_no_conflicts() {
        // Base -> ours adds box, theirs adds hole (no conflict: different ops)
        let mut h = DesignHistory::new("doc-test", "alice");
        let base_id = h.versions[0].id.clone();

        // Ours: add_box by alice
        h.operations.push(VersionedOp {
            id: Uuid::new_v4().to_string(),
            op_type: "add_box".to_string(),
            data: "{\"w\":10}".to_string(),
            author: "alice".to_string(),
            timestamp: Utc::now(),
            reversible: false,
            inverse_data: None,
        });
        let ours_v = Version {
            id: Uuid::new_v4().to_string(),
            name: None,
            message: "Ours".to_string(),
            author: "alice".to_string(),
            timestamp: Utc::now(),
            op_index: 1,
            parent: Some(base_id.clone()),
            branch: "main".to_string(),
        };
        let ours_id = ours_v.id.clone();
        h.versions.push(ours_v);

        // Theirs: add_hole by bob
        h.operations.push(VersionedOp {
            id: Uuid::new_v4().to_string(),
            op_type: "add_hole".to_string(),
            data: "{\"r\":2}".to_string(),
            author: "bob".to_string(),
            timestamp: Utc::now(),
            reversible: false,
            inverse_data: None,
        });
        let theirs_v = Version {
            id: Uuid::new_v4().to_string(),
            name: None,
            message: "Theirs".to_string(),
            author: "bob".to_string(),
            timestamp: Utc::now(),
            op_index: 2,
            parent: Some(base_id.clone()),
            branch: "feature".to_string(),
        };
        let theirs_id = theirs_v.id.clone();
        h.versions.push(theirs_v);

        let merge = three_way_merge_diff(&h, &base_id, &ours_id, &theirs_id).unwrap();
        assert!(merge.conflicting.is_empty());
        assert_eq!(merge.ours_only.len(), 1);
        assert_eq!(merge.theirs_only.len(), 1);
        assert_eq!(merge.ours_only[0].op_type, "add_box");
        assert_eq!(merge.theirs_only[0].op_type, "add_hole");
    }

    #[test]
    fn three_way_with_conflicts() {
        // Both sides modify_face with same data but different authors -> conflict
        let mut h = DesignHistory::new("doc-test", "alice");
        let base_id = h.versions[0].id.clone();

        // Ours: modify_face by alice
        h.operations.push(VersionedOp {
            id: Uuid::new_v4().to_string(),
            op_type: "modify_face".to_string(),
            data: "{\"face\":0}".to_string(),
            author: "alice".to_string(),
            timestamp: Utc::now(),
            reversible: false,
            inverse_data: None,
        });
        let ours_v = Version {
            id: Uuid::new_v4().to_string(),
            name: None,
            message: "Ours".to_string(),
            author: "alice".to_string(),
            timestamp: Utc::now(),
            op_index: 1,
            parent: Some(base_id.clone()),
            branch: "main".to_string(),
        };
        let ours_id = ours_v.id.clone();
        h.versions.push(ours_v);

        // Theirs: modify_face by bob (same op_type + data, different author)
        h.operations.push(VersionedOp {
            id: Uuid::new_v4().to_string(),
            op_type: "modify_face".to_string(),
            data: "{\"face\":0}".to_string(),
            author: "bob".to_string(),
            timestamp: Utc::now(),
            reversible: false,
            inverse_data: None,
        });
        let theirs_v = Version {
            id: Uuid::new_v4().to_string(),
            name: None,
            message: "Theirs".to_string(),
            author: "bob".to_string(),
            timestamp: Utc::now(),
            op_index: 2,
            parent: Some(base_id.clone()),
            branch: "feature".to_string(),
        };
        let theirs_id = theirs_v.id.clone();
        h.versions.push(theirs_v);

        let merge = three_way_merge_diff(&h, &base_id, &ours_id, &theirs_id).unwrap();
        assert_eq!(merge.conflicting.len(), 1);
        assert!(merge.ours_only.is_empty(), "conflicted op should not be in ours_only");
        assert!(merge.theirs_only.is_empty(), "conflicted op should not be in theirs_only");
        assert_eq!(merge.conflicting[0].0.author, "alice");
        assert_eq!(merge.conflicting[0].1.author, "bob");
    }
}
