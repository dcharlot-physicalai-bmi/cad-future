//! Facing operation — machine the top surface of stock flat.

use glam::DVec3;
use physical_mfg_toolpath::path::{MoveType, ToolpathSegment};

use crate::stock::Stock;

/// Generate facing toolpath: zigzag passes across the stock top.
///
/// Starts from one corner, moves in X direction, steps over in Y,
/// reverses X direction, repeats until the entire top is covered.
pub fn generate_facing(
    stock: &Stock,
    target_z: f64,
    step_over: f64,
    feed_rate: f64,
    safe_z: f64,
) -> Vec<ToolpathSegment> {
    let mut segments = Vec::new();

    let tool_radius = step_over / 2.0;
    let x_min = stock.min.x - tool_radius;
    let x_max = stock.max.x + tool_radius;
    let y_start = stock.min.y - tool_radius;
    let y_end = stock.max.y + tool_radius;

    let mut y = y_start;
    let mut forward = true;

    while y <= y_end {
        let (x0, x1) = if forward {
            (x_min, x_max)
        } else {
            (x_max, x_min)
        };

        // Rapid to start of pass
        segments.push(ToolpathSegment::rapid(
            DVec3::new(x0, y, safe_z),
            DVec3::new(x0, y, safe_z),
        ));

        // Plunge to cutting depth
        segments.push(ToolpathSegment {
            path: vec![DVec3::new(x0, y, safe_z), DVec3::new(x0, y, target_z)],
            feed_rate: feed_rate * 0.5, // Plunge at half feed
            move_type: MoveType::Plunge,
        });

        // Cut across
        segments.push(ToolpathSegment::cut(
            vec![DVec3::new(x0, y, target_z), DVec3::new(x1, y, target_z)],
            feed_rate,
        ));

        // Retract
        segments.push(ToolpathSegment {
            path: vec![DVec3::new(x1, y, target_z), DVec3::new(x1, y, safe_z)],
            feed_rate: 0.0,
            move_type: MoveType::Retract,
        });

        y += step_over;
        forward = !forward;
    }

    segments
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn facing_generates_passes() {
        let stock = Stock::block(DVec3::ZERO, DVec3::new(50.0, 30.0, 10.0));
        let segments = generate_facing(&stock, 9.5, 3.0, 1000.0, 15.0);
        assert!(!segments.is_empty());

        // Should have rapid, plunge, cut, retract for each pass
        let cuts: Vec<_> = segments.iter().filter(|s| s.move_type == MoveType::Cut).collect();
        assert!(cuts.len() >= 10, "50mm stock with 3mm stepover needs ~10+ passes");
    }
}
