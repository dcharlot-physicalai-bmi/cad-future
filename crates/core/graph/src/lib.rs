//! physical-graph: 16-dimension channel graph for CAD dependency tracking
//!
//! Tracks design features as nodes with typed edges representing dependencies
//! across 16 channels: geometric, parametric, material, manufacturing, etc.
//!
//! Built on petgraph with a domain-specific API for parametric CAD.

use petgraph::graph::{DiGraph, EdgeIndex, NodeIndex};
use petgraph::visit::EdgeRef;
use petgraph::Direction;
use serde::{Deserialize, Serialize};
use std::collections::{HashSet, VecDeque};

// ---------------------------------------------------------------------------
// Identifiers
// ---------------------------------------------------------------------------

/// Opaque node identifier (wraps petgraph's NodeIndex).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeId(pub(crate) NodeIndex);

/// Opaque edge identifier (wraps petgraph's EdgeIndex).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EdgeId(pub(crate) EdgeIndex);

// ---------------------------------------------------------------------------
// Node types
// ---------------------------------------------------------------------------

/// The kind of design entity a node represents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum NodeKind {
    /// 2D sketch (profiles, construction geometry)
    Sketch,
    /// 3D feature (extrude, revolve, fillet, chamfer, etc.)
    Feature,
    /// Named parameter (length, angle, ratio, etc.)
    Parameter,
    /// Material assignment
    Material,
    /// Geometric constraint (coincident, parallel, tangent, etc.)
    Constraint,
    /// Assembly node (top-level or sub-assembly)
    Assembly,
    /// Drawing view (front, section, detail, etc.)
    Drawing,
    /// Manufacturing operation (mill, turn, print layer, etc.)
    Manufacturing,
}

/// The 3D feature operation type.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum FeatureOp {
    Extrude { depth_m: f64 },
    Revolve { angle_rad: f64 },
    Fillet { radius_m: f64 },
    Chamfer { distance_m: f64 },
    Shell { thickness_m: f64 },
    Sweep,
    Loft,
    Boolean { op: BoolOp },
    Hole { diameter_m: f64, depth_m: f64 },
    Pattern { kind: PatternKind, count: u32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BoolOp {
    Union,
    Subtract,
    Intersect,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PatternKind {
    Linear,
    Circular,
    Mirror,
}

/// The geometric constraint type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConstraintKind {
    Coincident,
    Parallel,
    Perpendicular,
    Tangent,
    Equal,
    Horizontal,
    Vertical,
    Fixed,
    Concentric,
    Symmetric,
}

/// The assembly mate type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MateKind {
    Coincident,
    Concentric,
    Distance,
    Angle,
    Tangent,
    Gear,
}

/// Typed payload for each node kind.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum NodeData {
    Sketch {
        plane: String,
    },
    Feature {
        op: FeatureOp,
    },
    Parameter {
        name: String,
        value: f64,
        units: String,
    },
    Material {
        material_id: String,
    },
    Constraint {
        kind: ConstraintKind,
    },
    Assembly {
        name: String,
    },
    Drawing {
        view_type: String,
        scale: f64,
    },
    Manufacturing {
        process: String,
        tool_id: Option<String>,
    },
}

impl NodeData {
    /// Returns the NodeKind that matches this data variant.
    pub fn kind(&self) -> NodeKind {
        match self {
            NodeData::Sketch { .. } => NodeKind::Sketch,
            NodeData::Feature { .. } => NodeKind::Feature,
            NodeData::Parameter { .. } => NodeKind::Parameter,
            NodeData::Material { .. } => NodeKind::Material,
            NodeData::Constraint { .. } => NodeKind::Constraint,
            NodeData::Assembly { .. } => NodeKind::Assembly,
            NodeData::Drawing { .. } => NodeKind::Drawing,
            NodeData::Manufacturing { .. } => NodeKind::Manufacturing,
        }
    }
}

