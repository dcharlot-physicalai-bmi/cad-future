//! Immediate-mode UI context — tracks layout, input, and hot/active widget state.

use crate::draw::DrawList;
use crate::font;
use crate::theme::ThemeColors;

/// Visual style parameters for the UI.
pub struct UiStyle {
    /// Height of a text line in pixels.
    pub font_size: f32,
    /// Inner padding for widgets.
    pub padding: f32,
    /// Vertical spacing between widgets.
    pub spacing: f32,
    /// Default button height.
    pub button_height: f32,
    /// Panel / widget background.
    pub bg_color: [f32; 4],
    /// Default text color.
    pub text_color: [f32; 4],
    /// Accent color (sliders, progress bars).
    pub accent_color: [f32; 4],
    /// Hover highlight color.
    pub hover_color: [f32; 4],
    /// Active (pressed) highlight color.
    pub active_color: [f32; 4],
    /// Corner radius — reserved for future rounded-rect support.
    pub border_radius: f32,
}

impl UiStyle {
    /// Create a style from the given theme colors.
    pub fn from_theme(theme: &ThemeColors) -> Self {
        Self {
            font_size: 14.0,
            padding: 6.0,
            spacing: 4.0,
            button_height: 28.0,
            bg_color: theme.panel_bg,
            text_color: theme.text,
            accent_color: theme.accent,
            hover_color: theme.hover,
            active_color: theme.active,
            border_radius: 4.0,
        }
    }
}

impl Default for UiStyle {
    fn default() -> Self {
        use crate::theme::ThemeMode;
        let theme = ThemeMode::Auto.resolve(12);
        Self::from_theme(&theme)
    }
}

/// The immediate-mode UI context.
pub struct UiContext {
    // Layout state
    cursor: [f32; 2],
    width: f32,
    height: f32,
    same_line_cursor: Option<[f32; 2]>,

    // Input state
    mouse_pos: [f32; 2],
    mouse_down: bool,
    mouse_clicked: bool,
    prev_mouse_down: bool,

    // Hot/active widget tracking
    hot: Option<u64>,
    active: Option<u64>,

    // Draw output
    draw_lists: Vec<DrawList>,
    current_draw: DrawList,

    // Style
    style: UiStyle,

    // Panel stack (indentation x-offset)
    panel_depth: u32,
}

impl Default for UiContext {
    fn default() -> Self {
        Self::new()
    }
}

impl UiContext {
    /// Create a new UI context with default style.
    pub fn new() -> Self {
        Self {
            cursor: [0.0, 0.0],
            width: 0.0,
            height: 0.0,
            same_line_cursor: None,
            mouse_pos: [0.0, 0.0],
            mouse_down: false,
            mouse_clicked: false,
            prev_mouse_down: false,
            hot: None,
            active: None,
            draw_lists: Vec::new(),
            current_draw: DrawList::new(),
            style: UiStyle::default(),
            panel_depth: 0,
        }
    }

    /// Returns a mutable reference to the style for customisation.
    pub fn style_mut(&mut self) -> &mut UiStyle {
        &mut self.style
    }

    /// Apply a theme to the UI style.
    pub fn apply_theme(&mut self, theme: &ThemeColors) {
        self.style = UiStyle::from_theme(theme);
    }

    /// Begin a new frame. Call once per frame before any widgets.
    pub fn begin_frame(
        &mut self,
        width: f32,
        height: f32,
        mouse_x: f32,
        mouse_y: f32,
        mouse_down: bool,
    ) {
        self.width = width;
        self.height = height;
        self.mouse_pos = [mouse_x, mouse_y];
        self.mouse_clicked = mouse_down && !self.prev_mouse_down;
        self.mouse_down = mouse_down;
        self.prev_mouse_down = mouse_down;
        self.cursor = [self.style.padding, self.style.padding];
        self.same_line_cursor = None;
        self.hot = None;
        self.draw_lists.clear();
        self.current_draw = DrawList::new();
        self.panel_depth = 0;
    }

    /// Finish the frame and return the draw lists for rendering.
    pub fn end_frame(&mut self) -> Vec<DrawList> {
        // Flush the current draw list if non-empty.
        if !self.current_draw.vertices.is_empty() {
            let finished = std::mem::take(&mut self.current_draw);
            self.draw_lists.push(finished);
        }
        // If nothing was active this frame, clear active state.
        if !self.mouse_down {
            self.active = None;
        }
        std::mem::take(&mut self.draw_lists)
    }

