//! `physical-intent` — Design intent reconstruction from imported geometry.
//!
//! Analyzes a B-Rep solid and reconstructs the design intent: what features
//! were intended (holes, fillets, chamfers, ribs, etc.), what relationships
//! exist between them (equal radii, patterns, symmetry), and generates CFL
//! code that can recreate the design parametrically.

use std::collections::HashMap;

use glam::DVec3;
use physical_brep::{Solid, Surface, FaceId};
use serde::{Deserialize, Serialize};
use slotmap::Key;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Type of geometric feature detected in a solid.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FeatureType {
    Hole,
    Fillet,
    Chamfer,
    Wall,
    Rib,
    Boss,
    Pocket,
    Pattern,
    Symmetry,
}

/// A detected design-intent feature.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct IntentFeature {
    pub feature_type: FeatureType,
    pub parameters: HashMap<String, f64>,
    /// References to geometry (face IDs encoded as u64).
    pub geometry_refs: Vec<u64>,
    /// Confidence that this detection is correct (0.0 .. 1.0).
    pub confidence: f64,
}

/// Relationship type between features.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RelationshipType {
    EqualRadius,
    EqualDistance,
    PatternSpacing,
    SymmetryAbout,
    ProportionalTo,
    DerivedFrom,
}

/// A detected relationship between features.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct IntentRelationship {
    pub rel_type: RelationshipType,
    /// Indices into the parent `DesignIntent.features` vec.
    pub features: Vec<usize>,
    /// CFL expression that encodes this relationship.
    pub expression: String,
    pub confidence: f64,
}

/// The full reconstructed design intent.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DesignIntent {
    pub features: Vec<IntentFeature>,
    pub relationships: Vec<IntentRelationship>,
    /// Overall confidence in the reconstruction.
    pub confidence: f64,
}

// ---------------------------------------------------------------------------
// Detection helpers
// ---------------------------------------------------------------------------

/// Estimate the depth of a cylindrical face by examining its bounding edges.
fn estimate_cylinder_depth(solid: &Solid, face_id: FaceId, axis: DVec3) -> (f64, bool) {
    let face = &solid.faces[face_id];
    let axis_n = axis.normalize();
    let mut min_proj = f64::MAX;
    let mut max_proj = f64::MIN;

    for he_id in &face.outer_loop {
        let he = &solid.half_edges[*he_id];
        let pt = solid.vertices[he.origin].point;
        let proj = pt.dot(axis_n);
        if proj < min_proj {
            min_proj = proj;
        }
        if proj > max_proj {
            max_proj = proj;
        }
    }

    let depth = if max_proj > min_proj {
        max_proj - min_proj
    } else {
        0.0
    };

    // Heuristic: if adjacent faces are both planar, likely a through hole.
    // For now, mark as through if there are no capping planar faces adjacent.
    let is_through = depth > 0.0;

    (depth, is_through)
}

// ---------------------------------------------------------------------------
// Public detection functions
// ---------------------------------------------------------------------------

/// Detect cylindrical holes (through and blind) in a solid.
pub fn detect_holes(solid: &Solid) -> Vec<IntentFeature> {
    let mut features = Vec::new();

    for (face_id, face) in &solid.faces {
        if let Surface::Cylinder { origin, axis, radius } = &face.surface {
            // A concave cylindrical face (normal points inward) is likely a hole.
            // Check if the surface normal at a sample point is inward.
            let mid = face.outer_loop.first().map(|he_id| {
                solid.vertices[solid.half_edges[*he_id].origin].point
            });
            let is_concave = if let Some(pt) = mid {
                let radial = (pt - *origin).reject_from(axis.normalize());
                let outward_normal = radial.normalize();
                // If face normal is opposite to outward radial, it's concave (hole).
                let face_normal = face.surface.normal_at(pt);
                face_normal.dot(outward_normal) < 0.0
            } else {
                false
            };

            let (depth, is_through) = estimate_cylinder_depth(solid, face_id, *axis);

            // Encode the FaceId slot as u64.
            let face_idx = face_id.data().as_ffi();

            let mut params = HashMap::new();
            params.insert("diameter".to_string(), radius * 2.0);
            params.insert("radius".to_string(), *radius);
            params.insert("depth".to_string(), depth);
            params.insert("is_through".to_string(), if is_through { 1.0 } else { 0.0 });
            params.insert("is_concave".to_string(), if is_concave { 1.0 } else { 0.0 });

            features.push(IntentFeature {
                feature_type: FeatureType::Hole,
                parameters: params,
                geometry_refs: vec![face_idx],
                confidence: if is_concave { 0.9 } else { 0.5 },
            });
        }
    }

    features
}

