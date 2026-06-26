use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::action::ActionBinding;
use crate::keycode::{KeyCode, MouseButton};
use crate::state::ActionState;

/// Which mouse motion axis to read for an axis binding.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum MouseAxis {
    X,
    Y,
    Wheel,
}

/// Maps a virtual axis to digital actions and/or mouse axis.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AxisBinding {
    /// Name of the action contributing the positive (+1) direction.
    pub positive: String,
    /// Name of the action contributing the negative (-1) direction.
    pub negative: String,
    /// Optional mouse axis that also feeds into this virtual axis.
    pub mouse_axis: Option<MouseAxis>,
}

/// Serializable snapshot of all bindings (for save/load of rebindings).
#[derive(Serialize, Deserialize)]
struct BindingsSnapshot {
    actions: HashMap<String, ActionBinding>,
    axes: HashMap<String, AxisBinding>,
}

/// The central input mapping system.
///
/// Receives raw key/mouse events, tracks per-action state, and provides
/// high-level query methods for application logic.
pub struct InputMap {
    bindings: HashMap<String, ActionBinding>,
    states: HashMap<String, ActionState>,
    axes: HashMap<String, AxisBinding>,
    axis_values: HashMap<String, f32>,

    pressed_keys: HashSet<KeyCode>,
    pressed_mouse: HashSet<MouseButton>,

    mouse_dx: f32,
    mouse_dy: f32,
    mouse_wheel: f32,
}

impl InputMap {
    /// Creates a new, empty input map with no bindings.
    pub fn new() -> Self {
        Self {
            bindings: HashMap::new(),
            states: HashMap::new(),
            axes: HashMap::new(),
            axis_values: HashMap::new(),
            pressed_keys: HashSet::new(),
            pressed_mouse: HashSet::new(),
            mouse_dx: 0.0,
            mouse_dy: 0.0,
            mouse_wheel: 0.0,
        }
    }

    // ── Configuration ──────────────────────────────────────────────────

    /// Registers (or replaces) an action binding by name.
    pub fn bind_action(&mut self, name: &str, binding: ActionBinding) {
        self.bindings.insert(name.to_string(), binding);
        self.states.entry(name.to_string()).or_insert(ActionState::Idle);
    }

    /// Registers (or replaces) a virtual axis binding by name.
    pub fn bind_axis(&mut self, name: &str, binding: AxisBinding) {
        self.axes.insert(name.to_string(), binding);
        self.axis_values.entry(name.to_string()).or_insert(0.0);
    }

    // ── Raw event ingestion ────────────────────────────────────────────

    /// Called when a keyboard key is pressed.
    pub fn key_down(&mut self, code: KeyCode) {
        if self.pressed_keys.insert(code.clone()) {
            self.activate_actions_for_key(&code);
        }
    }

    /// Called when a keyboard key is released.
    pub fn key_up(&mut self, code: KeyCode) {
        if self.pressed_keys.remove(&code) {
            self.deactivate_actions_for_key(&code);
        }
    }

    /// Called when a mouse button is pressed.
    pub fn mouse_button_down(&mut self, button: MouseButton) {
        if self.pressed_mouse.insert(button) {
            self.activate_actions_for_mouse(button);
        }
    }

    /// Called when a mouse button is released.
    pub fn mouse_button_up(&mut self, button: MouseButton) {
        if self.pressed_mouse.remove(&button) {
            self.deactivate_actions_for_mouse(button);
        }
    }

    /// Called when the mouse moves. Deltas accumulate until `end_frame`.
    pub fn mouse_move(&mut self, dx: f32, dy: f32) {
        self.mouse_dx += dx;
        self.mouse_dy += dy;
    }

    /// Called when the mouse wheel scrolls. Deltas accumulate until `end_frame`.
    pub fn mouse_wheel(&mut self, delta: f32) {
        self.mouse_wheel += delta;
    }

    // ── Query ──────────────────────────────────────────────────────────

    /// Returns the current state of the named action.
    pub fn action(&self, name: &str) -> ActionState {
        self.states.get(name).copied().unwrap_or(ActionState::Idle)
    }

    /// Returns `true` if the action was just pressed this frame.
    pub fn pressed(&self, name: &str) -> bool {
        self.action(name) == ActionState::JustPressed
    }

    /// Returns `true` if the action is currently active (just pressed or held).
    pub fn held(&self, name: &str) -> bool {
        self.action(name).is_active()
    }

    /// Returns `true` if the action was just released this frame.
    pub fn released(&self, name: &str) -> bool {
        self.action(name) == ActionState::JustReleased
    }

    /// Returns the current value of a virtual axis.
    pub fn axis(&self, name: &str) -> f32 {
        self.axis_values.get(name).copied().unwrap_or(0.0)
    }

    // ── Frame lifecycle ────────────────────────────────────────────────

