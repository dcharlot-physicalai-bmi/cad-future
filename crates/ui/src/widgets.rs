//! Extended UI widgets — text input, dropdown, scroll area, tooltip, context menu.
//!
//! Supplements the core widgets in context.rs with richer interaction patterns
//! needed for a professional CAD application.

use crate::draw::DrawList;
use crate::font;

// ── Text Input ──────────────────────────────────────────────────────────────

/// State for a text input field.
pub struct TextInputState {
    pub text: String,
    pub cursor_pos: usize,
    pub focused: bool,
    pub selection_start: Option<usize>,
}

impl TextInputState {
    pub fn new(initial: &str) -> Self {
        Self {
            text: initial.to_string(),
            cursor_pos: initial.len(),
            focused: false,
            selection_start: None,
        }
    }

    /// Insert a character at the cursor.
    pub fn insert_char(&mut self, c: char) {
        if self.cursor_pos <= self.text.len() {
            self.text.insert(self.cursor_pos, c);
            self.cursor_pos += c.len_utf8();
        }
    }

    /// Delete character before cursor (backspace).
    pub fn backspace(&mut self) {
        if self.cursor_pos > 0 {
            let prev = self.text[..self.cursor_pos]
                .char_indices()
                .last()
                .map(|(i, _)| i)
                .unwrap_or(0);
            self.text.remove(prev);
            self.cursor_pos = prev;
        }
    }

    /// Delete character at cursor (delete key).
    pub fn delete(&mut self) {
        if self.cursor_pos < self.text.len() {
            self.text.remove(self.cursor_pos);
        }
    }

    /// Move cursor left.
    pub fn move_left(&mut self) {
        if self.cursor_pos > 0 {
            self.cursor_pos = self.text[..self.cursor_pos]
                .char_indices()
                .last()
                .map(|(i, _)| i)
                .unwrap_or(0);
        }
    }

    /// Move cursor right.
    pub fn move_right(&mut self) {
        if self.cursor_pos < self.text.len() {
            if let Some(c) = self.text[self.cursor_pos..].chars().next() {
                self.cursor_pos += c.len_utf8();
            }
        }
    }

    /// Move cursor to start.
    pub fn home(&mut self) {
        self.cursor_pos = 0;
    }

    /// Move cursor to end.
    pub fn end(&mut self) {
        self.cursor_pos = self.text.len();
    }
}

// ── Dropdown ────────────────────────────────────────────────────────────────

/// State for a dropdown selector.
pub struct DropdownState {
    pub selected: usize,
    pub open: bool,
    pub hovered_item: Option<usize>,
}

impl DropdownState {
    pub fn new(selected: usize) -> Self {
        Self { selected, open: false, hovered_item: None }
    }
}

// ── Scroll Area ─────────────────────────────────────────────────────────────

/// State for a scrollable area.
pub struct ScrollState {
    pub offset_y: f32,
    pub content_height: f32,
    pub viewport_height: f32,
    pub dragging_scrollbar: bool,
    drag_start_y: f32,
    drag_start_offset: f32,
}

impl ScrollState {
    pub fn new() -> Self {
        Self {
            offset_y: 0.0,
            content_height: 0.0,
            viewport_height: 0.0,
            dragging_scrollbar: false,
            drag_start_y: 0.0,
            drag_start_offset: 0.0,
        }
    }

    /// Scroll by a delta (positive = scroll down).
    pub fn scroll(&mut self, delta: f32) {
        self.offset_y = (self.offset_y + delta).clamp(0.0, self.max_offset());
    }

    /// Maximum scroll offset.
    pub fn max_offset(&self) -> f32 {
        (self.content_height - self.viewport_height).max(0.0)
    }

    /// Whether the content overflows the viewport.
    pub fn overflows(&self) -> bool {
        self.content_height > self.viewport_height
    }

    /// Scrollbar thumb position and height (normalized 0..1).
    pub fn scrollbar_thumb(&self) -> (f32, f32) {
        if self.content_height <= 0.0 || !self.overflows() {
            return (0.0, 1.0);
        }
        let ratio = self.viewport_height / self.content_height;
        let pos = self.offset_y / self.content_height;
        (pos, ratio)
    }

    /// Begin dragging the scrollbar.
    pub fn begin_scrollbar_drag(&mut self, mouse_y: f32) {
        self.dragging_scrollbar = true;
        self.drag_start_y = mouse_y;
        self.drag_start_offset = self.offset_y;
    }

