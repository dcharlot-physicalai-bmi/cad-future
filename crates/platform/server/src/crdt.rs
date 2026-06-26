//! CRDT-based conflict-free concurrent editing for CAD collaboration.
//!
//! Provides convergent replicated data types (CRDTs) that allow multiple
//! users to concurrently edit a design without coordination. All sites
//! are guaranteed to converge to the same state regardless of operation
//! ordering.
//!
//! ## Data structures
//!
//! - [`LwwRegister`] — Last-Writer-Wins register for scalar parameters
//! - [`GCounter`] / [`PnCounter`] — Monotonic counters for version tracking
//! - [`OrSet`] — Observed-Remove Set for feature tree add/remove
//! - [`CausalTree`] — Causally ordered operation log
//! - [`CrdtDocument`] — Top-level wrapper combining all CRDT state

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};

// ---------------------------------------------------------------------------
// Site ID
// ---------------------------------------------------------------------------

/// Unique identifier for a collaborating site (user session).
pub type SiteId = u64;

// ---------------------------------------------------------------------------
// LWW Register (Last-Writer-Wins)
// ---------------------------------------------------------------------------

/// A last-writer-wins register for scalar values.
///
/// Concurrent writes are resolved by timestamp; ties broken by site_id
/// (higher site_id wins). Used for dimension values, material assignments,
/// and other single-valued parameters.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LwwRegister<T: Clone + PartialEq> {
    pub value: T,
    pub timestamp: u64,
    pub site_id: SiteId,
}

impl<T: Clone + PartialEq> LwwRegister<T> {
    /// Create a new register with an initial value.
    pub fn new(value: T, timestamp: u64, site_id: SiteId) -> Self {
        Self {
            value,
            timestamp,
            site_id,
        }
    }

    /// Update the register locally.
    pub fn set(&mut self, value: T, timestamp: u64, site_id: SiteId) {
        if (timestamp, site_id) > (self.timestamp, self.site_id) {
            self.value = value;
            self.timestamp = timestamp;
            self.site_id = site_id;
        }
    }

    /// Merge with a remote register. Higher timestamp wins; ties broken
    /// by higher site_id.
    pub fn merge(&mut self, other: &LwwRegister<T>) {
        if (other.timestamp, other.site_id) > (self.timestamp, self.site_id) {
            self.value = other.value.clone();
            self.timestamp = other.timestamp;
            self.site_id = other.site_id;
        }
    }
}

// ---------------------------------------------------------------------------
// G-Counter (Grow-only Counter)
// ---------------------------------------------------------------------------

/// A grow-only counter where each site has a local monotonically increasing
/// value. Merge = element-wise max.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GCounter {
    counts: BTreeMap<SiteId, u64>,
}

impl GCounter {
    pub fn new() -> Self {
        Self {
            counts: BTreeMap::new(),
        }
    }

    /// Increment the counter for a site.
    pub fn increment(&mut self, site_id: SiteId) {
        let entry = self.counts.entry(site_id).or_insert(0);
        *entry += 1;
    }

    /// Get the total count across all sites.
    pub fn value(&self) -> u64 {
        self.counts.values().sum()
    }

    /// Get the count for a specific site.
    pub fn site_value(&self, site_id: SiteId) -> u64 {
        self.counts.get(&site_id).copied().unwrap_or(0)
    }

    /// Merge with another G-Counter (element-wise max).
    pub fn merge(&mut self, other: &GCounter) {
        for (&site, &count) in &other.counts {
            let entry = self.counts.entry(site).or_insert(0);
            *entry = (*entry).max(count);
        }
    }
}

impl Default for GCounter {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// PN-Counter (Positive-Negative Counter)
// ---------------------------------------------------------------------------

/// A counter that supports both increment and decrement.
/// Implemented as a pair of G-Counters (positive and negative).
/// Used for feature tree version tracking.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PnCounter {
    pub pos: GCounter,
    pub neg: GCounter,
}

impl PnCounter {
    pub fn new() -> Self {
        Self {
            pos: GCounter::new(),
            neg: GCounter::new(),
        }
    }

    /// Increment the counter for a site.
    pub fn increment(&mut self, site_id: SiteId) {
        self.pos.increment(site_id);
    }

    /// Decrement the counter for a site.
    pub fn decrement(&mut self, site_id: SiteId) {
        self.neg.increment(site_id);
    }

    /// Get the net value (positive - negative).
    pub fn value(&self) -> i64 {
        self.pos.value() as i64 - self.neg.value() as i64
    }

    /// Merge with another PN-Counter.
    pub fn merge(&mut self, other: &PnCounter) {
        self.pos.merge(&other.pos);
        self.neg.merge(&other.neg);
    }
}

impl Default for PnCounter {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// OR-Set (Observed-Remove Set)
// ---------------------------------------------------------------------------

/// A unique tag for an add operation in the OR-Set.
#[derive(Debug, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct AddTag {
    pub site_id: SiteId,
    pub counter: u64,
}

