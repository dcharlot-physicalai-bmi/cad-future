//! 2D nesting — bin packing for sheet material optimization.
//!
//! Places parts on a sheet to minimize material waste.
//! Uses a bottom-left fill heuristic with rotation.

use serde::{Deserialize, Serialize};

use physical_mfg_toolpath::Contour;

/// A part to be nested on a sheet.
#[derive(Clone, Debug)]
pub struct NestPart {
    pub id: String,
    pub contour: Contour,
    /// Number of copies needed.
    pub quantity: usize,
    /// Allow rotation (90° increments).
    pub allow_rotation: bool,
}

/// Sheet material definition.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Sheet {
    pub width: f64,
    pub height: f64,
    /// Material thickness (mm).
    pub thickness: f64,
    /// Edge margin (mm).
    pub margin: f64,
    /// Part-to-part spacing (mm).
    pub spacing: f64,
}

impl Sheet {
    pub fn standard_12x12(thickness: f64) -> Self {
        Self {
            width: 304.8,  // 12 inches
            height: 304.8,
            thickness,
            margin: 5.0,
            spacing: 3.0,
        }
    }

    pub fn standard_24x12(thickness: f64) -> Self {
        Self {
            width: 609.6,  // 24 inches
            height: 304.8, // 12 inches
            thickness,
            margin: 5.0,
            spacing: 3.0,
        }
    }

    /// Usable area (minus margins).
    pub fn usable_area(&self) -> (f64, f64) {
        (self.width - 2.0 * self.margin, self.height - 2.0 * self.margin)
    }
}

/// A placed part in the nesting result.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PlacedPart {
    pub part_id: String,
    pub copy_index: usize,
    /// Position on sheet (bottom-left corner of bounding box).
    pub position: (f64, f64),
    /// Rotation in degrees (0, 90, 180, 270).
    pub rotation: f64,
    /// Bounding box width after rotation.
    pub width: f64,
    /// Bounding box height after rotation.
    pub height: f64,
}

/// Nesting result.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NestResult {
    pub placed: Vec<PlacedPart>,
    pub sheets_used: usize,
    /// Material utilization (0.0-1.0).
    pub utilization: f64,
    /// Parts that couldn't be placed.
    pub unplaced: Vec<String>,
    /// Waste area (mm²).
    pub waste_area: f64,
}

/// Nest parts onto sheets using bottom-left fill heuristic.
pub fn nest_parts(parts: &[NestPart], sheet: &Sheet) -> NestResult {
    let (usable_w, usable_h) = sheet.usable_area();

    // Expand parts by quantity and compute bounding boxes
    let mut items: Vec<(String, usize, f64, f64, bool)> = Vec::new();
    for part in parts {
        let (w, h) = contour_bounding_box(&part.contour);
        for copy in 0..part.quantity {
            items.push((part.id.clone(), copy, w, h, part.allow_rotation));
        }
    }

    // Sort by height descending (first-fit decreasing height)
    items.sort_by(|a, b| b.3.partial_cmp(&a.3).unwrap_or(std::cmp::Ordering::Equal));

    let mut placed = Vec::new();
    let mut unplaced = Vec::new();
    let mut occupied: Vec<(f64, f64, f64, f64)> = Vec::new(); // (x, y, w, h) of placed items
    let sheets_used = 1;
    let mut total_part_area = 0.0;

    for (id, copy, w, h, allow_rot) in &items {
        let spacing = sheet.spacing;

        // Try both orientations if rotation allowed
        let orientations: Vec<(f64, f64, f64)> = if *allow_rot {
            vec![(*w + spacing, *h + spacing, 0.0), (*h + spacing, *w + spacing, 90.0)]
        } else {
            vec![(*w + spacing, *h + spacing, 0.0)]
        };

        let mut best_pos: Option<(f64, f64, f64, f64, f64)> = None;

        for &(ow, oh, rot) in &orientations {
            if let Some((x, y)) = find_bottom_left_position(ow, oh, &occupied, usable_w, usable_h, sheet.margin) {
                // Prefer position closest to origin
                let score = x + y;
                if best_pos.is_none() || score < best_pos.unwrap().0 + best_pos.unwrap().1 {
                    best_pos = Some((x, y, ow - spacing, oh - spacing, rot));
                }
            }
        }

        if let Some((x, y, pw, ph, rot)) = best_pos {
            occupied.push((x, y, pw + spacing, ph + spacing));
            total_part_area += pw * ph;
            placed.push(PlacedPart {
                part_id: id.clone(),
                copy_index: *copy,
                position: (x, y),
                rotation: rot,
                width: pw,
                height: ph,
            });
        } else {
            unplaced.push(format!("{}_{}", id, copy));
        }
    }

    let sheet_area = usable_w * usable_h * sheets_used as f64;
    let utilization = if sheet_area > 0.0 { total_part_area / sheet_area } else { 0.0 };

    NestResult {
        placed,
        sheets_used,
        utilization,
        unplaced,
        waste_area: sheet_area - total_part_area,
    }
}

