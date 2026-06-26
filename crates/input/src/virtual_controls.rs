//! Virtual on-screen controls for touch devices.

/// A virtual on-screen joystick.
pub struct VirtualJoystick {
    center_x: f32,
    center_y: f32,
    radius: f32,
    pub dead_zone: f32,
    stick_x: f32,
    stick_y: f32,
    active: bool,
}

impl VirtualJoystick {
    pub fn new(center_x: f32, center_y: f32, radius: f32) -> Self {
        Self {
            center_x, center_y, radius,
            dead_zone: 0.15,
            stick_x: 0.0, stick_y: 0.0,
            active: false,
        }
    }

    pub fn set_position(&mut self, x: f32, y: f32) {
        self.center_x = x;
        self.center_y = y;
    }

    pub fn on_touch(&mut self, touch_x: f32, touch_y: f32) -> bool {
        let dx = touch_x - self.center_x;
        let dy = touch_y - self.center_y;
        let dist = (dx * dx + dy * dy).sqrt();
        if dist <= self.radius {
            self.active = true;
            self.update_stick(dx, dy);
            true
        } else {
            false
        }
    }

    pub fn on_move(&mut self, touch_x: f32, touch_y: f32) {
        if self.active {
            let dx = touch_x - self.center_x;
            let dy = touch_y - self.center_y;
            self.update_stick(dx, dy);
        }
    }

    pub fn on_release(&mut self) {
        self.active = false;
        self.stick_x = 0.0;
        self.stick_y = 0.0;
    }

    pub fn direction(&self) -> (f32, f32) {
        if self.magnitude() < self.dead_zone { return (0.0, 0.0); }
        (self.stick_x, self.stick_y)
    }

    pub fn magnitude(&self) -> f32 {
        (self.stick_x * self.stick_x + self.stick_y * self.stick_y).sqrt().min(1.0)
    }

    pub fn is_active(&self) -> bool { self.active }

    fn update_stick(&mut self, dx: f32, dy: f32) {
        if self.radius > 0.0 {
            let clamped_dist = (dx * dx + dy * dy).sqrt().min(self.radius);
            let len = (dx * dx + dy * dy).sqrt();
            if len > 0.0 {
                self.stick_x = (dx / len) * (clamped_dist / self.radius);
                self.stick_y = (dy / len) * (clamped_dist / self.radius);
            }
        }
    }
}

/// A virtual on-screen button.
pub struct VirtualButton {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    label: String,
    pressed: bool,
    prev_pressed: bool,
}

impl VirtualButton {
    pub fn new(x: f32, y: f32, width: f32, height: f32, label: &str) -> Self {
        Self { x, y, width, height, label: label.to_string(), pressed: false, prev_pressed: false }
    }

    pub fn label(&self) -> &str { &self.label }

    pub fn on_touch(&mut self, touch_x: f32, touch_y: f32) -> bool {
        if touch_x >= self.x && touch_x <= self.x + self.width
            && touch_y >= self.y && touch_y <= self.y + self.height
        {
            self.pressed = true;
            true
        } else {
            false
        }
    }

    pub fn on_release(&mut self) { self.pressed = false; }
    pub fn is_pressed(&self) -> bool { self.pressed }
    pub fn was_just_pressed(&self) -> bool { self.pressed && !self.prev_pressed }
    pub fn was_just_released(&self) -> bool { !self.pressed && self.prev_pressed }
    pub fn update(&mut self) { self.prev_pressed = self.pressed; }
    pub fn set_position(&mut self, x: f32, y: f32) { self.x = x; self.y = y; }
}

/// Which side of the screen to place a virtual joystick.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LayoutSide { Left, Right }

/// Positioning hint for virtual buttons.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LayoutPosition {
    BottomLeft, BottomRight, TopLeft, TopRight,
    Custom(f32, f32),
}

/// Manages a set of virtual controls with automatic positioning.
pub struct VirtualControlLayout {
    screen_width: f32,
    screen_height: f32,
    joysticks: Vec<(VirtualJoystick, LayoutSide)>,
    buttons: Vec<(VirtualButton, LayoutPosition)>,
}

impl VirtualControlLayout {
    pub fn new(screen_width: f32, screen_height: f32) -> Self {
        Self { screen_width, screen_height, joysticks: Vec::new(), buttons: Vec::new() }
    }

