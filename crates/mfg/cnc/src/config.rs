//! CNC milling configuration.

use glam::DVec3;
use physical_mfg_toolpath::material::WorkMaterial;
use physical_mfg_toolpath::post::{self, GCodeDialect};
use physical_mfg_toolpath::tool::Tool;
use serde::{Deserialize, Serialize};

/// Stock shape definition.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum StockDefinition {
    /// Bounding box of part + uniform margin.
    BoundingBox { margin: f64 },
    /// Explicit rectangular block.
    Block { min: DVec3, max: DVec3 },
    /// Cylindrical stock.
    Cylinder {
        center_x: f64,
        center_y: f64,
        radius: f64,
        height: f64,
    },
}

impl Default for StockDefinition {
    fn default() -> Self {
        Self::BoundingBox { margin: 5.0 }
    }
}

/// CNC milling configuration.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CncConfig {
    pub stock: StockDefinition,
    pub tool: Tool,
    pub material: WorkMaterial,

    /// Axial depth of cut per pass (mm).
    pub step_down: f64,
    /// Radial width of cut as fraction of tool diameter (0.0-1.0).
    pub step_over: f64,
    /// Stock left for finishing pass (mm).
    pub finishing_allowance: f64,

    /// Safe Z height for rapid moves (mm above stock top).
    pub safe_height: f64,
    /// Z height to begin plunge (mm above stock top).
    pub feed_height: f64,

    /// Enable coolant.
    pub coolant: bool,

    /// G-code output dialect.
    pub dialect: GCodeDialect,
}

impl Default for CncConfig {
    fn default() -> Self {
        use physical_mfg_toolpath::tool::{ToolGeometry, ToolMaterial};

        Self {
            stock: StockDefinition::default(),
            tool: Tool {
                name: "6mm End Mill".into(),
                geometry: ToolGeometry::EndMill {
                    diameter: 6.0,
                    flute_length: 20.0,
                    flute_count: 2,
                },
                max_rpm: 24000.0,
                material: ToolMaterial::Carbide,
            },
            material: WorkMaterial::aluminum_6061(),
            step_down: 2.0,
            step_over: 0.4,
            finishing_allowance: 0.1,
            safe_height: 5.0,
            feed_height: 2.0,
            coolant: true,
            dialect: post::grbl(),
        }
    }
}

impl CncConfig {
    /// Get stock bounds, resolving BoundingBox relative to a part.
    pub fn stock_bounds(&self, part_min: DVec3, part_max: DVec3) -> (DVec3, DVec3) {
        match &self.stock {
            StockDefinition::BoundingBox { margin } => {
                let m = DVec3::splat(*margin);
                (part_min - m, part_max + m)
            }
            StockDefinition::Block { min, max } => (*min, *max),
            StockDefinition::Cylinder {
                center_x,
                center_y,
                radius,
                height,
            } => (
                DVec3::new(center_x - radius, center_y - radius, 0.0),
                DVec3::new(center_x + radius, center_y + radius, *height),
            ),
        }
    }

    /// Effective tool diameter.
    pub fn tool_diameter(&self) -> f64 {
        self.tool.geometry.diameter()
    }

    /// Radial step-over distance in mm.
    pub fn step_over_mm(&self) -> f64 {
        self.tool_diameter() * self.step_over
    }
}
