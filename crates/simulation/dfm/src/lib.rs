//! `physical-dfm` -- Design-for-Manufacturing validation engine.
//!
//! Performs real geometric analysis on B-Rep solids, comparing measured
//! geometry against the 117+ manufacturing constraints in the LUT.

use glam::DVec3;
use physical_brep::{Solid, Surface, FaceId};
use physical_lut::manufacturing::{self, ManufacturingConstraint, MaterialClass, Process};

// ---------------------------------------------------------------------------
// Severity & Category
// ---------------------------------------------------------------------------

/// Severity of a DFM issue.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    /// Informational hint -- manufacturing is possible but suboptimal.
    Info,
    /// Warning -- may cause quality issues.
    Warning,
    /// Error -- part cannot be manufactured with this process as-is.
    Error,
}

/// Category of a DFM check.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Category {
    WallThickness,
    HoleDiameter,
    DraftAngle,
    AspectRatio,
    CornerRadius,
    Undercut,
    Overhang,
    FeatureProximity,
    Envelope,
}

// ---------------------------------------------------------------------------
// DfmCheck -- individual check result
// ---------------------------------------------------------------------------

/// A single DFM validation check with geometric detail.
#[derive(Debug, Clone)]
pub enum DfmCheck {
    WallTooThin {
        measured_mm: f64,
        min_mm: f64,
        location: DVec3,
    },
    HoleTooSmall {
        diameter_mm: f64,
        min_mm: f64,
    },
    DraftAngleInsufficient {
        measured_deg: f64,
        min_deg: f64,
        face_id: FaceId,
    },
    AspectRatioExceeded {
        ratio: f64,
        max_ratio: f64,
    },
    CornerRadiusTooSmall {
        radius_mm: f64,
        min_mm: f64,
    },
    UndercutDetected {
        face_id: FaceId,
    },
    OverhangTooSteep {
        angle_deg: f64,
        max_deg: f64,
    },
    FeatureTooClose {
        distance_mm: f64,
        min_mm: f64,
    },
}

impl DfmCheck {
    /// Severity of this check.
    pub fn severity(&self) -> Severity {
        match self {
            Self::WallTooThin { .. } => Severity::Error,
            Self::HoleTooSmall { .. } => Severity::Error,
            Self::DraftAngleInsufficient { .. } => Severity::Warning,
            Self::AspectRatioExceeded { .. } => Severity::Warning,
            Self::CornerRadiusTooSmall { .. } => Severity::Warning,
            Self::UndercutDetected { .. } => Severity::Error,
            Self::OverhangTooSteep { .. } => Severity::Warning,
            Self::FeatureTooClose { .. } => Severity::Warning,
        }
    }

    /// Category of this check.
    pub fn category(&self) -> Category {
        match self {
            Self::WallTooThin { .. } => Category::WallThickness,
            Self::HoleTooSmall { .. } => Category::HoleDiameter,
            Self::DraftAngleInsufficient { .. } => Category::DraftAngle,
            Self::AspectRatioExceeded { .. } => Category::AspectRatio,
            Self::CornerRadiusTooSmall { .. } => Category::CornerRadius,
            Self::UndercutDetected { .. } => Category::Undercut,
            Self::OverhangTooSteep { .. } => Category::Overhang,
            Self::FeatureTooClose { .. } => Category::FeatureProximity,
        }
    }

