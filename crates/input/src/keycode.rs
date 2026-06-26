use serde::{Deserialize, Serialize};

/// Keyboard key codes matching browser `KeyboardEvent.code` values.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum KeyCode {
    // Letters
    KeyA, KeyB, KeyC, KeyD, KeyE, KeyF, KeyG, KeyH, KeyI, KeyJ,
    KeyK, KeyL, KeyM, KeyN, KeyO, KeyP, KeyQ, KeyR, KeyS, KeyT,
    KeyU, KeyV, KeyW, KeyX, KeyY, KeyZ,

    // Digits
    Digit0, Digit1, Digit2, Digit3, Digit4,
    Digit5, Digit6, Digit7, Digit8, Digit9,

    // Function keys
    F1, F2, F3, F4, F5, F6, F7, F8, F9, F10, F11, F12,

    // Arrow keys
    ArrowUp, ArrowDown, ArrowLeft, ArrowRight,

    // Modifiers
    ShiftLeft, ShiftRight, ControlLeft, ControlRight,
    AltLeft, AltRight, MetaLeft, MetaRight,

    // Whitespace / editing
    Space, Enter, Tab, Backspace, Delete, Insert, Escape,

    // Navigation
    Home, End, PageUp, PageDown,

    // Punctuation / symbols
    Minus, Equal, BracketLeft, BracketRight, Backslash,
    Semicolon, Quote, Backquote, Comma, Period, Slash,

    // Lock keys
    CapsLock, NumLock, ScrollLock,

    // Numpad
    Numpad0, Numpad1, Numpad2, Numpad3, Numpad4,
    Numpad5, Numpad6, Numpad7, Numpad8, Numpad9,
    NumpadAdd, NumpadSubtract, NumpadMultiply,
    NumpadDivide, NumpadDecimal, NumpadEnter,

    // Misc
    ContextMenu, PrintScreen, Pause,

    /// Fallback for unrecognized key codes.
    Unknown(String),
}

impl From<&str> for KeyCode {
    fn from(s: &str) -> Self {
        match s {
            "KeyA" => Self::KeyA, "KeyB" => Self::KeyB, "KeyC" => Self::KeyC,
            "KeyD" => Self::KeyD, "KeyE" => Self::KeyE, "KeyF" => Self::KeyF,
            "KeyG" => Self::KeyG, "KeyH" => Self::KeyH, "KeyI" => Self::KeyI,
            "KeyJ" => Self::KeyJ, "KeyK" => Self::KeyK, "KeyL" => Self::KeyL,
            "KeyM" => Self::KeyM, "KeyN" => Self::KeyN, "KeyO" => Self::KeyO,
            "KeyP" => Self::KeyP, "KeyQ" => Self::KeyQ, "KeyR" => Self::KeyR,
            "KeyS" => Self::KeyS, "KeyT" => Self::KeyT, "KeyU" => Self::KeyU,
            "KeyV" => Self::KeyV, "KeyW" => Self::KeyW, "KeyX" => Self::KeyX,
            "KeyY" => Self::KeyY, "KeyZ" => Self::KeyZ,

            "Digit0" => Self::Digit0, "Digit1" => Self::Digit1, "Digit2" => Self::Digit2,
            "Digit3" => Self::Digit3, "Digit4" => Self::Digit4, "Digit5" => Self::Digit5,
            "Digit6" => Self::Digit6, "Digit7" => Self::Digit7, "Digit8" => Self::Digit8,
            "Digit9" => Self::Digit9,

            "F1" => Self::F1, "F2" => Self::F2, "F3" => Self::F3, "F4" => Self::F4,
            "F5" => Self::F5, "F6" => Self::F6, "F7" => Self::F7, "F8" => Self::F8,
            "F9" => Self::F9, "F10" => Self::F10, "F11" => Self::F11, "F12" => Self::F12,

            "ArrowUp" => Self::ArrowUp, "ArrowDown" => Self::ArrowDown,
            "ArrowLeft" => Self::ArrowLeft, "ArrowRight" => Self::ArrowRight,

            "ShiftLeft" => Self::ShiftLeft, "ShiftRight" => Self::ShiftRight,
            "ControlLeft" => Self::ControlLeft, "ControlRight" => Self::ControlRight,
            "AltLeft" => Self::AltLeft, "AltRight" => Self::AltRight,
            "MetaLeft" => Self::MetaLeft, "MetaRight" => Self::MetaRight,

            "Space" => Self::Space, "Enter" => Self::Enter, "Tab" => Self::Tab,
            "Backspace" => Self::Backspace, "Delete" => Self::Delete,
            "Insert" => Self::Insert, "Escape" => Self::Escape,

            "Home" => Self::Home, "End" => Self::End,
            "PageUp" => Self::PageUp, "PageDown" => Self::PageDown,

            "Minus" => Self::Minus, "Equal" => Self::Equal,
            "BracketLeft" => Self::BracketLeft, "BracketRight" => Self::BracketRight,
            "Backslash" => Self::Backslash, "Semicolon" => Self::Semicolon,
            "Quote" => Self::Quote, "Backquote" => Self::Backquote,
            "Comma" => Self::Comma, "Period" => Self::Period, "Slash" => Self::Slash,

            "CapsLock" => Self::CapsLock, "NumLock" => Self::NumLock,
            "ScrollLock" => Self::ScrollLock,

            "Numpad0" => Self::Numpad0, "Numpad1" => Self::Numpad1,
            "Numpad2" => Self::Numpad2, "Numpad3" => Self::Numpad3,
            "Numpad4" => Self::Numpad4, "Numpad5" => Self::Numpad5,
            "Numpad6" => Self::Numpad6, "Numpad7" => Self::Numpad7,
            "Numpad8" => Self::Numpad8, "Numpad9" => Self::Numpad9,
            "NumpadAdd" => Self::NumpadAdd, "NumpadSubtract" => Self::NumpadSubtract,
            "NumpadMultiply" => Self::NumpadMultiply, "NumpadDivide" => Self::NumpadDivide,
            "NumpadDecimal" => Self::NumpadDecimal, "NumpadEnter" => Self::NumpadEnter,

            "ContextMenu" => Self::ContextMenu, "PrintScreen" => Self::PrintScreen,
            "Pause" => Self::Pause,

            other => Self::Unknown(other.to_string()),
        }
    }
}

