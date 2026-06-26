//! Print time and material estimation for FDM 3D printing.
//!
//! Estimates based on LUT-derived speeds × volume × infill density.

use serde::{Deserialize, Serialize};

use crate::config::{InfillPattern, SlicerConfig};

/// Print estimation result.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PrintEstimation {
    /// Estimated print time in seconds.
    pub time_seconds: f64,
    /// Estimated filament length in mm.
    pub filament_length_mm: f64,
    /// Estimated filament weight in grams.
    pub filament_weight_g: f64,
    /// Estimated filament cost (USD).
    pub filament_cost_usd: f64,
    /// Number of layers.
    pub layer_count: usize,
    /// Total travel distance (mm).
    pub total_travel_mm: f64,
    /// Total extrusion distance (mm).
    pub total_extrusion_mm: f64,
}

/// Filament properties for estimation.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FilamentInfo {
    pub name: String,
    /// Density in g/cm³.
    pub density: f64,
    /// Cost per kg (USD).
    pub cost_per_kg: f64,
}

impl FilamentInfo {
    pub fn pla() -> Self {
        Self { name: "PLA".into(), density: 1.24, cost_per_kg: 20.0 }
    }

    pub fn abs() -> Self {
        Self { name: "ABS".into(), density: 1.04, cost_per_kg: 22.0 }
    }

    pub fn petg() -> Self {
        Self { name: "PETG".into(), density: 1.27, cost_per_kg: 25.0 }
    }

    pub fn nylon() -> Self {
        Self { name: "Nylon".into(), density: 1.14, cost_per_kg: 40.0 }
    }

    pub fn tpu() -> Self {
        Self { name: "TPU".into(), density: 1.21, cost_per_kg: 35.0 }
    }
}

/// Estimate print time and material usage.
pub fn estimate_print(
    bounding_volume_mm3: f64,
    infill_fraction: f64,
    config: &SlicerConfig,
    filament: &FilamentInfo,
) -> PrintEstimation {
    let total_height = bounding_volume_mm3.cbrt(); // rough estimate
    let layer_count = (total_height / config.layer_height).ceil() as usize;

    let extrusion_width = config.extrusion_width();

    // Shell volume (walls + top/bottom)
    let shell_fraction = {
        let wall_thickness = config.wall_count as f64 * extrusion_width;
        let side = bounding_volume_mm3.cbrt();
        if side > 0.0 {
            let inner_side = (side - 2.0 * wall_thickness).max(0.0);
            1.0 - (inner_side / side).powi(3)
        } else {
            1.0
        }
    };

    // Total material volume
    let infill_volume = bounding_volume_mm3 * (1.0 - shell_fraction) * infill_fraction;
    let shell_volume = bounding_volume_mm3 * shell_fraction;
    let total_volume_mm3 = infill_volume + shell_volume;

    // Filament length: V = pi * r² * L → L = V / (pi * r²)
    let filament_r = config.filament_diameter / 2.0;
    let filament_area = std::f64::consts::PI * filament_r * filament_r;
    let filament_length = total_volume_mm3 / filament_area;

    // Weight: volume × density (convert mm³ to cm³)
    let weight_g = (total_volume_mm3 / 1000.0) * filament.density;

    // Cost
    let cost = weight_g / 1000.0 * filament.cost_per_kg;

    // Time estimation
    // Extrusion time: total extrusion length / print speed
    let extrusion_distance = total_volume_mm3 / (extrusion_width * config.layer_height);
    let extrusion_time = extrusion_distance / (config.print_speed * 60.0); // speed in mm/s → sec

    // Travel time (rough: ~30% of extrusion distance at travel speed)
    let travel_distance = extrusion_distance * 0.3;
    let travel_time = travel_distance / (config.travel_speed * 60.0);

    // Layer change time
    let layer_change_time = layer_count as f64 * 0.5; // ~0.5s per layer change

    // Infill pattern speed factor
    let infill_speed_factor = match config.infill_pattern {
        InfillPattern::Lines => 1.0,
        InfillPattern::Grid => 0.95,
        InfillPattern::Triangles => 0.90,
        InfillPattern::Honeycomb => 0.85,
        InfillPattern::Gyroid => 0.80,
    };

    let total_time = (extrusion_time + travel_time + layer_change_time) / infill_speed_factor;

    PrintEstimation {
        time_seconds: total_time,
        filament_length_mm: filament_length,
        filament_weight_g: weight_g,
        filament_cost_usd: cost,
        layer_count,
        total_travel_mm: travel_distance,
        total_extrusion_mm: extrusion_distance,
    }
}

// ---------------------------------------------------------------------------
// SliceLayer with multi-material support
// ---------------------------------------------------------------------------

/// A single slice layer with contours and optional material assignment.
#[derive(Clone, Debug)]
pub struct SliceLayer<'a> {
    /// Z height of the top of this layer (mm).
    pub z: f64,
    /// Layer thickness (mm).
    pub height: f64,
    /// Contour boundaries for this layer.
    pub contours: Vec<physical_mfg_toolpath::contour::Contour>,
    /// Optional material identifier for multi-material prints.
    pub material_id: Option<&'a str>,
}