    // ── Widgets ─────────────────────────────────────────────────────

    /// Draw a text label at the current cursor position.
    pub fn label(&mut self, text: &str) {
        let (x, y) = self.next_pos();
        self.emit_text(text, x, y, self.style.text_color);
        let h = self.style.font_size;
        let w = font::measure_text(text, h, None);
        self.advance(w, h);
    }

    /// Draw a button. Returns `true` the frame it is clicked.
    pub fn button(&mut self, text: &str) -> bool {
        let id = self.make_id(text);
        let (x, y) = self.next_pos();
        let text_w = font::measure_text(text, self.style.font_size, None);
        let w = text_w + self.style.padding * 2.0;
        let h = self.style.button_height;

        let hovered = self.rect_hovered(x, y, w, h);
        if hovered {
            self.hot = Some(id);
            if self.mouse_clicked {
                self.active = Some(id);
            }
        }

        let clicked = hovered && self.active == Some(id) && self.mouse_clicked;

        let bg = if self.active == Some(id) && hovered {
            self.style.active_color
        } else if hovered {
            self.style.hover_color
        } else {
            self.style.bg_color
        };

        self.current_draw.push_quad(x, y, w, h, bg);
        let text_y = y + (h - self.style.font_size) * 0.5;
        self.emit_text(text, x + self.style.padding, text_y, self.style.text_color);
        self.advance(w, h);
        clicked
    }

    /// Draw a slider. Returns `true` if the value changed.
    pub fn slider(&mut self, label: &str, value: &mut f32, min: f32, max: f32) -> bool {
        let id = self.make_id(label);
        let (x, y) = self.next_pos();
        let slider_w = 200.0;
        let h = self.style.button_height;
        let total_w = slider_w;

        // Track
        let track_h = 4.0;
        let track_y = y + (h - track_h) * 0.5;
        self.current_draw.push_quad(x, track_y, slider_w, track_h, self.style.bg_color);

        // Filled portion
        let range = max - min;
        let frac = if range > 0.0 { (*value - min) / range } else { 0.0 };
        let frac = frac.clamp(0.0, 1.0);
        self.current_draw.push_quad(x, track_y, slider_w * frac, track_h, self.style.accent_color);

        // Thumb
        let thumb_w = 12.0;
        let thumb_x = x + (slider_w - thumb_w) * frac;
        let hovered = self.rect_hovered(x, y, slider_w, h);
        if hovered {
            self.hot = Some(id);
            if self.mouse_clicked {
                self.active = Some(id);
            }
        }
        let thumb_color = if self.active == Some(id) {
            self.style.active_color
        } else if hovered {
            self.style.hover_color
        } else {
            [0.7, 0.7, 0.7, 1.0]
        };
        self.current_draw.push_quad(thumb_x, y, thumb_w, h, thumb_color);

        // Interaction
        let mut changed = false;
        if self.active == Some(id) && self.mouse_down {
            let new_frac = ((self.mouse_pos[0] - x) / slider_w).clamp(0.0, 1.0);
            let new_val = min + new_frac * range;
            if (new_val - *value).abs() > f32::EPSILON {
                *value = new_val;
                changed = true;
            }
        }

        // Label + value text
        let label_text = format!("{label}: {:.2}", *value);
        self.emit_text(&label_text, x, y + h + 2.0, self.style.text_color);
        self.advance(total_w, h + self.style.font_size + 2.0);
        changed
    }

    /// Draw a checkbox. Returns `true` if toggled.
    pub fn checkbox(&mut self, label: &str, checked: &mut bool) -> bool {
        let id = self.make_id(label);
        let (x, y) = self.next_pos();
        let box_size = self.style.font_size;
        let h = self.style.button_height;
        let box_y = y + (h - box_size) * 0.5;

        let hovered = self.rect_hovered(x, box_y, box_size, box_size);
        if hovered {
            self.hot = Some(id);
            if self.mouse_clicked {
                self.active = Some(id);
            }
        }

        let toggled = hovered && self.active == Some(id) && self.mouse_clicked;
        if toggled {
            *checked = !*checked;
        }

        let bg = if hovered { self.style.hover_color } else { self.style.bg_color };
        self.current_draw.push_quad(x, box_y, box_size, box_size, bg);

        if *checked {
            let inset = 3.0;
            self.current_draw.push_quad(
                x + inset,
                box_y + inset,
                box_size - inset * 2.0,
                box_size - inset * 2.0,
                self.style.accent_color,
            );
        }

        let text_x = x + box_size + self.style.padding;
        let text_y = y + (h - self.style.font_size) * 0.5;
        self.emit_text(label, text_x, text_y, self.style.text_color);
        let text_w = font::measure_text(label, self.style.font_size, None);
        self.advance(box_size + self.style.padding + text_w, h);
        toggled
    }