    /// Update scrollbar drag.
    pub fn update_scrollbar_drag(&mut self, mouse_y: f32) {
        if !self.dragging_scrollbar { return; }
        let dy = mouse_y - self.drag_start_y;
        let scale = self.content_height / self.viewport_height;
        self.offset_y = (self.drag_start_offset + dy * scale).clamp(0.0, self.max_offset());
    }

    /// End scrollbar drag.
    pub fn end_scrollbar_drag(&mut self) {
        self.dragging_scrollbar = false;
    }
}

impl Default for ScrollState {
    fn default() -> Self {
        Self::new()
    }
}

// ── Tooltip ─────────────────────────────────────────────────────────────────

/// Tooltip state tracked by the UI system.
pub struct TooltipState {
    /// Text to display.
    pub text: Option<String>,
    /// Screen position where to show the tooltip.
    pub x: f32,
    pub y: f32,
    /// Hover timer in seconds.
    pub hover_time: f32,
    /// Delay before showing tooltip.
    pub delay: f32,
}

impl TooltipState {
    pub fn new() -> Self {
        Self {
            text: None,
            x: 0.0,
            y: 0.0,
            hover_time: 0.0,
            delay: 0.5,
        }
    }

    /// Reset tooltip (mouse moved away).
    pub fn reset(&mut self) {
        self.text = None;
        self.hover_time = 0.0;
    }

    /// Set tooltip text and position (hover started).
    pub fn set(&mut self, text: &str, x: f32, y: f32) {
        self.text = Some(text.to_string());
        self.x = x;
        self.y = y;
    }

    /// Should the tooltip be visible?
    pub fn visible(&self) -> bool {
        self.text.is_some() && self.hover_time >= self.delay
    }

    /// Draw the tooltip into a draw list.
    pub fn draw(&self, draw: &mut DrawList) {
        if !self.visible() { return; }
        let text = match &self.text {
            Some(t) => t,
            None => return,
        };

        let font_size = 12.0;
        let padding = 4.0;
        let text_w = font::measure_text(text, font_size, None);
        let w = text_w + padding * 2.0;
        let h = font_size + padding * 2.0;
        let x = self.x;
        let y = self.y - h - 4.0; // above cursor

        // Background
        draw.push_quad(x, y, w, h, [0.1, 0.1, 0.12, 0.95]);
        // Border
        draw.push_quad(x, y, w, 1.0, [0.4, 0.4, 0.45, 0.9]);
        draw.push_quad(x, y + h - 1.0, w, 1.0, [0.4, 0.4, 0.45, 0.9]);
        draw.push_quad(x, y, 1.0, h, [0.4, 0.4, 0.45, 0.9]);
        draw.push_quad(x + w - 1.0, y, 1.0, h, [0.4, 0.4, 0.45, 0.9]);

        // Text
        let mut cx = x + padding;
        let ty = y + padding;
        for c in text.chars() {
            let params = font::CharQuadParams {
                c, x: cx, y: ty, size: font_size,
                color: [0.9, 0.9, 0.9, 1.0],
                atlas: None,
            };
            let advance = font::emit_char_quads(&params, &mut draw.vertices, &mut draw.indices);
            cx += advance;
        }
    }
}

impl Default for TooltipState {
    fn default() -> Self {
        Self::new()
    }
}

// ── Context Menu ────────────────────────────────────────────────────────────

/// A context menu item.
#[derive(Clone)]
pub struct MenuItem {
    pub label: String,
    pub shortcut: Option<String>,
    pub separator: bool,
    pub enabled: bool,
}

impl MenuItem {
    pub fn action(label: &str) -> Self {
        Self { label: label.to_string(), shortcut: None, separator: false, enabled: true }
    }

    pub fn with_shortcut(mut self, shortcut: &str) -> Self {
        self.shortcut = Some(shortcut.to_string());
        self
    }

    pub fn separator() -> Self {
        Self { label: String::new(), shortcut: None, separator: true, enabled: true }
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }
}

/// State for a context menu.
pub struct ContextMenuState {
    pub visible: bool,
    pub x: f32,
    pub y: f32,
    pub items: Vec<MenuItem>,
    pub hovered_item: Option<usize>,
}

impl ContextMenuState {
    pub fn new() -> Self {
        Self { visible: false, x: 0.0, y: 0.0, items: Vec::new(), hovered_item: None }
    }

