//! `physical-viewport` — 16-dimension composited renderer for OpenIE.
//!
//! Provides the orbit camera, grid rendering, mesh primitives, materials,
//! scene management, and forward rendering pipeline for the CAD viewport.

pub mod orbit_camera;
pub mod grid;
pub mod vertex;
pub mod mesh;
pub mod mesh_registry;
pub mod material;
pub mod scene;
pub mod forward;
pub mod gizmo;
pub mod gizmo_renderer;
pub mod wireframe;
pub mod undo;
pub mod measurement;
pub mod axes_indicator;
pub mod nav_cube;
pub mod outline;
pub mod clip_plane;
pub mod mate;

/// Display mode for the viewport.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShadingMode {
    /// Filled faces with Blinn-Phong lighting.
    Solid,
    /// Edge lines only, no filled faces.
    Wireframe,
    /// Filled faces with wireframe overlay.
    SolidWireframe,
}

impl ShadingMode {
    pub fn cycle(self) -> Self {
        match self {
            Self::Solid => Self::Wireframe,
            Self::Wireframe => Self::SolidWireframe,
            Self::SolidWireframe => Self::Solid,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Solid => "Solid",
            Self::Wireframe => "Wireframe",
            Self::SolidWireframe => "Solid+Wire",
        }
    }
}

pub use orbit_camera::OrbitCamera;
pub use vertex::Vertex;
pub use mesh_registry::MeshRegistry;
pub use material::{Material, MaterialId, MaterialStore};
pub use scene::{Scene, SceneNode, RenderObject};
pub use forward::{ForwardRenderer, ForwardFrameInput};
pub use grid::{GridRenderer, GridUniforms};
pub use gizmo::{Gizmo, GizmoMode, GizmoAxis};
pub use gizmo_renderer::GizmoRenderer;
pub use wireframe::WireframeRenderer;
pub use undo::{UndoStack, Action};
pub use measurement::{Measurement, MeasurementOverlay};
pub use axes_indicator::AxesIndicator;
pub use nav_cube::{NavCube, ViewPreset};
pub use outline::OutlineRenderer;
pub use clip_plane::ClipPlane;
pub use mate::{MateOp, MateSystem, MateConstraint, compute_mate};