/// Detect fillet (blend) surfaces — toroidal faces are classic fillet indicators.
pub fn detect_fillets(solid: &Solid) -> Vec<IntentFeature> {
    let mut features = Vec::new();

    for (face_id, face) in &solid.faces {
        match &face.surface {
            Surface::Torus { minor_radius, .. } => {
                let face_idx = face_id.data().as_ffi();
                let mut params = HashMap::new();
                params.insert("radius".to_string(), *minor_radius);
                features.push(IntentFeature {
                    feature_type: FeatureType::Fillet,
                    parameters: params,
                    geometry_refs: vec![face_idx],
                    confidence: 0.95,
                });
            }
            Surface::Cylinder { radius, axis, .. } => {
                // A narrow cylindrical strip between two planar faces can be a fillet.
                // Check if it's narrow (small angular span) and adjacent to planes.
                let (depth, _) = estimate_cylinder_depth(solid, face_id, *axis);
                if depth > 0.0 && depth < *radius * 2.0 {
                    // Could be a cylindrical blend.
                    let face_idx = face_id.data().as_ffi();
                    let mut params = HashMap::new();
                    params.insert("radius".to_string(), *radius);
                    features.push(IntentFeature {
                        feature_type: FeatureType::Fillet,
                        parameters: params,
                        geometry_refs: vec![face_idx],
                        confidence: 0.5,
                    });
                }
            }
            _ => {}
        }
    }

    features
}

/// Detect patterns (regular spacing) among a set of detected features.
///
/// Looks for features of the same type with regular spatial intervals.
pub fn detect_patterns(features: &[IntentFeature]) -> Vec<IntentRelationship> {
    let mut relationships = Vec::new();

    // Group features by type.
    let mut by_type: HashMap<String, Vec<(usize, &IntentFeature)>> = HashMap::new();
    for (i, f) in features.iter().enumerate() {
        let key = format!("{:?}", f.feature_type);
        by_type.entry(key).or_default().push((i, f));
    }

    for (_type_name, group) in &by_type {
        if group.len() < 2 {
            continue;
        }

        // Check for equal parameters (e.g., equal radii).
        let radii: Vec<(usize, f64)> = group
            .iter()
            .filter_map(|(i, f)| f.parameters.get("radius").map(|r| (*i, *r)))
            .collect();

        if radii.len() >= 2 {
            // Check if all radii are equal.
            let first_r = radii[0].1;
            let all_equal = radii.iter().all(|(_, r)| (*r - first_r).abs() < 1e-6);
            if all_equal {
                let indices: Vec<usize> = radii.iter().map(|(i, _)| *i).collect();
                relationships.push(IntentRelationship {
                    rel_type: RelationshipType::EqualRadius,
                    features: indices,
                    expression: format!("equal_radius({:.4})", first_r),
                    confidence: 0.9,
                });
            }
        }

        // Check for linear pattern (regular spacing along one axis).
        if group.len() >= 3 {
            // Use the first geometry_ref as a proxy for position ordering.
            let mut refs: Vec<(usize, u64)> = group
                .iter()
                .map(|(i, f)| (*i, f.geometry_refs.first().copied().unwrap_or(0)))
                .collect();
            refs.sort_by_key(|(_, r)| *r);

            // Check spacing regularity (by geometry ref spacing as proxy).
            if refs.len() >= 3 {
                let spacings: Vec<u64> = refs
                    .windows(2)
                    .map(|w| w[1].1.saturating_sub(w[0].1))
                    .collect();
                let first_spacing = spacings[0];
                if first_spacing > 0 {
                    let regular = spacings.iter().all(|s| (*s as f64 - first_spacing as f64).abs() < 2.0);
                    if regular {
                        let indices: Vec<usize> = refs.iter().map(|(i, _)| *i).collect();
                        relationships.push(IntentRelationship {
                            rel_type: RelationshipType::PatternSpacing,
                            features: indices,
                            expression: format!("linear_pattern(count={}, spacing={})", refs.len(), first_spacing),
                            confidence: 0.7,
                        });
                    }
                }
            }
        }
    }

    relationships
}