    /// Open the context menu at screen position.
    pub fn open(&mut self, x: f32, y: f32, items: Vec<MenuItem>) {
        self.visible = true;
        self.x = x;
        self.y = y;
        self.items = items;
        self.hovered_item = None;
    }

    /// Close the context menu.
    pub fn close(&mut self) {
        self.visible = false;
        self.items.clear();
        self.hovered_item = None;
    }

    /// Hit-test: returns the index of the item under the mouse.
    pub fn hit_test(&self, mx: f32, my: f32) -> Option<usize> {
        if !self.visible { return None; }
        let item_h = 24.0;
        let padding = 4.0;
        let width = self.menu_width();

        if mx < self.x || mx > self.x + width { return None; }

        let mut y = self.y + padding;
        for (i, item) in self.items.iter().enumerate() {
            if item.separator {
                y += 8.0;
                continue;
            }
            if my >= y && my < y + item_h && item.enabled {
                return Some(i);
            }
            y += item_h;
        }
        None
    }

    fn menu_width(&self) -> f32 {
        let font_size = 12.0;
        let padding = 24.0;
        let max_label = self.items.iter()
            .filter(|i| !i.separator)
            .map(|i| {
                let label_w = font::measure_text(&i.label, font_size, None);
                let shortcut_w = i.shortcut.as_ref()
                    .map(|s| font::measure_text(s, font_size, None) + 24.0)
                    .unwrap_or(0.0);
                label_w + shortcut_w
            })
            .fold(0.0_f32, f32::max);
        max_label + padding * 2.0
    }

    /// Draw the context menu.
    pub fn draw(&self, draw: &mut DrawList) {
        if !self.visible { return; }

        let item_h = 24.0;
        let padding = 4.0;
        let width = self.menu_width();
        let font_size = 12.0;

        // Calculate total height
        let total_h: f32 = self.items.iter().map(|i| if i.separator { 8.0 } else { item_h }).sum::<f32>() + padding * 2.0;

        // Background + shadow
        draw.push_quad(self.x + 2.0, self.y + 2.0, width, total_h, [0.0, 0.0, 0.0, 0.3]);
        draw.push_quad(self.x, self.y, width, total_h, [0.15, 0.16, 0.18, 0.97]);
        // Border
        draw.push_quad(self.x, self.y, width, 1.0, [0.35, 0.37, 0.40, 0.9]);
        draw.push_quad(self.x, self.y + total_h - 1.0, width, 1.0, [0.35, 0.37, 0.40, 0.9]);
        draw.push_quad(self.x, self.y, 1.0, total_h, [0.35, 0.37, 0.40, 0.9]);
        draw.push_quad(self.x + width - 1.0, self.y, 1.0, total_h, [0.35, 0.37, 0.40, 0.9]);

        let mut y = self.y + padding;
        for (i, item) in self.items.iter().enumerate() {
            if item.separator {
                draw.push_quad(self.x + 8.0, y + 3.0, width - 16.0, 1.0, [0.35, 0.37, 0.40, 0.5]);
                y += 8.0;
                continue;
            }

            // Hover highlight
            if self.hovered_item == Some(i) && item.enabled {
                draw.push_quad(self.x + 2.0, y, width - 4.0, item_h, [0.25, 0.45, 0.75, 0.6]);
            }

            let text_color = if item.enabled {
                [0.88, 0.88, 0.90, 1.0]
            } else {
                [0.45, 0.45, 0.48, 1.0]
            };

            // Label
            let text_y = y + (item_h - font_size) * 0.5;
            let mut cx = self.x + 12.0;
            for c in item.label.chars() {
                let params = font::CharQuadParams {
                    c, x: cx, y: text_y, size: font_size, color: text_color, atlas: None,
                };
                cx += font::emit_char_quads(&params, &mut draw.vertices, &mut draw.indices);
            }

            // Shortcut (right-aligned)
            if let Some(ref shortcut) = item.shortcut {
                let shortcut_w = font::measure_text(shortcut, font_size, None);
                let mut sx = self.x + width - 12.0 - shortcut_w;
                let shortcut_color = [0.55, 0.55, 0.58, 1.0];
                for c in shortcut.chars() {
                    let params = font::CharQuadParams {
                        c, x: sx, y: text_y, size: font_size, color: shortcut_color, atlas: None,
                    };
                    sx += font::emit_char_quads(&params, &mut draw.vertices, &mut draw.indices);
                }
            }

            y += item_h;
        }
    }
}

