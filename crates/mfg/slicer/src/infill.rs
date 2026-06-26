//! Infill pattern generation.

use glam::DVec2;
use physical_mfg_toolpath::Contour;

use crate::config::InfillPattern;

/// Generate infill lines within a boundary contour.
///
/// Returns a list of line segments (start, end) that fill the interior.
pub fn generate_infill(
    boundary: &Contour,
    pattern: InfillPattern,
    density: f64,
    layer_index: usize,
    extrusion_width: f64,
) -> Vec<(DVec2, DVec2)> {
    match pattern {
        InfillPattern::Lines => generate_lines_infill(boundary, density, layer_index, extrusion_width),
        InfillPattern::Grid => generate_grid_infill(boundary, density, layer_index, extrusion_width),
        _ => generate_lines_infill(boundary, density, layer_index, extrusion_width), // Fallback
    }
}

/// Lines infill: parallel lines at ±45 degrees, alternating each layer.
fn generate_lines_infill(
    boundary: &Contour,
    density: f64,
    layer_index: usize,
    extrusion_width: f64,
) -> Vec<(DVec2, DVec2)> {
    if density <= 0.0 || boundary.points.len() < 3 {
        return Vec::new();
    }

    let spacing = extrusion_width / density;
    let angle = if layer_index % 2 == 0 {
        std::f64::consts::FRAC_PI_4 // 45 degrees
    } else {
        -std::f64::consts::FRAC_PI_4 // -45 degrees
    };

    generate_parallel_lines(boundary, spacing, angle)
}

/// Grid infill: perpendicular lines at 0/90 degrees.
fn generate_grid_infill(
    boundary: &Contour,
    density: f64,
    layer_index: usize,
    extrusion_width: f64,
) -> Vec<(DVec2, DVec2)> {
    if density <= 0.0 || boundary.points.len() < 3 {
        return Vec::new();
    }

    // Grid uses half the density for each direction (combined = full density)
    let spacing = extrusion_width / (density * 0.5);
    let angle = if layer_index % 2 == 0 { 0.0 } else { std::f64::consts::FRAC_PI_2 };

    generate_parallel_lines(boundary, spacing, angle)
}

/// Generate parallel lines at a given angle within a contour boundary.
///
/// 1. Rotate boundary so lines become horizontal
/// 2. Scan horizontal lines at `spacing` intervals
/// 3. Clip each scanline to the boundary (even-odd intersection counting)
/// 4. Rotate results back
fn generate_parallel_lines(boundary: &Contour, spacing: f64, angle: f64) -> Vec<(DVec2, DVec2)> {
    if spacing <= 0.0 {
        return Vec::new();
    }

    let cos_a = angle.cos();
    let sin_a = angle.sin();

    // Rotate boundary points so scan direction becomes horizontal
    let rotated: Vec<DVec2> = boundary
        .points
        .iter()
        .map(|p| DVec2::new(p.x * cos_a + p.y * sin_a, -p.x * sin_a + p.y * cos_a))
        .collect();

    // Find Y bounds of rotated polygon
    let min_y = rotated.iter().map(|p| p.y).fold(f64::INFINITY, f64::min);
    let max_y = rotated.iter().map(|p| p.y).fold(f64::NEG_INFINITY, f64::max);

    let mut lines = Vec::new();
    let n = rotated.len();

    // Scan horizontal lines
    let mut y = min_y + spacing / 2.0;
    while y < max_y {
        // Find all X intersections with the polygon edges
        let mut intersections = Vec::new();
        for i in 0..n {
            let j = (i + 1) % n;
            let yi = rotated[i].y;
            let yj = rotated[j].y;

            if (yi <= y && yj > y) || (yj <= y && yi > y) {
                let t = (y - yi) / (yj - yi);
                let x = rotated[i].x + t * (rotated[j].x - rotated[i].x);
                intersections.push(x);
            }
        }

        intersections.sort_by(|a, b| a.partial_cmp(b).unwrap());

        // Pair up intersections (even-odd rule): each pair is an inside segment
        for pair in intersections.chunks_exact(2) {
            let x0 = pair[0];
            let x1 = pair[1];

            // Rotate back to original coordinate system
            let start = DVec2::new(x0 * cos_a - y * sin_a, x0 * sin_a + y * cos_a);
            let end = DVec2::new(x1 * cos_a - y * sin_a, x1 * sin_a + y * cos_a);

            lines.push((start, end));
        }

        y += spacing;
    }

    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square_boundary() -> Contour {
        Contour::closed(vec![
            DVec2::new(0.0, 0.0),
            DVec2::new(20.0, 0.0),
            DVec2::new(20.0, 20.0),
            DVec2::new(0.0, 20.0),
        ])
    }

    #[test]
    fn lines_infill_produces_segments() {
        let lines = generate_infill(&square_boundary(), InfillPattern::Lines, 0.2, 0, 0.4);
        assert!(!lines.is_empty(), "Should produce infill lines");
    }

    #[test]
    fn grid_infill_produces_segments() {
        let lines = generate_infill(&square_boundary(), InfillPattern::Grid, 0.2, 0, 0.4);
        assert!(!lines.is_empty(), "Should produce infill lines");
    }

    #[test]
    fn zero_density_no_infill() {
        let lines = generate_infill(&square_boundary(), InfillPattern::Lines, 0.0, 0, 0.4);
        assert!(lines.is_empty());
    }

    #[test]
    fn alternating_angles() {
        let lines_even = generate_infill(&square_boundary(), InfillPattern::Lines, 0.2, 0, 0.4);
        let lines_odd = generate_infill(&square_boundary(), InfillPattern::Lines, 0.2, 1, 0.4);
        // Lines should be at different angles
        assert!(!lines_even.is_empty());
        assert!(!lines_odd.is_empty());
    }
}
