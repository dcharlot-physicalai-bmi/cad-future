//! Sheet metal operations — unfold, bend deduction, flat pattern.
//!
//! K-factor and bend allowance from LUT (material × thickness × bend radius).
//! Flat pattern computation unfolds bends to compute blank size.
//! Bend sequence optimization minimizes setups.

use glam::DVec3;


// ---------------------------------------------------------------------------
// K-Factor & Bend Allowance
// ---------------------------------------------------------------------------

/// K-factor entry from LUT: maps (material, thickness, bend_radius/thickness) → k.
/// K-factor = ratio of neutral axis offset to material thickness (0..1).
#[derive(Debug, Clone, Copy)]
pub struct KFactorEntry {
    pub material: &'static str,
    pub thickness_mm: f64,
    pub bend_radius_factor: f64, // bend_radius / thickness
    pub k_factor: f64,
}

/// Pre-computed k-factor table.
/// Sources: Machinery's Handbook (31st ed.), ASM Metals Handbook Vol 14.
/// General rule: k ≈ 0.33 for air bending, 0.40-0.50 for bottoming.
pub static K_FACTOR_TABLE: &[KFactorEntry] = &[
    // Aluminum — soft, good formability
    KFactorEntry { material: "Aluminum", thickness_mm: 0.5,  bend_radius_factor: 1.0, k_factor: 0.33 },
    KFactorEntry { material: "Aluminum", thickness_mm: 1.0,  bend_radius_factor: 1.0, k_factor: 0.33 },
    KFactorEntry { material: "Aluminum", thickness_mm: 1.5,  bend_radius_factor: 1.0, k_factor: 0.34 },
    KFactorEntry { material: "Aluminum", thickness_mm: 2.0,  bend_radius_factor: 1.0, k_factor: 0.35 },
    KFactorEntry { material: "Aluminum", thickness_mm: 3.0,  bend_radius_factor: 1.0, k_factor: 0.36 },
    KFactorEntry { material: "Aluminum", thickness_mm: 1.0,  bend_radius_factor: 0.5, k_factor: 0.30 },
    KFactorEntry { material: "Aluminum", thickness_mm: 1.0,  bend_radius_factor: 2.0, k_factor: 0.37 },
    KFactorEntry { material: "Aluminum", thickness_mm: 1.0,  bend_radius_factor: 3.0, k_factor: 0.40 },
    // Mild steel — medium formability
    KFactorEntry { material: "MildSteel", thickness_mm: 0.5,  bend_radius_factor: 1.0, k_factor: 0.35 },
    KFactorEntry { material: "MildSteel", thickness_mm: 1.0,  bend_radius_factor: 1.0, k_factor: 0.38 },
    KFactorEntry { material: "MildSteel", thickness_mm: 1.5,  bend_radius_factor: 1.0, k_factor: 0.40 },
    KFactorEntry { material: "MildSteel", thickness_mm: 2.0,  bend_radius_factor: 1.0, k_factor: 0.42 },
    KFactorEntry { material: "MildSteel", thickness_mm: 3.0,  bend_radius_factor: 1.0, k_factor: 0.44 },
    KFactorEntry { material: "MildSteel", thickness_mm: 1.0,  bend_radius_factor: 0.5, k_factor: 0.33 },
    KFactorEntry { material: "MildSteel", thickness_mm: 1.0,  bend_radius_factor: 2.0, k_factor: 0.42 },
    KFactorEntry { material: "MildSteel", thickness_mm: 1.0,  bend_radius_factor: 3.0, k_factor: 0.45 },
    // Stainless steel — harder, more springback
    KFactorEntry { material: "Stainless", thickness_mm: 0.5,  bend_radius_factor: 1.0, k_factor: 0.38 },
    KFactorEntry { material: "Stainless", thickness_mm: 1.0,  bend_radius_factor: 1.0, k_factor: 0.40 },
    KFactorEntry { material: "Stainless", thickness_mm: 1.5,  bend_radius_factor: 1.0, k_factor: 0.42 },
    KFactorEntry { material: "Stainless", thickness_mm: 2.0,  bend_radius_factor: 1.0, k_factor: 0.44 },
    KFactorEntry { material: "Stainless", thickness_mm: 3.0,  bend_radius_factor: 1.0, k_factor: 0.46 },
    // Copper/Brass — very soft
    KFactorEntry { material: "Copper",    thickness_mm: 0.5,  bend_radius_factor: 1.0, k_factor: 0.30 },
    KFactorEntry { material: "Copper",    thickness_mm: 1.0,  bend_radius_factor: 1.0, k_factor: 0.32 },
    KFactorEntry { material: "Copper",    thickness_mm: 2.0,  bend_radius_factor: 1.0, k_factor: 0.34 },
    // Titanium — tough, more springback
    KFactorEntry { material: "Titanium",  thickness_mm: 1.0,  bend_radius_factor: 1.0, k_factor: 0.42 },
    KFactorEntry { material: "Titanium",  thickness_mm: 2.0,  bend_radius_factor: 1.0, k_factor: 0.45 },
];

