//! Stock definition and management.

use glam::DVec3;
use physical_brep::Solid;

use crate::config::CncConfig;

/// Resolved stock dimensions.
#[derive(Clone, Debug)]
pub struct Stock {
    pub min: DVec3,
    pub max: DVec3,
}

impl Stock {
    /// Resolve stock dimensions from config and part geometry.
    pub fn from_config(config: &CncConfig, solid: &Solid) -> Self {
        let (part_min, part_max) = solid.bounding_box();
        let (min, max) = config.stock_bounds(part_min, part_max);
        Self { min, max }
    }

    /// Create explicit rectangular stock.
    pub fn block(min: DVec3, max: DVec3) -> Self {
        Self { min, max }
    }

    /// Stock width (X extent).
    pub fn width(&self) -> f64 {
        self.max.x - self.min.x
    }

    /// Stock depth (Y extent).
    pub fn depth(&self) -> f64 {
        self.max.y - self.min.y
    }

    /// Stock height (Z extent).
    pub fn height(&self) -> f64 {
        self.max.z - self.min.z
    }

    /// Top Z of stock.
    pub fn top_z(&self) -> f64 {
        self.max.z
    }
}

/// Compute Z-level passes for a given total depth and step-down.
pub fn compute_z_levels(top_z: f64, bottom_z: f64, step_down: f64) -> Vec<f64> {
    if step_down <= 0.0 || top_z <= bottom_z {
        return vec![bottom_z];
    }

    let mut levels = Vec::new();
    let mut z = top_z - step_down;
    while z > bottom_z + 1e-6 {
        levels.push(z);
        z -= step_down;
    }
    levels.push(bottom_z);
    levels
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn z_levels_basic() {
        let levels = compute_z_levels(10.0, 0.0, 2.0);
        assert_eq!(levels, vec![8.0, 6.0, 4.0, 2.0, 0.0]);
    }

    #[test]
    fn z_levels_uneven() {
        let levels = compute_z_levels(10.0, 0.0, 3.0);
        // 10-3=7, 7-3=4, 4-3=1, 1>0 so next is 0
        assert_eq!(levels.len(), 4);
        assert!((levels.last().unwrap() - 0.0).abs() < 1e-8);
    }

    #[test]
    fn stock_from_block() {
        let s = Stock::block(DVec3::ZERO, DVec3::new(100.0, 80.0, 25.0));
        assert!((s.width() - 100.0).abs() < 1e-8);
        assert!((s.depth() - 80.0).abs() < 1e-8);
        assert!((s.height() - 25.0).abs() < 1e-8);
    }
}
