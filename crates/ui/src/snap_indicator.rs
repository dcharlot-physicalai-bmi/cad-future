//! Snap indicator — visual snap guides and axis constraint overlay.
//!
//! Shows grid snap points, axis constraint lines, and distance indicators
//! during move/rotate/scale operations.

use crate::draw::DrawList;
use crate::font;

/// Which axis is being constrained.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SnapAxis {
    None,
    X,
    Y,
    Z,
    XZ, // ground plane
}

impl SnapAxis {
    pub fn color(self) -> [f32; 4] {
        match self {
            Self::X => [0.9, 0.2, 0.2, 0.8],  // red
            Self::Y => [0.2, 0.9, 0.2, 0.8],  // green
            Self::Z => [0.2, 0.2, 0.9, 0.8],  // blue
            Self::XZ => [0.6, 0.6, 0.2, 0.6], // yellow
            Self::None => [0.5, 0.5, 0.5, 0.3],
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::None => "",
            Self::X => "X",
            Self::Y => "Y",
            Self::Z => "Z",
            Self::XZ => "XZ",
        }
    }
}

/// A single snap point indicator.
#[derive(Clone, Debug)]
pub struct SnapPoint {
    /// Screen coordinates.
    pub screen_x: f32,
    pub screen_y: f32,
    /// Whether this point is the active snap target.
    pub active: bool,
    /// Distance label (e.g., "0.50").
    pub distance_label: Option<String>,
}

/// Visual snap guides and axis constraint overlay.
pub struct SnapIndicator {
    /// Whether snap visualization is enabled.
    pub visible: bool,
    /// Current constraint axis.
    pub axis: SnapAxis,
    /// Snap points to draw.
    pub points: Vec<SnapPoint>,
    /// Origin screen position (where the drag started).
    pub origin_screen: Option<(f32, f32)>,
    /// Current position screen.
    pub current_screen: Option<(f32, f32)>,
    /// Whether we're currently in a drag operation.
    pub dragging: bool,
    /// Grid snap active.
    pub grid_snap: bool,
    /// Grid snap size.
    pub grid_size: f32,
    /// Distance readout.
    pub distance: Option<f32>,
}

impl SnapIndicator {
    pub fn new() -> Self {
        Self {
            visible: true,
            axis: SnapAxis::None,
            points: Vec::new(),
            origin_screen: None,
            current_screen: None,
            dragging: false,
            grid_snap: true,
            grid_size: 0.5,
            distance: None,
        }
    }

    /// Begin a drag operation from a screen position.
    pub fn begin_drag(&mut self, sx: f32, sy: f32) {
        self.dragging = true;
        self.origin_screen = Some((sx, sy));
        self.current_screen = Some((sx, sy));
        self.points.clear();
    }

    /// Update the current drag position.
    pub fn update_drag(&mut self, sx: f32, sy: f32) {
        self.current_screen = Some((sx, sy));
    }

    /// End the drag operation.
    pub fn end_drag(&mut self) {
        self.dragging = false;
        self.origin_screen = None;
        self.current_screen = None;
        self.points.clear();
        self.axis = SnapAxis::None;
        self.distance = None;
    }

    pub fn clear_points(&mut self) {
        self.points.clear();
    }

    pub fn add_point(&mut self, sx: f32, sy: f32, active: bool, label: Option<&str>) {
        self.points.push(SnapPoint {
            screen_x: sx,
            screen_y: sy,
            active,
            distance_label: label.map(|s| s.to_string()),
        });
    }

