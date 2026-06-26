//! Touch gesture recognition for mobile and tablet input.

/// Phase of a touch contact.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TouchPhase {
    Started,
    Moved,
    Ended,
}

/// A single active touch contact.
#[derive(Clone, Debug)]
pub struct ActiveTouch {
    pub id: u32,
    pub start_x: f32,
    pub start_y: f32,
    pub current_x: f32,
    pub current_y: f32,
    pub start_time: f32,
    pub phase: TouchPhase,
}

/// Tracks all active touch contacts for the current frame.
pub struct TouchState {
    touches: Vec<ActiveTouch>,
    elapsed: f32,
}

impl TouchState {
    pub fn new() -> Self {
        Self { touches: Vec::new(), elapsed: 0.0 }
    }

    pub fn on_touch_start(&mut self, id: u32, x: f32, y: f32) {
        self.touches.retain(|t| t.id != id);
        self.touches.push(ActiveTouch {
            id, start_x: x, start_y: y, current_x: x, current_y: y,
            start_time: self.elapsed, phase: TouchPhase::Started,
        });
    }

    pub fn on_touch_move(&mut self, id: u32, x: f32, y: f32) {
        if let Some(touch) = self.touches.iter_mut().find(|t| t.id == id) {
            touch.current_x = x;
            touch.current_y = y;
            touch.phase = TouchPhase::Moved;
        }
    }

    pub fn on_touch_end(&mut self, id: u32) {
        if let Some(touch) = self.touches.iter_mut().find(|t| t.id == id) {
            touch.phase = TouchPhase::Ended;
        }
    }

    pub fn clear_frame(&mut self, dt: f32) {
        self.elapsed += dt;
        self.touches.retain(|t| t.phase != TouchPhase::Ended);
    }

    pub fn active_touches(&self) -> &[ActiveTouch] {
        &self.touches
    }

    pub fn touch_count(&self) -> usize {
        self.touches.len()
    }
}

impl Default for TouchState {
    fn default() -> Self { Self::new() }
}

/// Cardinal direction of a swipe gesture.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SwipeDirection { Up, Down, Left, Right }

/// A recognized touch gesture.
#[derive(Clone, Debug, PartialEq)]
pub enum Gesture {
    Tap { x: f32, y: f32 },
    DoubleTap { x: f32, y: f32 },
    Swipe { direction: SwipeDirection, velocity: f32, start_x: f32, start_y: f32 },
    Pinch { scale: f32, center_x: f32, center_y: f32 },
    Rotate { angle: f32, center_x: f32, center_y: f32 },
    LongPress { x: f32, y: f32, duration: f32 },
}

/// Recognizes gestures from [`TouchState`] each frame.
pub struct GestureDetector {
    pub tap_max_distance: f32,
    pub tap_max_duration: f32,
    pub swipe_min_distance: f32,
    pub hold_min_duration: f32,
    pub double_tap_max_interval: f32,

    last_tap_time: f32,
    last_tap_x: f32,
    last_tap_y: f32,
    elapsed: f32,
    long_press_fired: Vec<u32>,
    prev_pinch_distance: Option<f32>,
    prev_pinch_angle: Option<f32>,
}

impl GestureDetector {
    pub fn new() -> Self {
        Self {
            tap_max_distance: 20.0,
            tap_max_duration: 0.3,
            swipe_min_distance: 50.0,
            hold_min_duration: 0.5,
            double_tap_max_interval: 0.3,
            last_tap_time: -1.0,
            last_tap_x: 0.0,
            last_tap_y: 0.0,
            elapsed: 0.0,
            long_press_fired: Vec::new(),
            prev_pinch_distance: None,
            prev_pinch_angle: None,
        }
    }

