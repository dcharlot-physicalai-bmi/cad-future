/// The per-frame state of a named action.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum ActionState {
    /// No input active for this action.
    #[default]
    Idle,
    /// The action was just activated this frame.
    JustPressed,
    /// The action is being held; contains accumulated duration in seconds.
    Held(f32),
    /// The action was just released this frame.
    JustReleased,
}

impl ActionState {
    /// Returns `true` if the action is currently active (pressed or held).
    pub fn is_active(self) -> bool {
        matches!(self, Self::JustPressed | Self::Held(_))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_idle() {
        assert_eq!(ActionState::default(), ActionState::Idle);
    }

    #[test]
    fn is_active_variants() {
        assert!(!ActionState::Idle.is_active());
        assert!(ActionState::JustPressed.is_active());
        assert!(ActionState::Held(0.5).is_active());
        assert!(!ActionState::JustReleased.is_active());
    }
}