// ---------------------------------------------------------------------------
// Graph node
// ---------------------------------------------------------------------------

/// A node in the design dependency graph.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GraphNode {
    pub id: NodeId,
    pub kind: NodeKind,
    pub label: String,
    pub data: NodeData,
}

// ---------------------------------------------------------------------------
// Edge types — the 16 channels
// ---------------------------------------------------------------------------

/// The 16 dependency channels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum EdgeKind {
    /// ch0: feature depends on sketch/parameter
    DependsOn = 0,
    /// ch1: dimension driven by parameter
    DrivenBy = 1,
    /// ch2: material assignment
    MaterialOf = 2,
    /// ch3: geometric constraint
    ConstrainedBy = 3,
    /// ch4: feature derived from another
    DerivedFrom = 4,
    /// ch5: pattern instance of original
    PatternOf = 5,
    /// ch6: mirror instance
    MirrorOf = 6,
    /// ch7: part belongs to assembly
    AssemblyOf = 7,
    /// ch8: assembly mate
    MateWith = 8,
    /// ch9: manufacturing process assignment
    ManufacturedBy = 9,
    /// ch10: GD&T tolerance
    TolerancedBy = 10,
    /// ch11: inspection/QA reference
    InspectedBy = 11,
    /// ch12: drawing view reference
    ReferencedBy = 12,
    /// ch13: version/branch link
    VersionOf = 13,
    /// ch14: AI similarity (from HyperDB)
    SimilarTo = 14,
    /// ch15: user-defined relationship
    CustomLink = 15,
}

impl EdgeKind {
    /// Channel number (0..15).
    pub fn channel(self) -> u8 {
        self as u8
    }
}

/// Edge weight stored in the petgraph.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GraphEdge {
    pub kind: EdgeKind,
}

// ---------------------------------------------------------------------------
// DesignGraph — the main API
// ---------------------------------------------------------------------------

/// The core dependency graph for a parametric CAD model.
///
/// Wraps a directed petgraph with domain-specific operations for
/// change propagation, cycle detection, and subgraph extraction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DesignGraph {
    inner: DiGraph<GraphNode, GraphEdge>,
}

impl DesignGraph {
    /// Create an empty design graph.
    pub fn new() -> Self {
        Self {
            inner: DiGraph::new(),
        }
    }

    /// Add a design node. Returns its stable identifier.
    pub fn add_node(&mut self, kind: NodeKind, label: impl Into<String>, data: NodeData) -> NodeId {
        debug_assert_eq!(
            kind,
            data.kind(),
            "NodeKind must match NodeData variant"
        );
        let label = label.into();
        let idx = self.inner.add_node(GraphNode {
            id: NodeId(NodeIndex::new(0)), // placeholder, patched below
            kind,
            label,
            data,
        });
        // Patch the stored id to match the actual index.
        self.inner[idx].id = NodeId(idx);
        NodeId(idx)
    }

    /// Add a typed dependency edge from `from` to `to`. Returns the edge id.
    pub fn add_edge(&mut self, from: NodeId, to: NodeId, kind: EdgeKind) -> EdgeId {
        let idx = self.inner.add_edge(from.0, to.0, GraphEdge { kind });
        EdgeId(idx)
    }

    /// Remove a node and all its incident edges.
    pub fn remove_node(&mut self, id: NodeId) {
        self.inner.remove_node(id.0);
    }

    /// Get a reference to a node by id.
    pub fn node(&self, id: NodeId) -> Option<&GraphNode> {
        self.inner.node_weight(id.0)
    }

    /// Get the edge weight.
    pub fn edge(&self, id: EdgeId) -> Option<&GraphEdge> {
        self.inner.edge_weight(id.0)
    }

    /// Number of nodes in the graph.
    pub fn node_count(&self) -> usize {
        self.inner.node_count()
    }

    /// Number of edges in the graph.
    pub fn edge_count(&self) -> usize {
        self.inner.edge_count()
    }

