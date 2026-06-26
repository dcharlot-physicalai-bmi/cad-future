//! Cut ordering — optimize the order contours are cut to minimize travel
//! and ensure holes are cut before outer profiles.

use physical_mfg_toolpath::Contour;

/// Order contours for cutting: holes (inner) first, then outer profiles.
///
/// Within each group, contours are ordered by nearest-neighbor travel
/// to minimize rapid move distances.
pub fn order_contours(contours: &[Contour]) -> Vec<usize> {
    if contours.is_empty() {
        return Vec::new();
    }

    // Separate into holes and outers
    let mut holes: Vec<usize> = Vec::new();
    let mut outers: Vec<usize> = Vec::new();

    for (i, c) in contours.iter().enumerate() {
        if c.is_outer() {
            outers.push(i);
        } else {
            holes.push(i);
        }
    }

    // Order each group by nearest-neighbor
    let mut ordered = Vec::new();
    ordered.extend(nearest_neighbor_order(contours, &holes));
    ordered.extend(nearest_neighbor_order(contours, &outers));

    ordered
}

/// Nearest-neighbor ordering: start from the contour closest to origin,
/// then always move to the nearest unvisited contour.
fn nearest_neighbor_order(contours: &[Contour], indices: &[usize]) -> Vec<usize> {
    if indices.is_empty() {
        return Vec::new();
    }

    let mut remaining: Vec<usize> = indices.to_vec();
    let mut ordered = Vec::with_capacity(remaining.len());

    // Start with contour nearest to origin
    let mut current_pos = glam::DVec2::ZERO;

    while !remaining.is_empty() {
        let mut best_idx = 0;
        let mut best_dist = f64::MAX;

        for (i, &contour_idx) in remaining.iter().enumerate() {
            let centroid = contours[contour_idx].centroid();
            let dist = (centroid - current_pos).length();
            if dist < best_dist {
                best_dist = dist;
                best_idx = i;
            }
        }

        let chosen = remaining.remove(best_idx);
        current_pos = contours[chosen].centroid();
        ordered.push(chosen);
    }

    ordered
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::DVec2;

    #[test]
    fn holes_before_outers() {
        // CCW = outer, CW = hole
        let outer = Contour::closed(vec![
            DVec2::new(0.0, 0.0),
            DVec2::new(20.0, 0.0),
            DVec2::new(20.0, 20.0),
            DVec2::new(0.0, 20.0),
        ]);
        let hole = Contour::closed(vec![
            DVec2::new(5.0, 5.0),
            DVec2::new(5.0, 15.0),
            DVec2::new(15.0, 15.0),
            DVec2::new(15.0, 5.0),
        ]); // CW winding = hole

        let contours = vec![outer, hole];
        let order = order_contours(&contours);
        assert_eq!(order.len(), 2);
        // The hole (CW, index 1) should come before the outer (CCW, index 0)
        // But the hole is CW only if signed_area < 0. Let's check:
        // The hole as given is CW, so signed_area < 0, so it's a hole.
        assert!(!contours[1].is_outer());
        // First in order should be the hole
        assert_eq!(order[0], 1, "Hole should be cut first");
        assert_eq!(order[1], 0, "Outer should be cut second");
    }
}