impl Default for ContextMenuState {
    fn default() -> Self {
        Self::new()
    }
}

// ── Command Palette ──────────────────────────────────────────────────────────

/// A command that can be executed from the palette.
#[derive(Clone)]
pub struct Command {
    pub name: String,
    pub shortcut: Option<String>,
    pub category: String,
}

impl Command {
    pub fn new(name: &str, category: &str) -> Self {
        Self { name: name.to_string(), shortcut: None, category: category.to_string() }
    }

    pub fn with_shortcut(mut self, shortcut: &str) -> Self {
        self.shortcut = Some(shortcut.to_string());
        self
    }
}

/// State for the command palette.
pub struct CommandPaletteState {
    pub visible: bool,
    pub query: TextInputState,
    pub commands: Vec<Command>,
    pub filtered: Vec<usize>,
    pub selected: usize,
}

impl CommandPaletteState {
    pub fn new(commands: Vec<Command>) -> Self {
        let filtered: Vec<usize> = (0..commands.len()).collect();
        Self {
            visible: false,
            query: TextInputState::new(""),
            commands,
            filtered,
            selected: 0,
        }
    }

    /// Open the command palette.
    pub fn open(&mut self) {
        self.visible = true;
        self.query = TextInputState::new("");
        self.query.focused = true;
        self.filter();
        self.selected = 0;
    }

    /// Close the command palette.
    pub fn close(&mut self) {
        self.visible = false;
    }

    /// Filter commands based on current query.
    pub fn filter(&mut self) {
        let q = self.query.text.to_lowercase();
        self.filtered = self.commands.iter().enumerate()
            .filter(|(_, cmd)| {
                if q.is_empty() { return true; }
                cmd.name.to_lowercase().contains(&q)
                    || cmd.category.to_lowercase().contains(&q)
            })
            .map(|(i, _)| i)
            .collect();
        if self.selected >= self.filtered.len() {
            self.selected = 0;
        }
    }

