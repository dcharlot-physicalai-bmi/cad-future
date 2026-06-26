//! Controller haptic feedback — queues rumble/vibration effects.

/// Identifies which controller should receive haptic feedback.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HapticTarget {
    Primary,
    Secondary,
    All,
}

/// A haptic effect to send to a controller.
#[derive(Debug, Clone, Copy)]
pub enum HapticEffect {
    Rumble { left: f32, right: f32, duration_ms: f32 },
    Stop,
}

/// A queued haptic command: target + effect.
#[derive(Debug, Clone, Copy)]
pub struct HapticCommand {
    pub target: HapticTarget,
    pub effect: HapticEffect,
}

/// Frame-buffered queue of haptic commands.
pub struct HapticQueue {
    pending: Vec<HapticCommand>,
}

impl HapticQueue {
    pub fn new() -> Self {
        Self { pending: Vec::new() }
    }

    pub fn rumble(&mut self, left: f32, right: f32, duration_ms: f32) {
        self.pending.push(HapticCommand {
            target: HapticTarget::Primary,
            effect: HapticEffect::Rumble {
                left: left.clamp(0.0, 1.0),
                right: right.clamp(0.0, 1.0),
                duration_ms,
            },
        });
    }

    pub fn stop_all(&mut self) {
        self.pending.push(HapticCommand {
            target: HapticTarget::All,
            effect: HapticEffect::Stop,
        });
    }

    pub fn drain(&mut self) -> Vec<HapticCommand> {
        std::mem::take(&mut self.pending)
    }

    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }
}

impl Default for HapticQueue {
    fn default() -> Self { Self::new() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_queue() {
        let queue = HapticQueue::new();
        assert_eq!(queue.pending_count(), 0);
    }

    #[test]
    fn rumble_queues_command() {
        let mut queue = HapticQueue::new();
        queue.rumble(0.5, 0.8, 200.0);
        assert_eq!(queue.pending_count(), 1);
        let cmds = queue.drain();
        assert_eq!(cmds.len(), 1);
        assert_eq!(cmds[0].target, HapticTarget::Primary);
    }

    #[test]
    fn intensity_is_clamped() {
        let mut queue = HapticQueue::new();
        queue.rumble(2.0, -0.5, 100.0);
        let cmds = queue.drain();
        match cmds[0].effect {
            HapticEffect::Rumble { left, right, .. } => {
                assert!((left - 1.0).abs() < f32::EPSILON);
                assert!(right.abs() < f32::EPSILON);
            }
            _ => panic!("expected Rumble"),
        }
    }

    #[test]
    fn drain_empties_queue() {
        let mut queue = HapticQueue::new();
        queue.rumble(0.5, 0.5, 100.0);
        queue.stop_all();
        assert_eq!(queue.pending_count(), 2);
        let cmds = queue.drain();
        assert_eq!(cmds.len(), 2);
        assert_eq!(queue.pending_count(), 0);
    }
}