impl std::fmt::Display for KeyCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unknown(s) => write!(f, "{s}"),
            other => write!(f, "{other:?}"),
        }
    }
}

/// Mouse button identifiers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
    Back,
    Forward,
}

impl From<u8> for MouseButton {
    fn from(value: u8) -> Self {
        match value {
            0 => Self::Left,
            1 => Self::Middle,
            2 => Self::Right,
            3 => Self::Back,
            4 => Self::Forward,
            _ => Self::Left,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keycode_from_str_letters() {
        assert_eq!(KeyCode::from("KeyA"), KeyCode::KeyA);
        assert_eq!(KeyCode::from("KeyZ"), KeyCode::KeyZ);
    }

    #[test]
    fn keycode_from_str_special() {
        assert_eq!(KeyCode::from("Space"), KeyCode::Space);
        assert_eq!(KeyCode::from("ShiftLeft"), KeyCode::ShiftLeft);
        assert_eq!(KeyCode::from("ArrowUp"), KeyCode::ArrowUp);
    }

    #[test]
    fn keycode_from_str_unknown() {
        let code = KeyCode::from("SomeFutureKey");
        assert_eq!(code, KeyCode::Unknown("SomeFutureKey".to_string()));
    }

    #[test]
    fn keycode_display() {
        assert_eq!(KeyCode::KeyA.to_string(), "KeyA");
        assert_eq!(KeyCode::Unknown("Foo".to_string()).to_string(), "Foo");
    }

    #[test]
    fn mouse_button_from_u8() {
        assert_eq!(MouseButton::from(0), MouseButton::Left);
        assert_eq!(MouseButton::from(1), MouseButton::Middle);
        assert_eq!(MouseButton::from(2), MouseButton::Right);
        assert_eq!(MouseButton::from(3), MouseButton::Back);
        assert_eq!(MouseButton::from(4), MouseButton::Forward);
        assert_eq!(MouseButton::from(255), MouseButton::Left);
    }

    #[test]
    fn keycode_serde_roundtrip() {
        let code = KeyCode::KeyW;
        let json = serde_json::to_string(&code).unwrap();
        let deserialized: KeyCode = serde_json::from_str(&json).unwrap();
        assert_eq!(code, deserialized);
    }

    #[test]
    fn mouse_button_serde_roundtrip() {
        let btn = MouseButton::Right;
        let json = serde_json::to_string(&btn).unwrap();
        let deserialized: MouseButton = serde_json::from_str(&json).unwrap();
        assert_eq!(btn, deserialized);
    }
}