    /// Move selection up.
    pub fn select_prev(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
        }
    }

    /// Move selection down.
    pub fn select_next(&mut self) {
        if self.selected + 1 < self.filtered.len() {
            self.selected += 1;
        }
    }

    /// Get the currently selected command index (into self.commands).
    pub fn selected_command(&self) -> Option<usize> {
        self.filtered.get(self.selected).copied()
    }

    /// Draw the command palette.
    pub fn draw(&self, draw: &mut DrawList, screen_w: f32, _screen_h: f32) {
        if !self.visible { return; }

        let palette_w = 500.0_f32.min(screen_w - 80.0);
        let x = (screen_w - palette_w) * 0.5;
        let y = 60.0;
        let item_h = 28.0;
        let input_h = 36.0;
        let font_size = 13.0;
        let max_visible = 10usize;
        let visible_count = self.filtered.len().min(max_visible);
        let total_h = input_h + visible_count as f32 * item_h + 8.0;

        // Backdrop shadow
        draw.push_quad(x + 3.0, y + 3.0, palette_w, total_h, [0.0, 0.0, 0.0, 0.4]);
        // Background
        draw.push_quad(x, y, palette_w, total_h, [0.12, 0.13, 0.15, 0.98]);
        // Border
        draw.push_quad(x, y, palette_w, 1.0, [0.3, 0.5, 0.8, 0.7]);
        draw.push_quad(x, y + total_h - 1.0, palette_w, 1.0, [0.3, 0.5, 0.8, 0.7]);
        draw.push_quad(x, y, 1.0, total_h, [0.3, 0.5, 0.8, 0.7]);
        draw.push_quad(x + palette_w - 1.0, y, 1.0, total_h, [0.3, 0.5, 0.8, 0.7]);

        // Input field background
        draw.push_quad(x + 4.0, y + 4.0, palette_w - 8.0, input_h - 8.0, [0.18, 0.19, 0.22, 1.0]);

        // Input text
        let input_text = if self.query.text.is_empty() { "Type a command..." } else { &self.query.text };
        let text_color = if self.query.text.is_empty() {
            [0.45, 0.45, 0.50, 1.0]
        } else {
            [0.9, 0.9, 0.92, 1.0]
        };
        let mut cx = x + 12.0;
        let ty = y + (input_h - font_size) * 0.5;
        for c in input_text.chars() {
            let params = font::CharQuadParams {
                c, x: cx, y: ty, size: font_size, color: text_color, atlas: None,
            };
            cx += font::emit_char_quads(&params, &mut draw.vertices, &mut draw.indices);
        }

        // Cursor
        if !self.query.text.is_empty() || self.query.focused {
            let cursor_x = x + 12.0 + font::measure_text(
                &self.query.text[..self.query.cursor_pos], font_size, None
            );
            draw.push_quad(cursor_x, y + 8.0, 1.0, input_h - 16.0, [0.5, 0.7, 1.0, 0.8]);
        }

        // Separator
        draw.push_quad(x + 4.0, y + input_h - 2.0, palette_w - 8.0, 1.0, [0.3, 0.3, 0.35, 0.5]);

        // Command list
        let mut iy = y + input_h + 2.0;
        for (display_idx, &cmd_idx) in self.filtered.iter().take(max_visible).enumerate() {
            let cmd = &self.commands[cmd_idx];

            // Selection highlight
            if display_idx == self.selected {
                draw.push_quad(x + 2.0, iy, palette_w - 4.0, item_h, [0.22, 0.38, 0.65, 0.7]);
            }

            // Category
            let cat_color = [0.45, 0.55, 0.7, 1.0];
            let mut tx = x + 12.0;
            for c in cmd.category.chars() {
                let params = font::CharQuadParams {
                    c, x: tx, y: iy + (item_h - font_size) * 0.5, size: font_size - 1.0,
                    color: cat_color, atlas: None,
                };
                tx += font::emit_char_quads(&params, &mut draw.vertices, &mut draw.indices);
            }
            tx += 6.0;

            // Name
            let name_color = [0.88, 0.88, 0.90, 1.0];
            for c in cmd.name.chars() {
                let params = font::CharQuadParams {
                    c, x: tx, y: iy + (item_h - font_size) * 0.5, size: font_size,
                    color: name_color, atlas: None,
                };
                tx += font::emit_char_quads(&params, &mut draw.vertices, &mut draw.indices);
            }

            // Shortcut (right-aligned)
            if let Some(ref shortcut) = cmd.shortcut {
                let sw = font::measure_text(shortcut, font_size - 1.0, None);
                let mut sx = x + palette_w - 12.0 - sw;
                let sc = [0.5, 0.5, 0.55, 1.0];
                for c in shortcut.chars() {
                    let params = font::CharQuadParams {
                        c, x: sx, y: iy + (item_h - font_size) * 0.5, size: font_size - 1.0,
                        color: sc, atlas: None,
                    };
                    sx += font::emit_char_quads(&params, &mut draw.vertices, &mut draw.indices);
                }
            }

            iy += item_h;
        }
    }
}

// ── Status Bar ──────────────────────────────────────────────────────────────

/// Information to display in the status bar.
pub struct StatusBarInfo {
    pub tool: String,
    pub coordinates: Option<String>,
    pub mode: String,
    pub hints: String,
    pub object_count: usize,
}

impl StatusBarInfo {
    pub fn new() -> Self {
        Self {
            tool: "Select".into(),
            coordinates: None,
            mode: "Object".into(),
            hints: String::new(),
            object_count: 0,
        }
    }

