//! Constraint icons — on-canvas sketch constraint indicators.
//!
//! Inspired by SolidWorks sketch constraints, Fusion 360 constraint glyphs,
//! and Onshape's visual constraint display. Shows small icons near geometry
//! to indicate applied constraints: parallel, perpendicular, tangent,
//! coincident, horizontal, vertical, fixed, equal, concentric, symmetric.

use crate::draw::DrawList;
use crate::font;
use glam::{Mat4, Vec4};

/// Types of geometric constraints.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConstraintKind {
    /// Lines are parallel.
    Parallel,
    /// Lines are perpendicular.
    Perpendicular,
    /// Curve is tangent to another.
    Tangent,
    /// Points are coincident.
    Coincident,
    /// Line is horizontal.
    Horizontal,
    /// Line is vertical.
    Vertical,
    /// Entity is fixed in space.
    Fixed,
    /// Entities have equal length/radius.
    Equal,
    /// Circles/arcs are concentric.
    Concentric,
    /// Entities are symmetric about an axis.
    Symmetric,
    /// Distance constraint with a value.
    Distance,
    /// Angle constraint with a value.
    Angle,
}

impl ConstraintKind {
    /// Icon character for this constraint.
    pub fn icon(self) -> &'static str {
        match self {
            Self::Parallel => "//",
            Self::Perpendicular => "_|",
            Self::Tangent => "T",
            Self::Coincident => ".",
            Self::Horizontal => "--",
            Self::Vertical => "|",
            Self::Fixed => "X",
            Self::Equal => "=",
            Self::Concentric => "()",
            Self::Symmetric => "<>",
            Self::Distance => "D",
            Self::Angle => "A",
        }
    }

    /// Color for this constraint type.
    pub fn color(self) -> [f32; 4] {
        match self {
            Self::Parallel | Self::Perpendicular => [0.3, 0.8, 0.3, 0.85],
            Self::Tangent | Self::Coincident => [0.8, 0.8, 0.3, 0.85],
            Self::Horizontal | Self::Vertical => [0.3, 0.6, 1.0, 0.85],
            Self::Fixed => [1.0, 0.3, 0.3, 0.85],
            Self::Equal | Self::Symmetric => [0.7, 0.5, 1.0, 0.85],
            Self::Concentric => [0.3, 0.9, 0.9, 0.85],
            Self::Distance | Self::Angle => [0.9, 0.6, 0.2, 0.85],
        }
    }

    /// Short label for tooltip/status.
    pub fn label(self) -> &'static str {
        match self {
            Self::Parallel => "Parallel",
            Self::Perpendicular => "Perpendicular",
            Self::Tangent => "Tangent",
            Self::Coincident => "Coincident",
            Self::Horizontal => "Horizontal",
            Self::Vertical => "Vertical",
            Self::Fixed => "Fixed",
            Self::Equal => "Equal",
            Self::Concentric => "Concentric",
            Self::Symmetric => "Symmetric",
            Self::Distance => "Distance",
            Self::Angle => "Angle",
        }
    }
}

/// A constraint icon placed in 3D space.
#[derive(Clone, Debug)]
pub struct ConstraintIcon {
    /// World-space position for the icon.
    pub position: [f32; 3],
    /// Type of constraint.
    pub kind: ConstraintKind,
    /// Optional value label (e.g., "25.0 mm" for distance).
    pub value_label: Option<String>,
    /// Whether this constraint is satisfied.
    pub satisfied: bool,
    /// Whether this constraint is selected/hovered.
    pub highlighted: bool,
    /// Entity indices this constraint relates to.
    pub entity_ids: [u32; 2],
}

impl ConstraintIcon {
    pub fn new(position: [f32; 3], kind: ConstraintKind) -> Self {
        Self {
            position,
            kind,
            value_label: None,
            satisfied: true,
            highlighted: false,
            entity_ids: [0, 0],
        }
    }

    pub fn with_value(mut self, label: &str) -> Self {
        self.value_label = Some(label.to_string());
        self
    }

    pub fn unsatisfied(mut self) -> Self {
        self.satisfied = false;
        self
    }

    pub fn between(mut self, a: u32, b: u32) -> Self {
        self.entity_ids = [a, b];
        self
    }
}

/// The constraint icon overlay.
pub struct ConstraintIcons {
    /// All constraint icons.
    pub icons: Vec<ConstraintIcon>,
    /// Whether constraint icons are visible.
    pub visible: bool,
    /// Hovered icon index.
    pub hovered: Option<usize>,
    /// Whether to show unsatisfied constraints with error styling.
    pub show_errors: bool,
    /// Icon display size (screen pixels).
    pub icon_size: f32,
}

impl ConstraintIcons {
    pub fn new() -> Self {
        Self {
            icons: Vec::new(),
            visible: true,
            hovered: None,
            show_errors: true,
            icon_size: 16.0,
        }
    }

    /// Add a constraint icon.
    pub fn add(&mut self, icon: ConstraintIcon) {
        self.icons.push(icon);
    }

    /// Clear all icons.
    pub fn clear(&mut self) {
        self.icons.clear();
    }

    /// Toggle visibility.
    pub fn toggle(&mut self) {
        self.visible = !self.visible;
    }

    /// Count unsatisfied constraints.
    pub fn unsatisfied_count(&self) -> usize {
        self.icons.iter().filter(|i| !i.satisfied).count()
    }

    /// Project a 3D point to screen.
    fn project(pos: [f32; 3], vp: Mat4, sw: f32, sh: f32) -> Option<(f32, f32)> {
        let clip = vp * Vec4::new(pos[0], pos[1], pos[2], 1.0);
        if clip.w <= 0.0 { return None; }
        let ndc = clip.truncate() / clip.w;
        Some((
            (ndc.x * 0.5 + 0.5) * sw,
            (1.0 - (ndc.y * 0.5 + 0.5)) * sh,
        ))
    }