    /// Human-readable message.
    pub fn message(&self) -> String {
        match self {
            Self::WallTooThin { measured_mm, min_mm, .. } => {
                format!("Wall thickness {measured_mm:.2} mm is below minimum {min_mm:.1} mm")
            }
            Self::HoleTooSmall { diameter_mm, min_mm } => {
                format!("Hole diameter {diameter_mm:.2} mm is below minimum {min_mm:.1} mm")
            }
            Self::DraftAngleInsufficient { measured_deg, min_deg, .. } => {
                format!("Draft angle {measured_deg:.1}\u{00b0} is below minimum {min_deg:.1}\u{00b0}")
            }
            Self::AspectRatioExceeded { ratio, max_ratio } => {
                format!("Aspect ratio {ratio:.1} exceeds maximum {max_ratio:.1}")
            }
            Self::CornerRadiusTooSmall { radius_mm, min_mm } => {
                format!("Corner radius {radius_mm:.2} mm is below minimum {min_mm:.1} mm")
            }
            Self::UndercutDetected { .. } => {
                "Undercut detected -- feature cannot be extracted from mold".into()
            }
            Self::OverhangTooSteep { angle_deg, max_deg } => {
                format!("Overhang angle {angle_deg:.1}\u{00b0} exceeds maximum {max_deg:.1}\u{00b0} for unsupported printing")
            }
            Self::FeatureTooClose { distance_mm, min_mm } => {
                format!("Feature spacing {distance_mm:.2} mm is below minimum {min_mm:.1} mm")
            }
        }
    }
}

// ---------------------------------------------------------------------------
// DfmReport
// ---------------------------------------------------------------------------

/// Complete DFM validation report for a solid against a process.
#[derive(Debug, Clone)]
pub struct DfmReport {
    /// All checks found.
    pub checks: Vec<DfmCheck>,
    /// True when no errors are present (warnings/info are OK).
    pub pass: bool,
    /// Human-readable summary.
    pub summary: String,
    /// Manufacturability score: 100 = perfect, 0 = unmakeable.
    pub score: f64,
}

// ---------------------------------------------------------------------------
// Intermediate analysis types
// ---------------------------------------------------------------------------

/// Measured wall thickness at a location.
#[derive(Debug, Clone)]
pub struct WallThickness {
    pub thickness_mm: f64,
    pub location: DVec3,
    pub face_id: FaceId,
}

/// Detected hole feature.
#[derive(Debug, Clone)]
pub struct HoleFeature {
    pub diameter_mm: f64,
    pub depth_mm: f64,
    pub face_id: FaceId,
}

/// Draft angle measurement for a face.
#[derive(Debug, Clone)]
pub struct DraftMeasurement {
    pub angle_deg: f64,
    pub face_id: FaceId,
}

/// Overhang face measurement (for AM).
#[derive(Debug, Clone)]
pub struct OverhangFace {
    pub angle_from_vertical_deg: f64,
    pub face_id: FaceId,
}

/// Corner radius measurement at an edge.
#[derive(Debug, Clone)]
pub struct CornerRadius {
    pub radius_mm: f64,
    pub location: DVec3,
    pub is_concave: bool,
}

// ---------------------------------------------------------------------------
// Analysis functions
// ---------------------------------------------------------------------------

/// Analyze wall thickness by ray-casting from each planar face centroid inward,
/// finding opposing faces and measuring the distance.
pub fn analyze_wall_thickness(solid: &Solid) -> Vec<WallThickness> {
    let mut results = Vec::new();

    let face_ids: Vec<FaceId> = solid.faces.keys().collect();

    for &fid in &face_ids {
        let face = &solid.faces[fid];
        let (origin, normal) = match &face.surface {
            Surface::Plane { origin, normal } => (*origin, normal.normalize()),
            _ => continue,
        };

        // Ray from face centroid inward (opposite to outward normal)
        let ray_dir = -normal;

        // Find closest opposing planar face
        let mut min_dist = f64::MAX;
        for &other_fid in &face_ids {
            if other_fid == fid {
                continue;
            }
            let other_face = &solid.faces[other_fid];
            let (other_origin, other_normal) = match &other_face.surface {
                Surface::Plane { origin, normal } => (*origin, normal.normalize()),
                _ => continue,
            };

            // Only consider faces whose outward normals are roughly anti-parallel
            // to this face (i.e. opposing walls).
            if normal.dot(other_normal) > -0.5 {
                continue;
            }

            // Ray-plane intersection: t = (other_origin - origin) . other_normal / (ray_dir . other_normal)
            let denom = ray_dir.dot(other_normal);
            if denom.abs() < 1e-12 {
                continue;
            }
            let t = (other_origin - origin).dot(other_normal) / denom;
            if t > 1e-6 && t < min_dist {
                min_dist = t;
            }
        }

        if min_dist < f64::MAX {
            results.push(WallThickness {
                thickness_mm: min_dist,
                location: origin,
                face_id: fid,
            });
        }
    }

    results
}