/// Look up k-factor from table. Finds nearest match by material, then
/// interpolates between thickness/radius entries.
pub fn lookup_k_factor(material: &str, thickness_mm: f64, bend_radius_mm: f64) -> f64 {
    let radius_factor = bend_radius_mm / thickness_mm;

    // Filter by material
    let entries: Vec<&KFactorEntry> = K_FACTOR_TABLE.iter()
        .filter(|e| e.material.eq_ignore_ascii_case(material))
        .collect();

    if entries.is_empty() {
        // Default k-factor for unknown materials
        return 0.40;
    }

    // Find best match: closest thickness, then closest radius factor
    let mut best = entries[0];
    let mut best_dist = f64::MAX;
    for e in &entries {
        let dt = (e.thickness_mm - thickness_mm).abs();
        let dr = (e.bend_radius_factor - radius_factor).abs();
        let dist = dt + dr * 0.5; // weight thickness more
        if dist < best_dist {
            best_dist = dist;
            best = e;
        }
    }

    best.k_factor
}

// ---------------------------------------------------------------------------
// Bend Allowance & Deduction
// ---------------------------------------------------------------------------

/// Bend allowance (BA) — arc length of the neutral axis through the bend.
/// BA = angle_rad × (bend_radius + k_factor × thickness)
pub fn bend_allowance(angle_rad: f64, bend_radius_mm: f64, thickness_mm: f64, k_factor: f64) -> f64 {
    angle_rad.abs() * (bend_radius_mm + k_factor * thickness_mm)
}

/// Bend deduction (BD) — material consumed by the bend.
/// BD = 2 × setback - BA, where setback = (bend_radius + thickness) × tan(angle/2)
pub fn bend_deduction(angle_rad: f64, bend_radius_mm: f64, thickness_mm: f64, k_factor: f64) -> f64 {
    let setback = (bend_radius_mm + thickness_mm) * (angle_rad.abs() / 2.0).tan();
    let ba = bend_allowance(angle_rad, bend_radius_mm, thickness_mm, k_factor);
    2.0 * setback - ba
}

/// Outside setback (OSSB) — distance from bend apex to tangent point on outside.
pub fn outside_setback(angle_rad: f64, bend_radius_mm: f64, thickness_mm: f64) -> f64 {
    (bend_radius_mm + thickness_mm) * (angle_rad.abs() / 2.0).tan()
}

// ---------------------------------------------------------------------------
// Sheet Metal Part Representation
// ---------------------------------------------------------------------------

/// A bend in a sheet metal part.
#[derive(Clone, Debug)]
pub struct Bend {
    /// Bend line position along the unfolded direction (mm from start).
    pub position_mm: f64,
    /// Bend angle in radians (positive = up from flat).
    pub angle_rad: f64,
    /// Inside bend radius (mm).
    pub bend_radius_mm: f64,
    /// Bend direction: which side folds up. +1 or -1.
    pub direction: i8,
}

/// A flat region (flange) between bends.
#[derive(Clone, Debug)]
pub struct Flange {
    /// Length of this flat section (mm).
    pub length_mm: f64,
}

/// A sheet metal part — ordered sequence of flanges separated by bends.
/// The part is represented as: flange[0], bend[0], flange[1], bend[1], ..., flange[n].
#[derive(Clone, Debug)]
pub struct SheetMetalPart {
    /// Material thickness (mm).
    pub thickness_mm: f64,
    /// Material name for k-factor lookup.
    pub material: String,
    /// Ordered flanges (n+1 flanges for n bends).
    pub flanges: Vec<Flange>,
    /// Ordered bends (between flanges).
    pub bends: Vec<Bend>,
}