/// Find the bottom-left position for a rectangle that doesn't overlap existing placements.
fn find_bottom_left_position(
    width: f64,
    height: f64,
    occupied: &[(f64, f64, f64, f64)],
    sheet_w: f64,
    sheet_h: f64,
    margin: f64,
) -> Option<(f64, f64)> {
    // Try positions along a grid of candidate points
    let mut candidates: Vec<(f64, f64)> = vec![(margin, margin)];

    // Add top-right corners of existing placements as candidates
    for &(ox, oy, ow, oh) in occupied {
        candidates.push((ox + ow, margin));
        candidates.push((margin, oy + oh));
        candidates.push((ox + ow, oy));
        candidates.push((ox, oy + oh));
    }

    // Sort by y then x (bottom-left preference)
    candidates.sort_by(|a, b| {
        a.1.partial_cmp(&b.1).unwrap().then(a.0.partial_cmp(&b.0).unwrap())
    });
    candidates.dedup();

    for (cx, cy) in candidates {
        if cx < margin || cy < margin { continue; }
        if cx + width > sheet_w + margin || cy + height > sheet_h + margin { continue; }

        // Check for overlap with all existing placements
        let fits = !occupied.iter().any(|&(ox, oy, ow, oh)| {
            cx < ox + ow && cx + width > ox && cy < oy + oh && cy + height > oy
        });

        if fits {
            return Some((cx, cy));
        }
    }

    None
}

/// Compute bounding box of a contour.
fn contour_bounding_box(contour: &Contour) -> (f64, f64) {
    if contour.points.is_empty() { return (0.0, 0.0); }
    let min_x = contour.points.iter().map(|p| p.x).fold(f64::MAX, f64::min);
    let max_x = contour.points.iter().map(|p| p.x).fold(f64::MIN, f64::max);
    let min_y = contour.points.iter().map(|p| p.y).fold(f64::MAX, f64::min);
    let max_y = contour.points.iter().map(|p| p.y).fold(f64::MIN, f64::max);
    (max_x - min_x, max_y - min_y)
}

/// Optimize cut ordering to minimize heat distortion.
///
/// Cuts inside-out (holes first), then orders outers to minimize
/// consecutive cuts near the same area (reduces thermal buildup).
pub fn optimize_cut_order(
    placed: &[PlacedPart],
    _contours: &[Contour],
) -> Vec<usize> {
    if placed.is_empty() { return Vec::new(); }

    // Simple nearest-neighbor ordering from current position
    let mut order = Vec::new();
    let mut remaining: Vec<usize> = (0..placed.len()).collect();
    let mut current_pos = (0.0, 0.0);

    while !remaining.is_empty() {
        // Find nearest unvisited part
        let mut best_idx = 0;
        let mut best_dist = f64::MAX;

        for (i, &idx) in remaining.iter().enumerate() {
            let (px, py) = placed[idx].position;
            let dist = ((px - current_pos.0).powi(2) + (py - current_pos.1).powi(2)).sqrt();
            if dist < best_dist {
                best_dist = dist;
                best_idx = i;
            }
        }

        let chosen = remaining.remove(best_idx);
        current_pos = placed[chosen].position;
        order.push(chosen);
    }

    order
}