    pub fn update(&mut self, touches: &TouchState, dt: f32) -> Vec<Gesture> {
        self.elapsed += dt;
        let mut gestures = Vec::new();

        // Long-press detection
        for touch in touches.active_touches() {
            if touch.phase == TouchPhase::Ended { continue; }
            let dx = touch.current_x - touch.start_x;
            let dy = touch.current_y - touch.start_y;
            let dist = (dx * dx + dy * dy).sqrt();
            let held = self.elapsed - touch.start_time + dt;
            if dist <= self.tap_max_distance
                && held >= self.hold_min_duration
                && !self.long_press_fired.contains(&touch.id)
            {
                gestures.push(Gesture::LongPress {
                    x: touch.current_x, y: touch.current_y, duration: held,
                });
                self.long_press_fired.push(touch.id);
            }
        }

        // Tap / swipe on touch end
        for touch in touches.active_touches() {
            if touch.phase != TouchPhase::Ended { continue; }
            self.long_press_fired.retain(|&id| id != touch.id);

            let dx = touch.current_x - touch.start_x;
            let dy = touch.current_y - touch.start_y;
            let dist = (dx * dx + dy * dy).sqrt();
            let duration = self.elapsed - touch.start_time + dt;

            if dist <= self.tap_max_distance && duration <= self.tap_max_duration {
                let since_last_tap = self.elapsed - self.last_tap_time;
                let tap_dx = touch.current_x - self.last_tap_x;
                let tap_dy = touch.current_y - self.last_tap_y;
                let tap_dist = (tap_dx * tap_dx + tap_dy * tap_dy).sqrt();

                if since_last_tap <= self.double_tap_max_interval
                    && tap_dist <= self.tap_max_distance * 2.0
                {
                    gestures.push(Gesture::DoubleTap { x: touch.current_x, y: touch.current_y });
                    self.last_tap_time = -1.0;
                } else {
                    gestures.push(Gesture::Tap { x: touch.current_x, y: touch.current_y });
                    self.last_tap_time = self.elapsed;
                    self.last_tap_x = touch.current_x;
                    self.last_tap_y = touch.current_y;
                }
            } else if dist >= self.swipe_min_distance {
                let velocity = if duration > 0.0 { dist / duration } else { 0.0 };
                let direction = if dx.abs() > dy.abs() {
                    if dx > 0.0 { SwipeDirection::Right } else { SwipeDirection::Left }
                } else if dy > 0.0 {
                    SwipeDirection::Down
                } else {
                    SwipeDirection::Up
                };
                gestures.push(Gesture::Swipe {
                    direction, velocity, start_x: touch.start_x, start_y: touch.start_y,
                });
            }
        }

        // Two-finger gestures: pinch and rotate
        let active: Vec<&ActiveTouch> = touches.active_touches().iter()
            .filter(|t| t.phase != TouchPhase::Ended).collect();

        if active.len() == 2 {
            let a = active[0];
            let b = active[1];
            let dx = b.current_x - a.current_x;
            let dy = b.current_y - a.current_y;
            let distance = (dx * dx + dy * dy).sqrt();
            let angle = dy.atan2(dx);
            let center_x = (a.current_x + b.current_x) * 0.5;
            let center_y = (a.current_y + b.current_y) * 0.5;

            if let Some(prev_dist) = self.prev_pinch_distance.filter(|&d| d > 0.0) {
                let scale = distance / prev_dist;
                if (scale - 1.0).abs() > 0.001 {
                    gestures.push(Gesture::Pinch { scale, center_x, center_y });
                }
            }

            if let Some(prev_angle) = self.prev_pinch_angle {
                let mut delta = angle - prev_angle;
                while delta > std::f32::consts::PI { delta -= 2.0 * std::f32::consts::PI; }
                while delta < -std::f32::consts::PI { delta += 2.0 * std::f32::consts::PI; }
                if delta.abs() > 0.001 {
                    gestures.push(Gesture::Rotate { angle: delta, center_x, center_y });
                }
            }

            self.prev_pinch_distance = Some(distance);
            self.prev_pinch_angle = Some(angle);
        } else {
            self.prev_pinch_distance = None;
            self.prev_pinch_angle = None;
        }

        gestures
    }
}

impl Default for GestureDetector {
    fn default() -> Self { Self::new() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn touch_lifecycle() {
        let mut state = TouchState::new();
        state.on_touch_start(1, 100.0, 200.0);
        assert_eq!(state.touch_count(), 1);
        assert_eq!(state.active_touches()[0].phase, TouchPhase::Started);
        state.on_touch_move(1, 110.0, 210.0);
        assert_eq!(state.active_touches()[0].current_x, 110.0);
        state.on_touch_end(1);
        assert_eq!(state.active_touches()[0].phase, TouchPhase::Ended);
        state.clear_frame(1.0 / 60.0);
        assert_eq!(state.touch_count(), 0);
    }

    #[test]
    fn tap_detected() {
        let mut state = TouchState::new();
        let mut detector = GestureDetector::new();
        state.on_touch_start(1, 100.0, 100.0);
        let gestures = detector.update(&state, 1.0 / 60.0);
        assert!(gestures.is_empty());
        state.on_touch_end(1);
        let gestures = detector.update(&state, 1.0 / 60.0);
        assert_eq!(gestures.len(), 1);
        assert!(matches!(gestures[0], Gesture::Tap { .. }));
    }

    #[test]
    fn pinch_detected() {
        let mut state = TouchState::new();
        let mut detector = GestureDetector::new();
        state.on_touch_start(1, 100.0, 100.0);
        state.on_touch_start(2, 200.0, 100.0);
        detector.update(&state, 1.0 / 60.0);
        state.on_touch_move(1, 80.0, 100.0);
        state.on_touch_move(2, 220.0, 100.0);
        let gestures = detector.update(&state, 1.0 / 60.0);
        assert!(gestures.iter().any(|g| matches!(g, Gesture::Pinch { scale, .. } if *scale > 1.0)));
    }
}