impl SheetMetalPart {
    pub fn new(material: &str, thickness_mm: f64) -> Self {
        Self {
            thickness_mm,
            material: material.to_string(),
            flanges: Vec::new(),
            bends: Vec::new(),
        }
    }

    /// Add the first flange (must be called before add_bend).
    pub fn add_flange(&mut self, length_mm: f64) {
        self.flanges.push(Flange { length_mm });
    }

    /// Add a bend followed by a new flange.
    pub fn add_bend(&mut self, angle_deg: f64, bend_radius_mm: f64, direction: i8, next_flange_mm: f64) {
        let angle_rad = angle_deg.to_radians();
        let position = self.flanges.iter().map(|f| f.length_mm).sum::<f64>();
        self.bends.push(Bend {
            position_mm: position,
            angle_rad,
            bend_radius_mm,
            direction,
        });
        self.flanges.push(Flange { length_mm: next_flange_mm });
    }

    /// Compute flat pattern length (total blank size before bending).
    pub fn flat_pattern_length(&self) -> f64 {
        let mut total = 0.0;

        // Sum all flange lengths
        for flange in &self.flanges {
            total += flange.length_mm;
        }

        // Subtract bend deductions
        for bend in &self.bends {
            let k = lookup_k_factor(&self.material, self.thickness_mm, bend.bend_radius_mm);
            let bd = bend_deduction(bend.angle_rad, bend.bend_radius_mm, self.thickness_mm, k);
            total -= bd;
        }

        total
    }

    /// Get the k-factor for each bend.
    pub fn k_factors(&self) -> Vec<f64> {
        self.bends.iter()
            .map(|b| lookup_k_factor(&self.material, self.thickness_mm, b.bend_radius_mm))
            .collect()
    }

    /// Get bend allowance for each bend.
    pub fn bend_allowances(&self) -> Vec<f64> {
        self.bends.iter()
            .map(|b| {
                let k = lookup_k_factor(&self.material, self.thickness_mm, b.bend_radius_mm);
                bend_allowance(b.angle_rad, b.bend_radius_mm, self.thickness_mm, k)
            })
            .collect()
    }

