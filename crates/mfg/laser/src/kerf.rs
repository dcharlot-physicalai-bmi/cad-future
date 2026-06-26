//! Kerf compensation — offset contours to account for laser beam width.

use physical_mfg_toolpath::Contour;

/// Apply kerf compensation to contours.
///
/// Outer contours are offset outward by half the kerf width (so the cut
/// line falls outside the desired part boundary).
/// Hole contours are offset inward by half the kerf width.
pub fn compensate_kerf(contours: &[Contour], kerf_width: f64) -> Vec<Contour> {
    if kerf_width <= 0.0 {
        return contours.to_vec();
    }

    let half_kerf = kerf_width / 2.0;
    let mut result = Vec::new();

    for contour in contours {
        let offset = if contour.is_outer() {
            half_kerf // Expand outer contours
        } else {
            -half_kerf // Shrink holes
        };

        if let Some(compensated) = contour.offset(offset) {
            result.push(compensated);
        }
        // If offset collapses the contour, skip it (feature too small for this kerf)
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::DVec2;

    #[test]
    fn kerf_expands_outer() {
        let outer = Contour::closed(vec![
            DVec2::new(0.0, 0.0),
            DVec2::new(20.0, 0.0),
            DVec2::new(20.0, 20.0),
            DVec2::new(0.0, 20.0),
        ]);
        let original_area = outer.signed_area();
        let compensated = compensate_kerf(&[outer], 0.2);
        assert_eq!(compensated.len(), 1);
        assert!(compensated[0].signed_area() > original_area);
    }

    #[test]
    fn zero_kerf_unchanged() {
        let outer = Contour::closed(vec![
            DVec2::new(0.0, 0.0),
            DVec2::new(10.0, 0.0),
            DVec2::new(10.0, 10.0),
            DVec2::new(0.0, 10.0),
        ]);
        let compensated = compensate_kerf(&[outer.clone()], 0.0);
        assert_eq!(compensated.len(), 1);
        assert!((compensated[0].signed_area() - outer.signed_area()).abs() < 1e-6);
    }
}