    /// Draw the status bar at the bottom of the screen.
    pub fn draw(&self, draw: &mut DrawList, screen_w: f32, screen_h: f32, theme: &[f32; 4], text_color: &[f32; 4]) {
        let bar_h = 22.0;
        let y = screen_h - bar_h;
        let font_size = 11.0;
        let padding = 8.0;

        // Background
        draw.push_quad(0.0, y, screen_w, bar_h, *theme);
        // Top border
        draw.push_quad(0.0, y, screen_w, 1.0, [theme[0] + 0.1, theme[1] + 0.1, theme[2] + 0.1, 0.8]);

        // Left section: tool + mode
        let mut cx = padding;
        let ty = y + (bar_h - font_size) * 0.5;

        let left_text = format!("{} | {}", self.tool, self.mode);
        for c in left_text.chars() {
            let params = font::CharQuadParams {
                c, x: cx, y: ty, size: font_size, color: *text_color, atlas: None,
            };
            cx += font::emit_char_quads(&params, &mut draw.vertices, &mut draw.indices);
        }

        // Center section: coordinates
        if let Some(ref coords) = self.coordinates {
            let coords_w = font::measure_text(coords, font_size, None);
            let mut ccx = (screen_w - coords_w) * 0.5;
            let coord_color = [text_color[0] * 0.8, text_color[1] * 0.8, text_color[2] * 0.8, text_color[3]];
            for c in coords.chars() {
                let params = font::CharQuadParams {
                    c, x: ccx, y: ty, size: font_size, color: coord_color, atlas: None,
                };
                ccx += font::emit_char_quads(&params, &mut draw.vertices, &mut draw.indices);
            }
        }

        // Right section: hints + object count
        let right_text = if self.hints.is_empty() {
            format!("{} objects", self.object_count)
        } else {
            format!("{} | {} objects", self.hints, self.object_count)
        };
        let right_w = font::measure_text(&right_text, font_size, None);
        let mut rx = screen_w - padding - right_w;
        let muted = [text_color[0] * 0.6, text_color[1] * 0.6, text_color[2] * 0.6, text_color[3]];
        for c in right_text.chars() {
            let params = font::CharQuadParams {
                c, x: rx, y: ty, size: font_size, color: muted, atlas: None,
            };
            rx += font::emit_char_quads(&params, &mut draw.vertices, &mut draw.indices);
        }
    }
}

impl Default for StatusBarInfo {
    fn default() -> Self {
        Self::new()
    }
}

// ── Toolbar ─────────────────────────────────────────────────────────────────

/// A toolbar button.
#[derive(Clone)]
pub struct ToolButton {
    pub label: String,
    pub tooltip: String,
    pub shortcut: Option<String>,
    pub active: bool,
}

impl ToolButton {
    pub fn new(label: &str, tooltip: &str) -> Self {
        Self { label: label.to_string(), tooltip: tooltip.to_string(), shortcut: None, active: false }
    }

    pub fn with_shortcut(mut self, shortcut: &str) -> Self {
        self.shortcut = Some(shortcut.to_string());
        self
    }
}

/// State for a toolbar strip.
pub struct Toolbar {
    pub buttons: Vec<ToolButton>,
    pub hovered: Option<usize>,
    pub orientation: ToolbarOrientation,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ToolbarOrientation {
    Horizontal,
    Vertical,
}

impl Toolbar {
    pub fn new(orientation: ToolbarOrientation) -> Self {
        Self { buttons: Vec::new(), hovered: None, orientation }
    }

    pub fn add(&mut self, button: ToolButton) {
        self.buttons.push(button);
    }

    /// Hit-test: returns the button index under the mouse.
    pub fn hit_test(&self, mx: f32, my: f32, start_x: f32, start_y: f32) -> Option<usize> {
        let btn_size = 32.0;
        let gap = 2.0;

        for (i, _) in self.buttons.iter().enumerate() {
            let (bx, by) = match self.orientation {
                ToolbarOrientation::Horizontal => (start_x + i as f32 * (btn_size + gap), start_y),
                ToolbarOrientation::Vertical => (start_x, start_y + i as f32 * (btn_size + gap)),
            };
            if mx >= bx && mx <= bx + btn_size && my >= by && my <= by + btn_size {
                return Some(i);
            }
        }
        None
    }