    /// All nodes that depend on `id` (direct downstream neighbors).
    ///
    /// These are nodes reachable via outgoing edges from `id`.
    pub fn dependents(&self, id: NodeId) -> Vec<NodeId> {
        self.inner
            .neighbors_directed(id.0, Direction::Outgoing)
            .map(NodeId)
            .collect()
    }

    /// All nodes that `id` depends on (direct upstream neighbors).
    ///
    /// These are nodes reachable via incoming edges to `id`.
    pub fn dependencies(&self, id: NodeId) -> Vec<NodeId> {
        self.inner
            .neighbors_directed(id.0, Direction::Incoming)
            .map(NodeId)
            .collect()
    }

    /// Topological sort for rebuild order.
    ///
    /// Returns nodes in dependency order: a node appears after all its
    /// dependencies. Returns `None` if the graph has cycles.
    pub fn propagation_order(&self) -> Option<Vec<NodeId>> {
        petgraph::algo::toposort(&self.inner, None)
            .ok()
            .map(|sorted| sorted.into_iter().map(NodeId).collect())
    }

    /// Transitive closure of dependents — everything downstream of `id`.
    ///
    /// BFS from `id` following outgoing edges. Does not include `id` itself.
    pub fn affected_by(&self, id: NodeId) -> Vec<NodeId> {
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();
        queue.push_back(id.0);
        visited.insert(id.0);

        while let Some(current) = queue.pop_front() {
            for neighbor in self.inner.neighbors_directed(current, Direction::Outgoing) {
                if visited.insert(neighbor) {
                    queue.push_back(neighbor);
                }
            }
        }

        visited.remove(&id.0);
        visited.into_iter().map(NodeId).collect()
    }

    /// Detect circular dependencies. Returns all cycles found.
    ///
    /// Uses Tarjan's SCC algorithm — any strongly-connected component
    /// with more than one node (or a self-loop) is a cycle.
    pub fn find_cycles(&self) -> Vec<Vec<NodeId>> {
        let sccs = petgraph::algo::tarjan_scc(&self.inner);
        let mut cycles = Vec::new();
        for scc in sccs {
            if scc.len() > 1 {
                cycles.push(scc.into_iter().map(NodeId).collect());
            } else if scc.len() == 1 {
                // Check for self-loop
                let n = scc[0];
                if self.inner.contains_edge(n, n) {
                    cycles.push(vec![NodeId(n)]);
                }
            }
        }
        cycles
    }

    /// Extract a subgraph containing only the specified nodes and
    /// the edges between them.
    pub fn subgraph(&self, ids: &[NodeId]) -> DesignGraph {
        let id_set: HashSet<NodeIndex> = ids.iter().map(|id| id.0).collect();
        let mut sub = DiGraph::new();
        let mut index_map = std::collections::HashMap::new();

        // Add nodes
        for &idx in &id_set {
            if let Some(node) = self.inner.node_weight(idx) {
                let new_idx = sub.add_node(node.clone());
                // Patch the id to reflect the new graph's index
                sub[new_idx].id = NodeId(new_idx);
                index_map.insert(idx, new_idx);
            }
        }

        // Add edges between included nodes
        for edge in self.inner.edge_references() {
            let src = edge.source();
            let tgt = edge.target();
            if let (Some(&new_src), Some(&new_tgt)) = (index_map.get(&src), index_map.get(&tgt)) {
                sub.add_edge(new_src, new_tgt, edge.weight().clone());
            }
        }

        DesignGraph { inner: sub }
    }

    /// Filter nodes by kind.
    pub fn by_kind(&self, kind: NodeKind) -> Vec<NodeId> {
        self.inner
            .node_indices()
            .filter(|&idx| self.inner[idx].kind == kind)
            .map(NodeId)
            .collect()
    }