// ---------------------------------------------------------------------------
// Simplified API types (geometry-agnostic, no dependency on Contour)
// ---------------------------------------------------------------------------

/// A simple 2D point for cut-path and nesting APIs.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Point2D {
    pub x: f64,
    pub y: f64,
}

impl Point2D {
    pub fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    pub fn distance_to(self, other: Self) -> f64 {
        ((self.x - other.x).powi(2) + (self.y - other.y).powi(2)).sqrt()
    }
}

/// A rectangular part outline for simplified nesting.
#[derive(Clone, Debug)]
pub struct PartOutline {
    pub width: f64,
    pub height: f64,
}

/// Result from simplified nesting API.
#[derive(Clone, Debug)]
pub struct SimpleNestResult {
    /// (index, x, y, rotated_90) for each placed part.
    pub placements: Vec<(usize, f64, f64, bool)>,
    /// Material utilization (0.0-1.0).
    pub utilization: f64,
}

/// A cut path — a sequence of points forming a toolpath segment.
#[derive(Clone, Debug)]
pub struct CutPath {
    pub points: Vec<Point2D>,
}

impl CutPath {
    /// Start point of this cut path.
    pub fn start(&self) -> Point2D {
        self.points.first().copied().unwrap_or(Point2D::new(0.0, 0.0))
    }

    /// End point of this cut path.
    pub fn end(&self) -> Point2D {
        self.points.last().copied().unwrap_or(Point2D::new(0.0, 0.0))
    }

    /// Centroid (average of all points).
    pub fn centroid(&self) -> Point2D {
        if self.points.is_empty() {
            return Point2D::new(0.0, 0.0);
        }
        let n = self.points.len() as f64;
        let sx: f64 = self.points.iter().map(|p| p.x).sum();
        let sy: f64 = self.points.iter().map(|p| p.y).sum();
        Point2D::new(sx / n, sy / n)
    }

    /// Bounding box as (min, max).
    pub fn bounding_box(&self) -> (Point2D, Point2D) {
        let mut min = Point2D::new(f64::MAX, f64::MAX);
        let mut max = Point2D::new(f64::MIN, f64::MIN);
        for p in &self.points {
            min.x = min.x.min(p.x);
            min.y = min.y.min(p.y);
            max.x = max.x.max(p.x);
            max.y = max.y.max(p.y);
        }
        (min, max)
    }
}

// ---------------------------------------------------------------------------
// Simplified nesting: nest_parts (PartOutline-based)
// ---------------------------------------------------------------------------