    /// Draw snap indicators.
    pub fn draw(
        &self,
        dl: &mut DrawList,
        _screen_w: f32,
        _screen_h: f32,
    ) {
        if !self.visible { return; }

        // Axis constraint line
        if self.dragging {
            if let (Some((ox, oy)), Some((cx, cy))) = (self.origin_screen, self.current_screen) {
                let color = self.axis.color();

                // Draw constraint line from origin to current
                let dx = cx - ox;
                let dy = cy - oy;
                let len = (dx * dx + dy * dy).sqrt();
                if len > 2.0 {
                    // Axis line (thick)
                    if (dx).abs() > (dy).abs() {
                        // More horizontal
                        let min_x = ox.min(cx);
                        dl.push_quad(min_x, oy - 1.0, (cx - ox).abs(), 2.0, color);
                    } else {
                        // More vertical
                        let min_y = oy.min(cy);
                        dl.push_quad(ox - 1.0, min_y, 2.0, (cy - oy).abs(), color);
                    }

                    // Origin crosshair
                    dl.push_quad(ox - 6.0, oy, 12.0, 1.0, color);
                    dl.push_quad(ox, oy - 6.0, 1.0, 12.0, color);

                    // Current position dot
                    dl.push_quad(cx - 3.0, cy - 3.0, 6.0, 6.0, color);
                }

                // Distance label
                if let Some(dist) = self.distance {
                    let label = format!("{:.2}", dist);
                    let lx = (ox + cx) * 0.5;
                    let ly = ((oy + cy) * 0.5) - 14.0;
                    let label_w = font::measure_text(&label, 11.0, None);

                    // Background pill
                    dl.push_quad(lx - 4.0, ly - 2.0, label_w + 8.0, 16.0,
                        [0.0, 0.0, 0.0, 0.7]);
                    emit_text(dl, &label, lx, ly, 11.0, [1.0, 1.0, 1.0, 0.9]);
                }

                // Axis label
                let axis_label = self.axis.label();
                if !axis_label.is_empty() {
                    emit_text(dl, axis_label, cx + 10.0, cy - 6.0, 12.0, color);
                }
            }
        }

        // Snap points
        for point in &self.points {
            let size = if point.active { 8.0 } else { 4.0 };
            let color = if point.active {
                [1.0, 0.8, 0.2, 0.9]
            } else {
                [0.5, 0.5, 0.5, 0.5]
            };

            // Diamond shape (rotated quad approximation)
            dl.push_quad(
                point.screen_x - size * 0.5,
                point.screen_y - size * 0.5,
                size, size,
                color,
            );

            // Ring around active point
            if point.active {
                let ring = size + 4.0;
                let rx = point.screen_x - ring * 0.5;
                let ry = point.screen_y - ring * 0.5;
                let ring_col = [1.0, 0.9, 0.3, 0.5];
                dl.push_quad(rx, ry, ring, 1.0, ring_col);
                dl.push_quad(rx, ry + ring, ring, 1.0, ring_col);
                dl.push_quad(rx, ry, 1.0, ring, ring_col);
                dl.push_quad(rx + ring, ry, 1.0, ring, ring_col);
            }

            // Distance label
            if let Some(ref label) = point.distance_label {
                emit_text(dl, label, point.screen_x + 8.0, point.screen_y - 6.0, 10.0,
                    [0.8, 0.8, 0.8, 0.8]);
            }
        }

        // Grid snap indicator
        if self.grid_snap && self.dragging {
            let label = format!("Grid: {:.1}", self.grid_size);
            emit_text(dl, &label, 8.0, 8.0, 9.0, [0.5, 0.5, 0.5, 0.4]);
        }
    }
}

impl Default for SnapIndicator {
    fn default() -> Self {
        Self::new()
    }
}

fn emit_text(dl: &mut DrawList, text: &str, x: f32, y: f32, size: f32, color: [f32; 4]) {
    let mut cx = x;
    for c in text.chars() {
        let params = font::CharQuadParams {
            c, x: cx, y, size, color, atlas: None,
        };
        cx += font::emit_char_quads(&params, &mut dl.vertices, &mut dl.indices);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn axis_colors_distinct() {
        let x_col = SnapAxis::X.color();
        let y_col = SnapAxis::Y.color();
        let z_col = SnapAxis::Z.color();
        // Red channel dominates X
        assert!(x_col[0] > x_col[1]);
        assert!(x_col[0] > x_col[2]);
        // Green channel dominates Y
        assert!(y_col[1] > y_col[0]);
        assert!(y_col[1] > y_col[2]);
        // Blue channel dominates Z
        assert!(z_col[2] > z_col[0]);
        assert!(z_col[2] > z_col[1]);
    }

    #[test]
    fn drag_lifecycle() {
        let mut si = SnapIndicator::new();
        assert!(!si.dragging);
        si.begin_drag(100.0, 200.0);
        assert!(si.dragging);
        assert_eq!(si.origin_screen, Some((100.0, 200.0)));
        si.update_drag(150.0, 200.0);
        assert_eq!(si.current_screen, Some((150.0, 200.0)));
        si.end_drag();
        assert!(!si.dragging);
        assert!(si.origin_screen.is_none());
    }

    #[test]
    fn snap_points() {
        let mut si = SnapIndicator::new();
        si.add_point(10.0, 20.0, true, Some("0.50"));
        si.add_point(30.0, 40.0, false, None);
        assert_eq!(si.points.len(), 2);
        assert!(si.points[0].active);
        assert!(!si.points[1].active);
        si.clear_points();
        assert!(si.points.is_empty());
    }
}