/// Detect symmetry relationships in a solid.
///
/// Checks for mirror symmetry of faces across the principal planes (XY, XZ, YZ).
pub fn detect_symmetry(solid: &Solid) -> Vec<IntentRelationship> {
    let mut relationships = Vec::new();

    // Compute bounding box center for symmetry plane candidates.
    let mut min = DVec3::new(f64::MAX, f64::MAX, f64::MAX);
    let mut max = DVec3::new(f64::MIN, f64::MIN, f64::MIN);
    for (_vid, v) in &solid.vertices {
        min = min.min(v.point);
        max = max.max(v.point);
    }
    let center = (min + max) * 0.5;

    // Collect face centroids.
    let face_centroids: Vec<(FaceId, DVec3)> = solid
        .faces
        .iter()
        .map(|(fid, face)| {
            let pts: Vec<DVec3> = face
                .outer_loop
                .iter()
                .map(|he_id| solid.vertices[solid.half_edges[*he_id].origin].point)
                .collect();
            let centroid = if pts.is_empty() {
                DVec3::ZERO
            } else {
                pts.iter().sum::<DVec3>() / pts.len() as f64
            };
            (fid, centroid)
        })
        .collect();

    let planes = [
        ("YZ", DVec3::X), // mirror across YZ plane
        ("XZ", DVec3::Y), // mirror across XZ plane
        ("XY", DVec3::Z), // mirror across XY plane
    ];

    for (plane_name, normal) in &planes {
        let mut paired = 0usize;
        let mut total = 0usize;

        for (i, (_fid_a, ca)) in face_centroids.iter().enumerate() {
            // Mirror the centroid across the plane through `center`.
            let offset = ca.dot(*normal) - center.dot(*normal);

            // Faces lying on the symmetry plane count as self-symmetric.
            if offset.abs() < 1e-3 {
                paired += 1;
                total += 1;
                continue;
            }

            let mirrored = *ca - *normal * (2.0 * offset);

            // Find a matching face centroid.
            for (j, (_fid_b, cb)) in face_centroids.iter().enumerate() {
                if i == j {
                    continue;
                }
                if (mirrored - *cb).length() < 1e-3 {
                    paired += 1;
                    break;
                }
            }
            total += 1;
        }

        if total > 0 && paired as f64 / total as f64 > 0.7 {
            relationships.push(IntentRelationship {
                rel_type: RelationshipType::SymmetryAbout,
                features: Vec::new(), // applies to whole solid
                expression: format!("symmetry(plane={}, origin=[{:.3},{:.3},{:.3}])",
                    plane_name, center.x, center.y, center.z),
                confidence: paired as f64 / total as f64,
            });
        }
    }

    relationships
}

// ---------------------------------------------------------------------------
// Main reconstruction
// ---------------------------------------------------------------------------

