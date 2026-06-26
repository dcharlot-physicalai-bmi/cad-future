//! Contour tree — nested hierarchy of outer boundaries and holes.

use serde::{Deserialize, Serialize};

use crate::contour::Contour;

/// A nested contour hierarchy: an outer boundary containing holes,
/// which may themselves contain islands (children).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ContourTree {
    /// Outer boundary (CCW winding).
    pub outer: Contour,
    /// Holes within this boundary (CW winding).
    pub holes: Vec<Contour>,
    /// Islands inside holes (recursive).
    pub children: Vec<ContourTree>,
}

/// Build a contour tree from a flat list of contours.
///
/// Contours with positive signed area (CCW) are outer boundaries.
/// Contours with negative signed area (CW) are holes.
/// Holes are assigned to the smallest outer boundary that contains them.
/// Islands inside holes become children (recursive).
pub fn build_contour_tree(contours: &[Contour]) -> Vec<ContourTree> {
    if contours.is_empty() {
        return Vec::new();
    }

    // Separate outers and holes
    let mut outers: Vec<(usize, &Contour)> = Vec::new();
    let mut holes: Vec<(usize, &Contour)> = Vec::new();

    for (i, c) in contours.iter().enumerate() {
        if c.signed_area() > 0.0 {
            outers.push((i, c));
        } else {
            holes.push((i, c));
        }
    }

    // Sort outers by area (smallest first) for correct nesting assignment
    outers.sort_by(|a, b| a.1.signed_area().partial_cmp(&b.1.signed_area()).unwrap());

    // Assign each hole to its containing outer
    let mut hole_assignment: Vec<Vec<Contour>> = vec![Vec::new(); outers.len()];

    for (_hi, hole) in &holes {
        let hole_centroid = hole.centroid();
        // Find the smallest outer that contains this hole's centroid
        for (oi, (_, outer)) in outers.iter().enumerate() {
            if outer.contains(hole_centroid) {
                let mut h = (*hole).clone();
                h.ensure_cw();
                hole_assignment[oi].push(h);
                break;
            }
        }
    }

    // Build trees
    outers
        .iter()
        .enumerate()
        .map(|(oi, (_, outer))| {
            let mut o = (*outer).clone();
            o.ensure_ccw();
            ContourTree {
                outer: o,
                holes: hole_assignment[oi].clone(),
                children: Vec::new(), // TODO: recursive island detection
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::DVec2;

    #[test]
    fn single_outer() {
        let outer = Contour::closed(vec![
            DVec2::new(0.0, 0.0),
            DVec2::new(10.0, 0.0),
            DVec2::new(10.0, 10.0),
            DVec2::new(0.0, 10.0),
        ]);
        let trees = build_contour_tree(&[outer]);
        assert_eq!(trees.len(), 1);
        assert!(trees[0].holes.is_empty());
    }

    #[test]
    fn outer_with_hole() {
        let outer = Contour::closed(vec![
            DVec2::new(0.0, 0.0),
            DVec2::new(20.0, 0.0),
            DVec2::new(20.0, 20.0),
            DVec2::new(0.0, 20.0),
        ]);
        // CW hole inside
        let hole = Contour::closed(vec![
            DVec2::new(5.0, 5.0),
            DVec2::new(5.0, 15.0),
            DVec2::new(15.0, 15.0),
            DVec2::new(15.0, 5.0),
        ]);
        let trees = build_contour_tree(&[outer, hole]);
        assert_eq!(trees.len(), 1);
        assert_eq!(trees[0].holes.len(), 1);
    }
}