impl<'a> SliceLayer<'a> {
    /// Total perimeter length of all contours in this layer (mm).
    pub fn perimeter_length(&self) -> f64 {
        self.contours.iter().map(|c| c.length()).sum()
    }
}

// ---------------------------------------------------------------------------
// Layer-based print time estimation
// ---------------------------------------------------------------------------

/// Breakdown of estimated print time.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PrintEstimate {
    /// Total estimated time in seconds.
    pub total_seconds: f64,
    /// Time spent printing (extruding) in seconds.
    pub print_seconds: f64,
    /// Time spent on travel moves in seconds.
    pub travel_seconds: f64,
    /// Time spent on layer changes in seconds.
    pub layer_change_seconds: f64,
    /// Number of layers.
    pub layer_count: usize,
}

/// Estimate print time from slice layers with a detailed breakdown.
///
/// * `layers` — slice layers with contour geometry
/// * `print_speed` — extrusion speed in mm/s
/// * `travel_speed` — non-extrusion travel speed in mm/s
/// * `layer_change_time` — time per layer change in seconds (Z hop + settle)
pub fn estimate_print_time(
    layers: &[SliceLayer<'_>],
    print_speed: f64,
    travel_speed: f64,
    layer_change_time: f64,
) -> PrintEstimate {
    let mut print_distance = 0.0;
    let mut travel_distance = 0.0;

    for (i, layer) in layers.iter().enumerate() {
        let perim = layer.perimeter_length();
        print_distance += perim;

        // Estimate travel as distance between end of one contour and start of next
        if layer.contours.len() > 1 {
            for j in 1..layer.contours.len() {
                if let (Some(end), Some(start)) = (
                    layer.contours[j - 1].points.last(),
                    layer.contours[j].points.first(),
                ) {
                    travel_distance += (*start - *end).length();
                }
            }
        }

        // Inter-layer travel (XY distance between last point of prev layer and first of this)
        if i > 0 {
            if let (Some(prev_last), Some(curr_first)) = (
                layers[i - 1].contours.last().and_then(|c| c.points.last()),
                layer.contours.first().and_then(|c| c.points.first()),
            ) {
                travel_distance += (*curr_first - *prev_last).length();
            }
        }
    }

    let print_seconds = if print_speed > 0.0 { print_distance / print_speed } else { 0.0 };
    let travel_seconds = if travel_speed > 0.0 { travel_distance / travel_speed } else { 0.0 };
    let layer_change_seconds = layers.len() as f64 * layer_change_time;
    let total_seconds = print_seconds + travel_seconds + layer_change_seconds;

    PrintEstimate {
        total_seconds,
        print_seconds,
        travel_seconds,
        layer_change_seconds,
        layer_count: layers.len(),
    }
}

// ---------------------------------------------------------------------------
// Layer-based material usage estimation
// ---------------------------------------------------------------------------

/// Material usage estimate.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MaterialEstimate {
    /// Filament length in meters.
    pub filament_meters: f64,
    /// Filament weight in grams.
    pub filament_grams: f64,
    /// Volume of extruded material in mm^3.
    pub volume_mm3: f64,
}