    /// Filter edges by channel kind, returning (source, target) pairs.
    pub fn edges_of_kind(&self, kind: EdgeKind) -> Vec<(NodeId, NodeId)> {
        self.inner
            .edge_references()
            .filter(|e| e.weight().kind == kind)
            .map(|e| (NodeId(e.source()), NodeId(e.target())))
            .collect()
    }

    /// Iterate over all node ids.
    pub fn node_ids(&self) -> impl Iterator<Item = NodeId> + '_ {
        self.inner.node_indices().map(NodeId)
    }

    /// Iterate over all edges as (source, target, kind).
    pub fn edges(&self) -> impl Iterator<Item = (NodeId, NodeId, EdgeKind)> + '_ {
        self.inner
            .edge_references()
            .map(|e| (NodeId(e.source()), NodeId(e.target()), e.weight().kind))
    }
}

impl Default for DesignGraph {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: build a parametric model graph:
    ///
    /// ```text
    /// param("length=50mm")
    ///   ↓ DependsOn
    /// sketch("base_profile")
    ///   ↓ DependsOn
    /// extrude("base_extrude")
    ///   ↓ DependsOn
    /// shell("thin_wall")
    ///   ↓ DependsOn
    /// fillet("edge_fillet")
    /// ```
    fn build_parametric_model() -> (DesignGraph, Vec<NodeId>) {
        let mut g = DesignGraph::new();

        let param = g.add_node(
            NodeKind::Parameter,
            "length",
            NodeData::Parameter {
                name: "length".into(),
                value: 0.050,
                units: "m".into(),
            },
        );

        let sketch = g.add_node(
            NodeKind::Sketch,
            "base_profile",
            NodeData::Sketch {
                plane: "XY".into(),
            },
        );

        let extrude = g.add_node(
            NodeKind::Feature,
            "base_extrude",
            NodeData::Feature {
                op: FeatureOp::Extrude { depth_m: 0.050 },
            },
        );

        let shell = g.add_node(
            NodeKind::Feature,
            "thin_wall",
            NodeData::Feature {
                op: FeatureOp::Shell {
                    thickness_m: 0.002,
                },
            },
        );

        let fillet = g.add_node(
            NodeKind::Feature,
            "edge_fillet",
            NodeData::Feature {
                op: FeatureOp::Fillet { radius_m: 0.001 },
            },
        );

        // param → sketch → extrude → shell → fillet
        g.add_edge(param, sketch, EdgeKind::DependsOn);
        g.add_edge(sketch, extrude, EdgeKind::DependsOn);
        g.add_edge(extrude, shell, EdgeKind::DependsOn);
        g.add_edge(shell, fillet, EdgeKind::DependsOn);

        // param also drives the extrude directly
        g.add_edge(param, extrude, EdgeKind::DrivenBy);

        (g, vec![param, sketch, extrude, shell, fillet])
    }

    #[test]
    fn test_add_nodes_and_edges() {
        let (g, nodes) = build_parametric_model();
        assert_eq!(g.node_count(), 5);
        assert_eq!(g.edge_count(), 5);
        assert_eq!(g.node(nodes[0]).unwrap().label, "length");
        assert_eq!(g.node(nodes[0]).unwrap().kind, NodeKind::Parameter);
    }

    #[test]
    fn test_dependents_and_dependencies() {
        let (g, nodes) = build_parametric_model();
        let [param, sketch, extrude, shell, _fillet] =
            [nodes[0], nodes[1], nodes[2], nodes[3], nodes[4]];

        // param's direct dependents: sketch (DependsOn) + extrude (DrivenBy)
        let deps = g.dependents(param);
        assert!(deps.contains(&sketch));
        assert!(deps.contains(&extrude));
        assert_eq!(deps.len(), 2);

        // extrude's dependencies (upstream): sketch + param
        let upstream = g.dependencies(extrude);
        assert!(upstream.contains(&sketch));
        assert!(upstream.contains(&param));
        assert_eq!(upstream.len(), 2);

        // shell depends on extrude only
        let shell_deps = g.dependencies(shell);
        assert_eq!(shell_deps, vec![extrude]);
    }

