//! Tool definitions for all manufacturing processes.

use serde::{Deserialize, Serialize};

/// Tool geometry variants for different manufacturing processes.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ToolGeometry {
    /// Flat end mill for CNC milling.
    EndMill {
        diameter: f64,
        flute_length: f64,
        flute_count: u8,
    },
    /// Ball nose end mill for 3D finishing.
    BallNose {
        diameter: f64,
        flute_length: f64,
        flute_count: u8,
    },
    /// Twist drill.
    Drill {
        diameter: f64,
        point_angle: f64,
    },
    /// Chamfer mill.
    Chamfer {
        diameter: f64,
        angle: f64,
    },
    /// V-bit for engraving.
    VBit {
        angle: f64,
        tip_diameter: f64,
    },
    /// FDM 3D printer nozzle.
    Nozzle {
        diameter: f64,
    },
    /// Laser beam.
    LaserBeam {
        spot_diameter: f64,
    },
}

impl ToolGeometry {
    /// Effective cutting diameter.
    pub fn diameter(&self) -> f64 {
        match self {
            Self::EndMill { diameter, .. } => *diameter,
            Self::BallNose { diameter, .. } => *diameter,
            Self::Drill { diameter, .. } => *diameter,
            Self::Chamfer { diameter, .. } => *diameter,
            Self::VBit { tip_diameter, .. } => *tip_diameter,
            Self::Nozzle { diameter } => *diameter,
            Self::LaserBeam { spot_diameter } => *spot_diameter,
        }
    }

    /// Number of cutting flutes (0 for non-rotary tools).
    pub fn flute_count(&self) -> u8 {
        match self {
            Self::EndMill { flute_count, .. } => *flute_count,
            Self::BallNose { flute_count, .. } => *flute_count,
            Self::Drill { .. } => 2,
            Self::Chamfer { .. } => 2,
            Self::VBit { .. } => 1,
            Self::Nozzle { .. } => 0,
            Self::LaserBeam { .. } => 0,
        }
    }
}

/// Tool material.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum ToolMaterial {
    HSS,
    Carbide,
    Cobalt,
    Diamond,
    Ceramic,
}

/// A complete tool definition.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Tool {
    pub name: String,
    pub geometry: ToolGeometry,
    pub max_rpm: f64,
    pub material: ToolMaterial,
}

/// A library of tools.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ToolLibrary {
    pub tools: Vec<Tool>,
}

impl ToolLibrary {
    pub fn new() -> Self {
        Self { tools: Vec::new() }
    }

    pub fn add(&mut self, tool: Tool) -> usize {
        let idx = self.tools.len();
        self.tools.push(tool);
        idx
    }

    pub fn get(&self, index: usize) -> Option<&Tool> {
        self.tools.get(index)
    }

    /// Create a default FDM tool library with a 0.4mm nozzle.
    pub fn default_fdm() -> Self {
        let mut lib = Self::new();
        lib.add(Tool {
            name: "0.4mm Nozzle".into(),
            geometry: ToolGeometry::Nozzle { diameter: 0.4 },
            max_rpm: 0.0,
            material: ToolMaterial::HSS, // Not applicable, placeholder
        });
        lib
    }

    /// Create a default laser tool library.
    pub fn default_laser() -> Self {
        let mut lib = Self::new();
        lib.add(Tool {
            name: "CO2 Laser".into(),
            geometry: ToolGeometry::LaserBeam { spot_diameter: 0.1 },
            max_rpm: 0.0,
            material: ToolMaterial::HSS,
        });
        lib
    }

    /// Create a basic CNC tool library.
    pub fn default_cnc() -> Self {
        let mut lib = Self::new();
        lib.add(Tool {
            name: "6mm Flat End Mill".into(),
            geometry: ToolGeometry::EndMill {
                diameter: 6.0,
                flute_length: 20.0,
                flute_count: 2,
            },
            max_rpm: 24000.0,
            material: ToolMaterial::Carbide,
        });
        lib.add(Tool {
            name: "3mm Ball Nose".into(),
            geometry: ToolGeometry::BallNose {
                diameter: 3.0,
                flute_length: 15.0,
                flute_count: 2,
            },
            max_rpm: 24000.0,
            material: ToolMaterial::Carbide,
        });
        lib.add(Tool {
            name: "5mm Drill".into(),
            geometry: ToolGeometry::Drill {
                diameter: 5.0,
                point_angle: 118.0,
            },
            max_rpm: 10000.0,
            material: ToolMaterial::HSS,
        });
        lib
    }
}
