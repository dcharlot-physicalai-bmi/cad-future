//! Laser cutting/engraving configuration.

use physical_mfg_toolpath::post::{self, GCodeDialect};
use serde::{Deserialize, Serialize};

/// Laser operation mode.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum LaserMode {
    /// Vector cutting — follow contour paths.
    Cut,
    /// Raster engraving — scanline passes over images.
    Engrave,
    /// Score — light vector pass without cutting through.
    Score,
}

/// Laser cutting/engraving configuration.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LaserConfig {
    // Kerf
    pub kerf_width: f64,

    // Power and speed by mode
    pub cut_power: f64,
    pub cut_speed: f64,
    pub engrave_power: f64,
    pub engrave_speed: f64,
    pub score_power: f64,
    pub score_speed: f64,

    // Travel
    pub travel_speed: f64,

    // Material
    pub material_thickness: f64,
    pub passes: usize,

    // Tabs (hold-downs)
    pub tabs_enabled: bool,
    pub tab_width: f64,
    pub tab_height: f64,
    pub tab_spacing: f64,

    // Lead-in/out
    pub leadin_radius: f64,

    // Output
    pub dialect: GCodeDialect,
}

impl Default for LaserConfig {
    fn default() -> Self {
        Self {
            kerf_width: 0.2,
            cut_power: 100.0,
            cut_speed: 600.0,
            engrave_power: 30.0,
            engrave_speed: 3000.0,
            score_power: 15.0,
            score_speed: 1000.0,
            travel_speed: 6000.0,
            material_thickness: 3.0,
            passes: 1,
            tabs_enabled: false,
            tab_width: 3.0,
            tab_height: 0.5,
            tab_spacing: 50.0,
            leadin_radius: 0.0,
            dialect: post::grbl(),
        }
    }
}
