//! Perimeter (wall) generation from slice contours.

use physical_mfg_toolpath::Contour;

/// Generate perimeter walls by offsetting contours inward.
///
/// Returns a list of wall loops, outermost first. Each wall is a contour
/// offset inward by `extrusion_width * wall_index`.
pub fn generate_perimeters(contour: &Contour, wall_count: usize, extrusion_width: f64) -> Vec<Contour> {
    let mut walls = Vec::new();

    for i in 0..wall_count {
        // Offset inward: first wall at half extrusion width (centerline on edge),
        // subsequent walls at full extrusion width spacing
        let offset = if i == 0 {
            -extrusion_width / 2.0
        } else {
            -(extrusion_width / 2.0 + extrusion_width * i as f64)
        };

        if let Some(wall) = contour.offset(offset) {
            walls.push(wall);
        } else {
            break; // Contour collapsed, no more room for walls
        }
    }

    walls
}

/// Get the innermost wall contour (used as the infill boundary).
pub fn infill_boundary(contour: &Contour, wall_count: usize, extrusion_width: f64) -> Option<Contour> {
    let offset = -(extrusion_width / 2.0 + extrusion_width * wall_count as f64);
    contour.offset(offset)
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::DVec2;

    fn big_square() -> Contour {
        Contour::closed(vec![
            DVec2::new(0.0, 0.0),
            DVec2::new(50.0, 0.0),
            DVec2::new(50.0, 50.0),
            DVec2::new(0.0, 50.0),
        ])
    }

    #[test]
    fn single_wall() {
        let walls = generate_perimeters(&big_square(), 1, 0.4);
        assert_eq!(walls.len(), 1);
        // Inner wall should be smaller
        assert!(walls[0].signed_area() < big_square().signed_area());
    }

    #[test]
    fn multiple_walls() {
        let walls = generate_perimeters(&big_square(), 3, 0.4);
        assert_eq!(walls.len(), 3);
        // Each successive wall should be smaller
        for i in 1..walls.len() {
            assert!(walls[i].signed_area() < walls[i - 1].signed_area());
        }
    }

    #[test]
    fn infill_boundary_inside_walls() {
        let boundary = infill_boundary(&big_square(), 2, 0.4);
        assert!(boundary.is_some());
        let b = boundary.unwrap();
        assert!(b.signed_area() < big_square().signed_area());
    }
}
