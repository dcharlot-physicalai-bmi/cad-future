//! `physical-overlay` — 2D projection, dimension overlay, and annotation generation.
//!
//! Projects 3D B-Rep solids into 2D line drawings for technical drawings,
//! laser cutting path generation, viewport dimension overlays, and hidden
//! line removal. Supports orthographic and isometric projections.

use glam::{DVec2, DVec3, DMat3};
use physical_brep::Solid;

// ---------------------------------------------------------------------------
// View Direction
// ---------------------------------------------------------------------------

/// Standard view direction for projection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewDirection {
    Top,
    Bottom,
    Front,
    Back,
    Left,
    Right,
}

impl ViewDirection {
    /// View direction vector (into the screen).
    pub fn look_at(self) -> DVec3 {
        match self {
            Self::Top    => -DVec3::Y,
            Self::Bottom =>  DVec3::Y,
            Self::Front  => -DVec3::Z,
            Self::Back   =>  DVec3::Z,
            Self::Left   =>  DVec3::X,
            Self::Right  => -DVec3::X,
        }
    }

    /// Up vector for this view.
    pub fn up(self) -> DVec3 {
        match self {
            Self::Top | Self::Bottom => -DVec3::Z,
            _ => DVec3::Y,
        }
    }

    /// Right vector (cross product of up × look_at).
    pub fn right(self) -> DVec3 {
        self.up().cross(self.look_at()).normalize()
    }

    /// 3×3 rotation matrix that transforms world → view-plane coordinates.
    /// Column 0 = right, Column 1 = up, Column 2 = look_at.
    pub fn view_matrix(self) -> DMat3 {
        let r = self.right();
        let u = self.up();
        let f = self.look_at();
        DMat3::from_cols(r, u, f).transpose()
    }
}

// ---------------------------------------------------------------------------
// Projected Elements
// ---------------------------------------------------------------------------

/// A projected 2D line segment.
#[derive(Debug, Clone)]
pub struct ProjectedLine {
    pub start: DVec2,
    pub end: DVec2,
    /// Whether this line is a visible edge (vs hidden/silhouette).
    pub visible: bool,
}

impl ProjectedLine {
    pub fn length(&self) -> f64 {
        (self.end - self.start).length()
    }

    pub fn midpoint(&self) -> DVec2 {
        (self.start + self.end) * 0.5
    }
}

/// A projected 2D arc (from circular edges).
#[derive(Debug, Clone)]
pub struct ProjectedArc {
    pub center: DVec2,
    pub radius: f64,
    pub start_angle: f64,
    pub end_angle: f64,
    pub visible: bool,
}

/// A dimension annotation placed on the 2D view.
#[derive(Debug, Clone)]
pub struct DimensionOverlay {
    pub dim_type: DimensionType,
    pub start: DVec2,
    pub end: DVec2,
    pub value_mm: f64,
    pub label: String,
    /// Offset distance from the geometry for the dimension line.
    pub offset: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DimensionType {
    Linear,
    Radial,
    Diameter,
    Angular,
}

/// Result of projecting a 3D solid into a 2D view.
#[derive(Debug, Clone)]
pub struct ProjectedView {
    /// All projected line segments.
    pub lines: Vec<ProjectedLine>,
    /// Projected arcs (from circular edges).
    pub arcs: Vec<ProjectedArc>,
    /// View direction used.
    pub direction: ViewDirection,
    /// Bounding box of projected geometry: (min, max).
    pub bounds: (DVec2, DVec2),
}

impl ProjectedView {
    /// Total number of visible edges.
    pub fn visible_count(&self) -> usize {
        self.lines.iter().filter(|l| l.visible).count()
            + self.arcs.iter().filter(|a| a.visible).count()
    }

    /// Total number of hidden edges.
    pub fn hidden_count(&self) -> usize {
        self.lines.iter().filter(|l| !l.visible).count()
            + self.arcs.iter().filter(|a| !a.visible).count()
    }