/// An Observed-Remove Set: supports concurrent add and remove without conflicts.
///
/// Each add is tagged with a unique (site_id, counter) pair. Removes reference
/// specific add-tags (the ones observed at the time of removal). An element is
/// in the set iff it has at least one add-tag not covered by a remove.
///
/// Used for the feature list: concurrent add/remove of features converges.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OrSet<T: Clone + Eq + std::hash::Hash> {
    /// Map from element → set of active (non-removed) add-tags.
    entries: HashMap<T, HashSet<AddTag>>,
    /// Per-site counter for generating unique add-tags.
    counters: HashMap<SiteId, u64>,
}

impl<T: Clone + Eq + std::hash::Hash> OrSet<T> {
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
            counters: HashMap::new(),
        }
    }

    /// Add an element. Returns the tag assigned to this add operation.
    pub fn add(&mut self, element: T, site_id: SiteId) -> AddTag {
        let counter = self.counters.entry(site_id).or_insert(0);
        *counter += 1;
        let tag = AddTag {
            site_id,
            counter: *counter,
        };
        self.entries
            .entry(element)
            .or_default()
            .insert(tag.clone());
        tag
    }

    /// Remove an element by removing all currently observed add-tags.
    /// Returns the set of tags that were removed (needed for broadcasting).
    pub fn remove(&mut self, element: &T) -> HashSet<AddTag> {
        self.entries.remove(element).unwrap_or_default()
    }

    /// Check if an element is in the set.
    pub fn contains(&self, element: &T) -> bool {
        self.entries
            .get(element)
            .is_some_and(|tags| !tags.is_empty())
    }

    /// Get all elements currently in the set.
    pub fn elements(&self) -> Vec<&T> {
        self.entries
            .iter()
            .filter(|(_, tags)| !tags.is_empty())
            .map(|(elem, _)| elem)
            .collect()
    }

    /// Number of elements in the set.
    pub fn len(&self) -> usize {
        self.entries
            .iter()
            .filter(|(_, tags)| !tags.is_empty())
            .count()
    }

    /// Whether the set is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Apply a remote add operation.
    pub fn apply_add(&mut self, element: T, tag: AddTag) {
        // Update counter tracking
        let counter = self.counters.entry(tag.site_id).or_insert(0);
        *counter = (*counter).max(tag.counter);

        self.entries.entry(element).or_default().insert(tag);
    }

    /// Apply a remote remove operation (remove specific tags).
    pub fn apply_remove(&mut self, element: &T, tags: &HashSet<AddTag>) {
        if let Some(entry_tags) = self.entries.get_mut(element) {
            for tag in tags {
                entry_tags.remove(tag);
            }
            if entry_tags.is_empty() {
                self.entries.remove(element);
            }
        }
    }

    /// Merge with another OR-Set. Union of all add-tags minus those
    /// removed in both.
    pub fn merge(&mut self, other: &OrSet<T>) {
        // Merge counters
        for (&site, &count) in &other.counters {
            let entry = self.counters.entry(site).or_insert(0);
            *entry = (*entry).max(count);
        }

        // Add entries from other that we don't have
        for (elem, other_tags) in &other.entries {
            let local_tags = self.entries.entry(elem.clone()).or_default();
            for tag in other_tags {
                local_tags.insert(tag.clone());
            }
        }
    }
}

impl<T: Clone + Eq + std::hash::Hash> Default for OrSet<T> {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Causal Tree (Ordered operation log)
// ---------------------------------------------------------------------------

/// Unique operation identifier within the causal tree.
#[derive(Debug, Clone, Hash, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub struct OpId {
    pub timestamp: u64,
    pub site_id: SiteId,
}

/// A node in the causal tree representing a single operation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CausalNode {
    pub id: OpId,
    /// Parent operation this depends on (None for root operations).
    pub parent: Option<OpId>,
    /// The operation payload.
    pub op: FeatureOp,
    /// Whether this node has been logically deleted.
    pub deleted: bool,
}

/// Operations on the feature tree.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum FeatureOp {
    /// Add a feature with a JSON descriptor.
    AddFeature {
        feature_id: String,
        feature_json: String,
    },
    /// Remove a feature by ID.
    RemoveFeature { feature_id: String },
    /// Modify a parameter on a feature.
    ModifyParameter {
        feature_id: String,
        param_name: String,
        value: String,
    },
    /// Reorder a feature in the tree.
    ReorderFeature {
        feature_id: String,
        new_position: usize,
    },
}

/// A causally ordered operation log for feature sequences.
///
/// Each operation has a parent reference establishing causal ordering.
/// Concurrent operations (neither is ancestor of the other) are ordered
/// deterministically by (timestamp, site_id) for convergence.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CausalTree {
    nodes: Vec<CausalNode>,
    /// Per-site logical clock.
    clocks: BTreeMap<SiteId, u64>,
}