    /// Draw the toolbar.
    pub fn draw(&self, draw: &mut DrawList, start_x: f32, start_y: f32, bg_color: [f32; 4], text_color: [f32; 4]) {
        let btn_size = 32.0;
        let gap = 2.0;
        let font_size = 14.0;

        for (i, btn) in self.buttons.iter().enumerate() {
            let (bx, by) = match self.orientation {
                ToolbarOrientation::Horizontal => (start_x + i as f32 * (btn_size + gap), start_y),
                ToolbarOrientation::Vertical => (start_x, start_y + i as f32 * (btn_size + gap)),
            };

            let color = if btn.active {
                [bg_color[0] + 0.15, bg_color[1] + 0.15, bg_color[2] + 0.2, bg_color[3]]
            } else if self.hovered == Some(i) {
                [bg_color[0] + 0.08, bg_color[1] + 0.08, bg_color[2] + 0.1, bg_color[3]]
            } else {
                bg_color
            };

            draw.push_quad(bx, by, btn_size, btn_size, color);

            // Active indicator bar
            if btn.active {
                draw.push_quad(bx, by + btn_size - 2.0, btn_size, 2.0, [0.3, 0.6, 1.0, 1.0]);
            }

            // Icon as first char of label
            if let Some(c) = btn.label.chars().next() {
                let char_w = font::measure_text(&btn.label[..c.len_utf8()], font_size, None);
                let tx = bx + (btn_size - char_w) * 0.5;
                let ty = by + (btn_size - font_size) * 0.5;
                let params = font::CharQuadParams {
                    c, x: tx, y: ty, size: font_size, color: text_color, atlas: None,
                };
                font::emit_char_quads(&params, &mut draw.vertices, &mut draw.indices);
            }
        }
    }
}

// ── Outliner / Scene Tree ────────────────────────────────────────────────────

/// An entry in the outliner representing one scene node.
pub struct OutlinerEntry {
    pub name: String,
    pub visible: bool,
    pub selected: bool,
    pub index: usize,
}

/// Action returned from outliner interaction.
#[derive(Debug, Clone, PartialEq)]
pub enum OutlinerAction {
    Select(usize),
    ToggleVisibility(usize),
}

/// State for the scene outliner panel.
pub struct OutlinerState {
    pub entries: Vec<OutlinerEntry>,
    pub hovered: Option<usize>,
    pub scroll_offset: f32,
}

impl OutlinerState {
    pub fn new() -> Self {
        Self { entries: Vec::new(), hovered: None, scroll_offset: 0.0 }
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn push(&mut self, name: &str, index: usize, visible: bool, selected: bool) {
        self.entries.push(OutlinerEntry {
            name: name.to_string(),
            visible,
            selected,
            index,
        });
    }

    /// Hit test: returns action if mouse is over a clickable area.
    /// `panel_x`, `panel_y` = top-left of the outliner area.
    pub fn hit_test(&self, mx: f32, my: f32, panel_x: f32, panel_y: f32) -> Option<OutlinerAction> {
        let row_h = 22.0;
        let eye_w = 20.0;
        let header_h = 20.0;

        let ry = my - panel_y - header_h + self.scroll_offset;
        if ry < 0.0 { return None; }

        let row = (ry / row_h) as usize;
        if row >= self.entries.len() { return None; }

        // Check if click is on the visibility eye icon (rightmost 20px)
        let panel_w = 200.0; // default width
        if mx >= panel_x + panel_w - eye_w - 4.0 {
            return Some(OutlinerAction::ToggleVisibility(self.entries[row].index));
        }

        Some(OutlinerAction::Select(self.entries[row].index))
    }

    pub fn scroll(&mut self, delta: f32) {
        self.scroll_offset = (self.scroll_offset + delta).max(0.0);
    }

    /// Draw the outliner into a DrawList.
    pub fn draw(
        &self,
        draw: &mut DrawList,
        x: f32, y: f32,
        w: f32, h: f32,
        bg: [f32; 4],
        text_color: [f32; 4],
    ) {
        let row_h = 22.0;
        let font_size = 11.0;
        let header_h = 20.0;
        let padding = 6.0;

        // Panel background
        draw.push_quad(x, y, w, h, bg);

        // Header
        let header_bg = [bg[0] + 0.05, bg[1] + 0.05, bg[2] + 0.06, bg[3]];
        draw.push_quad(x, y, w, header_h, header_bg);

        let mut tx = x + padding;
        let ty = y + (header_h - font_size) * 0.5;
        for c in "Outliner".chars() {
            let params = font::CharQuadParams {
                c, x: tx, y: ty, size: font_size, color: text_color, atlas: None,
            };
            tx += font::emit_char_quads(&params, &mut draw.vertices, &mut draw.indices);
        }

        // Separator line
        draw.push_quad(x, y + header_h, w, 1.0, [text_color[0] * 0.3, text_color[1] * 0.3, text_color[2] * 0.3, 0.5]);

        // Rows
        let content_y = y + header_h + 1.0;
        let max_visible = ((h - header_h - 1.0) / row_h) as usize;

        for (i, entry) in self.entries.iter().enumerate() {
            if i >= max_visible { break; }

            let ry = content_y + i as f32 * row_h - self.scroll_offset;
            if ry + row_h < content_y || ry > y + h { continue; }

            // Row background
            if entry.selected {
                draw.push_quad(x, ry, w, row_h, [0.2, 0.4, 0.7, 0.5]);
            } else if self.hovered == Some(i) {
                draw.push_quad(x, ry, w, row_h, [bg[0] + 0.06, bg[1] + 0.06, bg[2] + 0.08, bg[3]]);
            }

            // Visibility icon
            let eye_x = x + w - 24.0;
            let eye_y = ry + (row_h - font_size) * 0.5;
            let eye_char = if entry.visible { 'O' } else { '-' };
            let eye_color = if entry.visible {
                text_color
            } else {
                [text_color[0] * 0.4, text_color[1] * 0.4, text_color[2] * 0.4, text_color[3]]
            };
            let params = font::CharQuadParams {
                c: eye_char, x: eye_x, y: eye_y, size: font_size, color: eye_color, atlas: None,
            };
            font::emit_char_quads(&params, &mut draw.vertices, &mut draw.indices);

            // Name text
            let name_x = x + padding;
            let name_y = ry + (row_h - font_size) * 0.5;
            let name_color = if entry.visible {
                text_color
            } else {
                [text_color[0] * 0.5, text_color[1] * 0.5, text_color[2] * 0.5, text_color[3]]
            };
            let mut nx = name_x;
            for c in entry.name.chars() {
                let params = font::CharQuadParams {
                    c, x: nx, y: name_y, size: font_size, color: name_color, atlas: None,
                };
                nx += font::emit_char_quads(&params, &mut draw.vertices, &mut draw.indices);
            }
        }
    }
}

impl Default for OutlinerState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_input_insert_and_backspace() {
        let mut state = TextInputState::new("");
        state.insert_char('a');
        state.insert_char('b');
        assert_eq!(state.text, "ab");
        state.backspace();
        assert_eq!(state.text, "a");
    }

