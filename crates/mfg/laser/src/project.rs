//! Project 3D solid faces to 2D contours for laser cutting.

use glam::DVec2;
use physical_brep::Solid;
use physical_mfg_toolpath::contour::{chain_segments, Contour};
use physical_overlay::{generate_view, ViewDirection};

/// Project a solid's edges onto the XY plane (top-down view) as contours.
///
/// Uses the overlay crate's projection, then chains the projected edges
/// into closed contours suitable for laser cutting.
pub fn project_solid_top(solid: &Solid) -> Vec<Contour> {
    let view = generate_view(solid, ViewDirection::Top);

    let segments: Vec<(DVec2, DVec2)> = view
        .lines
        .iter()
        .map(|line| (line.start, line.end))
        .collect();

    chain_segments(&segments)
}

/// Project a solid's edges from the front (XZ plane) as contours.
pub fn project_solid_front(solid: &Solid) -> Vec<Contour> {
    let view = generate_view(solid, ViewDirection::Front);

    let segments: Vec<(DVec2, DVec2)> = view
        .lines
        .iter()
        .map(|line| (line.start, line.end))
        .collect();

    chain_segments(&segments)
}

/// Project a solid from any standard view direction as contours.
pub fn project_solid(solid: &Solid, direction: ViewDirection) -> Vec<Contour> {
    let view = generate_view(solid, direction);

    let segments: Vec<(DVec2, DVec2)> = view
        .lines
        .iter()
        .map(|line| (line.start, line.end))
        .collect();

    chain_segments(&segments)
}

#[cfg(test)]
mod tests {
    use super::*;
    use physical_brep::builder::make_box;

    #[test]
    fn project_box_top() {
        let b = make_box(20.0, 10.0, 15.0);
        let contours = project_solid_top(&b);
        // Box top view should produce at least one contour
        assert!(!contours.is_empty(), "Box projection should produce contours");
    }

    #[test]
    fn project_box_front() {
        let b = make_box(20.0, 10.0, 15.0);
        let contours = project_solid_front(&b);
        assert!(!contours.is_empty());
    }
}