impl CausalTree {
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            clocks: BTreeMap::new(),
        }
    }

    /// Generate a new unique OpId for a site.
    pub fn next_id(&mut self, site_id: SiteId) -> OpId {
        let clock = self.clocks.entry(site_id).or_insert(0);
        *clock += 1;
        OpId {
            timestamp: *clock,
            site_id,
        }
    }

    /// Get the latest OpId (for use as parent reference).
    pub fn latest_id(&self) -> Option<OpId> {
        self.nodes.last().map(|n| n.id.clone())
    }

    /// Apply a local operation. Returns the created node.
    pub fn apply_local(
        &mut self,
        site_id: SiteId,
        parent: Option<OpId>,
        op: FeatureOp,
    ) -> CausalNode {
        let id = self.next_id(site_id);
        let node = CausalNode {
            id,
            parent,
            op,
            deleted: false,
        };
        self.insert_node(node.clone());
        node
    }

    /// Apply a remote operation (merge).
    pub fn apply_remote(&mut self, node: CausalNode) {
        // Update our clock to be at least as high as the remote
        let clock = self.clocks.entry(node.id.site_id).or_insert(0);
        *clock = (*clock).max(node.id.timestamp);

        if !self.nodes.iter().any(|n| n.id == node.id) {
            self.insert_node(node);
        }
    }

    /// Insert a node in causal order.
    fn insert_node(&mut self, node: CausalNode) {
        // Find the correct insertion point: after parent, ordered by OpId
        // among concurrent siblings.
        let insert_pos = if let Some(ref parent_id) = node.parent {
            // Find parent position
            let parent_pos = self
                .nodes
                .iter()
                .position(|n| n.id == *parent_id)
                .map(|p| p + 1)
                .unwrap_or(self.nodes.len());

            // Find position among siblings (concurrent ops with same parent)
            let mut pos = parent_pos;
            while pos < self.nodes.len() {
                let existing = &self.nodes[pos];
                // Stop if we hit a node with a different parent that isn't
                // a descendant of our parent
                if existing.parent.as_ref() != node.parent.as_ref()
                    && existing.parent.as_ref() != Some(&node.id)
                {
                    break;
                }
                // Among concurrent siblings, order by OpId (timestamp, site_id)
                if existing.parent == node.parent && existing.id > node.id {
                    break;
                }
                pos += 1;
            }
            pos
        } else {
            // Root operations go at the end, ordered by OpId
            let mut pos = 0;
            while pos < self.nodes.len() && self.nodes[pos].id < node.id {
                pos += 1;
            }
            pos
        };

        self.nodes.insert(insert_pos, node);
    }

    /// Get all nodes in causal order.
    pub fn nodes(&self) -> &[CausalNode] {
        &self.nodes
    }

    /// Get all non-deleted nodes in order.
    pub fn active_nodes(&self) -> Vec<&CausalNode> {
        self.nodes.iter().filter(|n| !n.deleted).collect()
    }

    /// Mark a node as deleted.
    pub fn delete_node(&mut self, id: &OpId) {
        if let Some(node) = self.nodes.iter_mut().find(|n| n.id == *id) {
            node.deleted = true;
        }
    }

    /// Merge with another causal tree.
    pub fn merge(&mut self, other: &CausalTree) {
        // Merge clocks
        for (&site, &clock) in &other.clocks {
            let entry = self.clocks.entry(site).or_insert(0);
            *entry = (*entry).max(clock);
        }

        // Insert any nodes we don't have
        for node in &other.nodes {
            if !self.nodes.iter().any(|n| n.id == node.id) {
                self.insert_node(node.clone());
            } else if node.deleted {
                // Propagate deletions
                if let Some(existing) = self.nodes.iter_mut().find(|n| n.id == node.id) {
                    existing.deleted = true;
                }
            }
        }
    }

    /// Number of nodes (including deleted).
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Whether the tree is empty.
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }
}

impl Default for CausalTree {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// CRDT Delta (for broadcasting)
// ---------------------------------------------------------------------------

/// A delta representing changes to broadcast to other sites.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CrdtDelta {
    /// A parameter register was updated.
    RegisterUpdate {
        feature_id: String,
        param_name: String,
        value: String,
        timestamp: u64,
        site_id: SiteId,
    },
    /// A feature was added to the OR-Set.
    FeatureAdded {
        feature_id: String,
        tag: AddTag,
    },
    /// A feature was removed from the OR-Set.
    FeatureRemoved {
        feature_id: String,
        removed_tags: HashSet<AddTag>,
    },
    /// A causal tree node was added.
    CausalOp {
        node: CausalNode,
    },
    /// Version counter incremented.
    VersionTick {
        site_id: SiteId,
    },
}

// ---------------------------------------------------------------------------
// Conflict Detection
// ---------------------------------------------------------------------------

/// Types of conflicts that can be detected between concurrent operations.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ConflictType {
    /// Two sites edited the same parameter on the same feature.
    SameParameterEdit {
        feature_id: String,
        param_name: String,
        value_a: String,
        site_a: SiteId,
        value_b: String,
        site_b: SiteId,
    },
    /// Two sites reordered features in conflicting ways.
    ReorderConflict {
        feature_id: String,
        position_a: usize,
        site_a: SiteId,
        position_b: usize,
        site_b: SiteId,
    },
    /// One site deleted a feature while another edited it.
    DeleteEditConflict {
        feature_id: String,
        deleter: SiteId,
        editor: SiteId,
    },
}