/// Detect cylindrical faces and measure hole diameter.
pub fn analyze_holes(solid: &Solid) -> Vec<HoleFeature> {
    let mut results = Vec::new();

    for (fid, face) in &solid.faces {
        if let Surface::Cylinder { origin, axis, radius } = &face.surface {
            let axis_n = axis.normalize();

            // Estimate hole depth from vertex extent along axis
            let edge_ids = solid.edges_of_face(fid);
            let mut min_t = f64::MAX;
            let mut max_t = f64::MIN;
            for eid in &edge_ids {
                let (v_start, v_end) = solid.vertices_of_edge(*eid);
                for vid in [v_start, v_end] {
                    let p = solid.vertices[vid].point;
                    let t = (p - *origin).dot(axis_n);
                    min_t = min_t.min(t);
                    max_t = max_t.max(t);
                }
            }
            let depth = (max_t - min_t).abs();

            results.push(HoleFeature {
                diameter_mm: radius * 2.0,
                depth_mm: depth,
                face_id: fid,
            });
        }
    }

    results
}

/// Analyze draft angles relative to a pull direction (typically Z).
/// Draft angle = 90 - angle between face normal and pull direction.
/// Vertical faces (normal perpendicular to pull) have 0 draft.
pub fn analyze_draft_angles(solid: &Solid, pull_direction: DVec3) -> Vec<DraftMeasurement> {
    let pull = pull_direction.normalize();
    let mut results = Vec::new();

    for (fid, face) in &solid.faces {
        let normal = match &face.surface {
            Surface::Plane { normal, .. } => normal.normalize(),
            _ => continue,
        };

        // cos(angle_between_normal_and_pull) gives how aligned the normal is with pull.
        // A face perpendicular to pull (top/bottom) has cos = +/-1, draft = 90.
        // A face parallel to pull (vertical wall) has cos = 0, draft = 0.
        let cos_angle = normal.dot(pull).abs();
        let draft_deg = cos_angle.asin().to_degrees();

        results.push(DraftMeasurement {
            angle_deg: draft_deg,
            face_id: fid,
        });
    }

    results
}

/// Analyze overhangs relative to a build direction (AM).
/// Overhang angle = angle of face normal from the build-up direction.
/// Faces pointing downward beyond a threshold need support.
pub fn analyze_overhangs(solid: &Solid, build_direction: DVec3) -> Vec<OverhangFace> {
    let build = build_direction.normalize();
    let mut results = Vec::new();

    for (fid, face) in &solid.faces {
        let normal = match &face.surface {
            Surface::Plane { normal, .. } => normal.normalize(),
            _ => continue,
        };

        // Dot product with build direction:
        // +1 = facing up (no overhang)
        //  0 = vertical (90 deg from vertical = horizontal overhang)
        // -1 = facing down (180 deg overhang, full downward)
        let cos_angle = normal.dot(build);

        // Angle from vertical measured as angle between normal and build direction
        let angle_from_vertical = cos_angle.acos().to_degrees();

        // Only report faces that point downward (angle > 90)
        if angle_from_vertical > 90.0 {
            results.push(OverhangFace {
                angle_from_vertical_deg: angle_from_vertical,
                face_id: fid,
            });
        }
    }

    results
}

