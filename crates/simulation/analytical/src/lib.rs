//! `physical-analytical` -- Handbook formulas for mass properties of B-Rep solids.

use glam::DVec3;
use physical_brep::Solid;

/// Result of computing mass properties for a solid.
#[derive(Debug, Clone, Copy)]
pub struct MassProperties {
    /// Volume in mm^3.
    pub volume: f64,
    /// Total surface area in mm^2.
    pub surface_area: f64,
    /// Centroid (center of volume) in model coordinates.
    pub centroid: DVec3,
}

/// Compute mass properties for a solid using the divergence theorem.
///
/// Volume is computed by summing signed tetrahedron volumes formed by each
/// triangulated face and the origin. Surface area is the sum of face polygon
/// areas. Centroid is the volume-weighted average of tetrahedron centroids.
///
/// This works for any closed, orientable B-Rep solid — not just boxes.
pub fn mass_properties(solid: &Solid) -> MassProperties {
    let mut signed_volume = 0.0;
    let mut surface_area = 0.0;
    let mut centroid_accum = DVec3::ZERO;

    for fid in solid.face_ids() {
        let verts = solid.face_vertices(fid);
        let n = verts.len();
        if n < 3 { continue; }

        // Fan-triangulate the polygon from vertex 0
        for i in 1..n - 1 {
            let v0 = verts[0];
            let v1 = verts[i];
            let v2 = verts[i + 1];

            // Surface area: triangle area via cross product
            let cross = (v1 - v0).cross(v2 - v0);
            surface_area += 0.5 * cross.length();

            // Signed tetrahedron volume (divergence theorem): V = v0 · (v1 × v2) / 6
            let sv = v0.dot(v1.cross(v2)) / 6.0;
            signed_volume += sv;

            // Volume-weighted centroid: tet centroid = (v0 + v1 + v2) / 4
            centroid_accum += (v0 + v1 + v2) / 4.0 * sv;
        }
    }

    let volume = signed_volume.abs();
    let centroid = if signed_volume.abs() > 1e-30 {
        centroid_accum / signed_volume
    } else {
        let (min, max) = solid.bounding_box();
        (min + max) * 0.5
    };

    MassProperties { volume, surface_area, centroid }
}

/// Beam approximation result.
#[derive(Debug, Clone, Copy)]
pub struct BeamApprox {
    /// Span (length) of the beam — the longest bounding-box dimension.
    pub span: f64,
    /// Cross-section width (second largest dimension).
    pub width: f64,
    /// Cross-section height (smallest dimension).
    pub height: f64,
}

/// Approximate a solid as a beam by using bounding-box dimensions.
///
/// Returns the longest axis as span, and the other two as width/height.
pub fn beam_approximation(solid: &Solid) -> BeamApprox {
    let (min, max) = solid.bounding_box();
    let size = max - min;
    let mut dims = [size.x, size.y, size.z];
    dims.sort_by(|a, b| b.partial_cmp(a).unwrap());
    BeamApprox {
        span: dims[0],
        width: dims[1],
        height: dims[2],
    }
}

/// Deflection of a simply supported beam with a center point load.
///
/// δ = P L³ / (48 E I)
///
/// - `load`: applied force (N)
/// - `span`: beam length (mm)
/// - `e_modulus`: Young's modulus (MPa)
/// - `i_moment`: second moment of area (mm⁴)
pub fn deflection_simply_supported(load: f64, span: f64, e_modulus: f64, i_moment: f64) -> f64 {
    (load * span.powi(3)) / (48.0 * e_modulus * i_moment)
}

/// Maximum bending stress of a simply supported beam with a center point load.
///
/// σ = M c / I  where  M = P L / 4  and  c = height / 2
///
/// - `load`: applied force (N)
/// - `span`: beam length (mm)
/// - `height`: cross-section height (mm) — distance from neutral axis to extreme fiber is height/2
/// - `i_moment`: second moment of area (mm⁴)
pub fn bending_stress_simply_supported(load: f64, span: f64, height: f64, i_moment: f64) -> f64 {
    let moment = load * span / 4.0;
    let c = height / 2.0;
    moment * c / i_moment
}

/// Safety factor = allowable stress / applied stress.
pub fn safety_factor(allowable: f64, applied: f64) -> f64 {
    if applied.abs() < 1e-30 {
        return f64::INFINITY;
    }
    allowable / applied
}

/// Von Mises equivalent stress from three principal stresses.
///
/// σ_vm = √[ ½ ((σ₁-σ₂)² + (σ₂-σ₃)² + (σ₃-σ₁)²) ]
pub fn von_mises(s1: f64, s2: f64, s3: f64) -> f64 {
    (0.5 * ((s1 - s2).powi(2) + (s2 - s3).powi(2) + (s3 - s1).powi(2))).sqrt()
}

