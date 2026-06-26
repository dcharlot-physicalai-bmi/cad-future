//! Cross-section clip plane — cuts the scene to reveal internal structure.
//!
//! Renders a visual plane indicator and provides clip plane data
//! for the forward shader. The clip plane is defined by a point and normal.

use glam::Vec3;

/// A cross-section clipping plane.
#[derive(Clone, Debug)]
pub struct ClipPlane {
    /// Whether the clip plane is active.
    pub enabled: bool,
    /// Which axis the plane is perpendicular to.
    pub axis: ClipAxis,
    /// Position along the axis (world units).
    pub position: f32,
    /// Whether to show the intersection fill.
    pub show_cap: bool,
    /// Flip which side is clipped.
    pub flipped: bool,
}

/// Axis-aligned clip plane direction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClipAxis {
    X,
    Y,
    Z,
}

impl ClipAxis {
    pub fn normal(self) -> Vec3 {
        match self {
            Self::X => Vec3::X,
            Self::Y => Vec3::Y,
            Self::Z => Vec3::Z,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::X => "X",
            Self::Y => "Y",
            Self::Z => "Z",
        }
    }

    pub fn cycle(self) -> Self {
        match self {
            Self::X => Self::Y,
            Self::Y => Self::Z,
            Self::Z => Self::X,
        }
    }
}

impl ClipPlane {
    pub fn new() -> Self {
        Self {
            enabled: false,
            axis: ClipAxis::Y,
            position: 1.0,
            show_cap: true,
            flipped: false,
        }
    }

    /// Clip plane equation as [A, B, C, D] where Ax + By + Cz + D = 0.
    /// Points on the positive side are kept, negative side is clipped.
    pub fn equation(&self) -> [f32; 4] {
        let n = if self.flipped {
            -self.axis.normal()
        } else {
            self.axis.normal()
        };
        [n.x, n.y, n.z, -n.dot(self.axis.normal() * self.position)]
    }

    /// Move the clip plane along its axis.
    pub fn slide(&mut self, delta: f32) {
        self.position += delta;
    }

    /// Toggle the clip plane on/off.
    pub fn toggle(&mut self) {
        self.enabled = !self.enabled;
    }

    /// Flip which side of the plane is clipped.
    pub fn flip(&mut self) {
        self.flipped = !self.flipped;
    }
}

impl Default for ClipPlane {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_state() {
        let cp = ClipPlane::new();
        assert!(!cp.enabled);
        assert_eq!(cp.axis, ClipAxis::Y);
    }

    #[test]
    fn equation_y_axis() {
        let cp = ClipPlane {
            enabled: true,
            axis: ClipAxis::Y,
            position: 2.0,
            show_cap: true,
            flipped: false,
        };
        let eq = cp.equation();
        // Normal is (0, 1, 0), D = -1 * 2.0 = -2.0
        assert!((eq[0]).abs() < 1e-6);
        assert!((eq[1] - 1.0).abs() < 1e-6);
        assert!((eq[2]).abs() < 1e-6);
        assert!((eq[3] + 2.0).abs() < 1e-6);
    }

    #[test]
    fn flip_negates_normal() {
        let mut cp = ClipPlane::new();
        cp.axis = ClipAxis::X;
        cp.position = 1.0;
        let eq_normal = cp.equation();
        cp.flip();
        let eq_flipped = cp.equation();
        assert!((eq_normal[0] + eq_flipped[0]).abs() < 1e-6);
    }

    #[test]
    fn axis_cycle() {
        assert_eq!(ClipAxis::X.cycle(), ClipAxis::Y);
        assert_eq!(ClipAxis::Y.cycle(), ClipAxis::Z);
        assert_eq!(ClipAxis::Z.cycle(), ClipAxis::X);
    }
}