/// Detect sharp internal corners from dihedral angles at edges.
/// A concave (internal) edge has dihedral angle > PI; the effective internal
/// radius is 0 for a sharp edge.
pub fn analyze_corner_radii(solid: &Solid) -> Vec<CornerRadius> {
    let mut results = Vec::new();

    for eid in solid.edge_ids() {
        if let Some(dihedral) = solid.dihedral_angle(eid) {
            let midpoint = solid.edge_midpoint(eid);
            // dihedral < PI means convex (external corner)
            // dihedral > PI means concave (internal corner)
            let is_concave = dihedral > std::f64::consts::PI + 1e-6;

            // For sharp edges (both convex and concave), radius is 0.
            // For filleted edges, the edge curve would be an arc and we could
            // read the radius, but for lines it is effectively 0.
            let edge = &solid.edges[eid];
            let radius = match &edge.curve {
                physical_brep::Curve::Arc { radius, .. } => *radius,
                physical_brep::Curve::Circle { radius, .. } => *radius,
                _ => 0.0,
            };

            if is_concave {
                results.push(CornerRadius {
                    radius_mm: radius,
                    location: midpoint,
                    is_concave,
                });
            }
        }
    }

    results
}

// ---------------------------------------------------------------------------
// Main entry point
// ---------------------------------------------------------------------------

/// Run full DFM validation on a solid for a given manufacturing process and material.
///
/// Looks up the manufacturing constraint from the LUT and compares measured
/// geometry values against the constraint limits.
pub fn check_dfm(
    solid: &Solid,
    process: Process,
    material_class: MaterialClass,
) -> DfmReport {
    let constraint = match manufacturing::lookup(process, material_class) {
        Some(c) => c,
        None => {
            return DfmReport {
                checks: vec![],
                pass: false,
                summary: format!(
                    "No manufacturing constraint found for {:?} / {:?}",
                    process, material_class
                ),
                score: 0.0,
            };
        }
    };

    check_dfm_with_constraint(solid, constraint)
}