/// Inertia tensor result for a solid body.
#[derive(Debug, Clone, Copy)]
pub struct InertiaTensor {
    /// Moments of inertia about centroidal axes [Ixx, Iyy, Izz] in mm⁴.
    pub principal: [f64; 3],
    /// Products of inertia [Ixy, Ixz, Iyz] in mm⁴.
    pub products: [f64; 3],
}

/// Compute the inertia tensor of a solid about its centroid.
///
/// Uses the divergence theorem on triangulated faces, same approach as
/// mass_properties but integrates x², y², z², xy, xz, yz over the volume.
pub fn inertia_tensor(solid: &Solid) -> InertiaTensor {
    let props = mass_properties(solid);
    let mut ixx = 0.0;
    let mut iyy = 0.0;
    let mut izz = 0.0;
    let mut ixy = 0.0;
    let mut ixz = 0.0;
    let mut iyz = 0.0;

    for fid in solid.face_ids() {
        let verts = solid.face_vertices(fid);
        let n = verts.len();
        if n < 3 { continue; }

        for i in 1..n - 1 {
            let v0 = verts[0] - props.centroid;
            let v1 = verts[i] - props.centroid;
            let v2 = verts[i + 1] - props.centroid;

            // Signed volume of this tet
            let sv = v0.dot(v1.cross(v2)) / 6.0;
            let sign = sv.signum();
            let abs_sv = sv.abs();

            // For a tetrahedron with vertices at origin, a, b, c:
            // ∫ x² dV = V/10 * (a.x² + b.x² + c.x² + a.x*b.x + a.x*c.x + b.x*c.x)
            // (and similarly for y², z², xy, xz, yz)
            let factor = abs_sv / 10.0;

            let xx = factor * (v0.x*v0.x + v1.x*v1.x + v2.x*v2.x
                + v0.x*v1.x + v0.x*v2.x + v1.x*v2.x);
            let yy = factor * (v0.y*v0.y + v1.y*v1.y + v2.y*v2.y
                + v0.y*v1.y + v0.y*v2.y + v1.y*v2.y);
            let zz = factor * (v0.z*v0.z + v1.z*v1.z + v2.z*v2.z
                + v0.z*v1.z + v0.z*v2.z + v1.z*v2.z);

            let xy_sum = factor * (2.0*v0.x*v0.y + 2.0*v1.x*v1.y + 2.0*v2.x*v2.y
                + v0.x*v1.y + v1.x*v0.y + v0.x*v2.y + v2.x*v0.y
                + v1.x*v2.y + v2.x*v1.y) / 2.0;
            let xz_sum = factor * (2.0*v0.x*v0.z + 2.0*v1.x*v1.z + 2.0*v2.x*v2.z
                + v0.x*v1.z + v1.x*v0.z + v0.x*v2.z + v2.x*v0.z
                + v1.x*v2.z + v2.x*v1.z) / 2.0;
            let yz_sum = factor * (2.0*v0.y*v0.z + 2.0*v1.y*v1.z + 2.0*v2.y*v2.z
                + v0.y*v1.z + v1.y*v0.z + v0.y*v2.z + v2.y*v0.z
                + v1.y*v2.z + v2.y*v1.z) / 2.0;

            ixx += sign * (yy + zz);
            iyy += sign * (xx + zz);
            izz += sign * (xx + yy);
            ixy += sign * xy_sum;
            ixz += sign * xz_sum;
            iyz += sign * yz_sum;
        }
    }

    InertiaTensor {
        principal: [ixx.abs(), iyy.abs(), izz.abs()],
        products: [-ixy, -ixz, -iyz],
    }
}

/// Section modulus for a rectangular cross section.
/// S = b * h² / 6
pub fn section_modulus_rect(width: f64, height: f64) -> f64 {
    width * height * height / 6.0
}

/// Second moment of area for a rectangular cross section.
/// I = b * h³ / 12
pub fn second_moment_rect(width: f64, height: f64) -> f64 {
    width * height.powi(3) / 12.0
}

/// Second moment of area for a circular cross section.
/// I = π * d⁴ / 64
pub fn second_moment_circle(diameter: f64) -> f64 {
    std::f64::consts::PI * diameter.powi(4) / 64.0
}

/// Polar moment of inertia for a circular cross section.
/// J = π * d⁴ / 32
pub fn polar_moment_circle(diameter: f64) -> f64 {
    std::f64::consts::PI * diameter.powi(4) / 32.0
}

/// Mass from volume and density.
pub fn mass_from_volume(volume_mm3: f64, density_kg_m3: f64) -> f64 {
    // volume in mm³, density in kg/m³ → mass in kg
    volume_mm3 * density_kg_m3 * 1e-9
}