/// A detected conflict between two concurrent operations.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Conflict {
    pub conflict_type: ConflictType,
    /// Human-readable description of the conflict.
    pub description: String,
    /// Whether this was auto-resolved by CRDT semantics.
    pub auto_resolved: bool,
    /// Description of the resolution applied.
    pub resolution: String,
}

/// Detect conflicts between two concurrent feature operations.
pub fn detect_conflicts(op1: &FeatureOp, op2: &FeatureOp) -> Option<Conflict> {
    match (op1, op2) {
        // Same parameter edited by different operations
        (
            FeatureOp::ModifyParameter {
                feature_id: f1,
                param_name: p1,
                value: v1,
            },
            FeatureOp::ModifyParameter {
                feature_id: f2,
                param_name: p2,
                value: v2,
            },
        ) if f1 == f2 && p1 == p2 && v1 != v2 => Some(Conflict {
            conflict_type: ConflictType::SameParameterEdit {
                feature_id: f1.clone(),
                param_name: p1.clone(),
                value_a: v1.clone(),
                site_a: 0, // filled in by caller
                value_b: v2.clone(),
                site_b: 0,
            },
            description: format!(
                "Concurrent edits to {}.{}: '{}' vs '{}'",
                f1, p1, v1, v2
            ),
            auto_resolved: true,
            resolution: "LWW: later timestamp wins".into(),
        }),

        // Concurrent reorder of the same feature
        (
            FeatureOp::ReorderFeature {
                feature_id: f1,
                new_position: p1,
            },
            FeatureOp::ReorderFeature {
                feature_id: f2,
                new_position: p2,
            },
        ) if f1 == f2 && p1 != p2 => Some(Conflict {
            conflict_type: ConflictType::ReorderConflict {
                feature_id: f1.clone(),
                position_a: *p1,
                site_a: 0,
                position_b: *p2,
                site_b: 0,
            },
            description: format!(
                "Concurrent reorder of {}: position {} vs {}",
                f1, p1, p2
            ),
            auto_resolved: true,
            resolution: "LWW: later timestamp wins for position".into(),
        }),

        // One removes a feature the other edits
        (
            FeatureOp::RemoveFeature { feature_id: f1 },
            FeatureOp::ModifyParameter {
                feature_id: f2, ..
            },
        ) if f1 == f2 => Some(Conflict {
            conflict_type: ConflictType::DeleteEditConflict {
                feature_id: f1.clone(),
                deleter: 0,
                editor: 0,
            },
            description: format!("Feature {} deleted while being edited", f1),
            auto_resolved: true,
            resolution: "OR-Set semantics: delete wins".into(),
        }),

        (
            FeatureOp::ModifyParameter {
                feature_id: f1, ..
            },
            FeatureOp::RemoveFeature { feature_id: f2 },
        ) if f1 == f2 => Some(Conflict {
            conflict_type: ConflictType::DeleteEditConflict {
                feature_id: f1.clone(),
                deleter: 0,
                editor: 0,
            },
            description: format!("Feature {} edited while being deleted", f1),
            auto_resolved: true,
            resolution: "OR-Set semantics: delete wins".into(),
        }),

        _ => None,
    }
}

// ---------------------------------------------------------------------------
// CRDT Document
// ---------------------------------------------------------------------------

/// Top-level CRDT document wrapping a feature tree with conflict-free state.
///
/// Combines OR-Set for feature membership, LWW registers for parameters,
/// a causal tree for ordered operations, and a PN-Counter for version tracking.
#[derive(Debug, Clone)]
pub struct CrdtDocument {
    /// This site's ID.
    pub site_id: SiteId,
    /// Feature membership (add/remove features concurrently).
    pub features: OrSet<String>,
    /// Per-feature, per-parameter LWW registers.
    pub parameters: HashMap<String, HashMap<String, LwwRegister<String>>>,
    /// Causally ordered operation log.
    pub op_log: CausalTree,
    /// Version counter for the feature tree.
    pub version: PnCounter,
    /// Pending deltas to broadcast.
    pending: Vec<CrdtDelta>,
}

impl CrdtDocument {
    /// Create a new empty CRDT document for a given site.
    pub fn new(site_id: SiteId) -> Self {
        Self {
            site_id,
            features: OrSet::new(),
            parameters: HashMap::new(),
            op_log: CausalTree::new(),
            version: PnCounter::new(),
            pending: Vec::new(),
        }
    }