/// Run full DFM validation using an explicit constraint (useful for testing
/// or custom constraint overrides).
pub fn check_dfm_with_constraint(
    solid: &Solid,
    constraint: &ManufacturingConstraint,
) -> DfmReport {
    let mut checks = Vec::new();

    let min_wall = constraint.min_wall_thickness.to_mm();
    let min_hole = constraint.min_hole_diameter.to_mm();
    let min_corner = constraint.min_corner_radius.to_mm();
    let min_draft = constraint.draft_angle_min.to_deg();
    let max_aspect = constraint.max_aspect_ratio;

    let is_am = matches!(
        constraint.process,
        Process::Fdm | Process::Fdm02 | Process::Fdm04 | Process::Fdm06 | Process::Fdm08
            | Process::Sla | Process::Dlp | Process::Sls | Process::Mjf | Process::Dmls
    );

    // --- Wall thickness ---
    let walls = analyze_wall_thickness(solid);
    for w in &walls {
        if min_wall > 0.0 && w.thickness_mm < min_wall {
            checks.push(DfmCheck::WallTooThin {
                measured_mm: w.thickness_mm,
                min_mm: min_wall,
                location: w.location,
            });
        }
    }

    // --- Holes ---
    let holes = analyze_holes(solid);
    for h in &holes {
        if min_hole > 0.0 && h.diameter_mm < min_hole {
            checks.push(DfmCheck::HoleTooSmall {
                diameter_mm: h.diameter_mm,
                min_mm: min_hole,
            });
        }
    }

    // --- Draft angles (for molding/casting processes) ---
    if min_draft > 0.0 {
        let pull_dir = DVec3::Z;
        let drafts = analyze_draft_angles(solid, pull_dir);
        for d in &drafts {
            // Skip top/bottom faces (draft > 45 deg) -- they are parting plane faces
            if d.angle_deg > 45.0 {
                continue;
            }
            if d.angle_deg < min_draft {
                checks.push(DfmCheck::DraftAngleInsufficient {
                    measured_deg: d.angle_deg,
                    min_deg: min_draft,
                    face_id: d.face_id,
                });
            }
        }
    }

    // --- Aspect ratio ---
    if max_aspect > 0.0 {
        let (min_bb, max_bb) = solid.bounding_box();
        let size = max_bb - min_bb;
        let dims = [size.x, size.y, size.z];
        let max_dim = dims.iter().cloned().fold(0.0_f64, f64::max);
        let min_dim = dims.iter().cloned().filter(|&d| d > 1e-6).fold(f64::MAX, f64::min);
        if min_dim < f64::MAX && min_dim > 0.0 {
            let ratio = max_dim / min_dim;
            if ratio > max_aspect {
                checks.push(DfmCheck::AspectRatioExceeded {
                    ratio,
                    max_ratio: max_aspect,
                });
            }
        }
    }

    // --- Corner radii ---
    if min_corner > 0.0 {
        let corners = analyze_corner_radii(solid);
        for c in &corners {
            if c.radius_mm < min_corner {
                checks.push(DfmCheck::CornerRadiusTooSmall {
                    radius_mm: c.radius_mm,
                    min_mm: min_corner,
                });
            }
        }
    }

    // --- Overhangs (AM only) ---
    if is_am {
        let build_dir = DVec3::Y; // typical build direction: up
        let overhangs = analyze_overhangs(solid, build_dir);
        // AM overhang threshold: faces beyond 135 deg from build direction
        // (i.e. more than 45 deg past horizontal)
        let max_overhang_deg = 135.0;
        for o in &overhangs {
            if o.angle_from_vertical_deg > max_overhang_deg {
                checks.push(DfmCheck::OverhangTooSteep {
                    angle_deg: o.angle_from_vertical_deg,
                    max_deg: max_overhang_deg,
                });
            }
        }
    }

    // --- Undercut detection (for mold/cast) ---
    if matches!(
        constraint.process,
        Process::InjectionMold | Process::DieCasting | Process::InvestmentCast
    ) {
        let pull_dir = DVec3::Z;
        for (fid, face) in &solid.faces {
            let normal = match &face.surface {
                Surface::Plane { normal, .. } => normal.normalize(),
                Surface::Cylinder { axis, .. } => {
                    // A cylindrical face perpendicular to pull is an undercut
                    let a = axis.normalize();
                    if a.dot(pull_dir).abs() < 0.1 {
                        // Axis perpendicular to pull -- possible undercut
                        checks.push(DfmCheck::UndercutDetected { face_id: fid });
                    }
                    continue;
                }
                _ => continue,
            };

            // Face normal perpendicular to pull that would trap the mold
            let dot = normal.dot(pull_dir);
            // Faces with normals that point inward relative to the pull
            // direction on sides could indicate undercuts. We skip faces
            // that are top/bottom (dot ~ +/-1) or simple side walls.
            let _ = dot; // undercut detection for planes requires more topology
        }
    }

    // --- Compute score ---
    let error_count = checks.iter().filter(|c| c.severity() == Severity::Error).count();
    let warning_count = checks.iter().filter(|c| c.severity() == Severity::Warning).count();

    let score = (100.0 - (error_count as f64 * 20.0) - (warning_count as f64 * 5.0))
        .clamp(0.0, 100.0);

    let pass = error_count == 0;

    let summary = if pass && warning_count == 0 {
        format!(
            "Part passes DFM validation for {:?} / {:?} with no issues. Score: {score:.0}/100",
            constraint.process, constraint.material_class
        )
    } else if pass {
        format!(
            "Part passes DFM for {:?} / {:?} with {warning_count} warning(s). Score: {score:.0}/100",
            constraint.process, constraint.material_class
        )
    } else {
        format!(
            "Part FAILS DFM for {:?} / {:?}: {error_count} error(s), {warning_count} warning(s). Score: {score:.0}/100",
            constraint.process, constraint.material_class
        )
    };

    DfmReport {
        checks,
        pass,
        summary,
        score,
    }
}

