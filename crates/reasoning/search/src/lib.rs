//! CAD feature indexing — extract geometric feature vectors from B-Rep solids
//! and index them in HyperDB for similar part search and design reuse.
//!
//! # Feature Vector Layout (16 × f32)
//!
//! | Dim   | Meaning                                           |
//! |-------|---------------------------------------------------|
//! | 0–2   | Bounding box dimensions (normalized, sorted desc) |
//! | 3     | Volume (log-scaled)                               |
//! | 4     | Surface area (log-scaled)                         |
//! | 5     | Compactness ratio (volume / bbox_volume)           |
//! | 6     | Face count (log-scaled)                           |
//! | 7     | Edge count (log-scaled)                           |
//! | 8     | Vertex count (log-scaled)                         |
//! | 9     | Aspect ratio (max_dim / min_dim)                  |
//! | 10–11 | Material family encoding (density, stiffness)     |
//! | 12    | Manufacturing complexity (faces × edges, norm.)   |
//! | 13    | Symmetry score (0–1)                              |
//! | 14–15 | Reserved (0.0)                                    |

use std::collections::HashMap;

use hyperdb::{Distance, HyperDB, Props, Value};
use physical_brep::Solid;

/// Number of dimensions in our feature vector.
pub const FEATURE_DIM: usize = 16;

/// A fixed-dimension feature vector encoding a CAD part's geometric signature.
#[derive(Debug, Clone)]
pub struct FeatureVector {
    pub data: [f32; FEATURE_DIM],
}

impl FeatureVector {
    /// View as a slice (for HyperDB insertion).
    pub fn as_slice(&self) -> &[f32] {
        &self.data
    }

    /// Convert to a Vec<f32> (for HyperDB API).
    pub fn to_vec(&self) -> Vec<f32> {
        self.data.to_vec()
    }
}

/// Result of a similarity search.
#[derive(Debug, Clone)]
pub struct SearchResult {
    pub part_name: String,
    pub similarity_score: f32,
    pub feature_vector: FeatureVector,
    pub metadata: HashMap<String, String>,
}

// ---------------------------------------------------------------------------
// Feature extraction
// ---------------------------------------------------------------------------

/// Extract a 16-dimensional feature vector from a CAD solid.
///
/// The vector encodes geometric topology (face/edge/vertex counts),
/// bounding-box shape, compactness, aspect ratio, material properties,
/// manufacturing complexity, and symmetry.
pub fn extract_features(solid: &Solid, material_id: Option<&str>) -> FeatureVector {
    let (bb_min, bb_max) = solid.bounding_box();
    let dims = bb_max - bb_min;

    // Sort dimensions descending for rotation-invariance.
    let mut sorted = [dims.x as f32, dims.y as f32, dims.z as f32];
    sorted.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));

    let max_dim = sorted[0].max(f32::EPSILON);

    // Normalized bounding box (relative to largest dimension).
    let bb_norm = [
        sorted[0] / max_dim,
        sorted[1] / max_dim,
        sorted[2] / max_dim,
    ];

    // Volume estimate: product of bbox dims (true B-Rep volume would need
    // divergence theorem integration; bbox product is a fast proxy).
    let bbox_volume = (sorted[0] * sorted[1] * sorted[2]).max(f32::EPSILON);
    let volume = bbox_volume; // proxy — exact volume requires mesh integration
    let log_volume = (volume.max(f32::EPSILON)).ln();

    // Surface area estimate: 2(wh + wd + hd).
    let surface_area =
        2.0 * (sorted[0] * sorted[1] + sorted[0] * sorted[2] + sorted[1] * sorted[2]);
    let log_surface_area = surface_area.max(f32::EPSILON).ln();

    // Compactness: actual solid's face-count-derived proxy vs bbox.
    // For a box this is 1.0; for complex parts < 1.0.
    let face_count = solid.face_count() as f32;
    let edge_count = solid.edge_count() as f32;
    let vertex_count = solid.vertex_count() as f32;

    // A perfect box has 6 faces. More faces → more material carved away → lower compactness.
    let compactness = (6.0 / face_count.max(1.0)).min(1.0);

    let log_faces = (face_count.max(1.0)).ln();
    let log_edges = (edge_count.max(1.0)).ln();
    let log_vertices = (vertex_count.max(1.0)).ln();

    // Aspect ratio.
    let min_dim = sorted[2].max(f32::EPSILON);
    let aspect_ratio = (max_dim / min_dim).min(100.0); // cap at 100

    // Material encoding.
    let (density_bucket, stiffness_bucket) = material_buckets(material_id);

    // Manufacturing complexity: face × edge count, log-normalized.
    let complexity_raw = face_count * edge_count;
    let manufacturing_complexity = (complexity_raw.max(1.0)).ln() / 10.0; // normalize to ~0-1 range

    // Symmetry score: how cube-like the bbox is.
    // 1.0 for a perfect cube, lower for elongated shapes.
    let symmetry = (bb_norm[1] * bb_norm[2]).sqrt(); // geometric mean of the two smaller ratios

    FeatureVector {
        data: [
            bb_norm[0],              // 0
            bb_norm[1],              // 1
            bb_norm[2],              // 2
            log_volume,              // 3
            log_surface_area,        // 4
            compactness,             // 5
            log_faces,               // 6
            log_edges,               // 7
            log_vertices,            // 8
            aspect_ratio,            // 9
            density_bucket,          // 10
            stiffness_bucket,        // 11
            manufacturing_complexity, // 12
            symmetry,                // 13
            0.0,                     // 14 reserved
            0.0,                     // 15 reserved
        ],
    }
}