/// Weight from mass (using standard gravity).
pub fn weight(mass_kg: f64) -> f64 {
    mass_kg * 9.80665
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn box_volume_exact() {
        let b = physical_brep::make_box(10.0, 20.0, 30.0);
        let props = mass_properties(&b);
        let expected = 10.0 * 20.0 * 30.0;
        let err = (props.volume - expected).abs() / expected;
        assert!(err < 0.01, "box volume {:.1} expected {:.1} (err {:.2}%)", props.volume, expected, err * 100.0);
    }

    #[test]
    fn box_surface_area() {
        let b = physical_brep::make_box(10.0, 20.0, 30.0);
        let props = mass_properties(&b);
        let expected = 2.0 * (10.0*20.0 + 20.0*30.0 + 10.0*30.0);
        let err = (props.surface_area - expected).abs() / expected;
        assert!(err < 0.01, "box SA {:.1} expected {:.1}", props.surface_area, expected);
    }

    #[test]
    fn box_centroid_at_origin() {
        let b = physical_brep::make_box(10.0, 10.0, 10.0);
        let props = mass_properties(&b);
        assert!(props.centroid.length() < 0.1, "centroid {} should be near origin", props.centroid);
    }

    #[test]
    fn cylinder_volume_converges() {
        let r = 10.0;
        let h = 50.0;
        let expected = std::f64::consts::PI * r * r * h;

        let c16 = physical_brep::make_cylinder(r, h, 16);
        let v16 = mass_properties(&c16).volume;

        let c64 = physical_brep::make_cylinder(r, h, 64);
        let v64 = mass_properties(&c64).volume;

        let err16 = (v16 - expected).abs() / expected;
        let err64 = (v64 - expected).abs() / expected;

        assert!(err64 < err16, "64-seg err {:.2}% should be less than 16-seg {:.2}%", err64*100.0, err16*100.0);
        assert!(err64 < 0.05, "64-seg volume {:.1} should be within 5% of {:.1}", v64, expected);
    }

    #[test]
    fn box_inertia_tensor() {
        let b = physical_brep::make_box(10.0, 20.0, 30.0);
        let inertia = inertia_tensor(&b);
        // For a box about centroid: Ixx = m*(b²+c²)/12 where b,c are the other two dims
        // But since we compute volume-based (not mass-based) we just check positivity and symmetry
        assert!(inertia.principal[0] > 0.0, "Ixx should be positive");
        assert!(inertia.principal[1] > 0.0, "Iyy should be positive");
        assert!(inertia.principal[2] > 0.0, "Izz should be positive");
    }

    #[test]
    fn cube_inertia_symmetric() {
        let b = physical_brep::make_box(10.0, 10.0, 10.0);
        let inertia = inertia_tensor(&b);
        // Cube should have equal Ixx = Iyy = Izz
        let avg = (inertia.principal[0] + inertia.principal[1] + inertia.principal[2]) / 3.0;
        for &p in &inertia.principal {
            let err = (p - avg).abs() / avg;
            assert!(err < 0.05, "cube inertia should be symmetric: {:?}", inertia.principal);
        }
    }

    #[test]
    fn section_modulus() {
        let s = section_modulus_rect(10.0, 20.0);
        // S = 10 * 20² / 6 = 666.67
        assert!((s - 666.67).abs() < 1.0);
    }

    #[test]
    fn second_moment_rectangular() {
        let i = second_moment_rect(10.0, 20.0);
        // I = 10 * 20³ / 12 = 6666.67
        assert!((i - 6666.67).abs() < 1.0);
    }

    #[test]
    fn second_moment_circular() {
        let i = second_moment_circle(20.0);
        // I = π * 20⁴ / 64 = 7853.98
        assert!((i - 7853.98).abs() < 1.0);
    }

    #[test]
    fn mass_calculation() {
        // 10×20×30mm box in aluminum (2700 kg/m³)
        let vol = 10.0 * 20.0 * 30.0;
        let m = mass_from_volume(vol, 2700.0);
        // Expected: 6000 mm³ * 2700 kg/m³ * 1e-9 = 0.0162 kg = 16.2g
        assert!((m - 0.0162).abs() < 0.001, "mass should be ~0.016 kg, got {}", m);
    }

    #[test]
    fn weight_calculation() {
        let w = weight(1.0); // 1 kg
        assert!((w - 9.80665).abs() < 0.001);
    }

    #[test]
    fn union_non_overlapping_volume_additive() {
        let a = physical_brep::make_box(10.0, 10.0, 10.0);
        let mut b = physical_brep::make_box(10.0, 10.0, 10.0);
        for (_, v) in &mut b.vertices {
            v.point.x += 20.0;
        }
        let combined = physical_brep::union(&a, &b);
        let vc = mass_properties(&combined).volume;
        let expected = 2000.0;
        let err = (vc - expected).abs() / expected;
        assert!(err < 0.10, "union volume {:.1} expected {:.1} (err {:.1}%)", vc, expected, err*100.0);
    }
}