// ---------------------------------------------------------------------------
// Legacy API (kept for backward compatibility)
// ---------------------------------------------------------------------------

/// Configuration describing the constraints of a manufacturing process.
#[derive(Debug, Clone)]
pub struct DfmConfig {
    /// Human-readable process name.
    pub process: String,
    /// Minimum wall thickness (mm).
    pub min_wall_thickness: f64,
    /// Minimum internal corner radius (mm).
    pub min_corner_radius: f64,
    /// Maximum part envelope [x, y, z] in mm.
    pub max_envelope: [f64; 3],
    /// Minimum draft angle in degrees (for injection molding).
    pub min_draft_angle: f64,
}

/// A single DFM validation issue (legacy type).
#[derive(Debug, Clone)]
pub struct DfmIssue {
    pub severity: Severity,
    pub message: String,
    pub category: String,
    pub location: DVec3,
}

/// Return a default DFM config for CNC milling.
pub fn cnc_config() -> DfmConfig {
    DfmConfig {
        process: "CNC Milling".into(),
        min_wall_thickness: 0.8,
        min_corner_radius: 0.5,
        max_envelope: [1000.0, 600.0, 500.0],
        min_draft_angle: 0.0,
    }
}

/// Return a default DFM config for injection molding.
pub fn injection_mold_config() -> DfmConfig {
    DfmConfig {
        process: "Injection Molding".into(),
        min_wall_thickness: 1.0,
        min_corner_radius: 0.5,
        max_envelope: [500.0, 500.0, 300.0],
        min_draft_angle: 1.0,
    }
}