    pub fn add_joystick(&mut self, side: LayoutSide) -> usize {
        let radius = self.screen_height * 0.1;
        let margin = radius * 1.5;
        let (cx, cy) = match side {
            LayoutSide::Left => (margin, self.screen_height - margin),
            LayoutSide::Right => (self.screen_width - margin, self.screen_height - margin),
        };
        self.joysticks.push((VirtualJoystick::new(cx, cy, radius), side));
        self.joysticks.len() - 1
    }

    pub fn add_button(&mut self, label: &str, position: LayoutPosition) -> usize {
        let bw = 80.0;
        let bh = 60.0;
        let margin = 20.0;
        let (x, y) = Self::compute_position_static(position, bw, bh, margin, self.screen_width, self.screen_height);
        self.buttons.push((VirtualButton::new(x, y, bw, bh, label), position));
        self.buttons.len() - 1
    }

    pub fn resize(&mut self, width: f32, height: f32) {
        self.screen_width = width;
        self.screen_height = height;

        for (joystick, side) in &mut self.joysticks {
            let radius = height * 0.1;
            let margin = radius * 1.5;
            let (cx, cy) = match *side {
                LayoutSide::Left => (margin, height - margin),
                LayoutSide::Right => (width - margin, height - margin),
            };
            joystick.set_position(cx, cy);
        }

        let bw = 80.0;
        let bh = 60.0;
        let margin = 20.0;
        let positions: Vec<LayoutPosition> = self.buttons.iter().map(|(_, p)| *p).collect();
        for (i, (button, _)) in self.buttons.iter_mut().enumerate() {
            let (x, y) = Self::compute_position_static(positions[i], bw, bh, margin, self.screen_width, self.screen_height);
            button.set_position(x, y);
        }
    }

    pub fn joystick(&self, index: usize) -> &VirtualJoystick { &self.joysticks[index].0 }
    pub fn joystick_mut(&mut self, index: usize) -> &mut VirtualJoystick { &mut self.joysticks[index].0 }
    pub fn button(&self, index: usize) -> &VirtualButton { &self.buttons[index].0 }
    pub fn button_mut(&mut self, index: usize) -> &mut VirtualButton { &mut self.buttons[index].0 }

    fn compute_position_static(position: LayoutPosition, bw: f32, bh: f32, margin: f32, sw: f32, sh: f32) -> (f32, f32) {
        match position {
            LayoutPosition::BottomLeft => (margin, sh - margin - bh),
            LayoutPosition::BottomRight => (sw - margin - bw, sh - margin - bh),
            LayoutPosition::TopLeft => (margin, margin),
            LayoutPosition::TopRight => (sw - margin - bw, margin),
            LayoutPosition::Custom(x, y) => (x, y),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn joystick_touch_inside() {
        let mut js = VirtualJoystick::new(100.0, 100.0, 50.0);
        assert!(js.on_touch(110.0, 110.0));
        assert!(js.is_active());
    }

    #[test]
    fn joystick_touch_outside() {
        let mut js = VirtualJoystick::new(100.0, 100.0, 50.0);
        assert!(!js.on_touch(200.0, 200.0));
        assert!(!js.is_active());
    }

    #[test]
    fn joystick_dead_zone() {
        let mut js = VirtualJoystick::new(100.0, 100.0, 50.0);
        js.on_touch(100.0, 100.0);
        js.on_move(102.0, 102.0);
        let (dx, dy) = js.direction();
        assert_eq!(dx, 0.0);
        assert_eq!(dy, 0.0);
    }

    #[test]
    fn button_hit_test() {
        let mut btn = VirtualButton::new(10.0, 10.0, 80.0, 60.0, "select");
        assert!(btn.on_touch(50.0, 40.0));
        assert!(btn.is_pressed());
    }

    #[test]
    fn layout_add_and_resize() {
        let mut layout = VirtualControlLayout::new(800.0, 600.0);
        let js_idx = layout.add_joystick(LayoutSide::Left);
        let btn_idx = layout.add_button("A", LayoutPosition::BottomRight);
        assert!(layout.joystick(js_idx).center_x < 400.0);
        assert_eq!(layout.button(btn_idx).label(), "A");
        layout.resize(1920.0, 1080.0);
        assert!(layout.joystick(js_idx).center_y > 900.0);
    }
}
