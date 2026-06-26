//! Marking menu — 8-slot radial right-click menu with gesture support.
//!
//! Inspired by Fusion 360 marking menu and SolidWorks mouse gestures.
//! 8 directional slots (N, NE, E, SE, S, SW, W, NW) for muscle-memory access.

use crate::draw::DrawList;
use crate::font;

use std::f32::consts::PI;

/// A single entry in a marking menu slot.
#[derive(Clone, Debug)]
pub struct MarkingEntry {
    /// Display label.
    pub label: String,
    /// Action ID dispatched on selection.
    pub action_id: &'static str,
    /// Icon character.
    pub icon: &'static str,
}

impl MarkingEntry {
    pub fn new(label: &str, action_id: &'static str, icon: &'static str) -> Self {
        Self {
            label: label.to_string(),
            action_id,
            icon,
        }
    }
}

/// Direction slots for the 8-way radial menu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MarkingSlot {
    N = 0,
    NE = 1,
    E = 2,
    SE = 3,
    S = 4,
    SW = 5,
    W = 6,
    NW = 7,
}

impl MarkingSlot {
    /// Angle in radians for each slot (0 = right, CCW).
    pub fn angle(self) -> f32 {
        match self {
            Self::E => 0.0,
            Self::NE => PI * 0.25,
            Self::N => PI * 0.5,
            Self::NW => PI * 0.75,
            Self::W => PI,
            Self::SW => PI * 1.25,
            Self::S => PI * 1.5,
            Self::SE => PI * 1.75,
        }
    }

    pub fn all() -> &'static [MarkingSlot] {
        &[
            Self::N, Self::NE, Self::E, Self::SE,
            Self::S, Self::SW, Self::W, Self::NW,
        ]
    }

    /// Determine which slot a direction vector points to.
    pub fn from_direction(dx: f32, dy: f32) -> Self {
        // dy is inverted (screen coords: down = positive)
        let angle = (-dy).atan2(dx);
        let normalized = if angle < 0.0 { angle + 2.0 * PI } else { angle };
        let sector = ((normalized + PI / 8.0) / (PI / 4.0)) as usize % 8;
        match sector {
            0 => Self::E,
            1 => Self::NE,
            2 => Self::N,
            3 => Self::NW,
            4 => Self::W,
            5 => Self::SW,
            6 => Self::S,
            7 => Self::SE,
            _ => Self::E,
        }
    }
}

/// The radial marking menu.
pub struct MarkingMenu {
    /// Whether the menu is currently shown.
    pub visible: bool,
    /// Center position (where right-click occurred).
    pub center_x: f32,
    pub center_y: f32,
    /// Entries in each slot (8 slots, some may be empty).
    pub slots: [Option<MarkingEntry>; 8],
    /// Currently hovered slot.
    pub hovered_slot: Option<MarkingSlot>,
    /// Radius of the radial menu.
    pub radius: f32,
    /// Dead zone radius (must drag past this to select).
    pub dead_zone: f32,
    /// Time the menu has been open (for gesture detection).
    pub open_time: f32,
}

impl MarkingMenu {
    pub fn new() -> Self {
        Self {
            visible: false,
            center_x: 0.0,
            center_y: 0.0,
            slots: Default::default(),
            hovered_slot: None,
            radius: 80.0,
            dead_zone: 20.0,
            open_time: 0.0,
        }
    }

    /// Open the menu at a given position with a set of entries.
    pub fn open(&mut self, x: f32, y: f32, slots: [Option<MarkingEntry>; 8]) {
        self.visible = true;
        self.center_x = x;
        self.center_y = y;
        self.slots = slots;
        self.hovered_slot = None;
        self.open_time = 0.0;
    }

    pub fn close(&mut self) {
        self.visible = false;
        self.hovered_slot = None;
    }

    /// Update hover state based on mouse position.
    pub fn update_hover(&mut self, mx: f32, my: f32) {
        if !self.visible { return; }
        let dx = mx - self.center_x;
        let dy = my - self.center_y;
        let dist = (dx * dx + dy * dy).sqrt();
        if dist < self.dead_zone {
            self.hovered_slot = None;
        } else {
            let slot = MarkingSlot::from_direction(dx, dy);
            // Only highlight if the slot has an entry
            if self.slots[slot as usize].is_some() {
                self.hovered_slot = Some(slot);
            } else {
                self.hovered_slot = None;
            }
        }
    }