    #[test]
    fn text_input_cursor_movement() {
        let mut state = TextInputState::new("hello");
        assert_eq!(state.cursor_pos, 5);
        state.home();
        assert_eq!(state.cursor_pos, 0);
        state.move_right();
        assert_eq!(state.cursor_pos, 1);
        state.end();
        assert_eq!(state.cursor_pos, 5);
    }

    #[test]
    fn dropdown_default() {
        let state = DropdownState::new(0);
        assert_eq!(state.selected, 0);
        assert!(!state.open);
    }

    #[test]
    fn scroll_state() {
        let mut state = ScrollState::new();
        state.content_height = 500.0;
        state.viewport_height = 200.0;
        assert!(state.overflows());
        assert_eq!(state.max_offset(), 300.0);
        state.scroll(100.0);
        assert_eq!(state.offset_y, 100.0);
        state.scroll(500.0);
        assert_eq!(state.offset_y, 300.0);
    }

    #[test]
    fn tooltip_visibility() {
        let mut state = TooltipState::new();
        assert!(!state.visible());
        state.set("Hello", 100.0, 200.0);
        assert!(!state.visible()); // not enough time
        state.hover_time = 1.0;
        assert!(state.visible());
        state.reset();
        assert!(!state.visible());
    }

    #[test]
    fn context_menu_hit_test() {
        let state = ContextMenuState::new();
        assert!(state.hit_test(100.0, 100.0).is_none());
    }

    #[test]
    fn command_palette_filter() {
        let mut palette = CommandPaletteState::new(vec![
            Command::new("Select All", "Edit"),
            Command::new("Delete", "Edit"),
            Command::new("Zoom to Fit", "View"),
        ]);
        palette.open();
        assert_eq!(palette.filtered.len(), 3);
        palette.query.text = "zoom".to_string();
        palette.filter();
        assert_eq!(palette.filtered.len(), 1);
    }

    #[test]
    fn toolbar_hit_test() {
        let mut tb = Toolbar::new(ToolbarOrientation::Horizontal);
        tb.add(ToolButton::new("S", "Select"));
        tb.add(ToolButton::new("M", "Move"));
        assert_eq!(tb.hit_test(16.0, 16.0, 0.0, 0.0), Some(0));
        assert_eq!(tb.hit_test(35.0, 16.0, 0.0, 0.0), Some(1));
        assert!(tb.hit_test(200.0, 16.0, 0.0, 0.0).is_none());
    }

    #[test]
    fn status_bar_draw_produces_geometry() {
        let info = StatusBarInfo::new();
        let mut draw = DrawList::new();
        info.draw(&mut draw, 1920.0, 1080.0, &[0.1, 0.1, 0.12, 0.95], &[0.8, 0.8, 0.8, 1.0]);
        assert!(!draw.vertices.is_empty());
    }
}