/// Map a material ID to (density_bucket, stiffness_bucket) in [0, 1].
fn material_buckets(material_id: Option<&str>) -> (f32, f32) {
    let Some(id) = material_id else {
        return (0.0, 0.0);
    };

    if let Some(mat) = physical_lut::materials::lookup(id) {
        // Density: 0–10000 kg/m³ mapped to 0–1.
        let density = (mat.density.value() as f32 / 10_000.0).clamp(0.0, 1.0);
        // Stiffness (elastic modulus): 0–500 GPa mapped to 0–1.
        let stiffness = (mat.elastic_modulus.value() as f32 / 500e9).clamp(0.0, 1.0);
        (density, stiffness)
    } else {
        (0.0, 0.0)
    }
}

// ---------------------------------------------------------------------------
// Part index
// ---------------------------------------------------------------------------

/// A searchable index of CAD parts backed by HyperDB.
pub struct PartIndex {
    db: HyperDB,
}

impl PartIndex {
    /// Create a new in-memory part index.
    pub fn new() -> Self {
        Self {
            db: HyperDB::memory(FEATURE_DIM, Distance::Euclidean),
        }
    }

    /// Index a part: extract features and store in HyperDB.
    pub fn index_part(
        &self,
        solid: &Solid,
        name: &str,
        material_id: Option<&str>,
        metadata: HashMap<String, String>,
    ) -> FeatureVector {
        let features = extract_features(solid, material_id);

        let mut props = Props::new();
        props.insert("name", Value::String(name.to_string()));
        if let Some(mat) = material_id {
            props.insert("material", Value::String(mat.to_string()));
        }
        for (k, v) in &metadata {
            props.insert(k.clone(), Value::String(v.clone()));
        }

        self.db
            .insert_node_with_embedding("part", props, features.to_vec());

        features
    }

    /// Search for parts similar to the given solid.
    pub fn search_similar(
        &self,
        solid: &Solid,
        material_id: Option<&str>,
        top_k: usize,
    ) -> Vec<SearchResult> {
        let features = extract_features(solid, material_id);
        self.search_by_features(&features, top_k)
    }

    /// Search by a pre-computed feature vector.
    pub fn search_by_features(&self, features: &FeatureVector, top_k: usize) -> Vec<SearchResult> {
        let hits = self.db.vector_search(features.as_slice(), top_k);

        hits.into_iter()
            .filter_map(|(node_id, distance)| {
                let node = self.db.get_node(node_id)?;
                let part_name = node
                    .props
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown")
                    .to_string();

                // Reconstruct metadata (all string props except "name" and "material").
                let mut metadata = HashMap::new();
                for (k, v) in node.props.iter() {
                    if k != "name" && k != "material" {
                        if let Some(s) = v.as_str() {
                            metadata.insert(k.clone(), s.to_string());
                        }
                    }
                }

                // Recover feature vector from HyperDB.
                let fv = self
                    .db
                    .vector
                    .get_vector(node_id)
                    .map(|v| {
                        let mut data = [0.0f32; FEATURE_DIM];
                        for (i, val) in v.iter().enumerate().take(FEATURE_DIM) {
                            data[i] = *val;
                        }
                        FeatureVector { data }
                    })
                    .unwrap_or(FeatureVector {
                        data: [0.0; FEATURE_DIM],
                    });

                // Convert distance to similarity (lower distance = higher similarity).
                // For Euclidean: similarity = 1 / (1 + distance).
                let similarity_score = 1.0 / (1.0 + distance);

                Some(SearchResult {
                    part_name,
                    similarity_score,
                    feature_vector: fv,
                    metadata,
                })
            })
            .collect()
    }