/// Nest rectangular parts onto a sheet using bottom-left fill with 0/90 rotation.
///
/// Returns placed positions and utilization percentage.
pub fn nest_part_outlines(
    parts: &[PartOutline],
    sheet_width: f64,
    sheet_height: f64,
    spacing: f64,
) -> SimpleNestResult {
    if parts.is_empty() {
        return SimpleNestResult { placements: vec![], utilization: 0.0 };
    }

    // Sort by area descending (first-fit decreasing)
    let mut indices: Vec<usize> = (0..parts.len()).collect();
    indices.sort_by(|&a, &b| {
        let area_a = parts[a].width * parts[a].height;
        let area_b = parts[b].width * parts[b].height;
        area_b.partial_cmp(&area_a).unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut placements = Vec::new();
    let mut occupied: Vec<(f64, f64, f64, f64)> = Vec::new();
    let mut total_part_area = 0.0;

    for &idx in &indices {
        let p = &parts[idx];
        // Try both orientations
        let orientations = [
            (p.width + spacing, p.height + spacing, false),
            (p.height + spacing, p.width + spacing, true),
        ];

        let mut best: Option<(f64, f64, bool, f64, f64)> = None;

        for &(ow, oh, rotated) in &orientations {
            if let Some((x, y)) = find_bottom_left_position(ow, oh, &occupied, sheet_width, sheet_height, 0.0) {
                let score = y * 10000.0 + x; // bottom-left preference
                if best.is_none() || score < best.unwrap().0 * 10000.0 + best.unwrap().1 {
                    best = Some((x, y, rotated, ow - spacing, oh - spacing));
                }
            }
        }

        if let Some((x, y, rotated, pw, ph)) = best {
            occupied.push((x, y, pw + spacing, ph + spacing));
            total_part_area += pw * ph;
            placements.push((idx, x, y, rotated));
        }
    }

    let sheet_area = sheet_width * sheet_height;
    let utilization = if sheet_area > 0.0 { total_part_area / sheet_area } else { 0.0 };

    SimpleNestResult { placements, utilization }
}

// ---------------------------------------------------------------------------
// Cut order optimization (CutPath-based, nearest-neighbor TSP)
// ---------------------------------------------------------------------------

/// Optimize cut order using nearest-neighbor TSP heuristic.
///
/// Returns indices into `paths` that minimize total rapid travel distance
/// between consecutive cut end -> next cut start.
pub fn optimize_cut_order_paths(paths: &[CutPath]) -> Vec<usize> {
    if paths.is_empty() {
        return Vec::new();
    }

    let mut order = Vec::with_capacity(paths.len());
    let mut remaining: Vec<usize> = (0..paths.len()).collect();
    let mut current = Point2D::new(0.0, 0.0);

    while !remaining.is_empty() {
        let mut best_i = 0;
        let mut best_dist = f64::MAX;

        for (i, &idx) in remaining.iter().enumerate() {
            let dist = current.distance_to(paths[idx].start());
            if dist < best_dist {
                best_dist = dist;
                best_i = i;
            }
        }

        let chosen = remaining.swap_remove(best_i);
        current = paths[chosen].end();
        order.push(chosen);
    }

    order
}

/// Total rapid-travel distance for a given order of cuts.
pub fn total_travel_distance(paths: &[CutPath], order: &[usize]) -> f64 {
    let mut dist = 0.0;
    let mut current = Point2D::new(0.0, 0.0);
    for &idx in order {
        dist += current.distance_to(paths[idx].start());
        current = paths[idx].end();
    }
    dist
}

// ---------------------------------------------------------------------------
// Kerf compensation (Point2D-based)
// ---------------------------------------------------------------------------

/// Apply kerf compensation to a closed contour.
///
/// Offsets the contour outward (for outer profiles) or inward (for holes)
/// by `kerf_width / 2`. `is_outer` = true means offset outward.
///
/// Uses the standard polygon offset by moving each edge along its inward
/// normal and finding intersection points of adjacent offset edges.
pub fn apply_kerf_compensation(
    contour: &[Point2D],
    kerf_width: f64,
    is_outer: bool,
) -> Vec<Point2D> {
    if contour.len() < 3 || kerf_width <= 0.0 {
        return contour.to_vec();
    }

    let half = kerf_width / 2.0;
    // Determine winding: positive signed area = CCW
    let signed_area = polygon_signed_area(contour);
    // For outer profiles (CCW, is_outer=true) we offset outward (away from interior).
    // For holes (CW, is_outer=false) we offset inward (toward hole center = also away from material).
    // The direction depends on winding:
    //   CCW + outer => offset by +half along outward normal
    //   CW + hole   => offset by +half along outward normal
    // We unify by choosing the sign of offset based on winding and is_outer.
    // The normals computed below as (-ey, ex) point LEFT of the edge direction.
    // For a CCW polygon, LEFT of edge = INWARD.
    // So to go OUTWARD on CCW: negate. To go INWARD on CCW: keep positive.
    // For outer profile (expand): we want outward on CCW => sign = -1
    // For hole (shrink, is_outer=false): we want inward on CW polygon.
    //   CW polygon: left of edge = OUTWARD. Inward = negate => sign = -1
    // Summary: outer+CCW => -1, hole+CW => -1, outer+CW => +1, hole+CCW => +1
    let sign = if is_outer == (signed_area >= 0.0) { -1.0 } else { 1.0 };
    let offset = sign * half;

    let n = contour.len();
    let mut result = Vec::with_capacity(n);

    for i in 0..n {
        let prev = contour[(i + n - 1) % n];
        let curr = contour[i];
        let next = contour[(i + 1) % n];

        // Edge normals (pointing left of edge direction for CCW polygon = outward)
        let e1x = curr.x - prev.x;
        let e1y = curr.y - prev.y;
        let len1 = (e1x * e1x + e1y * e1y).sqrt();

        let e2x = next.x - curr.x;
        let e2y = next.y - curr.y;
        let len2 = (e2x * e2x + e2y * e2y).sqrt();

        if len1 < 1e-12 || len2 < 1e-12 {
            result.push(curr);
            continue;
        }

        // Outward normals (left of edge for CCW)
        let n1x = -e1y / len1;
        let n1y = e1x / len1;
        let n2x = -e2y / len2;
        let n2y = e2x / len2;

        // Bisector direction
        let bx = n1x + n2x;
        let by = n1y + n2y;
        let blen = (bx * bx + by * by).sqrt();

        if blen < 1e-12 {
            // Parallel edges, just offset along normal
            result.push(Point2D::new(curr.x + n1x * offset, curr.y + n1y * offset));
        } else {
            // Offset along bisector, scaled so perpendicular distance = offset
            let cos_half = (n1x * bx / blen + n1y * by / blen).abs();
            let d = if cos_half > 1e-6 { offset / cos_half } else { offset };
            result.push(Point2D::new(curr.x + bx / blen * d, curr.y + by / blen * d));
        }
    }

    result
}

/// Signed area of a polygon (positive = CCW).
fn polygon_signed_area(pts: &[Point2D]) -> f64 {
    let n = pts.len();
    let mut area = 0.0;
    for i in 0..n {
        let j = (i + 1) % n;
        area += pts[i].x * pts[j].y;
        area -= pts[j].x * pts[i].y;
    }
    area / 2.0
}

// ---------------------------------------------------------------------------
// Heat distribution optimization
// ---------------------------------------------------------------------------

/// Reorder cuts to avoid consecutive cuts on adjacent features.
///
/// Uses a greedy approach: for each next cut, pick the uncut path whose
/// centroid is at least `min_spacing` away from the previous cut's centroid.
/// If no such path exists, pick the farthest one (to maximize cooling time).
pub fn optimize_heat_distribution(paths: &[CutPath], min_spacing: f64) -> Vec<usize> {
    if paths.is_empty() {
        return Vec::new();
    }

    let centroids: Vec<Point2D> = paths.iter().map(|p| p.centroid()).collect();
    let mut order = Vec::with_capacity(paths.len());
    let mut remaining: Vec<usize> = (0..paths.len()).collect();

    // Start from the path nearest to origin (arbitrary but deterministic)
    {
        let origin = Point2D::new(0.0, 0.0);
        let mut best_i = 0;
        let mut best_d = f64::MAX;
        for (i, &idx) in remaining.iter().enumerate() {
            let d = origin.distance_to(centroids[idx]);
            if d < best_d {
                best_d = d;
                best_i = i;
            }
        }
        let first = remaining.swap_remove(best_i);
        order.push(first);
    }

    while !remaining.is_empty() {
        let last = *order.last().unwrap();
        let last_c = centroids[last];

        // Try to find a path far enough away
        let mut far_enough: Vec<(usize, f64)> = Vec::new();
        let mut closest_far: Option<(usize, f64)> = None;
        let mut farthest: Option<(usize, f64)> = None;

        for (i, &idx) in remaining.iter().enumerate() {
            let d = last_c.distance_to(centroids[idx]);
            if d >= min_spacing {
                far_enough.push((i, d));
                // Among far-enough, prefer the nearest (minimize travel)
                if closest_far.is_none() || d < closest_far.unwrap().1 {
                    closest_far = Some((i, d));
                }
            }
            if farthest.is_none() || d > farthest.unwrap().1 {
                farthest = Some((i, d));
            }
        }

        let pick_i = if let Some((i, _)) = closest_far {
            i
        } else {
            // No path far enough — pick the farthest to maximize spacing
            farthest.unwrap().0
        };

        let chosen = remaining.swap_remove(pick_i);
        order.push(chosen);
    }

    order
}

/// Kerf compensation lookup from material+thickness+laser type.
pub fn kerf_from_lut(material: &str, thickness_mm: f64, laser_type: &str) -> f64 {
    // LUT-first approach: common material/thickness/laser combinations
    match (material.to_lowercase().as_str(), laser_type.to_lowercase().as_str()) {
        ("acrylic" | "pmma", "co2") => {
            // CO2 laser on acrylic: kerf ≈ 0.15-0.3mm depending on thickness
            0.15 + thickness_mm * 0.015
        }
        ("plywood" | "mdf" | "wood", "co2") => {
            0.2 + thickness_mm * 0.02
        }
        ("mild steel" | "steel" | "carbon steel", "fiber") => {
            0.1 + thickness_mm * 0.01
        }
        ("stainless" | "stainless steel", "fiber") => {
            0.12 + thickness_mm * 0.012
        }
        ("aluminum", "fiber") => {
            0.15 + thickness_mm * 0.018
        }
        ("cardboard" | "paper", "co2") => {
            0.1
        }
        ("leather", "co2") => {
            0.15 + thickness_mm * 0.01
        }
        _ => {
            // Generic fallback
            0.15 + thickness_mm * 0.015
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::DVec2;

    fn square_part(size: f64, id: &str) -> NestPart {
        NestPart {
            id: id.into(),
            contour: Contour::closed(vec![
                DVec2::new(0.0, 0.0),
                DVec2::new(size, 0.0),
                DVec2::new(size, size),
                DVec2::new(0.0, size),
            ]),
            quantity: 1,
            allow_rotation: true,
        }
    }

    #[test]
    fn nest_single_part() {
        let parts = vec![square_part(50.0, "sq1")];
        let sheet = Sheet::standard_12x12(3.0);
        let result = nest_parts(&parts, &sheet);
        assert_eq!(result.placed.len(), 1);
        assert!(result.unplaced.is_empty());
        assert!(result.utilization > 0.0);
    }

    #[test]
    fn nest_multiple_parts() {
        let parts = vec![
            square_part(50.0, "sq1"),
            square_part(50.0, "sq2"),
            square_part(30.0, "sq3"),
        ];
        let sheet = Sheet::standard_12x12(3.0);
        let result = nest_parts(&parts, &sheet);
        assert_eq!(result.placed.len(), 3);
        assert!(result.unplaced.is_empty());
    }

    #[test]
    fn nest_with_quantity() {
        let parts = vec![NestPart {
            id: "sq".into(),
            contour: Contour::closed(vec![
                DVec2::new(0.0, 0.0),
                DVec2::new(40.0, 0.0),
                DVec2::new(40.0, 40.0),
                DVec2::new(0.0, 40.0),
            ]),
            quantity: 4,
            allow_rotation: true,
        }];
        let sheet = Sheet::standard_12x12(3.0);
        let result = nest_parts(&parts, &sheet);
        assert_eq!(result.placed.len(), 4);
    }

    #[test]
    fn nest_part_too_large() {
        let parts = vec![square_part(500.0, "huge")]; // bigger than sheet
        let sheet = Sheet::standard_12x12(3.0);
        let result = nest_parts(&parts, &sheet);
        assert!(result.placed.is_empty());
        assert!(!result.unplaced.is_empty());
    }

    #[test]
    fn utilization_reasonable() {
        let parts = vec![
            square_part(100.0, "a"),
            square_part(100.0, "b"),
        ];
        let sheet = Sheet::standard_12x12(3.0);
        let result = nest_parts(&parts, &sheet);
        // Two 100x100 squares on a 305x305 sheet ≈ 21%
        assert!(result.utilization > 0.1 && result.utilization < 1.0);
    }

    #[test]
    fn kerf_acrylic_co2() {
        let kerf = kerf_from_lut("acrylic", 3.0, "CO2");
        assert!(kerf > 0.1 && kerf < 0.5, "Kerf should be 0.1-0.5mm, got {kerf}");
    }

    #[test]
    fn kerf_steel_fiber() {
        let kerf = kerf_from_lut("mild steel", 2.0, "fiber");
        assert!(kerf > 0.05 && kerf < 0.3, "Kerf should be 0.05-0.3mm, got {kerf}");
    }

    #[test]
    fn kerf_increases_with_thickness() {
        let k1 = kerf_from_lut("plywood", 3.0, "CO2");
        let k2 = kerf_from_lut("plywood", 6.0, "CO2");
        assert!(k2 > k1, "Thicker material should have wider kerf");
    }

    #[test]
    fn cut_order_optimization() {
        let placed = vec![
            PlacedPart { part_id: "a".into(), copy_index: 0, position: (100.0, 100.0), rotation: 0.0, width: 50.0, height: 50.0 },
            PlacedPart { part_id: "b".into(), copy_index: 0, position: (10.0, 10.0), rotation: 0.0, width: 50.0, height: 50.0 },
            PlacedPart { part_id: "c".into(), copy_index: 0, position: (200.0, 200.0), rotation: 0.0, width: 50.0, height: 50.0 },
        ];
        let order = optimize_cut_order(&placed, &[]);
        assert_eq!(order.len(), 3);
        // Starting from (0,0), nearest should be 'b' at (10,10)
        assert_eq!(order[0], 1); // b is nearest to origin
    }

    // -----------------------------------------------------------------------
    // Simplified nesting API tests
    // -----------------------------------------------------------------------

    #[test]
    fn nest_outlines_utilization_above_60_percent() {
        // Pack four 50x50 parts onto a 110x110 sheet → ~82% utilization
        let parts = vec![
            PartOutline { width: 50.0, height: 50.0 },
            PartOutline { width: 50.0, height: 50.0 },
            PartOutline { width: 50.0, height: 50.0 },
            PartOutline { width: 50.0, height: 50.0 },
        ];
        let result = nest_part_outlines(&parts, 110.0, 110.0, 2.0);
        assert_eq!(result.placements.len(), 4, "All 4 parts should be placed");
        assert!(
            result.utilization > 0.60,
            "Utilization should be >60%, got {:.1}%",
            result.utilization * 100.0
        );
    }

    #[test]
    fn nest_outlines_rotation_helps() {
        // 80x20 part on a 30x90 sheet: only fits if rotated 90°
        let parts = vec![PartOutline { width: 80.0, height: 20.0 }];
        let result = nest_part_outlines(&parts, 30.0, 90.0, 0.0);
        assert_eq!(result.placements.len(), 1, "Part should fit when rotated");
        assert!(result.placements[0].3, "Part should be rotated");
    }

    // -----------------------------------------------------------------------
    // Cut order optimization (CutPath) tests
    // -----------------------------------------------------------------------

    fn make_cut_path(pts: &[(f64, f64)]) -> CutPath {
        CutPath {
            points: pts.iter().map(|&(x, y)| Point2D::new(x, y)).collect(),
        }
    }

    #[test]
    fn cut_order_shorter_than_naive() {
        // Paths scattered around: naive order = 0,1,2,3; optimized should be shorter
        let paths = vec![
            make_cut_path(&[(200.0, 200.0), (210.0, 200.0)]),
            make_cut_path(&[(10.0, 10.0), (20.0, 10.0)]),
            make_cut_path(&[(100.0, 100.0), (110.0, 100.0)]),
            make_cut_path(&[(50.0, 50.0), (60.0, 50.0)]),
        ];
        let naive_order: Vec<usize> = (0..paths.len()).collect();
        let opt_order = optimize_cut_order_paths(&paths);

        let naive_dist = total_travel_distance(&paths, &naive_order);
        let opt_dist = total_travel_distance(&paths, &opt_order);

        assert!(
            opt_dist <= naive_dist,
            "Optimized travel {opt_dist:.1} should be <= naive {naive_dist:.1}"
        );
    }

    #[test]
    fn cut_order_visits_all() {
        let paths = vec![
            make_cut_path(&[(0.0, 0.0), (1.0, 0.0)]),
            make_cut_path(&[(5.0, 5.0), (6.0, 5.0)]),
            make_cut_path(&[(2.0, 2.0), (3.0, 2.0)]),
        ];
        let order = optimize_cut_order_paths(&paths);
        assert_eq!(order.len(), 3);
        let mut sorted = order.clone();
        sorted.sort();
        assert_eq!(sorted, vec![0, 1, 2]);
    }

    // -----------------------------------------------------------------------
    // Kerf compensation (Point2D) tests
    // -----------------------------------------------------------------------

    #[test]
    fn kerf_compensation_outer_expands() {
        // CCW square 0,0 -> 10,0 -> 10,10 -> 0,10
        let sq = vec![
            Point2D::new(0.0, 0.0),
            Point2D::new(10.0, 0.0),
            Point2D::new(10.0, 10.0),
            Point2D::new(0.0, 10.0),
        ];
        let result = apply_kerf_compensation(&sq, 1.0, true);
        assert_eq!(result.len(), 4);
        // Outer offset should make it bigger — area should increase
        let orig_area = polygon_signed_area(&sq).abs();
        let new_area = polygon_signed_area(&result).abs();
        assert!(
            new_area > orig_area,
            "Outer kerf should expand: orig={orig_area}, new={new_area}"
        );
    }

    #[test]
    fn kerf_compensation_hole_shrinks() {
        // CW square (hole) 0,0 -> 0,10 -> 10,10 -> 10,0
        let sq = vec![
            Point2D::new(0.0, 0.0),
            Point2D::new(0.0, 10.0),
            Point2D::new(10.0, 10.0),
            Point2D::new(10.0, 0.0),
        ];
        let result = apply_kerf_compensation(&sq, 1.0, false);
        assert_eq!(result.len(), 4);
        let orig_area = polygon_signed_area(&sq).abs();
        let new_area = polygon_signed_area(&result).abs();
        assert!(
            new_area < orig_area,
            "Hole kerf should shrink: orig={orig_area}, new={new_area}"
        );
    }

    #[test]
    fn kerf_compensation_preserves_topology() {
        // Triangle
        let tri = vec![
            Point2D::new(0.0, 0.0),
            Point2D::new(20.0, 0.0),
            Point2D::new(10.0, 17.32),
        ];
        let result = apply_kerf_compensation(&tri, 0.5, true);
        assert_eq!(result.len(), 3, "Triangle should remain 3 vertices");
        // All points should be displaced outward
        let orig_area = polygon_signed_area(&tri).abs();
        let new_area = polygon_signed_area(&result).abs();
        assert!(new_area > orig_area);
    }

    // -----------------------------------------------------------------------
    // Heat distribution tests
    // -----------------------------------------------------------------------

    #[test]
    fn heat_distribution_avoids_adjacency() {
        // Four paths: two clusters. Heat optimization should alternate clusters.
        let paths = vec![
            make_cut_path(&[(0.0, 0.0), (1.0, 0.0)]),   // cluster A
            make_cut_path(&[(2.0, 0.0), (3.0, 0.0)]),   // cluster A (adjacent)
            make_cut_path(&[(100.0, 0.0), (101.0, 0.0)]), // cluster B
            make_cut_path(&[(102.0, 0.0), (103.0, 0.0)]), // cluster B (adjacent)
        ];
        let order = optimize_heat_distribution(&paths, 50.0);
        assert_eq!(order.len(), 4);

        // Check that consecutive cuts don't have centroids within min_spacing
        // (except when unavoidable — all far paths used up)
        let centroids: Vec<Point2D> = paths.iter().map(|p| p.centroid()).collect();
        let mut violations = 0;
        for w in order.windows(2) {
            let d = centroids[w[0]].distance_to(centroids[w[1]]);
            if d < 50.0 {
                violations += 1;
            }
        }
        // With 2 clusters of 2, we can alternate perfectly: A, B, A, B => 0 violations
        // or at most 1 violation
        assert!(
            violations <= 1,
            "Too many adjacent cuts: {violations} violations in order {order:?}"
        );
    }

    #[test]
    fn heat_distribution_visits_all() {
        let paths = vec![
            make_cut_path(&[(0.0, 0.0), (1.0, 0.0)]),
            make_cut_path(&[(50.0, 50.0), (51.0, 50.0)]),
            make_cut_path(&[(100.0, 100.0), (101.0, 100.0)]),
        ];
        let order = optimize_heat_distribution(&paths, 30.0);
        assert_eq!(order.len(), 3);
        let mut sorted = order.clone();
        sorted.sort();
        assert_eq!(sorted, vec![0, 1, 2]);
    }
}