    /// Advances action states and computes axis values for this frame.
    pub fn update(&mut self, dt: f32) {
        for state in self.states.values_mut() {
            *state = match *state {
                ActionState::JustPressed => ActionState::Held(dt),
                ActionState::Held(t) => ActionState::Held(t + dt),
                ActionState::JustReleased => ActionState::Idle,
                ActionState::Idle => ActionState::Idle,
            };
        }

        for (axis_name, binding) in &self.axes {
            let mut value = 0.0_f32;

            if self.is_action_active(&binding.positive) {
                value += 1.0;
            }
            if self.is_action_active(&binding.negative) {
                value -= 1.0;
            }

            if let Some(ref mouse_axis) = binding.mouse_axis {
                let analog = match mouse_axis {
                    MouseAxis::X => self.mouse_dx,
                    MouseAxis::Y => self.mouse_dy,
                    MouseAxis::Wheel => self.mouse_wheel,
                };
                value += analog;
            }

            if binding.mouse_axis.is_none() {
                value = value.clamp(-1.0, 1.0);
            }

            self.axis_values.insert(axis_name.clone(), value);
        }
    }

    /// Resets per-frame accumulators. Call once per frame after all queries.
    pub fn end_frame(&mut self) {
        self.mouse_dx = 0.0;
        self.mouse_dy = 0.0;
        self.mouse_wheel = 0.0;
    }

    // ── Serialization ──────────────────────────────────────────────────

    /// Serializes all bindings to a JSON string.
    pub fn save_bindings(&self) -> String {
        let snapshot = BindingsSnapshot {
            actions: self.bindings.clone(),
            axes: self.axes.clone(),
        };
        serde_json::to_string_pretty(&snapshot).expect("bindings should always serialize")
    }

    /// Loads bindings from a JSON string, replacing all current bindings.
    pub fn load_bindings(&mut self, json: &str) -> Result<(), serde_json::Error> {
        let snapshot: BindingsSnapshot = serde_json::from_str(json)?;

        self.bindings = snapshot.actions;
        self.axes = snapshot.axes;

        self.states.clear();
        for name in self.bindings.keys() {
            self.states.insert(name.clone(), ActionState::Idle);
        }
        self.axis_values.clear();
        for name in self.axes.keys() {
            self.axis_values.insert(name.clone(), 0.0);
        }
        self.pressed_keys.clear();
        self.pressed_mouse.clear();
        self.mouse_dx = 0.0;
        self.mouse_dy = 0.0;
        self.mouse_wheel = 0.0;

        Ok(())
    }

    // ── Presets ─────────────────────────────────────────────────────────

    /// Creates an input map pre-configured for CAD viewport navigation:
    ///
    /// - **orbit**: Mouse Right (hold + drag to orbit)
    /// - **pan**: Mouse Middle (hold + drag to pan)
    /// - **select**: Mouse Left
    /// - **zoom_in / zoom_out**: via mouse wheel axis
    /// - **undo**: Ctrl+Z, **redo**: Ctrl+Shift+Z
    /// - **delete**: Delete/Backspace
    /// - **escape**: Escape
    /// - **look_x / look_y / scroll**: mouse axes
    pub fn cad_preset() -> Self {
        let mut map = Self::new();

        map.bind_action("select", ActionBinding::from_mouse(MouseButton::Left));
        map.bind_action("orbit", ActionBinding::from_mouse(MouseButton::Right));
        map.bind_action("pan", ActionBinding::from_mouse(MouseButton::Middle));
        map.bind_action("delete", ActionBinding::from_keys(vec![KeyCode::Delete, KeyCode::Backspace]));
        map.bind_action("escape", ActionBinding::from_key(KeyCode::Escape));
        map.bind_action("undo", ActionBinding::from_key(KeyCode::KeyZ));
        map.bind_action("redo", ActionBinding::from_key(KeyCode::KeyY));
        map.bind_action("ctrl_mod", ActionBinding::from_keys(vec![KeyCode::ControlLeft, KeyCode::ControlRight, KeyCode::MetaLeft, KeyCode::MetaRight]));
        map.bind_action("shift_mod", ActionBinding::from_keys(vec![KeyCode::ShiftLeft, KeyCode::ShiftRight]));

        map.bind_axis("look_x", AxisBinding {
            positive: String::new(),
            negative: String::new(),
            mouse_axis: Some(MouseAxis::X),
        });
        map.bind_axis("look_y", AxisBinding {
            positive: String::new(),
            negative: String::new(),
            mouse_axis: Some(MouseAxis::Y),
        });
        map.bind_axis("scroll", AxisBinding {
            positive: String::new(),
            negative: String::new(),
            mouse_axis: Some(MouseAxis::Wheel),
        });

        map
    }

    // ── Private helpers ────────────────────────────────────────────────

    fn is_action_active(&self, name: &str) -> bool {
        if name.is_empty() {
            return false;
        }
        self.states.get(name).is_some_and(|s| s.is_active())
    }

    fn activate_actions_for_key(&mut self, code: &KeyCode) {
        let names: Vec<String> = self
            .bindings
            .iter()
            .filter(|(_, b)| b.keys.contains(code))
            .map(|(name, _)| name.clone())
            .collect();

        for name in names {
            self.try_activate(&name);
        }
    }