    /// Call on mouse release. Returns the action ID if a slot was selected.
    pub fn release(&mut self) -> Option<&'static str> {
        if !self.visible { return None; }
        let result = self.hovered_slot.and_then(|slot| {
            self.slots[slot as usize].as_ref().map(|e| e.action_id)
        });
        self.close();
        result
    }

    pub fn update(&mut self, dt: f32) {
        if self.visible {
            self.open_time += dt;
        }
    }

    /// Draw the radial menu.
    pub fn draw(
        &self,
        dl: &mut DrawList,
        _screen_w: f32,
        _screen_h: f32,
        bg_color: [f32; 4],
        text_color: [f32; 4],
        accent_color: [f32; 4],
    ) {
        if !self.visible { return; }

        let cx = self.center_x;
        let cy = self.center_y;

        // Backdrop circle (approximated with quads in a ring)
        // Center dot
        dl.push_quad(cx - 4.0, cy - 4.0, 8.0, 8.0, accent_color);

        // Draw each slot
        for &slot in MarkingSlot::all() {
            let entry = match &self.slots[slot as usize] {
                Some(e) => e,
                None => continue,
            };

            let angle = slot.angle();
            let slot_x = cx + angle.cos() * self.radius;
            let slot_y = cy - angle.sin() * self.radius; // screen Y inverted

            let is_hovered = self.hovered_slot == Some(slot);

            // Slot background pill
            let pill_w = 72.0;
            let pill_h = 28.0;
            let pill_x = slot_x - pill_w * 0.5;
            let pill_y = slot_y - pill_h * 0.5;

            let pill_bg = if is_hovered {
                accent_color
            } else {
                [bg_color[0], bg_color[1], bg_color[2], 0.92]
            };
            dl.push_quad(pill_x, pill_y, pill_w, pill_h, pill_bg);

            // Border
            let border_col = if is_hovered {
                [1.0, 1.0, 1.0, 0.4]
            } else {
                [bg_color[0] + 0.15, bg_color[1] + 0.15, bg_color[2] + 0.15, 0.6]
            };
            dl.push_quad(pill_x, pill_y, pill_w, 1.0, border_col);
            dl.push_quad(pill_x, pill_y + pill_h - 1.0, pill_w, 1.0, border_col);
            dl.push_quad(pill_x, pill_y, 1.0, pill_h, border_col);
            dl.push_quad(pill_x + pill_w - 1.0, pill_y, 1.0, pill_h, border_col);

            // Icon + label
            let label_color = if is_hovered {
                [1.0, 1.0, 1.0, 1.0]
            } else {
                text_color
            };
            emit_text(dl, entry.icon, pill_x + 6.0, pill_y + 7.0, 12.0, label_color);
            emit_text(dl, &entry.label, pill_x + 20.0, pill_y + 8.0, 11.0, label_color);

            // Connection line from center to slot
            // Approximate with a thin quad
            let line_len = self.radius - 20.0;
            let lx = cx + angle.cos() * 10.0;
            let ly = cy - angle.sin() * 10.0;
            let ex = cx + angle.cos() * (line_len - 10.0);
            let ey = cy - angle.sin() * (line_len - 10.0);
            let line_col = [bg_color[0] + 0.1, bg_color[1] + 0.1, bg_color[2] + 0.1, 0.3];
            // Horizontal-ish or vertical-ish line
            if (ex - lx).abs() > (ey - ly).abs() {
                let min_x = lx.min(ex);
                let w = (ex - lx).abs().max(1.0);
                dl.push_quad(min_x, ly.min(ey), w, 1.0, line_col);
            } else {
                let min_y = ly.min(ey);
                let h = (ey - ly).abs().max(1.0);
                dl.push_quad(lx.min(ex), min_y, 1.0, h, line_col);
            }
        }
    }
}

impl Default for MarkingMenu {
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
    fn slot_direction() {
        // Pure right = E
        assert_eq!(MarkingSlot::from_direction(1.0, 0.0), MarkingSlot::E);
        // Pure up (screen: negative Y) = N
        assert_eq!(MarkingSlot::from_direction(0.0, -1.0), MarkingSlot::N);
        // Pure left = W
        assert_eq!(MarkingSlot::from_direction(-1.0, 0.0), MarkingSlot::W);
        // Pure down = S
        assert_eq!(MarkingSlot::from_direction(0.0, 1.0), MarkingSlot::S);
    }

    #[test]
    fn open_and_hover() {
        let mut mm = MarkingMenu::new();
        let mut slots: [Option<MarkingEntry>; 8] = Default::default();
        slots[MarkingSlot::N as usize] = Some(MarkingEntry::new("Undo", "edit.undo", "U"));
        slots[MarkingSlot::S as usize] = Some(MarkingEntry::new("Redo", "edit.redo", "R"));
        mm.open(100.0, 100.0, slots);
        assert!(mm.visible);

        // Move mouse north (up)
        mm.update_hover(100.0, 30.0);
        assert_eq!(mm.hovered_slot, Some(MarkingSlot::N));

        // Move mouse south (down)
        mm.update_hover(100.0, 170.0);
        assert_eq!(mm.hovered_slot, Some(MarkingSlot::S));
    }

    #[test]
    fn dead_zone() {
        let mut mm = MarkingMenu::new();
        let mut slots: [Option<MarkingEntry>; 8] = Default::default();
        slots[0] = Some(MarkingEntry::new("Test", "test", "T"));
        mm.open(100.0, 100.0, slots);
        // In dead zone
        mm.update_hover(105.0, 100.0);
        assert!(mm.hovered_slot.is_none());
    }

    #[test]
    fn release_returns_action() {
        let mut mm = MarkingMenu::new();
        let mut slots: [Option<MarkingEntry>; 8] = Default::default();
        slots[MarkingSlot::N as usize] = Some(MarkingEntry::new("Undo", "edit.undo", "U"));
        mm.open(100.0, 100.0, slots);
        mm.update_hover(100.0, 30.0);
        let action = mm.release();
        assert_eq!(action, Some("edit.undo"));
        assert!(!mm.visible);
    }
}
