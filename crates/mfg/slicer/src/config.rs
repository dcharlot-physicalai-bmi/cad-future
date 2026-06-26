//! Slicer configuration for FDM 3D printing.

use physical_mfg_toolpath::post::{self, GCodeDialect};
use serde::{Deserialize, Serialize};

/// Infill pattern.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum InfillPattern {
    /// Parallel lines at 45 degrees, alternating ±45 each layer.
    Lines,
    /// Perpendicular grid (0/90 degrees).
    Grid,
    /// Triangular grid (0/60/120 degrees).
    Triangles,
    /// Hexagonal honeycomb.
    Honeycomb,
    /// Gyroid — triply periodic minimal surface.
    Gyroid,
}

/// Seam placement strategy.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum SeamPosition {
    /// Nearest point to previous layer end.
    Nearest,
    /// Aligned to same position each layer.
    Aligned,
    /// Random position each layer.
    Random,
    /// Sharpest convex corner (hides seam).
    SharpestCorner,
}

/// Complete slicer configuration.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SlicerConfig {
    // Layer
    pub layer_height: f64,
    pub first_layer_height: f64,

    // Nozzle
    pub nozzle_diameter: f64,
    pub filament_diameter: f64,

    // Walls
    pub wall_count: usize,

    // Top/bottom
    pub top_layers: usize,
    pub bottom_layers: usize,

    // Infill
    pub infill_pattern: InfillPattern,
    pub infill_density: f64,

    // Speeds (mm/s)
    pub print_speed: f64,
    pub travel_speed: f64,
    pub first_layer_speed: f64,
    pub infill_speed: f64,

    // Retraction
    pub retraction_distance: f64,
    pub retraction_speed: f64,

    // Temperature
    pub extruder_temp: f64,
    pub bed_temp: f64,

    // Fan
    pub fan_speed: f64,
    pub fan_start_layer: usize,

    // Support
    pub support_enabled: bool,
    pub support_angle: f64,

    // Seam
    pub seam_position: SeamPosition,

    // Output
    pub dialect: GCodeDialect,
}

impl Default for SlicerConfig {
    fn default() -> Self {
        Self {
            layer_height: 0.2,
            first_layer_height: 0.3,
            nozzle_diameter: 0.4,
            filament_diameter: 1.75,
            wall_count: 2,
            top_layers: 4,
            bottom_layers: 4,
            infill_pattern: InfillPattern::Lines,
            infill_density: 0.20,
            print_speed: 60.0,
            travel_speed: 150.0,
            first_layer_speed: 30.0,
            infill_speed: 80.0,
            retraction_distance: 1.0,
            retraction_speed: 40.0,
            extruder_temp: 200.0,
            bed_temp: 60.0,
            fan_speed: 100.0,
            fan_start_layer: 2,
            support_enabled: false,
            support_angle: 45.0,
            seam_position: SeamPosition::SharpestCorner,
            dialect: post::marlin(),
        }
    }
}

impl SlicerConfig {
    /// Extrusion width — typically slightly wider than nozzle for good adhesion.
    pub fn extrusion_width(&self) -> f64 {
        self.nozzle_diameter * 1.1
    }

    /// Cross-sectional area of extruded filament (rectangular approximation).
    pub fn extrusion_area(&self) -> f64 {
        self.extrusion_width() * self.layer_height
    }

    /// Filament cross-section area.
    pub fn filament_area(&self) -> f64 {
        std::f64::consts::PI * (self.filament_diameter / 2.0).powi(2)
    }

    /// E-axis distance per mm of XY travel (volume conservation).
    pub fn e_per_mm(&self) -> f64 {
        self.extrusion_area() / self.filament_area()
    }
}