    /// Apply a local operation, generating CRDT deltas for broadcast.
    pub fn apply_local(&mut self, op: FeatureOp) -> Vec<CrdtDelta> {
        let mut deltas = Vec::new();

        match &op {
            FeatureOp::AddFeature {
                feature_id,
                feature_json: _,
            } => {
                let tag = self.features.add(feature_id.clone(), self.site_id);
                deltas.push(CrdtDelta::FeatureAdded {
                    feature_id: feature_id.clone(),
                    tag,
                });
            }
            FeatureOp::RemoveFeature { feature_id } => {
                let removed_tags = self.features.remove(feature_id);
                if !removed_tags.is_empty() {
                    // Also remove parameters for this feature
                    self.parameters.remove(feature_id);
                    deltas.push(CrdtDelta::FeatureRemoved {
                        feature_id: feature_id.clone(),
                        removed_tags,
                    });
                }
            }
            FeatureOp::ModifyParameter {
                feature_id,
                param_name,
                value,
            } => {
                let timestamp = self
                    .op_log
                    .clocks
                    .get(&self.site_id)
                    .copied()
                    .unwrap_or(0)
                    + 1;
                let reg = LwwRegister::new(value.clone(), timestamp, self.site_id);

                self.parameters
                    .entry(feature_id.clone())
                    .or_default()
                    .insert(param_name.clone(), reg);

                deltas.push(CrdtDelta::RegisterUpdate {
                    feature_id: feature_id.clone(),
                    param_name: param_name.clone(),
                    value: value.clone(),
                    timestamp,
                    site_id: self.site_id,
                });
            }
            FeatureOp::ReorderFeature { .. } => {
                // Reorder is captured in the causal tree
            }
        }

        // Record in causal tree
        let parent = self.op_log.latest_id();
        let node = self.op_log.apply_local(self.site_id, parent, op);
        deltas.push(CrdtDelta::CausalOp { node });

        // Bump version
        self.version.increment(self.site_id);
        deltas.push(CrdtDelta::VersionTick {
            site_id: self.site_id,
        });

        self.pending.extend(deltas.clone());
        deltas
    }

    /// Merge an incoming remote delta.
    pub fn merge_remote(&mut self, delta: &CrdtDelta) {
        match delta {
            CrdtDelta::RegisterUpdate {
                feature_id,
                param_name,
                value,
                timestamp,
                site_id,
            } => {
                let remote_reg = LwwRegister::new(value.clone(), *timestamp, *site_id);
                let params = self.parameters.entry(feature_id.clone()).or_default();
                match params.get_mut(param_name) {
                    Some(local_reg) => local_reg.merge(&remote_reg),
                    None => {
                        params.insert(param_name.clone(), remote_reg);
                    }
                }
            }
            CrdtDelta::FeatureAdded { feature_id, tag } => {
                self.features.apply_add(feature_id.clone(), tag.clone());
            }
            CrdtDelta::FeatureRemoved {
                feature_id,
                removed_tags,
            } => {
                self.features.apply_remove(feature_id, removed_tags);
                // If the feature is now fully removed, clean up parameters
                if !self.features.contains(feature_id) {
                    self.parameters.remove(feature_id);
                }
            }
            CrdtDelta::CausalOp { node } => {
                self.op_log.apply_remote(node.clone());
            }
            CrdtDelta::VersionTick { site_id } => {
                self.version.pos.merge(&{
                    let mut g = GCounter::new();
                    let current = self.version.pos.site_value(*site_id);
                    // Simulate a merge where the remote site is at current+1
                    for _ in 0..=current {
                        g.increment(*site_id);
                    }
                    g
                });
            }
        }
    }

    /// Materialize the current feature operations from CRDT state.
    pub fn to_feature_ops(&self) -> Vec<FeatureOp> {
        self.op_log
            .active_nodes()
            .iter()
            .map(|node| node.op.clone())
            .collect()
    }

    /// Drain pending deltas that need to be broadcast to other sites.
    pub fn pending_deltas(&mut self) -> Vec<CrdtDelta> {
        std::mem::take(&mut self.pending)
    }

    /// Get the current value of a parameter (from LWW register).
    pub fn get_parameter(&self, feature_id: &str, param_name: &str) -> Option<&str> {
        self.parameters
            .get(feature_id)
            .and_then(|params| params.get(param_name))
            .map(|reg| reg.value.as_str())
    }

    /// Check if a feature exists in the document.
    pub fn has_feature(&self, feature_id: &str) -> bool {
        self.features.contains(&feature_id.to_string())
    }