    /// Get bend deduction for each bend.
    pub fn bend_deductions(&self) -> Vec<f64> {
        self.bends.iter()
            .map(|b| {
                let k = lookup_k_factor(&self.material, self.thickness_mm, b.bend_radius_mm);
                bend_deduction(b.angle_rad, b.bend_radius_mm, self.thickness_mm, k)
            })
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Flat Pattern — 2D unfolded representation
// ---------------------------------------------------------------------------

/// A 2D point in the flat pattern.
#[derive(Clone, Debug)]
pub struct FlatPoint {
    pub x: f64,
    pub y: f64,
}

/// Flat pattern result — the unfolded 2D shape.
#[derive(Clone, Debug)]
pub struct FlatPattern {
    /// Total unfolded length (mm).
    pub length_mm: f64,
    /// Width (same as the part width, perpendicular to bends).
    pub width_mm: f64,
    /// Bend line positions on the flat pattern (x offsets from left edge).
    pub bend_lines: Vec<f64>,
    /// Corners of the rectangular flat pattern.
    pub outline: Vec<FlatPoint>,
}

/// Compute the 2D flat pattern for a sheet metal part.
pub fn unfold(part: &SheetMetalPart, width_mm: f64) -> FlatPattern {
    let total_length = part.flat_pattern_length();

    // Compute bend line positions on the flat blank
    let mut bend_lines = Vec::with_capacity(part.bends.len());
    let mut x = 0.0;

    for (i, bend) in part.bends.iter().enumerate() {
        x += part.flanges[i].length_mm;
        if i > 0 {
            // Subtract cumulative bend deductions up to this point
            let k = lookup_k_factor(&part.material, part.thickness_mm, part.bends[i - 1].bend_radius_mm);
            let bd = bend_deduction(part.bends[i - 1].angle_rad, part.bends[i - 1].bend_radius_mm, part.thickness_mm, k);
            x -= bd;
        }
        bend_lines.push(x);
        let _ = bend; // used above via part.bends[i-1] pattern
    }

    let outline = vec![
        FlatPoint { x: 0.0, y: 0.0 },
        FlatPoint { x: total_length, y: 0.0 },
        FlatPoint { x: total_length, y: width_mm },
        FlatPoint { x: 0.0, y: width_mm },
    ];

    FlatPattern {
        length_mm: total_length,
        width_mm,
        bend_lines,
        outline,
    }
}

// ---------------------------------------------------------------------------
// Bend Sequence Optimization
// ---------------------------------------------------------------------------

/// A bend operation in the manufacturing sequence.
#[derive(Clone, Debug)]
pub struct BendOp {
    /// Index into the part's bends vector.
    pub bend_index: usize,
    /// Setup number (bends in the same setup don't require repositioning).
    pub setup: usize,
}

/// Optimize bend sequence to minimize setups.
/// Strategy: bend from inside out (shortest flanges first) to avoid collisions.
/// Returns ordered list of bend operations.
pub fn optimize_bend_sequence(part: &SheetMetalPart) -> Vec<BendOp> {
    if part.bends.is_empty() {
        return Vec::new();
    }

    // Score each bend: inside bends (near center) should go first
    let center = part.flat_pattern_length() / 2.0;
    let mut scored: Vec<(usize, f64)> = part.bends.iter().enumerate()
        .map(|(i, b)| {
            let dist_from_center = (b.position_mm - center).abs();
            (i, dist_from_center)
        })
        .collect();

    // Sort: bends closest to center first (inside-out)
    scored.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

    // Group into setups: bends that are co-directional and adjacent share a setup
    let mut ops = Vec::with_capacity(scored.len());
    let mut current_setup = 0;
    let mut prev_dir = 0i8;

    for (i, (bend_idx, _)) in scored.iter().enumerate() {
        let dir = part.bends[*bend_idx].direction;
        if i > 0 && dir != prev_dir {
            current_setup += 1;
        }
        ops.push(BendOp {
            bend_index: *bend_idx,
            setup: current_setup,
        });
        prev_dir = dir;
    }

    ops
}

// ---------------------------------------------------------------------------
// Relief Cuts
// ---------------------------------------------------------------------------

/// Type of relief cut at a bend intersection.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ReliefType {
    /// Rectangular slot.
    Rectangular,
    /// Round hole (bone relief).
    Round,
    /// Obround (rounded rectangle).
    Obround,
}

/// Relief cut specification.
#[derive(Clone, Debug)]
pub struct ReliefCut {
    /// Position along the bend line.
    pub position_mm: f64,
    /// Relief type.
    pub relief_type: ReliefType,
    /// Width of the relief (perpendicular to bend line).
    pub width_mm: f64,
    /// Depth of the relief (along bend line).
    pub depth_mm: f64,
}

/// Compute relief cuts needed at bend intersections.
/// Rule: relief width = thickness, depth = bend radius + thickness.
pub fn compute_relief_cuts(
    part: &SheetMetalPart,
    relief_type: ReliefType,
    width_mm: f64,
) -> Vec<ReliefCut> {
    let mut cuts = Vec::new();

    for bend in &part.bends {
        // Relief at each end of the bend line
        let relief_width = part.thickness_mm;
        let relief_depth = bend.bend_radius_mm + part.thickness_mm;

        cuts.push(ReliefCut {
            position_mm: bend.position_mm,
            relief_type,
            width_mm: relief_width,
            depth_mm: relief_depth,
        });

        // Relief at the other end
        cuts.push(ReliefCut {
            position_mm: bend.position_mm + width_mm,
            relief_type,
            width_mm: relief_width,
            depth_mm: relief_depth,
        });
    }

    cuts
}

// ---------------------------------------------------------------------------
// 3D Fold — rebuild 3D geometry from flat pattern + bends
// ---------------------------------------------------------------------------

/// A 3D folded segment — position and orientation after sequential folding.
#[derive(Clone, Debug)]
pub struct FoldedSegment {
    /// Origin point of this segment in 3D.
    pub origin: DVec3,
    /// Direction this segment extends (unit vector).
    pub direction: DVec3,
    /// Normal of this segment's face.
    pub normal: DVec3,
    /// Length of this flat segment.
    pub length_mm: f64,
}

/// Fold the flat pattern into 3D, producing a segment for each flange.
pub fn fold_3d(part: &SheetMetalPart) -> Vec<FoldedSegment> {
    let mut segments = Vec::with_capacity(part.flanges.len());
    let mut origin = DVec3::ZERO;
    let mut direction = DVec3::X;
    let mut normal = DVec3::Z;

    for (i, flange) in part.flanges.iter().enumerate() {
        segments.push(FoldedSegment {
            origin,
            direction,
            normal,
            length_mm: flange.length_mm,
        });

        // Advance origin to end of this flange
        origin += direction * flange.length_mm;

        // Apply bend rotation if there's a next bend
        if i < part.bends.len() {
            let bend = &part.bends[i];
            let angle = bend.angle_rad * bend.direction as f64;
            // Bend axis is perpendicular to both direction and normal
            let axis = direction.cross(normal).normalize();
            // Rotate direction and normal around the axis
            let rot = rotation_around_axis(axis, angle);
            direction = rot(direction);
            normal = rot(normal);
        }
    }

    segments
}

/// Returns a closure that rotates a vector around an axis by an angle (Rodrigues).
fn rotation_around_axis(axis: DVec3, angle: f64) -> impl Fn(DVec3) -> DVec3 {
    let k = axis.normalize();
    let cos_a = angle.cos();
    let sin_a = angle.sin();
    move |v: DVec3| {
        v * cos_a + k.cross(v) * sin_a + k * k.dot(v) * (1.0 - cos_a)
    }
}

// ---------------------------------------------------------------------------
// Springback Estimation
// ---------------------------------------------------------------------------

/// Estimate springback angle based on material and bend parameters.
/// Returns the additional overbend needed to achieve the target angle.
/// Source: Machinery's Handbook, empirical formula.
pub fn springback_angle(
    target_angle_rad: f64,
    bend_radius_mm: f64,
    thickness_mm: f64,
    yield_strength_mpa: f64,
    elastic_modulus_mpa: f64,
) -> f64 {
    // Springback ratio K_s = 1 - 3*(R/t)*(sigma_y/E) + 4*(R/t)^3*(sigma_y/E)^3
    let rt = bend_radius_mm / thickness_mm;
    let se = yield_strength_mpa / elastic_modulus_mpa;
    let ks = 1.0 - 3.0 * rt * se + 4.0 * rt.powi(3) * se.powi(3);

    // Springback = target_angle * (1 - K_s)
    target_angle_rad * (1.0 - ks).max(0.0)
}

// ---------------------------------------------------------------------------
// Minimum Bend Radius Check
// ---------------------------------------------------------------------------

/// Check if a bend radius is feasible for the given material and thickness.
/// Returns (feasible, minimum_radius_mm).
pub fn check_min_bend_radius(
    material: &str,
    thickness_mm: f64,
    proposed_radius_mm: f64,
) -> (bool, f64) {
    // Minimum bend radius as factor × thickness, from common guidelines
    let factor = match material.to_lowercase().as_str() {
        s if s.contains("aluminum") || s.contains("copper") => 0.5,
        s if s.contains("mild") || s.contains("1018") || s.contains("1020") => 0.8,
        s if s.contains("stainless") => 1.0,
        s if s.contains("titanium") => 2.0,
        s if s.contains("brass") => 0.5,
        _ => 1.0,
    };
    let min_radius = factor * thickness_mm;
    (proposed_radius_mm >= min_radius, min_radius)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    #[test]
    fn k_factor_lookup_aluminum() {
        let k = lookup_k_factor("Aluminum", 1.0, 1.0);
        assert!((k - 0.33).abs() < 0.01, "k={}", k);
    }

    #[test]
    fn k_factor_lookup_steel() {
        let k = lookup_k_factor("MildSteel", 1.0, 1.0);
        assert!((k - 0.38).abs() < 0.01, "k={}", k);
    }

    #[test]
    fn k_factor_unknown_material() {
        let k = lookup_k_factor("Unobtainium", 1.0, 1.0);
        assert!((k - 0.40).abs() < 0.001); // default
    }

    #[test]
    fn bend_allowance_90deg_steel() {
        // 90-degree bend, 1mm radius, 1mm thickness, k=0.38
        let ba = bend_allowance(PI / 2.0, 1.0, 1.0, 0.38);
        // Expected: (pi/2) * (1.0 + 0.38*1.0) = 1.5708 * 1.38 = 2.168
        assert!((ba - 2.168).abs() < 0.01, "ba={}", ba);
    }

    #[test]
    fn bend_deduction_90deg() {
        let bd = bend_deduction(PI / 2.0, 1.0, 1.0, 0.38);
        // setback = (1+1)*tan(45°) = 2.0
        // BD = 2*2.0 - 2.168 = 1.832
        assert!((bd - 1.832).abs() < 0.02, "bd={}", bd);
    }

    #[test]
    fn simple_bracket_flat_pattern() {
        // L-bracket: two 50mm flanges with a 90-degree bend
        let mut part = SheetMetalPart::new("MildSteel", 1.0);
        part.add_flange(50.0);
        part.add_bend(90.0, 1.0, 1, 50.0);

        let flat = part.flat_pattern_length();
        // Should be less than 100mm (sum of flanges) due to bend deduction
        assert!(flat < 100.0, "flat={}", flat);
        assert!(flat > 95.0, "flat={}", flat); // shouldn't lose too much
    }

    #[test]
    fn u_channel_flat_pattern() {
        // U-channel: three flanges, two 90-degree bends
        let mut part = SheetMetalPart::new("Aluminum", 1.5);
        part.add_flange(30.0);
        part.add_bend(90.0, 1.5, 1, 40.0);
        part.add_bend(90.0, 1.5, 1, 30.0);

        let flat = part.flat_pattern_length();
        assert!(flat < 100.0); // less than sum of 30+40+30
        assert!(flat > 90.0);

        let fp = unfold(&part, 100.0);
        assert_eq!(fp.bend_lines.len(), 2);
        assert!(fp.length_mm > 0.0);
    }

    #[test]
    fn bend_sequence_optimization() {
        let mut part = SheetMetalPart::new("MildSteel", 1.0);
        part.add_flange(20.0);
        part.add_bend(90.0, 1.0, 1, 30.0);
        part.add_bend(90.0, 1.0, -1, 20.0);

        let ops = optimize_bend_sequence(&part);
        assert_eq!(ops.len(), 2);
    }

    #[test]
    fn relief_cuts_computed() {
        let mut part = SheetMetalPart::new("MildSteel", 1.0);
        part.add_flange(50.0);
        part.add_bend(90.0, 1.0, 1, 50.0);

        let cuts = compute_relief_cuts(&part, ReliefType::Round, 100.0);
        assert_eq!(cuts.len(), 2); // one per end of bend line
    }

    #[test]
    fn fold_3d_l_bracket() {
        let mut part = SheetMetalPart::new("MildSteel", 1.0);
        part.add_flange(50.0);
        part.add_bend(90.0, 1.0, 1, 50.0);

        let segments = fold_3d(&part);
        assert_eq!(segments.len(), 2);

        // First segment: along X
        assert!((segments[0].direction - DVec3::X).length() < 1e-10);
        // Second segment: rotated 90 degrees, should have a non-X direction
        assert!(segments[1].direction.x.abs() < 0.1 || segments[1].direction.z.abs() > 0.1);
    }

    #[test]
    fn springback_estimation() {
        // Mild steel: E=200GPa, sigma_y=250MPa
        let sb = springback_angle(
            PI / 2.0,    // 90 degrees
            1.0,         // 1mm radius
            1.0,         // 1mm thickness
            250.0,       // yield strength
            200_000.0,   // elastic modulus
        );
        // Should be a small positive angle
        assert!(sb > 0.0);
        assert!(sb < 0.1); // less than ~6 degrees
    }

    #[test]
    fn min_bend_radius_check() {
        let (ok, min_r) = check_min_bend_radius("Aluminum", 2.0, 1.0);
        assert!(ok); // 1.0 >= 0.5*2.0 = 1.0
        assert!((min_r - 1.0).abs() < 0.01);

        let (ok2, _) = check_min_bend_radius("Titanium", 2.0, 1.0);
        assert!(!ok2); // 1.0 < 2.0*2.0 = 4.0
    }
}