    #[test]
    fn test_propagation_order() {
        let (g, nodes) = build_parametric_model();
        let order = g.propagation_order().expect("no cycles");

        // Every node should appear after its dependencies
        for (i, &node) in order.iter().enumerate() {
            for dep in g.dependencies(node) {
                let dep_pos = order.iter().position(|&n| n == dep).unwrap();
                assert!(
                    dep_pos < i,
                    "dependency {:?} should come before {:?}",
                    dep,
                    node
                );
            }
        }

        // All 5 nodes present
        assert_eq!(order.len(), 5);
        // param must be first (no dependencies)
        assert_eq!(order[0], nodes[0]);
    }

    #[test]
    fn test_affected_by() {
        let (g, nodes) = build_parametric_model();
        let [param, sketch, extrude, shell, fillet] =
            [nodes[0], nodes[1], nodes[2], nodes[3], nodes[4]];

        // Changing the parameter affects everything downstream
        let affected = g.affected_by(param);
        assert_eq!(affected.len(), 4);
        assert!(affected.contains(&sketch));
        assert!(affected.contains(&extrude));
        assert!(affected.contains(&shell));
        assert!(affected.contains(&fillet));

        // Changing the extrude affects shell and fillet
        let affected2 = g.affected_by(extrude);
        assert_eq!(affected2.len(), 2);
        assert!(affected2.contains(&shell));
        assert!(affected2.contains(&fillet));

        // Changing the fillet affects nothing
        let affected3 = g.affected_by(fillet);
        assert!(affected3.is_empty());
    }

    #[test]
    fn test_find_cycles_none() {
        let (g, _) = build_parametric_model();
        let cycles = g.find_cycles();
        assert!(cycles.is_empty(), "DAG should have no cycles");
    }

    #[test]
    fn test_find_cycles_present() {
        let mut g = DesignGraph::new();

        let a = g.add_node(
            NodeKind::Feature,
            "A",
            NodeData::Feature {
                op: FeatureOp::Extrude { depth_m: 0.01 },
            },
        );
        let b = g.add_node(
            NodeKind::Feature,
            "B",
            NodeData::Feature {
                op: FeatureOp::Extrude { depth_m: 0.01 },
            },
        );
        let c = g.add_node(
            NodeKind::Feature,
            "C",
            NodeData::Feature {
                op: FeatureOp::Extrude { depth_m: 0.01 },
            },
        );

        // A → B → C → A (cycle)
        g.add_edge(a, b, EdgeKind::DependsOn);
        g.add_edge(b, c, EdgeKind::DependsOn);
        g.add_edge(c, a, EdgeKind::DependsOn);

        let cycles = g.find_cycles();
        assert_eq!(cycles.len(), 1);
        assert_eq!(cycles[0].len(), 3);

        // Topo sort should fail
        assert!(g.propagation_order().is_none());
    }

    #[test]
    fn test_subgraph_extraction() {
        let (g, nodes) = build_parametric_model();
        let [_param, sketch, extrude, shell, _fillet] =
            [nodes[0], nodes[1], nodes[2], nodes[3], nodes[4]];

        // Extract sketch → extrude → shell
        let sub = g.subgraph(&[sketch, extrude, shell]);
        assert_eq!(sub.node_count(), 3);

        // Only edges between included nodes survive.
        // sketch→extrude (DependsOn), extrude→shell (DependsOn) = 2 edges
        // param→extrude (DrivenBy) dropped because param not included
        assert_eq!(sub.edge_count(), 2);
    }