    /// List all feature IDs currently in the document.
    pub fn feature_ids(&self) -> Vec<String> {
        self.features
            .elements()
            .into_iter()
            .cloned()
            .collect()
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // LWW Register tests
    // -----------------------------------------------------------------------

    #[test]
    fn lww_register_later_timestamp_wins() {
        let mut reg_a = LwwRegister::new("10mm".to_string(), 1, 1);
        let reg_b = LwwRegister::new("20mm".to_string(), 2, 2);

        reg_a.merge(&reg_b);
        assert_eq!(reg_a.value, "20mm");
        assert_eq!(reg_a.timestamp, 2);
    }

    #[test]
    fn lww_register_tie_broken_by_site_id() {
        let mut reg_a = LwwRegister::new("10mm".to_string(), 5, 1);
        let reg_b = LwwRegister::new("20mm".to_string(), 5, 2);

        reg_a.merge(&reg_b);
        // Same timestamp, higher site_id wins
        assert_eq!(reg_a.value, "20mm");
        assert_eq!(reg_a.site_id, 2);
    }

    #[test]
    fn lww_register_earlier_timestamp_ignored() {
        let mut reg_a = LwwRegister::new("20mm".to_string(), 5, 2);
        let reg_b = LwwRegister::new("10mm".to_string(), 3, 1);

        reg_a.merge(&reg_b);
        // Earlier timestamp does not overwrite
        assert_eq!(reg_a.value, "20mm");
        assert_eq!(reg_a.timestamp, 5);
    }

    #[test]
    fn lww_register_concurrent_writes() {
        // Simulate two sites making concurrent writes
        let mut site1 = LwwRegister::new("initial".to_string(), 0, 0);
        let mut site2 = site1.clone();

        // Site 1 writes at t=3
        site1.set("from_site1".to_string(), 3, 1);
        // Site 2 writes at t=4
        site2.set("from_site2".to_string(), 4, 2);

        // Merge in both directions — should converge
        let mut merged1 = site1.clone();
        merged1.merge(&site2);

        let mut merged2 = site2.clone();
        merged2.merge(&site1);

        assert_eq!(merged1.value, merged2.value);
        assert_eq!(merged1.value, "from_site2");
    }

    // -----------------------------------------------------------------------
    // G-Counter / PN-Counter tests
    // -----------------------------------------------------------------------

    #[test]
    fn gcounter_increment_and_merge() {
        let mut c1 = GCounter::new();
        let mut c2 = GCounter::new();

        c1.increment(1);
        c1.increment(1);
        c2.increment(2);
        c2.increment(2);
        c2.increment(2);

        c1.merge(&c2);
        assert_eq!(c1.value(), 5); // 2 from site 1 + 3 from site 2
    }

    #[test]
    fn pncounter_increment_decrement() {
        let mut counter = PnCounter::new();
        counter.increment(1);
        counter.increment(1);
        counter.increment(1);
        counter.decrement(2);

        assert_eq!(counter.value(), 2); // 3 - 1
    }

    #[test]
    fn pncounter_merge_converges() {
        let mut c1 = PnCounter::new();
        let mut c2 = PnCounter::new();

        c1.increment(1);
        c1.increment(1);
        c2.increment(2);
        c2.decrement(2);

        c1.merge(&c2);
        c2.merge(&c1);

        assert_eq!(c1.value(), c2.value());
        assert_eq!(c1.value(), 2); // site1: +2, site2: +1 -1 = 0, total = 2
    }

    // -----------------------------------------------------------------------
    // OR-Set tests
    // -----------------------------------------------------------------------

    #[test]
    fn orset_add_remove_readd() {
        let mut set = OrSet::new();
        set.add("box".to_string(), 1);
        assert!(set.contains(&"box".to_string()));

        set.remove(&"box".to_string());
        assert!(!set.contains(&"box".to_string()));

        // Re-add after remove
        set.add("box".to_string(), 1);
        assert!(set.contains(&"box".to_string()));
    }

    #[test]
    fn orset_concurrent_add_remove() {
        // Site 1 adds "box"
        let mut site1 = OrSet::new();
        let tag = site1.add("box".to_string(), 1);

        // Site 2 has a copy, then removes "box"
        let mut site2 = site1.clone();
        let removed_tags = site2.remove(&"box".to_string());
        assert!(!site2.contains(&"box".to_string()));

        // Meanwhile site 1 adds "box" again (concurrent with the remove)
        let _tag2 = site1.add("box".to_string(), 1);

        // Merge: site 2 applies site 1's second add
        // The remove only covered tag1, not tag2, so "box" should be present
        site2.apply_add("box".to_string(), AddTag { site_id: 1, counter: 2 });
        assert!(site2.contains(&"box".to_string()));
    }

    #[test]
    fn orset_merge_concurrent_adds() {
        let mut site1 = OrSet::new();
        let mut site2 = OrSet::new();

        site1.add("box".to_string(), 1);
        site1.add("cylinder".to_string(), 1);

        site2.add("sphere".to_string(), 2);

        site1.merge(&site2);
        assert!(site1.contains(&"box".to_string()));
        assert!(site1.contains(&"cylinder".to_string()));
        assert!(site1.contains(&"sphere".to_string()));
        assert_eq!(site1.len(), 3);
    }

    #[test]
    fn orset_delete_wins_over_concurrent_edit() {
        // Simulates OR-Set delete semantics: if one site removes
        // and the other has only the same tag, the element disappears.
        let mut site1 = OrSet::new();
        let tag = site1.add("feature_a".to_string(), 1);
        let mut site2 = site1.clone();

        // Site 1 removes
        let removed = site1.remove(&"feature_a".to_string());
        assert!(!site1.contains(&"feature_a".to_string()));

        // Site 2 still has it, but when it receives the remove for the
        // observed tags, the element is gone
        site2.apply_remove(&"feature_a".to_string(), &removed);
        assert!(!site2.contains(&"feature_a".to_string()));
    }

    // -----------------------------------------------------------------------
    // Causal Tree tests
    // -----------------------------------------------------------------------

    #[test]
    fn causal_tree_preserves_order() {
        let mut tree = CausalTree::new();

        let n1 = tree.apply_local(
            1,
            None,
            FeatureOp::AddFeature {
                feature_id: "box".into(),
                feature_json: "{}".into(),
            },
        );
        let n2 = tree.apply_local(
            1,
            Some(n1.id.clone()),
            FeatureOp::AddFeature {
                feature_id: "hole".into(),
                feature_json: "{}".into(),
            },
        );

        let nodes = tree.active_nodes();
        assert_eq!(nodes.len(), 2);
        // First node should be "box", second "hole"
        match &nodes[0].op {
            FeatureOp::AddFeature { feature_id, .. } => assert_eq!(feature_id, "box"),
            _ => panic!("expected AddFeature"),
        }
        match &nodes[1].op {
            FeatureOp::AddFeature { feature_id, .. } => assert_eq!(feature_id, "hole"),
            _ => panic!("expected AddFeature"),
        }
    }

    #[test]
    fn causal_tree_concurrent_operations_ordered_by_site_id() {
        let mut tree1 = CausalTree::new();
        let mut tree2 = CausalTree::new();

        // Both sites create an op with no parent (concurrent root ops)
        let n1 = tree1.apply_local(
            1,
            None,
            FeatureOp::AddFeature {
                feature_id: "from_site1".into(),
                feature_json: "{}".into(),
            },
        );
        let n2 = tree2.apply_local(
            2,
            None,
            FeatureOp::AddFeature {
                feature_id: "from_site2".into(),
                feature_json: "{}".into(),
            },
        );

        // Merge both ways
        tree1.apply_remote(n2.clone());
        tree2.apply_remote(n1.clone());

        // Both should have the same order
        let order1: Vec<String> = tree1
            .active_nodes()
            .iter()
            .filter_map(|n| match &n.op {
                FeatureOp::AddFeature { feature_id, .. } => Some(feature_id.clone()),
                _ => None,
            })
            .collect();
        let order2: Vec<String> = tree2
            .active_nodes()
            .iter()
            .filter_map(|n| match &n.op {
                FeatureOp::AddFeature { feature_id, .. } => Some(feature_id.clone()),
                _ => None,
            })
            .collect();

        assert_eq!(order1, order2);
        assert_eq!(order1.len(), 2);
    }

    #[test]
    fn causal_tree_merge_converges() {
        let mut tree1 = CausalTree::new();
        let mut tree2 = CausalTree::new();

        let n1 = tree1.apply_local(
            1,
            None,
            FeatureOp::AddFeature {
                feature_id: "a".into(),
                feature_json: "{}".into(),
            },
        );

        let n2 = tree2.apply_local(
            2,
            None,
            FeatureOp::AddFeature {
                feature_id: "b".into(),
                feature_json: "{}".into(),
            },
        );

        let n3 = tree1.apply_local(
            1,
            Some(n1.id.clone()),
            FeatureOp::AddFeature {
                feature_id: "c".into(),
                feature_json: "{}".into(),
            },
        );

        // Full merge
        tree1.merge(&tree2);
        tree2.merge(&tree1);

        assert_eq!(tree1.len(), tree2.len());
        assert_eq!(tree1.len(), 3);
    }

    // -----------------------------------------------------------------------
    // Conflict Detection tests
    // -----------------------------------------------------------------------

    #[test]
    fn detect_same_parameter_conflict() {
        let op1 = FeatureOp::ModifyParameter {
            feature_id: "box1".into(),
            param_name: "width".into(),
            value: "10".into(),
        };
        let op2 = FeatureOp::ModifyParameter {
            feature_id: "box1".into(),
            param_name: "width".into(),
            value: "20".into(),
        };

        let conflict = detect_conflicts(&op1, &op2);
        assert!(conflict.is_some());
        let c = conflict.unwrap();
        assert!(matches!(c.conflict_type, ConflictType::SameParameterEdit { .. }));
        assert!(c.auto_resolved);
    }

    #[test]
    fn detect_delete_edit_conflict() {
        let op1 = FeatureOp::RemoveFeature {
            feature_id: "box1".into(),
        };
        let op2 = FeatureOp::ModifyParameter {
            feature_id: "box1".into(),
            param_name: "width".into(),
            value: "10".into(),
        };

        let conflict = detect_conflicts(&op1, &op2);
        assert!(conflict.is_some());
        assert!(matches!(
            conflict.unwrap().conflict_type,
            ConflictType::DeleteEditConflict { .. }
        ));
    }

    #[test]
    fn no_conflict_different_features() {
        let op1 = FeatureOp::ModifyParameter {
            feature_id: "box1".into(),
            param_name: "width".into(),
            value: "10".into(),
        };
        let op2 = FeatureOp::ModifyParameter {
            feature_id: "box2".into(),
            param_name: "width".into(),
            value: "20".into(),
        };

        assert!(detect_conflicts(&op1, &op2).is_none());
    }

    // -----------------------------------------------------------------------
    // CrdtDocument integration tests
    // -----------------------------------------------------------------------

    #[test]
    fn document_add_features_and_materialize() {
        let mut doc = CrdtDocument::new(1);

        doc.apply_local(FeatureOp::AddFeature {
            feature_id: "box".into(),
            feature_json: r#"{"type":"Box","width":10}"#.into(),
        });
        doc.apply_local(FeatureOp::AddFeature {
            feature_id: "hole".into(),
            feature_json: r#"{"type":"Hole","radius":2}"#.into(),
        });

        assert!(doc.has_feature("box"));
        assert!(doc.has_feature("hole"));
        assert_eq!(doc.to_feature_ops().len(), 2);
    }

    #[test]
    fn document_concurrent_adds_merge() {
        let mut doc1 = CrdtDocument::new(1);
        let mut doc2 = CrdtDocument::new(2);

        let deltas1 = doc1.apply_local(FeatureOp::AddFeature {
            feature_id: "box".into(),
            feature_json: "{}".into(),
        });
        let deltas2 = doc2.apply_local(FeatureOp::AddFeature {
            feature_id: "sphere".into(),
            feature_json: "{}".into(),
        });

        // Cross-merge
        for d in &deltas2 {
            doc1.merge_remote(d);
        }
        for d in &deltas1 {
            doc2.merge_remote(d);
        }

        // Both should have both features
        assert!(doc1.has_feature("box"));
        assert!(doc1.has_feature("sphere"));
        assert!(doc2.has_feature("box"));
        assert!(doc2.has_feature("sphere"));
    }

    #[test]
    fn document_parameter_lww_resolution() {
        let mut doc1 = CrdtDocument::new(1);
        let mut doc2 = CrdtDocument::new(2);

        // Both add the same feature
        let d1 = doc1.apply_local(FeatureOp::AddFeature {
            feature_id: "box".into(),
            feature_json: "{}".into(),
        });
        for d in &d1 {
            doc2.merge_remote(d);
        }

        // Site 1 sets width=10, site 2 sets width=20
        // Site 2's op will have a higher clock value since it applied
        // remote ops first.
        let d_param1 = doc1.apply_local(FeatureOp::ModifyParameter {
            feature_id: "box".into(),
            param_name: "width".into(),
            value: "10".into(),
        });
        let d_param2 = doc2.apply_local(FeatureOp::ModifyParameter {
            feature_id: "box".into(),
            param_name: "width".into(),
            value: "20".into(),
        });

        // Cross-merge
        for d in &d_param2 {
            doc1.merge_remote(d);
        }
        for d in &d_param1 {
            doc2.merge_remote(d);
        }

        // Both should converge to the same value
        let v1 = doc1.get_parameter("box", "width").unwrap();
        let v2 = doc2.get_parameter("box", "width").unwrap();
        assert_eq!(v1, v2);
    }

    #[test]
    fn document_delete_removes_feature() {
        let mut doc1 = CrdtDocument::new(1);
        let mut doc2 = CrdtDocument::new(2);

        // Site 1 adds a feature
        let d_add = doc1.apply_local(FeatureOp::AddFeature {
            feature_id: "box".into(),
            feature_json: "{}".into(),
        });
        for d in &d_add {
            doc2.merge_remote(d);
        }

        // Site 2 removes it
        let d_remove = doc2.apply_local(FeatureOp::RemoveFeature {
            feature_id: "box".into(),
        });
        for d in &d_remove {
            doc1.merge_remote(d);
        }

        // Both should agree it's gone
        assert!(!doc1.has_feature("box"));
        assert!(!doc2.has_feature("box"));
    }

    #[test]
    fn document_pending_deltas_drained() {
        let mut doc = CrdtDocument::new(1);

        doc.apply_local(FeatureOp::AddFeature {
            feature_id: "box".into(),
            feature_json: "{}".into(),
        });

        let deltas = doc.pending_deltas();
        assert!(!deltas.is_empty());

        // Should be empty after draining
        let deltas2 = doc.pending_deltas();
        assert!(deltas2.is_empty());
    }

    #[test]
    fn multiple_sites_converge() {
        let mut doc1 = CrdtDocument::new(1);
        let mut doc2 = CrdtDocument::new(2);
        let mut doc3 = CrdtDocument::new(3);

        // Each site adds a different feature
        let d1 = doc1.apply_local(FeatureOp::AddFeature {
            feature_id: "box".into(),
            feature_json: "{}".into(),
        });
        let d2 = doc2.apply_local(FeatureOp::AddFeature {
            feature_id: "sphere".into(),
            feature_json: "{}".into(),
        });
        let d3 = doc3.apply_local(FeatureOp::AddFeature {
            feature_id: "cylinder".into(),
            feature_json: "{}".into(),
        });

        // Fan-out merge: everyone gets everyone's deltas
        for d in &d2 {
            doc1.merge_remote(d);
            doc3.merge_remote(d);
        }
        for d in &d1 {
            doc2.merge_remote(d);
            doc3.merge_remote(d);
        }
        for d in &d3 {
            doc1.merge_remote(d);
            doc2.merge_remote(d);
        }

        // All three should have all three features
        for doc in [&doc1, &doc2, &doc3] {
            assert!(doc.has_feature("box"));
            assert!(doc.has_feature("sphere"));
            assert!(doc.has_feature("cylinder"));
        }

        // Feature ID lists should have the same length
        assert_eq!(doc1.feature_ids().len(), 3);
        assert_eq!(doc2.feature_ids().len(), 3);
        assert_eq!(doc3.feature_ids().len(), 3);
    }
}