    /// Begin a panel with a title bar and background.
    pub fn panel_begin(&mut self, title: &str) {
        let (x, y) = self.next_pos();
        let panel_w = self.width - x - self.style.padding;

        let title_h = self.style.button_height;
        self.current_draw.push_quad(x, y, panel_w, title_h, self.style.accent_color);
        let text_y = y + (title_h - self.style.font_size) * 0.5;
        self.emit_text(title, x + self.style.padding, text_y, [1.0, 1.0, 1.0, 1.0]);
        self.cursor[1] = y + title_h + self.style.spacing;

        self.panel_depth += 1;
        self.cursor[0] = self.style.padding * (1.0 + self.panel_depth as f32);
    }

    /// End the current panel.
    pub fn panel_end(&mut self) {
        if self.panel_depth > 0 {
            self.panel_depth -= 1;
        }
        self.cursor[0] = self.style.padding * (1.0 + self.panel_depth as f32);
        self.cursor[1] += self.style.spacing;
    }

    /// Draw a horizontal separator line.
    pub fn separator(&mut self) {
        let (x, y) = self.next_pos();
        let w = self.width - x - self.style.padding;
        self.current_draw.push_quad(x, y, w, 1.0, [0.4, 0.4, 0.4, 1.0]);
        self.advance(w, 1.0);
    }

    /// Add vertical space.
    pub fn spacing(&mut self, pixels: f32) {
        self.cursor[1] += pixels;
    }

    /// Place the next widget on the same line as the previous one.
    pub fn same_line(&mut self) {
        if let Some(prev) = self.same_line_cursor.take() {
            self.same_line_cursor = Some(prev);
        }
        self.same_line_cursor = Some(self.cursor);
    }

    /// Draw a progress bar.
    pub fn progress_bar(&mut self, fraction: f32, label: &str) {
        let (x, y) = self.next_pos();
        let bar_w = 200.0;
        let h = self.style.button_height;
        let frac = fraction.clamp(0.0, 1.0);

        self.current_draw.push_quad(x, y, bar_w, h, self.style.bg_color);
        self.current_draw.push_quad(x, y, bar_w * frac, h, self.style.accent_color);

        let text_w = font::measure_text(label, self.style.font_size, None);
        let text_x = x + (bar_w - text_w) * 0.5;
        let text_y = y + (h - self.style.font_size) * 0.5;
        self.emit_text(label, text_x, text_y, self.style.text_color);
        self.advance(bar_w, h);
    }

    // ── Internals ───────────────────────────────────────────────────

    fn next_pos(&mut self) -> (f32, f32) {
        if let Some(pos) = self.same_line_cursor.take() {
            (pos[0], pos[1])
        } else {
            (self.cursor[0], self.cursor[1])
        }
    }

    fn advance(&mut self, w: f32, h: f32) {
        let right_x = if let Some(slc) = self.same_line_cursor {
            slc[0] + w + self.style.spacing
        } else {
            self.cursor[0] + w + self.style.spacing
        };
        let current_y = self.cursor[1];

        self.cursor[1] += h + self.style.spacing;

        self.same_line_cursor = Some([right_x, current_y]);
    }

    fn rect_hovered(&self, x: f32, y: f32, w: f32, h: f32) -> bool {
        self.mouse_pos[0] >= x
            && self.mouse_pos[0] <= x + w
            && self.mouse_pos[1] >= y
            && self.mouse_pos[1] <= y + h
    }

    fn make_id(&self, label: &str) -> u64 {
        // FNV-1a hash.
        let mut hash: u64 = 0xcbf29ce484222325;
        for byte in label.bytes() {
            hash ^= byte as u64;
            hash = hash.wrapping_mul(0x100000001b3);
        }
        hash
    }

    fn emit_text(&mut self, text: &str, x: f32, y: f32, color: [f32; 4]) {
        let mut cx = x;
        for c in text.chars() {
            let params = font::CharQuadParams {
                c,
                x: cx,
                y,
                size: self.style.font_size,
                color,
                atlas: None,
            };
            let advance = font::emit_char_quads(
                &params,
                &mut self.current_draw.vertices,
                &mut self.current_draw.indices,
            );
            cx += advance;
        }
    }
}