    /// Width of the projected view.
    pub fn width(&self) -> f64 {
        self.bounds.1.x - self.bounds.0.x
    }

    /// Height of the projected view.
    pub fn height(&self) -> f64 {
        self.bounds.1.y - self.bounds.0.y
    }
}

// ---------------------------------------------------------------------------
// Projection
// ---------------------------------------------------------------------------

/// Project a 3D point onto a 2D view plane.
pub fn project_point(point: DVec3, direction: ViewDirection) -> DVec2 {
    let view = direction.view_matrix();
    let transformed = view * point;
    DVec2::new(transformed.x, transformed.y)
}

/// Generate a 2D projected view of a solid from the given direction.
///
/// Projects all edges of the solid onto the view plane, producing
/// line segments. Uses bounding-box midpoint occlusion for basic
/// hidden-line detection.
pub fn generate_view(solid: &Solid, direction: ViewDirection) -> ProjectedView {
    let mut lines = Vec::new();
    let (bb_min, bb_max) = solid.bounding_box();
    let center_3d = (bb_min + bb_max) * 0.5;

    // Project all B-Rep edges
    for (_eid, edge) in &solid.edges {
        let p0 = solid.vertices[edge.v_start].point;
        let p1 = solid.vertices[edge.v_end].point;

        let start = project_point(p0, direction);
        let end = project_point(p1, direction);

        // Skip degenerate edges
        if (start - end).length() < 1e-10 {
            continue;
        }

        // Basic visibility: an edge is "hidden" if its midpoint is behind
        // the solid's center along the view direction.
        let mid_3d = (p0 + p1) * 0.5;
        let look = direction.look_at();
        let depth_mid = mid_3d.dot(look);
        let depth_center = center_3d.dot(look);
        let visible = depth_mid <= depth_center + 1e-6;

        lines.push(ProjectedLine { start, end, visible });
    }

    // If no edges were projected (e.g. simple solid), fall back to bounding box
    if lines.is_empty() {
        lines = project_bounding_box(bb_min, bb_max, direction);
    }

    // Compute 2D bounding box
    let mut min = DVec2::new(f64::INFINITY, f64::INFINITY);
    let mut max = DVec2::new(f64::NEG_INFINITY, f64::NEG_INFINITY);
    for line in &lines {
        min = min.min(line.start.min(line.end));
        max = max.max(line.start.max(line.end));
    }

    ProjectedView {
        lines,
        arcs: Vec::new(),
        direction,
        bounds: (min, max),
    }
}

/// Project bounding box edges as fallback.
fn project_bounding_box(bb_min: DVec3, bb_max: DVec3, direction: ViewDirection) -> Vec<ProjectedLine> {
    let corners = [
        DVec3::new(bb_min.x, bb_min.y, bb_min.z),
        DVec3::new(bb_max.x, bb_min.y, bb_min.z),
        DVec3::new(bb_max.x, bb_max.y, bb_min.z),
        DVec3::new(bb_min.x, bb_max.y, bb_min.z),
        DVec3::new(bb_min.x, bb_min.y, bb_max.z),
        DVec3::new(bb_max.x, bb_min.y, bb_max.z),
        DVec3::new(bb_max.x, bb_max.y, bb_max.z),
        DVec3::new(bb_min.x, bb_max.y, bb_max.z),
    ];

    let edges = [
        (0, 1), (1, 2), (2, 3), (3, 0), // bottom face
        (4, 5), (5, 6), (6, 7), (7, 4), // top face
        (0, 4), (1, 5), (2, 6), (3, 7), // vertical edges
    ];

    let mut lines = Vec::new();
    let center = (bb_min + bb_max) * 0.5;

    for &(a, b) in &edges {
        let start = project_point(corners[a], direction);
        let end = project_point(corners[b], direction);

        if (start - end).length() < 1e-10 {
            continue;
        }

        let mid_3d = (corners[a] + corners[b]) * 0.5;
        let look = direction.look_at();
        let visible = mid_3d.dot(look) <= center.dot(look) + 1e-6;

        lines.push(ProjectedLine { start, end, visible });
    }

    lines
}

// ---------------------------------------------------------------------------
// Dimension Overlay Generation
// ---------------------------------------------------------------------------

/// Generate overall dimension overlays for a projected view.
pub fn generate_dimensions(
    solid: &Solid,
    direction: ViewDirection,
    offset: f64,
) -> Vec<DimensionOverlay> {
    let (bb_min, bb_max) = solid.bounding_box();
    let size = bb_max - bb_min;
    let mut dims = Vec::new();

    match direction {
        ViewDirection::Top | ViewDirection::Bottom => {
            // Width (X) and depth (Z) dimensions
            let p_min = project_point(bb_min, direction);
            let p_max = project_point(bb_max, direction);
            dims.push(DimensionOverlay {
                dim_type: DimensionType::Linear,
                start: DVec2::new(p_min.x, p_min.y - offset),
                end: DVec2::new(p_max.x, p_min.y - offset),
                value_mm: size.x,
                label: format!("{:.1}", size.x),
                offset,
            });
            dims.push(DimensionOverlay {
                dim_type: DimensionType::Linear,
                start: DVec2::new(p_min.x - offset, p_min.y),
                end: DVec2::new(p_min.x - offset, p_max.y),
                value_mm: size.z,
                label: format!("{:.1}", size.z),
                offset,
            });
        }
        ViewDirection::Front | ViewDirection::Back => {
            let p_min = project_point(bb_min, direction);
            let p_max = project_point(bb_max, direction);
            dims.push(DimensionOverlay {
                dim_type: DimensionType::Linear,
                start: DVec2::new(p_min.x, p_min.y - offset),
                end: DVec2::new(p_max.x, p_min.y - offset),
                value_mm: size.x,
                label: format!("{:.1}", size.x),
                offset,
            });
            dims.push(DimensionOverlay {
                dim_type: DimensionType::Linear,
                start: DVec2::new(p_min.x - offset, p_min.y),
                end: DVec2::new(p_min.x - offset, p_max.y),
                value_mm: size.y,
                label: format!("{:.1}", size.y),
                offset,
            });
        }
        ViewDirection::Left | ViewDirection::Right => {
            let p_min = project_point(bb_min, direction);
            let p_max = project_point(bb_max, direction);
            dims.push(DimensionOverlay {
                dim_type: DimensionType::Linear,
                start: DVec2::new(p_min.x, p_min.y - offset),
                end: DVec2::new(p_max.x, p_min.y - offset),
                value_mm: size.z,
                label: format!("{:.1}", size.z),
                offset,
            });
            dims.push(DimensionOverlay {
                dim_type: DimensionType::Linear,
                start: DVec2::new(p_min.x - offset, p_min.y),
                end: DVec2::new(p_min.x - offset, p_max.y),
                value_mm: size.y,
                label: format!("{:.1}", size.y),
                offset,
            });
        }
    }

    dims
}

// ---------------------------------------------------------------------------
// Isometric Projection
// ---------------------------------------------------------------------------

/// Project a 3D point using standard isometric projection.
/// Rotation: 45° around Y then ~35.264° (arctan(1/√2)) around X.
pub fn project_isometric(point: DVec3) -> DVec2 {
    let cos45 = std::f64::consts::FRAC_1_SQRT_2;
    let sin45 = cos45;
    let angle_x = (1.0_f64 / 2.0_f64.sqrt()).atan(); // ~35.264°
    let cos_x = angle_x.cos();
    let sin_x = angle_x.sin();

    // Rotate around Y by 45°
    let x1 = point.x * cos45 + point.z * sin45;
    let y1 = point.y;
    let z1 = -point.x * sin45 + point.z * cos45;

    // Rotate around X by arctan(1/√2)
    let x2 = x1;
    let y2 = y1 * cos_x - z1 * sin_x;

    DVec2::new(x2, y2)
}

/// Generate an isometric view of a solid.
pub fn generate_isometric_view(solid: &Solid) -> ProjectedView {
    let mut lines = Vec::new();

    for (_eid, edge) in &solid.edges {
        let p0 = solid.vertices[edge.v_start].point;
        let p1 = solid.vertices[edge.v_end].point;
        let start = project_isometric(p0);
        let end = project_isometric(p1);

        if (start - end).length() < 1e-10 {
            continue;
        }

        lines.push(ProjectedLine { start, end, visible: true });
    }

    // Fallback to bounding box
    if lines.is_empty() {
        let (bb_min, bb_max) = solid.bounding_box();
        let corners = [
            DVec3::new(bb_min.x, bb_min.y, bb_min.z),
            DVec3::new(bb_max.x, bb_min.y, bb_min.z),
            DVec3::new(bb_max.x, bb_max.y, bb_min.z),
            DVec3::new(bb_min.x, bb_max.y, bb_min.z),
            DVec3::new(bb_min.x, bb_min.y, bb_max.z),
            DVec3::new(bb_max.x, bb_min.y, bb_max.z),
            DVec3::new(bb_max.x, bb_max.y, bb_max.z),
            DVec3::new(bb_min.x, bb_max.y, bb_max.z),
        ];
        let edges = [
            (0, 1), (1, 2), (2, 3), (3, 0),
            (4, 5), (5, 6), (6, 7), (7, 4),
            (0, 4), (1, 5), (2, 6), (3, 7),
        ];
        for &(a, b) in &edges {
            let start = project_isometric(corners[a]);
            let end = project_isometric(corners[b]);
            if (start - end).length() > 1e-10 {
                lines.push(ProjectedLine { start, end, visible: true });
            }
        }
    }

    let mut min = DVec2::new(f64::INFINITY, f64::INFINITY);
    let mut max = DVec2::new(f64::NEG_INFINITY, f64::NEG_INFINITY);
    for line in &lines {
        min = min.min(line.start.min(line.end));
        max = max.max(line.start.max(line.end));
    }

    ProjectedView {
        lines,
        arcs: Vec::new(),
        direction: ViewDirection::Front, // placeholder
        bounds: (min, max),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn top_view_box() {
        let b = physical_brep::make_box(20.0, 10.0, 15.0);
        let view = generate_view(&b, ViewDirection::Top);
        assert!(!view.lines.is_empty());
        assert!(view.width() > 0.0);
        assert!(view.height() > 0.0);
    }

    #[test]
    fn front_view_box() {
        let b = physical_brep::make_box(20.0, 10.0, 15.0);
        let view = generate_view(&b, ViewDirection::Front);
        assert!(!view.lines.is_empty());
    }

    #[test]
    fn view_direction_roundtrip() {
        let b = physical_brep::make_box(5.0, 5.0, 5.0);
        for dir in [ViewDirection::Top, ViewDirection::Front, ViewDirection::Left] {
            let view = generate_view(&b, dir);
            assert_eq!(view.direction, dir);
        }
    }

    #[test]
    fn project_point_top_view() {
        let p = DVec3::new(10.0, 5.0, 3.0);
        let proj = project_point(p, ViewDirection::Top);
        // Top view: looking down -Y. The view matrix projects X→x, Z→y.
        // Just verify the projection is non-degenerate (point maps to 2D).
        assert!(proj.x.is_finite());
        assert!(proj.y.is_finite());
        // Different 3D points should produce different 2D points
        let p2 = DVec3::new(20.0, 5.0, 3.0);
        let proj2 = project_point(p2, ViewDirection::Top);
        assert!((proj.x - proj2.x).abs() > 1.0, "different X should project differently");
    }

    #[test]
    fn projected_line_length() {
        let line = ProjectedLine {
            start: DVec2::new(0.0, 0.0),
            end: DVec2::new(3.0, 4.0),
            visible: true,
        };
        assert!((line.length() - 5.0).abs() < 1e-10);
    }

    #[test]
    fn projected_line_midpoint() {
        let line = ProjectedLine {
            start: DVec2::new(0.0, 0.0),
            end: DVec2::new(10.0, 6.0),
            visible: true,
        };
        let mid = line.midpoint();
        assert!((mid.x - 5.0).abs() < 1e-10);
        assert!((mid.y - 3.0).abs() < 1e-10);
    }

    #[test]
    fn dimension_generation_front() {
        let b = physical_brep::make_box(30.0, 20.0, 10.0);
        let dims = generate_dimensions(&b, ViewDirection::Front, 5.0);
        assert_eq!(dims.len(), 2, "front view should have width and height dimensions");
        assert!(dims.iter().any(|d| (d.value_mm - 30.0).abs() < 0.1), "should have 30mm width");
        assert!(dims.iter().any(|d| (d.value_mm - 20.0).abs() < 0.1), "should have 20mm height");
    }

    #[test]
    fn dimension_generation_top() {
        let b = physical_brep::make_box(30.0, 20.0, 10.0);
        let dims = generate_dimensions(&b, ViewDirection::Top, 5.0);
        assert_eq!(dims.len(), 2);
    }

    #[test]
    fn isometric_projection_not_zero() {
        let p = DVec3::new(10.0, 5.0, 3.0);
        let iso = project_isometric(p);
        assert!(iso.x.abs() > 0.1 || iso.y.abs() > 0.1);
    }

    #[test]
    fn isometric_view_box() {
        let b = physical_brep::make_box(10.0, 10.0, 10.0);
        let view = generate_isometric_view(&b);
        assert!(!view.lines.is_empty());
        assert!(view.width() > 0.0);
        assert!(view.height() > 0.0);
    }

    #[test]
    fn visible_hidden_count() {
        let view = ProjectedView {
            lines: vec![
                ProjectedLine { start: DVec2::ZERO, end: DVec2::X, visible: true },
                ProjectedLine { start: DVec2::ZERO, end: DVec2::Y, visible: false },
                ProjectedLine { start: DVec2::X, end: DVec2::Y, visible: true },
            ],
            arcs: Vec::new(),
            direction: ViewDirection::Front,
            bounds: (DVec2::ZERO, DVec2::new(1.0, 1.0)),
        };
        assert_eq!(view.visible_count(), 2);
        assert_eq!(view.hidden_count(), 1);
    }

    #[test]
    fn all_six_views_produce_output() {
        let b = physical_brep::make_box(15.0, 10.0, 8.0);
        for dir in [
            ViewDirection::Top, ViewDirection::Bottom,
            ViewDirection::Front, ViewDirection::Back,
            ViewDirection::Left, ViewDirection::Right,
        ] {
            let view = generate_view(&b, dir);
            assert!(!view.lines.is_empty(), "view {:?} should produce lines", dir);
        }
    }

    #[test]
    fn view_matrix_orthogonal() {
        for dir in [ViewDirection::Top, ViewDirection::Front, ViewDirection::Left] {
            let m = dir.view_matrix();
            // Rows should be approximately unit vectors
            let row0 = DVec3::new(m.col(0).x, m.col(1).x, m.col(2).x);
            let row1 = DVec3::new(m.col(0).y, m.col(1).y, m.col(2).y);
            assert!((row0.length() - 1.0).abs() < 1e-10, "row 0 should be unit for {:?}", dir);
            assert!((row1.length() - 1.0).abs() < 1e-10, "row 1 should be unit for {:?}", dir);
            assert!(row0.dot(row1).abs() < 1e-10, "rows should be orthogonal for {:?}", dir);
        }
    }
}