/// Estimate material usage from slice layers.
///
/// * `layers` — slice layers with contour geometry
/// * `filament_diameter` — filament diameter in mm (typically 1.75)
/// * `infill_density` — fraction 0.0-1.0
pub fn estimate_material(
    layers: &[SliceLayer<'_>],
    filament_diameter: f64,
    infill_density: f64,
) -> MaterialEstimate {
    let filament_radius = filament_diameter / 2.0;
    let filament_area = std::f64::consts::PI * filament_radius * filament_radius;

    let mut total_volume = 0.0;
    let extrusion_width = 0.44; // typical for 0.4mm nozzle

    for layer in layers {
        let perim_length = layer.perimeter_length();
        // Perimeter volume: length * extrusion_width * layer_height
        let perim_volume = perim_length * extrusion_width * layer.height;

        // Infill volume: approximate from contour area * layer_height * density
        let infill_volume: f64 = layer.contours.iter()
            .map(|c| c.signed_area().abs() * layer.height * infill_density)
            .sum();

        total_volume += perim_volume + infill_volume;
    }

    let filament_length_mm = if filament_area > 0.0 { total_volume / filament_area } else { 0.0 };
    // PLA density ~1.24 g/cm^3 = 0.00124 g/mm^3
    let density_g_per_mm3 = 0.00124;
    let filament_grams = total_volume * density_g_per_mm3;

    MaterialEstimate {
        filament_meters: filament_length_mm / 1000.0,
        filament_grams,
        volume_mm3: total_volume,
    }
}

/// Format time as human-readable string.
pub fn format_time(seconds: f64) -> String {
    let hours = (seconds / 3600.0).floor() as u64;
    let mins = ((seconds % 3600.0) / 60.0).floor() as u64;
    if hours > 0 {
        format!("{}h {}m", hours, mins)
    } else {
        format!("{}m", mins)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_config() -> SlicerConfig {
        SlicerConfig::default()
    }

    #[test]
    fn estimate_small_cube() {
        let config = default_config();
        let filament = FilamentInfo::pla();
        // 20mm cube = 8000 mm³
        let est = estimate_print(8000.0, 0.2, &config, &filament);

        assert!(est.time_seconds > 0.0);
        assert!(est.filament_length_mm > 0.0);
        assert!(est.filament_weight_g > 0.0);
        assert!(est.filament_cost_usd > 0.0);
        assert!(est.layer_count > 0);
    }

    #[test]
    fn higher_infill_uses_more_material() {
        let config = default_config();
        let filament = FilamentInfo::pla();

        let est_20 = estimate_print(8000.0, 0.2, &config, &filament);
        let est_80 = estimate_print(8000.0, 0.8, &config, &filament);

        assert!(est_80.filament_weight_g > est_20.filament_weight_g);
        assert!(est_80.time_seconds > est_20.time_seconds);
    }

    #[test]
    fn filament_densities_different() {
        let config = default_config();

        let est_pla = estimate_print(8000.0, 0.2, &config, &FilamentInfo::pla());
        let est_abs = estimate_print(8000.0, 0.2, &config, &FilamentInfo::abs());

        // PLA is denser than ABS
        assert!(est_pla.filament_weight_g > est_abs.filament_weight_g);
    }

    #[test]
    fn format_time_hours() {
        assert_eq!(format_time(3720.0), "1h 2m");
    }

    #[test]
    fn format_time_minutes() {
        assert_eq!(format_time(120.0), "2m");
    }

    #[test]
    fn zero_volume_safe() {
        let config = default_config();
        let est = estimate_print(0.0, 0.2, &config, &FilamentInfo::pla());
        assert_eq!(est.filament_weight_g, 0.0);
    }

    // --- SliceLayer / multi-material tests ---

    fn make_square_layer<'a>(z: f64, height: f64, material_id: Option<&'a str>) -> SliceLayer<'a> {
        use glam::DVec2;
        let contour = physical_mfg_toolpath::contour::Contour::closed(vec![
            DVec2::new(0.0, 0.0),
            DVec2::new(10.0, 0.0),
            DVec2::new(10.0, 10.0),
            DVec2::new(0.0, 10.0),
        ]);
        SliceLayer { z, height, contours: vec![contour], material_id }
    }

    #[test]
    fn slice_layer_material_id() {
        let layer = make_square_layer(0.2, 0.2, Some("PLA"));
        assert_eq!(layer.material_id, Some("PLA"));
        let layer_none = make_square_layer(0.2, 0.2, None);
        assert_eq!(layer_none.material_id, None);
    }

    #[test]
    fn slice_layer_perimeter_length() {
        let layer = make_square_layer(0.2, 0.2, None);
        let perim = layer.perimeter_length();
        assert!((perim - 40.0).abs() < 0.01, "10mm square perimeter = 40mm, got {perim}");
    }

    // --- estimate_print_time tests ---

    #[test]
    fn estimate_print_time_basic() {
        let layers = vec![
            make_square_layer(0.2, 0.2, None),
            make_square_layer(0.4, 0.2, None),
        ];
        let est = estimate_print_time(&layers, 60.0, 150.0, 0.5);
        assert!(est.total_seconds > 0.0, "Total time should be positive");
        assert!(est.print_seconds > 0.0, "Print time should be positive");
        assert!(est.layer_change_seconds > 0.0, "Layer change time should be positive");
        assert_eq!(est.layer_count, 2);
        // Breakdown should sum to total
        let sum = est.print_seconds + est.travel_seconds + est.layer_change_seconds;
        assert!((est.total_seconds - sum).abs() < 0.01);
    }

    #[test]
    fn estimate_print_time_empty() {
        let est = estimate_print_time(&[], 60.0, 150.0, 0.5);
        assert_eq!(est.total_seconds, 0.0);
        assert_eq!(est.layer_count, 0);
    }

    // --- estimate_material tests ---

    #[test]
    fn estimate_material_basic() {
        let layers = vec![
            make_square_layer(0.2, 0.2, None),
            make_square_layer(0.4, 0.2, None),
        ];
        let est = estimate_material(&layers, 1.75, 0.2);
        assert!(est.filament_meters > 0.0, "Should use some filament");
        assert!(est.filament_grams > 0.0, "Should weigh something");
        assert!(est.volume_mm3 > 0.0, "Should have volume");
    }

    #[test]
    fn estimate_material_higher_infill_uses_more() {
        let layers = vec![make_square_layer(0.2, 0.2, None)];
        let est_low = estimate_material(&layers, 1.75, 0.1);
        let est_high = estimate_material(&layers, 1.75, 0.8);
        assert!(est_high.filament_grams > est_low.filament_grams,
            "Higher infill should use more material");
    }

    #[test]
    fn estimate_material_empty() {
        let est = estimate_material(&[], 1.75, 0.2);
        assert_eq!(est.filament_meters, 0.0);
        assert_eq!(est.filament_grams, 0.0);
    }
}
