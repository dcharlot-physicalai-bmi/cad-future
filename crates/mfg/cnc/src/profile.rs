//! 2D profile/contour cutting — machine the outline of a part.

use glam::DVec3;
use physical_mfg_toolpath::path::{MoveType, ToolpathSegment};
use physical_mfg_toolpath::Contour;

use crate::stock::compute_z_levels;

/// Side of the contour to cut on.
#[derive(Clone, Copy, Debug)]
pub enum CutSide {
    /// Tool center follows inside the contour (pocket-like).
    Inside,
    /// Tool center follows outside the contour (part profile).
    Outside,
    /// Tool center follows exactly on the contour (no offset).
    OnLine,
}

/// Generate profile cutting toolpath.
///
/// Cuts along a 2D contour at multiple Z levels, with the tool offset
/// to the specified side.
pub fn generate_profile(
    contour: &Contour,
    cut_side: CutSide,
    top_z: f64,
    bottom_z: f64,
    step_down: f64,
    tool_diameter: f64,
    feed_rate: f64,
    safe_z: f64,
) -> Vec<ToolpathSegment> {
    let z_levels = compute_z_levels(top_z, bottom_z, step_down);
    let tool_radius = tool_diameter / 2.0;

    // Compute offset contour
    let offset_dist = match cut_side {
        CutSide::Inside => -tool_radius,
        CutSide::Outside => tool_radius,
        CutSide::OnLine => 0.0,
    };

    let cut_contour = if offset_dist.abs() < 1e-8 {
        contour.clone()
    } else {
        match contour.offset(offset_dist) {
            Some(c) => c,
            None => return Vec::new(),
        }
    };

    let mut segments = Vec::new();

    for &z in &z_levels {
        if cut_contour.points.is_empty() {
            continue;
        }

        let start = cut_contour.points[0];

        // Rapid to start
        segments.push(ToolpathSegment::rapid(
            DVec3::new(start.x, start.y, safe_z),
            DVec3::new(start.x, start.y, safe_z),
        ));

        // Plunge
        segments.push(ToolpathSegment {
            path: vec![
                DVec3::new(start.x, start.y, safe_z),
                DVec3::new(start.x, start.y, z),
            ],
            feed_rate: feed_rate * 0.3,
            move_type: MoveType::Plunge,
        });

        // Cut along contour
        let mut path_3d: Vec<DVec3> = cut_contour
            .points
            .iter()
            .map(|p| DVec3::new(p.x, p.y, z))
            .collect();
        // Close the contour
        if cut_contour.is_closed && !cut_contour.points.is_empty() {
            path_3d.push(DVec3::new(
                cut_contour.points[0].x,
                cut_contour.points[0].y,
                z,
            ));
        }
        segments.push(ToolpathSegment::cut(path_3d, feed_rate));

        // Retract
        let last = cut_contour.points.last().unwrap_or(&start);
        segments.push(ToolpathSegment {
            path: vec![
                DVec3::new(last.x, last.y, z),
                DVec3::new(last.x, last.y, safe_z),
            ],
            feed_rate: 0.0,
            move_type: MoveType::Retract,
        });
    }

    segments
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square() -> Contour {
        use glam::DVec2;
        Contour::closed(vec![
            DVec2::new(0.0, 0.0),
            DVec2::new(40.0, 0.0),
            DVec2::new(40.0, 40.0),
            DVec2::new(0.0, 40.0),
        ])
    }

    #[test]
    fn outside_profile() {
        let segments = generate_profile(&square(), CutSide::Outside, 10.0, 0.0, 2.0, 6.0, 1000.0, 15.0);
        assert!(!segments.is_empty());
        let cuts: Vec<_> = segments.iter().filter(|s| s.move_type == MoveType::Cut).collect();
        assert_eq!(cuts.len(), 5, "5 Z-levels at 2mm step-down from 10 to 0");
    }

    #[test]
    fn inside_profile() {
        let segments = generate_profile(&square(), CutSide::Inside, 10.0, 5.0, 2.5, 6.0, 1000.0, 15.0);
        assert!(!segments.is_empty());
    }

    #[test]
    fn on_line_profile() {
        let segments = generate_profile(&square(), CutSide::OnLine, 10.0, 8.0, 2.0, 6.0, 1000.0, 15.0);
        assert!(!segments.is_empty());
    }
}