    #[test]
    fn test_by_kind() {
        let (g, _) = build_parametric_model();
        let features = g.by_kind(NodeKind::Feature);
        assert_eq!(features.len(), 3); // extrude, shell, fillet

        let params = g.by_kind(NodeKind::Parameter);
        assert_eq!(params.len(), 1);

        let sketches = g.by_kind(NodeKind::Sketch);
        assert_eq!(sketches.len(), 1);
    }

    #[test]
    fn test_edges_of_kind() {
        let (g, nodes) = build_parametric_model();
        let [param, sketch, extrude, shell, fillet] =
            [nodes[0], nodes[1], nodes[2], nodes[3], nodes[4]];

        let depends_on = g.edges_of_kind(EdgeKind::DependsOn);
        assert_eq!(depends_on.len(), 4);
        assert!(depends_on.contains(&(param, sketch)));
        assert!(depends_on.contains(&(sketch, extrude)));
        assert!(depends_on.contains(&(extrude, shell)));
        assert!(depends_on.contains(&(shell, fillet)));

        let driven_by = g.edges_of_kind(EdgeKind::DrivenBy);
        assert_eq!(driven_by.len(), 1);
        assert_eq!(driven_by[0], (param, extrude));

        let material = g.edges_of_kind(EdgeKind::MaterialOf);
        assert!(material.is_empty());
    }

    #[test]
    fn test_remove_node() {
        let (mut g, nodes) = build_parametric_model();
        // Remove a leaf node (fillet) to avoid petgraph's swap-remove
        // invalidating other ids we're checking.
        let fillet = nodes[4];
        assert_eq!(g.node_count(), 5);

        g.remove_node(fillet);
        assert_eq!(g.node_count(), 4);
        // The fillet node slot is gone
        assert!(g.node(fillet).is_none());

        // Remaining nodes still accessible
        assert!(g.node(nodes[0]).is_some()); // param
        assert!(g.node(nodes[1]).is_some()); // sketch
        assert!(g.node(nodes[2]).is_some()); // extrude
        assert!(g.node(nodes[3]).is_some()); // shell
    }

    #[test]
    fn test_assembly_graph() {
        let mut g = DesignGraph::new();

        let asm = g.add_node(
            NodeKind::Assembly,
            "top_assembly",
            NodeData::Assembly {
                name: "Robot Arm".into(),
            },
        );

        let base = g.add_node(
            NodeKind::Feature,
            "base_plate",
            NodeData::Feature {
                op: FeatureOp::Extrude { depth_m: 0.01 },
            },
        );

        let bracket = g.add_node(
            NodeKind::Feature,
            "bracket",
            NodeData::Feature {
                op: FeatureOp::Extrude { depth_m: 0.005 },
            },
        );

        let steel = g.add_node(
            NodeKind::Material,
            "A36 steel",
            NodeData::Material {
                material_id: "A36".into(),
            },
        );

        g.add_edge(base, asm, EdgeKind::AssemblyOf);
        g.add_edge(bracket, asm, EdgeKind::AssemblyOf);
        g.add_edge(base, bracket, EdgeKind::MateWith);
        g.add_edge(steel, base, EdgeKind::MaterialOf);
        g.add_edge(steel, bracket, EdgeKind::MaterialOf);

        assert_eq!(g.node_count(), 4);
        assert_eq!(g.edge_count(), 5);

        let assembly_edges = g.edges_of_kind(EdgeKind::AssemblyOf);
        assert_eq!(assembly_edges.len(), 2);

        let material_edges = g.edges_of_kind(EdgeKind::MaterialOf);
        assert_eq!(material_edges.len(), 2);
    }

    #[test]
    fn test_edge_kind_channels() {
        assert_eq!(EdgeKind::DependsOn.channel(), 0);
        assert_eq!(EdgeKind::DrivenBy.channel(), 1);
        assert_eq!(EdgeKind::CustomLink.channel(), 15);
    }

    #[test]
    fn test_default() {
        let g = DesignGraph::default();
        assert_eq!(g.node_count(), 0);
        assert_eq!(g.edge_count(), 0);
    }
}