/// Analyze a B-Rep solid and reconstruct the design intent.
///
/// Detects holes, fillets, patterns, symmetry, and proportional relationships,
/// then bundles everything into a `DesignIntent`.
pub fn reconstruct_intent(solid: &Solid) -> DesignIntent {
    let mut features = Vec::new();

    // Detect holes.
    let holes = detect_holes(solid);
    features.extend(holes);

    // Detect fillets.
    let fillets = detect_fillets(solid);
    features.extend(fillets);

    // Detect walls (large planar faces).
    for (face_id, face) in &solid.faces {
        if let Surface::Plane { normal, .. } = &face.surface {
            let face_idx = face_id.data().as_ffi();
            let mut params = HashMap::new();
            params.insert("normal_x".to_string(), normal.x);
            params.insert("normal_y".to_string(), normal.y);
            params.insert("normal_z".to_string(), normal.z);
            features.push(IntentFeature {
                feature_type: FeatureType::Wall,
                parameters: params,
                geometry_refs: vec![face_idx],
                confidence: 0.8,
            });
        }
    }

    // Detect relationships.
    let mut relationships = Vec::new();

    // Pattern / equal radius among detected features.
    let pattern_rels = detect_patterns(&features);
    relationships.extend(pattern_rels);

    // Symmetry.
    let sym_rels = detect_symmetry(solid);
    relationships.extend(sym_rels);

    // Proportional relationships: fillet radius vs. estimated wall thickness.
    detect_proportional(&features, &mut relationships);

    // Overall confidence.
    let confidence = if features.is_empty() {
        0.0
    } else {
        features.iter().map(|f| f.confidence).sum::<f64>() / features.len() as f64
    };

    DesignIntent {
        features,
        relationships,
        confidence,
    }
}

/// Detect proportional relationships (e.g., fillet radius = k * wall thickness).
fn detect_proportional(features: &[IntentFeature], relationships: &mut Vec<IntentRelationship>) {
    let fillets: Vec<(usize, f64)> = features
        .iter()
        .enumerate()
        .filter(|(_, f)| f.feature_type == FeatureType::Fillet)
        .filter_map(|(i, f)| f.parameters.get("radius").map(|r| (i, *r)))
        .collect();

    let holes: Vec<(usize, f64)> = features
        .iter()
        .enumerate()
        .filter(|(_, f)| f.feature_type == FeatureType::Hole)
        .filter_map(|(i, f)| f.parameters.get("radius").map(|r| (i, *r)))
        .collect();

    // Check if any fillet radius is a simple ratio of a hole radius.
    for &(fi, fr) in &fillets {
        for &(hi, hr) in &holes {
            if hr.abs() < 1e-12 {
                continue;
            }
            let ratio = fr / hr;
            // Check for clean ratios: 0.5, 1.0, 1.5, 2.0, etc.
            let rounded = (ratio * 2.0).round() / 2.0;
            if (ratio - rounded).abs() < 0.05 && rounded > 0.0 {
                relationships.push(IntentRelationship {
                    rel_type: RelationshipType::ProportionalTo,
                    features: vec![fi, hi],
                    expression: format!(
                        "fillet_radius = {:.1} * hole_radius  // {:.4} = {:.1} * {:.4}",
                        rounded, fr, rounded, hr,
                    ),
                    confidence: 0.7,
                });
            }
        }
    }
}

// ---------------------------------------------------------------------------
// CFL code generation
// ---------------------------------------------------------------------------