    fn activate_actions_for_mouse(&mut self, button: MouseButton) {
        let names: Vec<String> = self
            .bindings
            .iter()
            .filter(|(_, b)| b.mouse_buttons.contains(&button))
            .map(|(name, _)| name.clone())
            .collect();

        for name in names {
            self.try_activate(&name);
        }
    }

    fn try_activate(&mut self, name: &str) {
        if let Some(state) = self.states.get_mut(name)
            && !state.is_active()
        {
            *state = ActionState::JustPressed;
        }
    }

    fn deactivate_actions_for_key(&mut self, code: &KeyCode) {
        let names: Vec<String> = self
            .bindings
            .iter()
            .filter(|(_, b)| b.keys.contains(code))
            .map(|(name, _)| name.clone())
            .collect();

        for name in names {
            self.try_deactivate(&name);
        }
    }

    fn deactivate_actions_for_mouse(&mut self, button: MouseButton) {
        let names: Vec<String> = self
            .bindings
            .iter()
            .filter(|(_, b)| b.mouse_buttons.contains(&button))
            .map(|(name, _)| name.clone())
            .collect();

        for name in names {
            self.try_deactivate(&name);
        }
    }

    fn try_deactivate(&mut self, name: &str) {
        if let Some(binding) = self.bindings.get(name) {
            let any_key_held = binding
                .keys
                .iter()
                .any(|k| self.pressed_keys.contains(k));
            let any_mouse_held = binding
                .mouse_buttons
                .iter()
                .any(|b| self.pressed_mouse.contains(b));

            if !any_key_held
                && !any_mouse_held
                && let Some(state) = self.states.get_mut(name)
                && state.is_active()
            {
                *state = ActionState::JustReleased;
            }
        }
    }
}

impl Default for InputMap {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idle_to_just_pressed() {
        let mut map = InputMap::new();
        map.bind_action("select", ActionBinding::from_mouse(MouseButton::Left));
        assert_eq!(map.action("select"), ActionState::Idle);
        map.mouse_button_down(MouseButton::Left);
        assert_eq!(map.action("select"), ActionState::JustPressed);
    }

    #[test]
    fn just_pressed_to_held() {
        let mut map = InputMap::new();
        map.bind_action("orbit", ActionBinding::from_mouse(MouseButton::Right));
        map.mouse_button_down(MouseButton::Right);
        map.update(1.0 / 60.0);
        match map.action("orbit") {
            ActionState::Held(t) => assert!(t > 0.0),
            other => panic!("expected Held, got {other:?}"),
        }
    }

    #[test]
    fn release_to_just_released_then_idle() {
        let mut map = InputMap::new();
        map.bind_action("select", ActionBinding::from_mouse(MouseButton::Left));
        map.mouse_button_down(MouseButton::Left);
        map.update(0.016);
        map.mouse_button_up(MouseButton::Left);
        assert_eq!(map.action("select"), ActionState::JustReleased);
        map.update(0.016);
        assert_eq!(map.action("select"), ActionState::Idle);
    }

    #[test]
    fn unknown_action_returns_idle() {
        let map = InputMap::new();
        assert_eq!(map.action("nonexistent"), ActionState::Idle);
        assert!(!map.pressed("nonexistent"));
        assert!(!map.held("nonexistent"));
        assert!(!map.released("nonexistent"));
        assert_eq!(map.axis("nonexistent"), 0.0);
    }

    #[test]
    fn mouse_axis_x() {
        let mut map = InputMap::new();
        map.bind_axis("look_x", AxisBinding {
            positive: String::new(),
            negative: String::new(),
            mouse_axis: Some(MouseAxis::X),
        });
        map.mouse_move(5.0, -3.0);
        map.update(0.016);
        assert_eq!(map.axis("look_x"), 5.0);
    }

    #[test]
    fn end_frame_resets_mouse_deltas() {
        let mut map = InputMap::new();
        map.bind_axis("look_x", AxisBinding {
            positive: String::new(),
            negative: String::new(),
            mouse_axis: Some(MouseAxis::X),
        });
        map.mouse_move(10.0, 0.0);
        map.update(0.016);
        assert_eq!(map.axis("look_x"), 10.0);
        map.end_frame();
        map.update(0.016);
        assert_eq!(map.axis("look_x"), 0.0);
    }

    #[test]
    fn save_and_load_bindings_roundtrip() {
        let original = InputMap::cad_preset();
        let json = original.save_bindings();
        let mut loaded = InputMap::new();
        loaded.load_bindings(&json).expect("valid JSON");
        assert_eq!(loaded.bindings.len(), original.bindings.len());
        assert_eq!(loaded.axes.len(), original.axes.len());
    }

    #[test]
    fn cad_preset_has_expected_bindings() {
        let map = InputMap::cad_preset();
        assert!(map.bindings.contains_key("select"));
        assert!(map.bindings.contains_key("orbit"));
        assert!(map.bindings.contains_key("pan"));
        assert!(map.axes.contains_key("look_x"));
        assert!(map.axes.contains_key("look_y"));
        assert!(map.axes.contains_key("scroll"));
    }
}