    /// Hit test: returns icon index if mouse is near an icon.
    pub fn hit_test(
        &self, mx: f32, my: f32,
        vp: Mat4, sw: f32, sh: f32,
    ) -> Option<usize> {
        if !self.visible { return None; }
        let half = self.icon_size * 0.5;
        for (i, icon) in self.icons.iter().enumerate() {
            if let Some((sx, sy)) = Self::project(icon.position, vp, sw, sh) {
                if mx >= sx - half && mx <= sx + half
                    && my >= sy - half && my <= sy + half
                {
                    return Some(i);
                }
            }
        }
        None
    }

    /// Draw all constraint icons.
    pub fn draw(
        &self,
        dl: &mut DrawList,
        vp: Mat4,
        sw: f32,
        sh: f32,
    ) {
        if !self.visible { return; }

        for (i, icon) in self.icons.iter().enumerate() {
            let Some((sx, sy)) = Self::project(icon.position, vp, sw, sh) else {
                continue;
            };

            let size = self.icon_size;
            let half = size * 0.5;

            // Determine color
            let base_color = icon.kind.color();
            let color = if !icon.satisfied && self.show_errors {
                [1.0, 0.2, 0.2, 0.95] // error red
            } else if icon.highlighted || self.hovered == Some(i) {
                [1.0, 1.0, 1.0, 1.0] // bright white on hover
            } else {
                base_color
            };

            // Background pill
            let bg_alpha = if icon.highlighted || self.hovered == Some(i) { 0.85 } else { 0.6 };
            dl.push_quad(
                sx - half - 1.0, sy - half - 1.0,
                size + 2.0, size + 2.0,
                [0.0, 0.0, 0.0, bg_alpha],
            );

            // Border
            let border_color = if !icon.satisfied && self.show_errors {
                [1.0, 0.3, 0.3, 0.7]
            } else {
                [color[0], color[1], color[2], 0.4]
            };
            dl.push_quad(sx - half - 1.0, sy - half - 1.0, size + 2.0, 1.0, border_color);
            dl.push_quad(sx - half - 1.0, sy + half, size + 2.0, 1.0, border_color);
            dl.push_quad(sx - half - 1.0, sy - half - 1.0, 1.0, size + 2.0, border_color);
            dl.push_quad(sx + half, sy - half - 1.0, 1.0, size + 2.0, border_color);

            // Icon text
            let icon_text = icon.kind.icon();
            let tw = font::measure_text(icon_text, 9.0, None);
            emit_text(dl, icon_text, sx - tw * 0.5, sy - 4.0, 9.0, color);

            // Value label (shown to the right if present)
            if let Some(ref val) = icon.value_label {
                let vx = sx + half + 4.0;
                let vy = sy - 4.0;
                let vw = font::measure_text(val, 9.0, None);
                dl.push_quad(vx - 2.0, vy - 1.0, vw + 4.0, 12.0, [0.0, 0.0, 0.0, 0.6]);
                emit_text(dl, val, vx, vy, 9.0, color);
            }

            // Unsatisfied error marker (small "!" badge)
            if !icon.satisfied && self.show_errors {
                dl.push_quad(sx + half - 3.0, sy - half - 3.0, 8.0, 8.0, [1.0, 0.15, 0.15, 0.95]);
                emit_text(dl, "!", sx + half - 1.0, sy - half - 2.0, 7.0, [1.0, 1.0, 1.0, 1.0]);
            }
        }
    }
}

impl Default for ConstraintIcons {
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
    fn add_and_clear() {
        let mut ci = ConstraintIcons::new();
        ci.add(ConstraintIcon::new([0.0, 0.0, 0.0], ConstraintKind::Parallel));
        ci.add(ConstraintIcon::new([1.0, 0.0, 0.0], ConstraintKind::Fixed));
        assert_eq!(ci.icons.len(), 2);
        ci.clear();
        assert!(ci.icons.is_empty());
    }

    #[test]
    fn unsatisfied_count() {
        let mut ci = ConstraintIcons::new();
        ci.add(ConstraintIcon::new([0.0, 0.0, 0.0], ConstraintKind::Parallel));
        ci.add(ConstraintIcon::new([1.0, 0.0, 0.0], ConstraintKind::Perpendicular).unsatisfied());
        assert_eq!(ci.unsatisfied_count(), 1);
    }

    #[test]
    fn all_kinds_have_icons() {
        let kinds = [
            ConstraintKind::Parallel, ConstraintKind::Perpendicular,
            ConstraintKind::Tangent, ConstraintKind::Coincident,
            ConstraintKind::Horizontal, ConstraintKind::Vertical,
            ConstraintKind::Fixed, ConstraintKind::Equal,
            ConstraintKind::Concentric, ConstraintKind::Symmetric,
            ConstraintKind::Distance, ConstraintKind::Angle,
        ];
        for k in kinds {
            assert!(!k.icon().is_empty());
            assert!(!k.label().is_empty());
            let c = k.color();
            assert!(c[3] > 0.0);
        }
    }

    #[test]
    fn value_label() {
        let icon = ConstraintIcon::new([0.0, 0.0, 0.0], ConstraintKind::Distance)
            .with_value("25.0 mm");
        assert_eq!(icon.value_label.as_deref(), Some("25.0 mm"));
    }

    #[test]
    fn toggle_visibility() {
        let mut ci = ConstraintIcons::new();
        assert!(ci.visible);
        ci.toggle();
        assert!(!ci.visible);
    }
}