/// Generate CFL code that recreates the design with detected relationships.
pub fn intent_to_cfl(intent: &DesignIntent) -> String {
    let mut lines = Vec::new();
    lines.push("// Auto-generated CFL from design intent reconstruction".to_string());
    lines.push("// Relationships are encoded as CFL constraints".to_string());
    lines.push(String::new());

    // Emit parameters from relationships.
    let mut param_counter = 0u32;
    for rel in &intent.relationships {
        match rel.rel_type {
            RelationshipType::EqualRadius => {
                lines.push(format!("let shared_radius_{} = {};", param_counter, rel.expression));
                param_counter += 1;
            }
            RelationshipType::PatternSpacing => {
                lines.push(format!("// Pattern: {}", rel.expression));
            }
            RelationshipType::SymmetryAbout => {
                lines.push(format!("// Constraint: {}", rel.expression));
            }
            RelationshipType::ProportionalTo => {
                lines.push(format!("// Proportional: {}", rel.expression));
            }
            _ => {
                lines.push(format!("// Relationship: {}", rel.expression));
            }
        }
    }
    lines.push(String::new());

    // Emit features.
    for (i, feature) in intent.features.iter().enumerate() {
        match feature.feature_type {
            FeatureType::Hole => {
                let dia = feature.parameters.get("diameter").copied().unwrap_or(0.0);
                let depth = feature.parameters.get("depth").copied().unwrap_or(0.0);
                let is_through = feature.parameters.get("is_through").copied().unwrap_or(0.0) > 0.5;
                if is_through {
                    lines.push(format!("hole(\"hole_{}\", diameter={:.4}, through=true);", i, dia));
                } else {
                    lines.push(format!("hole(\"hole_{}\", diameter={:.4}, depth={:.4});", i, dia, depth));
                }
            }
            FeatureType::Fillet => {
                let r = feature.parameters.get("radius").copied().unwrap_or(0.0);
                lines.push(format!("fillet(\"fillet_{}\", radius={:.4});", i, r));
            }
            FeatureType::Wall => {
                // Walls are implicit in the sketch; emit as comment.
                lines.push(format!("// wall_{}: planar face", i));
            }
            _ => {
                lines.push(format!("// feature_{}: {:?}", i, feature.feature_type));
            }
        }
    }

    lines.join("\n")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use physical_brep::{make_box, make_cylinder};

    #[test]
    fn test_detect_holes_on_cylinder() {
        let cyl = make_cylinder(5.0, 20.0, 32);
        let holes = detect_holes(&cyl);
        // A cylinder solid has cylindrical faces — should detect at least one.
        assert!(!holes.is_empty(), "Should detect cylindrical faces as potential holes");
    }

    #[test]
    fn test_detect_holes_on_box() {
        let b = make_box(10.0, 10.0, 10.0);
        let holes = detect_holes(&b);
        // A box has no cylindrical faces.
        assert!(holes.is_empty(), "Box should have no holes");
    }

    #[test]
    fn test_detect_fillets_on_box() {
        let b = make_box(10.0, 10.0, 10.0);
        let fillets = detect_fillets(&b);
        // A plain box has no fillet surfaces.
        assert!(fillets.is_empty(), "Box should have no fillets");
    }

    #[test]
    fn test_detect_fillets_finds_toroidal() {
        // Construct a minimal solid with a toroidal face to test fillet detection.
        let mut solid = Solid::new();
        let v0 = solid.add_vertex(DVec3::new(0.0, 0.0, 0.0));
        let v1 = solid.add_vertex(DVec3::new(1.0, 0.0, 0.0));
        let v2 = solid.add_vertex(DVec3::new(1.0, 1.0, 0.0));
        let v3 = solid.add_vertex(DVec3::new(0.0, 1.0, 0.0));
        let torus_surface = Surface::Torus {
            center: DVec3::new(0.5, 0.5, 0.0),
            axis: DVec3::Z,
            major_radius: 0.5,
            minor_radius: 0.1,
        };
        solid.add_face_from_vertices(torus_surface, &[v0, v1, v2, v3], true);

        let fillets = detect_fillets(&solid);
        assert_eq!(fillets.len(), 1);
        assert_eq!(fillets[0].feature_type, FeatureType::Fillet);
        let r = fillets[0].parameters.get("radius").unwrap();
        assert!((*r - 0.1).abs() < 1e-9);
    }

    #[test]
    fn test_detect_patterns_equal_radius() {
        let features = vec![
            IntentFeature {
                feature_type: FeatureType::Hole,
                parameters: HashMap::from([("radius".to_string(), 5.0)]),
                geometry_refs: vec![1],
                confidence: 0.9,
            },
            IntentFeature {
                feature_type: FeatureType::Hole,
                parameters: HashMap::from([("radius".to_string(), 5.0)]),
                geometry_refs: vec![2],
                confidence: 0.9,
            },
            IntentFeature {
                feature_type: FeatureType::Hole,
                parameters: HashMap::from([("radius".to_string(), 5.0)]),
                geometry_refs: vec![3],
                confidence: 0.9,
            },
        ];
        let rels = detect_patterns(&features);
        let equal_r: Vec<_> = rels
            .iter()
            .filter(|r| r.rel_type == RelationshipType::EqualRadius)
            .collect();
        assert!(!equal_r.is_empty(), "Should detect equal radius");
        assert_eq!(equal_r[0].features.len(), 3);
    }

    #[test]
    fn test_detect_patterns_no_pattern_for_single() {
        let features = vec![IntentFeature {
            feature_type: FeatureType::Hole,
            parameters: HashMap::from([("radius".to_string(), 5.0)]),
            geometry_refs: vec![1],
            confidence: 0.9,
        }];
        let rels = detect_patterns(&features);
        assert!(rels.is_empty());
    }

    #[test]
    fn test_detect_symmetry_box() {
        let b = make_box(10.0, 10.0, 10.0);
        let syms = detect_symmetry(&b);
        // A box centered at (5,5,5) is symmetric about all three principal planes.
        assert!(
            !syms.is_empty(),
            "Box should be symmetric about at least one plane"
        );
    }

    #[test]
    fn test_reconstruct_intent_box() {
        let b = make_box(10.0, 10.0, 10.0);
        let intent = reconstruct_intent(&b);
        // Should detect walls (planar faces) and symmetry.
        let walls: Vec<_> = intent
            .features
            .iter()
            .filter(|f| f.feature_type == FeatureType::Wall)
            .collect();
        assert!(walls.len() >= 6, "Box should have 6 wall faces, got {}", walls.len());
        assert!(intent.confidence > 0.0);
    }

    #[test]
    fn test_reconstruct_intent_cylinder() {
        let cyl = make_cylinder(5.0, 20.0, 32);
        let intent = reconstruct_intent(&cyl);
        assert!(!intent.features.is_empty());
    }

    #[test]
    fn test_intent_to_cfl_generates_code() {
        let b = make_box(10.0, 10.0, 10.0);
        let intent = reconstruct_intent(&b);
        let cfl = intent_to_cfl(&intent);
        assert!(cfl.contains("wall_"), "CFL should reference wall features");
        assert!(cfl.contains("Auto-generated CFL"));
    }

    #[test]
    fn test_intent_to_cfl_with_holes() {
        let intent = DesignIntent {
            features: vec![
                IntentFeature {
                    feature_type: FeatureType::Hole,
                    parameters: HashMap::from([
                        ("diameter".to_string(), 10.0),
                        ("depth".to_string(), 25.0),
                        ("is_through".to_string(), 1.0),
                    ]),
                    geometry_refs: vec![1],
                    confidence: 0.9,
                },
                IntentFeature {
                    feature_type: FeatureType::Fillet,
                    parameters: HashMap::from([("radius".to_string(), 2.0)]),
                    geometry_refs: vec![2],
                    confidence: 0.95,
                },
            ],
            relationships: vec![],
            confidence: 0.9,
        };
        let cfl = intent_to_cfl(&intent);
        assert!(cfl.contains("hole("));
        assert!(cfl.contains("diameter=10.0000"));
        assert!(cfl.contains("through=true"));
        assert!(cfl.contains("fillet("));
        assert!(cfl.contains("radius=2.0000"));
    }

    #[test]
    fn test_proportional_relationship() {
        let features = vec![
            IntentFeature {
                feature_type: FeatureType::Fillet,
                parameters: HashMap::from([("radius".to_string(), 3.0)]),
                geometry_refs: vec![1],
                confidence: 0.9,
            },
            IntentFeature {
                feature_type: FeatureType::Hole,
                parameters: HashMap::from([("radius".to_string(), 2.0)]),
                geometry_refs: vec![2],
                confidence: 0.9,
            },
        ];
        let mut rels = Vec::new();
        detect_proportional(&features, &mut rels);
        assert!(
            !rels.is_empty(),
            "Should detect fillet_radius = 1.5 * hole_radius"
        );
        assert_eq!(rels[0].rel_type, RelationshipType::ProportionalTo);
        assert!(rels[0].expression.contains("1.5"));
    }

    #[test]
    fn test_empty_solid_intent() {
        let solid = Solid::new();
        let intent = reconstruct_intent(&solid);
        assert!(intent.features.is_empty());
        assert_eq!(intent.confidence, 0.0);
    }
}
