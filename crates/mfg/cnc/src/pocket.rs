//! 2D pocket clearing — remove material from an enclosed region.

use glam::{DVec2, DVec3};
use physical_mfg_toolpath::path::{MoveType, ToolpathSegment};
use physical_mfg_toolpath::Contour;

use crate::stock::compute_z_levels;

/// Pocket clearing strategy.
#[derive(Clone, Copy, Debug)]
pub enum PocketStrategy {
    /// Contour-parallel: offset contour inward repeatedly.
    ContourParallel,
    /// Zigzag: parallel lines clipped to boundary.
    Zigzag,
}

/// Generate pocket clearing toolpath.
///
/// Removes material inside the given 2D contour at the specified depth,
/// using multiple Z-level passes.
pub fn generate_pocket(
    contour: &Contour,
    top_z: f64,
    bottom_z: f64,
    step_down: f64,
    step_over: f64,
    tool_diameter: f64,
    feed_rate: f64,
    safe_z: f64,
    strategy: PocketStrategy,
) -> Vec<ToolpathSegment> {
    let z_levels = compute_z_levels(top_z, bottom_z, step_down);
    let mut segments = Vec::new();

    for &z in &z_levels {
        let passes = match strategy {
            PocketStrategy::ContourParallel => {
                contour_parallel_passes(contour, step_over, tool_diameter)
            }
            PocketStrategy::Zigzag => {
                zigzag_passes(contour, step_over, tool_diameter)
            }
        };

        for pass in &passes {
            if pass.is_empty() {
                continue;
            }

            // Rapid to start
            let start = pass[0];
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

            // Cut along pass
            let path_3d: Vec<DVec3> = pass.iter().map(|p| DVec3::new(p.x, p.y, z)).collect();
            segments.push(ToolpathSegment::cut(path_3d, feed_rate));

            // Retract
            let last = pass.last().unwrap();
            segments.push(ToolpathSegment {
                path: vec![DVec3::new(last.x, last.y, z), DVec3::new(last.x, last.y, safe_z)],
                feed_rate: 0.0,
                move_type: MoveType::Retract,
            });
        }
    }

    segments
}

/// Contour-parallel pocket clearing: offset the boundary inward repeatedly
/// until the pocket is fully cleared.
fn contour_parallel_passes(
    contour: &Contour,
    step_over: f64,
    tool_diameter: f64,
) -> Vec<Vec<DVec2>> {
    let mut passes = Vec::new();
    let tool_radius = tool_diameter / 2.0;

    // First pass: offset inward by tool radius (centerline on edge)
    let mut offset = -tool_radius;
    let max_passes = 1000; // Safety limit
    for _ in 0..max_passes {
        if let Some(c) = contour.offset(offset) {
            if c.signed_area().abs() < 1e-4 {
                break; // Too small to be useful
            }
            passes.push(c.points.clone());
            offset -= step_over;
        } else {
            break;
        }
    }

    passes
}

/// Zigzag pocket clearing: parallel lines clipped to the contour boundary.
fn zigzag_passes(contour: &Contour, step_over: f64, tool_diameter: f64) -> Vec<Vec<DVec2>> {
    let tool_radius = tool_diameter / 2.0;

    // Inset boundary by tool radius
    let boundary = match contour.offset(-tool_radius) {
        Some(b) => b,
        None => return Vec::new(),
    };

    let (min, max) = boundary.bounds();
    let mut passes = Vec::new();
    let mut y = min.y + step_over / 2.0;
    let mut forward = true;

    while y <= max.y {
        // Intersect horizontal scanline with boundary
        let intersections = scanline_intersections(&boundary, y);

        for pair in intersections.chunks_exact(2) {
            let x0 = pair[0];
            let x1 = pair[1];
            let pass = if forward {
                vec![DVec2::new(x0, y), DVec2::new(x1, y)]
            } else {
                vec![DVec2::new(x1, y), DVec2::new(x0, y)]
            };
            passes.push(pass);
        }

        y += step_over;
        forward = !forward;
    }

    passes
}

/// Find X intersections of a horizontal scanline at Y with a contour.
fn scanline_intersections(contour: &Contour, y: f64) -> Vec<f64> {
    let n = contour.points.len();
    let mut xs = Vec::new();

    for i in 0..n {
        let j = (i + 1) % n;
        let yi = contour.points[i].y;
        let yj = contour.points[j].y;

        if (yi <= y && yj > y) || (yj <= y && yi > y) {
            let t = (y - yi) / (yj - yi);
            let x = contour.points[i].x + t * (contour.points[j].x - contour.points[i].x);
            xs.push(x);
        }
    }

    xs.sort_by(|a, b| a.partial_cmp(b).unwrap());
    xs
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square_contour() -> Contour {
        Contour::closed(vec![
            DVec2::new(0.0, 0.0),
            DVec2::new(30.0, 0.0),
            DVec2::new(30.0, 30.0),
            DVec2::new(0.0, 30.0),
        ])
    }

    #[test]
    fn contour_parallel_pocket() {
        let segments = generate_pocket(
            &square_contour(),
            10.0,
            0.0,
            2.0,
            2.0,
            6.0,
            1000.0,
            15.0,
            PocketStrategy::ContourParallel,
        );
        assert!(!segments.is_empty());
        let cuts: Vec<_> = segments.iter().filter(|s| s.move_type == MoveType::Cut).collect();
        assert!(!cuts.is_empty(), "Should have cutting passes");
    }

    #[test]
    fn zigzag_pocket() {
        let segments = generate_pocket(
            &square_contour(),
            10.0,
            0.0,
            2.0,
            2.0,
            6.0,
            1000.0,
            15.0,
            PocketStrategy::Zigzag,
        );
        assert!(!segments.is_empty());
    }
}
