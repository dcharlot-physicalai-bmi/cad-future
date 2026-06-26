//! `physical-input` — action-mapping input system for OpenIE.
//!
//! Sits between raw platform events (keyboard, mouse, touch) and application logic.
//! Translates physical inputs into named **actions** and virtual **axes** that
//! the UI and viewport can query each frame.
//!
//! Ported from game-studio's `studio-input` and adapted for CAD use.

pub mod action;
pub mod haptics;
pub mod input_map;
pub mod keycode;
pub mod state;
pub mod touch;
pub mod virtual_controls;

pub use action::ActionBinding;
pub use haptics::{HapticCommand, HapticEffect, HapticQueue, HapticTarget};
pub use input_map::{AxisBinding, InputMap, MouseAxis};
pub use keycode::{KeyCode, MouseButton};
pub use state::ActionState;
pub use touch::{ActiveTouch, Gesture, GestureDetector, SwipeDirection, TouchPhase, TouchState};
pub use virtual_controls::{
    LayoutPosition, LayoutSide, VirtualButton, VirtualControlLayout, VirtualJoystick,
};
