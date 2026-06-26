//! Work material definitions and feeds/speeds calculations.

use serde::{Deserialize, Serialize};

/// Category of work material.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum MaterialCategory {
    Aluminum,
    Steel,
    StainlessSteel,
    Titanium,
    Plastic,
    Wood,
    Foam,
    Composite,
}

/// Work material properties for feeds/speeds calculation.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkMaterial {
    pub name: String,
    pub category: MaterialCategory,
    /// Recommended surface speed in m/min for carbide tools.
    pub surface_speed: f64,
    /// Chip load per tooth in mm at reference conditions.
    pub chip_load: f64,
    /// Hardness (HRC or HB equivalent).
    pub hardness: f64,
}

impl WorkMaterial {
    /// 6061-T6 aluminum — the most common prototyping material.
    pub fn aluminum_6061() -> Self {
        Self {
            name: "Aluminum 6061-T6".into(),
            category: MaterialCategory::Aluminum,
            surface_speed: 250.0,
            chip_load: 0.05,
            hardness: 95.0, // HB
        }
    }

    /// Mild steel (1018/A36).
    pub fn mild_steel() -> Self {
        Self {
            name: "Mild Steel 1018".into(),
            category: MaterialCategory::Steel,
            surface_speed: 60.0,
            chip_load: 0.04,
            hardness: 130.0, // HB
        }
    }

    /// 304 stainless steel.
    pub fn stainless_304() -> Self {
        Self {
            name: "Stainless 304".into(),
            category: MaterialCategory::StainlessSteel,
            surface_speed: 40.0,
            chip_load: 0.03,
            hardness: 200.0, // HB
        }
    }

    /// ABS plastic.
    pub fn abs_plastic() -> Self {
        Self {
            name: "ABS".into(),
            category: MaterialCategory::Plastic,
            surface_speed: 300.0,
            chip_load: 0.1,
            hardness: 10.0,
        }
    }

    /// Hardwood (maple, oak).
    pub fn hardwood() -> Self {
        Self {
            name: "Hardwood".into(),
            category: MaterialCategory::Wood,
            surface_speed: 400.0,
            chip_load: 0.15,
            hardness: 5.0,
        }
    }

    /// Foam (tooling board, Renshape).
    pub fn foam() -> Self {
        Self {
            name: "Tooling Foam".into(),
            category: MaterialCategory::Foam,
            surface_speed: 600.0,
            chip_load: 0.25,
            hardness: 1.0,
        }
    }
}

/// Calculate spindle RPM from surface speed (m/min) and tool diameter (mm).
pub fn calc_rpm(surface_speed_m_min: f64, diameter_mm: f64) -> f64 {
    if diameter_mm <= 0.0 {
        return 0.0;
    }
    (surface_speed_m_min * 1000.0) / (std::f64::consts::PI * diameter_mm)
}

/// Calculate feed rate (mm/min) from RPM, chip load (mm/tooth), and flute count.
pub fn calc_feed(rpm: f64, chip_load: f64, flute_count: u8) -> f64 {
    rpm * chip_load * flute_count as f64
}

/// Calculate recommended RPM and feed rate for a given material and tool.
pub fn recommended_feeds_speeds(
    material: &WorkMaterial,
    tool_diameter: f64,
    flute_count: u8,
    max_rpm: f64,
) -> (f64, f64) {
    let rpm = calc_rpm(material.surface_speed, tool_diameter).min(max_rpm);
    let feed = calc_feed(rpm, material.chip_load, flute_count);
    (rpm, feed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rpm_calculation() {
        // 250 m/min surface speed, 6mm tool
        let rpm = calc_rpm(250.0, 6.0);
        // Expected: 250000 / (pi * 6) ≈ 13263
        assert!((rpm - 13263.0).abs() < 100.0);
    }

    #[test]
    fn feed_calculation() {
        let feed = calc_feed(10000.0, 0.05, 2);
        assert!((feed - 1000.0).abs() < 1e-8); // 10000 * 0.05 * 2 = 1000 mm/min
    }

    #[test]
    fn recommended_aluminum() {
        let mat = WorkMaterial::aluminum_6061();
        let (rpm, feed) = recommended_feeds_speeds(&mat, 6.0, 2, 24000.0);
        assert!(rpm > 0.0);
        assert!(feed > 0.0);
        assert!(rpm <= 24000.0);
    }
}