    /// Number of indexed parts.
    pub fn part_count(&self) -> usize {
        self.db.vector_count()
    }
}

impl Default for PartIndex {
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
    use physical_brep::builder::make_box;
    use physical_brep::{Profile, ProfileSegment, extrude_z};
    use glam::DVec2;

    #[test]
    fn feature_extraction_produces_valid_16_dim_vector() {
        let cube = make_box(10.0, 10.0, 10.0);
        let fv = extract_features(&cube, None);
        assert_eq!(fv.data.len(), FEATURE_DIM);

        // All values should be finite.
        for (i, val) in fv.data.iter().enumerate() {
            assert!(val.is_finite(), "dim {i} is not finite: {val}");
        }

        // Normalized bbox dims for a cube should all be ~1.0.
        assert!((fv.data[0] - 1.0).abs() < 0.01, "dim 0: {}", fv.data[0]);
        assert!((fv.data[1] - 1.0).abs() < 0.01, "dim 1: {}", fv.data[1]);
        assert!((fv.data[2] - 1.0).abs() < 0.01, "dim 2: {}", fv.data[2]);

        // Reserved dims should be 0.
        assert_eq!(fv.data[14], 0.0);
        assert_eq!(fv.data[15], 0.0);
    }

    /// Build an L-bracket as a simple extruded L-profile.
    fn make_l_bracket() -> Solid {
        let profile = Profile::new(vec![
            ProfileSegment::line(DVec2::new(0.0, 0.0), DVec2::new(30.0, 0.0)),
            ProfileSegment::line(DVec2::new(30.0, 0.0), DVec2::new(30.0, 5.0)),
            ProfileSegment::line(DVec2::new(30.0, 5.0), DVec2::new(5.0, 5.0)),
            ProfileSegment::line(DVec2::new(5.0, 5.0), DVec2::new(5.0, 20.0)),
            ProfileSegment::line(DVec2::new(5.0, 20.0), DVec2::new(0.0, 20.0)),
            ProfileSegment::line(DVec2::new(0.0, 20.0), DVec2::new(0.0, 0.0)),
        ]);
        extrude_z(&profile, 10.0)
    }

    #[test]
    fn similar_parts_rank_higher() {
        let box_a = make_box(10.0, 10.0, 10.0);
        let box_b = make_box(11.0, 10.0, 10.0); // very similar box
        let l_bracket = make_l_bracket();

        let fv_a = extract_features(&box_a, None);
        let fv_b = extract_features(&box_b, None);
        let fv_l = extract_features(&l_bracket, None);

        // Euclidean distance: box_a vs box_b should be much smaller than box_a vs L-bracket.
        let dist_ab = euclidean(&fv_a.data, &fv_b.data);
        let dist_al = euclidean(&fv_a.data, &fv_l.data);

        assert!(
            dist_ab < dist_al,
            "box-box distance ({dist_ab:.4}) should be less than box-L distance ({dist_al:.4})"
        );
    }

    #[test]
    fn index_and_retrieve_roundtrip() {
        let index = PartIndex::new();
        let cube = make_box(10.0, 10.0, 10.0);

        index.index_part(&cube, "test-cube", None, HashMap::new());
        assert_eq!(index.part_count(), 1);

        let results = index.search_similar(&cube, None, 1);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].part_name, "test-cube");
        assert!(results[0].similarity_score > 0.5, "similarity should be high for identical part");
    }

    #[test]
    fn empty_index_returns_empty_results() {
        let index = PartIndex::new();
        let cube = make_box(5.0, 5.0, 5.0);

        let results = index.search_similar(&cube, None, 10);
        assert!(results.is_empty());
    }

    /// Euclidean distance helper for tests.
    fn euclidean(a: &[f32], b: &[f32]) -> f32 {
        a.iter()
            .zip(b.iter())
            .map(|(x, y)| (x - y).powi(2))
            .sum::<f32>()
            .sqrt()
    }
}