/// Validate a solid against a DFM configuration (legacy API).
///
/// Returns a list of issues found. An empty list means the part passes.
pub fn validate(solid: &Solid, config: &DfmConfig) -> Vec<DfmIssue> {
    let mut issues = Vec::new();

    let (min, max) = solid.bounding_box();
    let size = max - min;
    let center = (min + max) * 0.5;

    // Check envelope
    if size.x > config.max_envelope[0]
        || size.y > config.max_envelope[1]
        || size.z > config.max_envelope[2]
    {
        issues.push(DfmIssue {
            severity: Severity::Error,
            message: format!(
                "Part size ({:.1} x {:.1} x {:.1} mm) exceeds {} envelope ({:.0} x {:.0} x {:.0} mm)",
                size.x, size.y, size.z,
                config.process,
                config.max_envelope[0], config.max_envelope[1], config.max_envelope[2],
            ),
            category: "Envelope".into(),
            location: center,
        });
    }

    // Check thin walls: if any dimension is below min_wall_thickness
    let dims = [size.x, size.y, size.z];
    for (i, &d) in dims.iter().enumerate() {
        if d < config.min_wall_thickness && d > 0.0 {
            issues.push(DfmIssue {
                severity: Severity::Error,
                message: format!(
                    "Wall thickness {:.2}mm on axis {} is below minimum {:.1}mm for {}",
                    d, ["X", "Y", "Z"][i], config.min_wall_thickness, config.process
                ),
                category: "Thin wall".into(),
                location: center,
            });
        }
    }

    // Check draft angle for injection molding
    if config.min_draft_angle > 0.0 {
        let pull_dir = DVec3::Z;

        for (_, face) in &solid.faces {
            let face_normal = match &face.surface {
                Surface::Plane { normal, .. } => *normal,
                _ => continue,
            };

            let normal = face_normal.normalize();
            let cos_angle = normal.dot(pull_dir).abs();
            let draft_deg = cos_angle.asin().to_degrees();

            if draft_deg < config.min_draft_angle {
                let face_loc = match &face.surface {
                    Surface::Plane { origin, .. } => *origin,
                    _ => center,
                };
                issues.push(DfmIssue {
                    severity: Severity::Warning,
                    message: format!(
                        "Face has {:.1}\u{00b0} draft angle, below minimum {:.1}\u{00b0} for {}",
                        draft_deg, config.min_draft_angle, config.process
                    ),
                    category: "Insufficient draft".into(),
                    location: face_loc,
                });
            }
        }
    }

    issues
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use physical_brep::{make_box, make_cylinder};
    use physical_lut::manufacturing::{Process, MaterialClass};

    /// Helper: create a ManufacturingConstraint for CNC mill / aluminum.
    fn cnc_aluminum() -> ManufacturingConstraint {
        *manufacturing::lookup(Process::CncMill3Ax, MaterialClass::Aluminum).unwrap()
    }

    /// Helper: create a ManufacturingConstraint for injection molding / plastic.
    fn injection_plastic() -> ManufacturingConstraint {
        *manufacturing::lookup(Process::InjectionMold, MaterialClass::Plastic).unwrap()
    }

    /// Helper: create a ManufacturingConstraint for FDM 0.4mm / PLA.
    fn fdm_pla() -> ManufacturingConstraint {
        *manufacturing::lookup(Process::Fdm04, MaterialClass::Pla).unwrap()
    }

    #[test]
    fn box_passes_cnc_check() {
        // A 50x50x50 box should pass CNC mill check with no issues
        let solid = make_box(50.0, 50.0, 50.0);
        let constraint = cnc_aluminum();
        let report = check_dfm_with_constraint(&solid, &constraint);

        assert!(report.pass, "Simple box should pass CNC: {}", report.summary);
        let errors: Vec<_> = report.checks.iter().filter(|c| c.severity() == Severity::Error).collect();
        assert!(errors.is_empty(), "No errors expected for simple box: {:?}", errors);
        assert!(report.score > 50.0, "Score should be reasonable: {}", report.score);
    }

    #[test]
    fn thin_wall_triggers_error() {
        // A very thin box: 50 wide, 50 deep, but only 0.3mm tall
        // Wall thickness along Y should be 0.3mm, below CNC min of 1.0mm
        let solid = make_box(50.0, 0.3, 50.0);
        let constraint = cnc_aluminum();
        let report = check_dfm_with_constraint(&solid, &constraint);

        let thin_walls: Vec<_> = report.checks.iter().filter(|c| {
            matches!(c, DfmCheck::WallTooThin { .. })
        }).collect();

        assert!(!thin_walls.is_empty(), "Should detect thin wall: {:?}", report.checks);
        assert!(!report.pass, "Thin-walled part should fail: {}", report.summary);
    }

    #[test]
    fn small_hole_triggers_error() {
        // Create a cylinder with a very small radius (diameter 0.5mm)
        // CNC aluminum min hole is 1.0mm
        let solid = make_cylinder(0.25, 10.0, 16);
        let constraint = cnc_aluminum();

        let holes = analyze_holes(&solid);
        assert!(!holes.is_empty(), "Should detect cylindrical hole features");

        let report = check_dfm_with_constraint(&solid, &constraint);
        let small_holes: Vec<_> = report.checks.iter().filter(|c| {
            matches!(c, DfmCheck::HoleTooSmall { .. })
        }).collect();

        assert!(!small_holes.is_empty(), "Should detect small hole: {:?}", report.checks);
    }

    #[test]
    fn no_draft_triggers_warning_for_injection() {
        // A box with vertical walls (0 draft) should trigger draft warnings
        // for injection molding
        let solid = make_box(50.0, 50.0, 50.0);
        let constraint = injection_plastic();
        let report = check_dfm_with_constraint(&solid, &constraint);

        let draft_issues: Vec<_> = report.checks.iter().filter(|c| {
            matches!(c, DfmCheck::DraftAngleInsufficient { .. })
        }).collect();

        assert!(
            !draft_issues.is_empty(),
            "Box should have insufficient draft for injection molding: {:?}",
            report.checks
        );
    }

    #[test]
    fn overhang_detection_for_am() {
        // Build a box -- the bottom face (-Y) faces directly down, which is
        // a severe overhang for FDM printing with build direction +Y
        let solid = make_box(50.0, 50.0, 50.0);
        let constraint = fdm_pla();
        let report = check_dfm_with_constraint(&solid, &constraint);

        let overhangs: Vec<_> = report.checks.iter().filter(|c| {
            matches!(c, DfmCheck::OverhangTooSteep { .. })
        }).collect();

        assert!(
            !overhangs.is_empty(),
            "Bottom face of box should be detected as overhang for AM: {:?}",
            report.checks
        );
    }

    #[test]
    fn simple_box_scores_higher_than_complex() {
        let simple = make_box(50.0, 50.0, 50.0);
        let constraint = cnc_aluminum();
        let simple_report = check_dfm_with_constraint(&simple, &constraint);

        // "Complex" part: thin wall + small holes
        let complex = make_box(50.0, 0.3, 50.0);
        let complex_report = check_dfm_with_constraint(&complex, &constraint);

        assert!(
            simple_report.score > complex_report.score,
            "Simple box ({}) should score higher than thin-walled part ({})",
            simple_report.score,
            complex_report.score
        );
    }

    #[test]
    fn wall_thickness_analysis_basic() {
        let solid = make_box(10.0, 5.0, 20.0);
        let walls = analyze_wall_thickness(&solid);
        assert!(!walls.is_empty(), "Should measure wall thicknesses");

        // The thinnest wall should be 5.0mm (Y dimension)
        let min_wall = walls.iter().map(|w| w.thickness_mm).fold(f64::MAX, f64::min);
        assert!(
            (min_wall - 5.0).abs() < 0.1,
            "Minimum wall thickness should be ~5.0mm, got {min_wall}"
        );
    }

    #[test]
    fn draft_angle_analysis() {
        let solid = make_box(10.0, 10.0, 10.0);
        let drafts = analyze_draft_angles(&solid, DVec3::Z);
        assert_eq!(drafts.len(), 6, "Box has 6 planar faces");

        // Vertical walls (normals along X, Y) should have 0 draft
        let zero_draft = drafts.iter().filter(|d| d.angle_deg.abs() < 0.1).count();
        assert_eq!(zero_draft, 4, "4 vertical walls should have ~0 draft angle");

        // Top/bottom faces (normals along Z) should have 90 draft
        let full_draft = drafts.iter().filter(|d| (d.angle_deg - 90.0).abs() < 0.1).count();
        assert_eq!(full_draft, 2, "2 horizontal faces should have ~90 draft angle");
    }

    #[test]
    fn hole_detection() {
        let solid = make_cylinder(5.0, 20.0, 16);
        let holes = analyze_holes(&solid);
        assert!(!holes.is_empty(), "Should detect cylindrical faces as holes");
        assert!(
            (holes[0].diameter_mm - 10.0).abs() < 0.1,
            "Hole diameter should be ~10.0mm, got {}",
            holes[0].diameter_mm
        );
    }

    #[test]
    fn corner_radius_detection() {
        let solid = make_box(10.0, 10.0, 10.0);
        let corners = analyze_corner_radii(&solid);
        // A box has 12 edges. Internal concave corners depend on orientation;
        // for a convex box, dihedral angles are < PI (all convex), so no
        // concave corners are reported.
        // The box is convex, so no concave corners.
        assert!(
            corners.is_empty(),
            "Convex box should have no concave internal corners"
        );
    }

    #[test]
    fn lut_driven_check() {
        // Verify we can drive the checker from a real LUT lookup
        let solid = make_box(50.0, 50.0, 50.0);
        let report = check_dfm(
            &solid,
            Process::CncMill3Ax,
            MaterialClass::Aluminum,
        );
        assert!(report.pass, "50mm box should pass CNC aluminum: {}", report.summary);
    }
}
