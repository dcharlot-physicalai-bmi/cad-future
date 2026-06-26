use serde::{Deserialize, Serialize};

use crate::keycode::{KeyCode, MouseButton};

/// Defines which physical inputs trigger a named action.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ActionBinding {
    /// Keyboard keys that activate this action (any one triggers it).
    pub keys: Vec<KeyCode>,
    /// Mouse buttons that activate this action (any one triggers it).
    pub mouse_buttons: Vec<MouseButton>,
}

impl ActionBinding {
    /// Creates a binding with no inputs assigned.
    pub fn empty() -> Self {
        Self {
            keys: Vec::new(),
            mouse_buttons: Vec::new(),
        }
    }

    /// Creates a binding from keyboard keys only.
    pub fn from_keys(keys: Vec<KeyCode>) -> Self {
        Self {
            keys,
            mouse_buttons: Vec::new(),
        }
    }

    /// Creates a binding from a single key.
    pub fn from_key(key: KeyCode) -> Self {
        Self::from_keys(vec![key])
    }

    /// Creates a binding from a single mouse button.
    pub fn from_mouse(button: MouseButton) -> Self {
        Self {
            keys: Vec::new(),
            mouse_buttons: vec![button],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_binding() {
        let b = ActionBinding::empty();
        assert!(b.keys.is_empty());
        assert!(b.mouse_buttons.is_empty());
    }

    #[test]
    fn from_keys_binding() {
        let b = ActionBinding::from_keys(vec![KeyCode::KeyW, KeyCode::ArrowUp]);
        assert_eq!(b.keys.len(), 2);
        assert!(b.mouse_buttons.is_empty());
    }

    #[test]
    fn from_mouse_binding() {
        let b = ActionBinding::from_mouse(MouseButton::Left);
        assert!(b.keys.is_empty());
        assert_eq!(b.mouse_buttons.len(), 1);
    }

    #[test]
    fn serde_roundtrip() {
        let b = ActionBinding {
            keys: vec![KeyCode::Space, KeyCode::KeyE],
            mouse_buttons: vec![MouseButton::Left],
        };
        let json = serde_json::to_string(&b).unwrap();
        let deserialized: ActionBinding = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.keys.len(), 2);
        assert_eq!(deserialized.mouse_buttons.len(), 1);
    }
}
