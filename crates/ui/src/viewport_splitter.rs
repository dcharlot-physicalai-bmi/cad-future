//! Viewport splitter — multi-view layout control (quad view).
//!
//! Inspired by SolidWorks four-viewport layout, Blender quad view (Ctrl+Alt+Q),
//! and AutoCAD viewports. Splits the 3D viewport into 1, 2, or 4 panes
//! showing different camera angles simultaneously.

use crate::draw::DrawList;
use crate::font;

/// Layout mode for the viewport.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ViewportLayout {
    /// Single viewport (default).
    Single,
    /// Two viewports side by side (left/right).
    SplitHorizontal,
    /// Two viewports stacked (top/bottom).
    SplitVertical,
    /// Four viewports (quad view: Front/Right/Top/Perspective).
    Quad,
}

impl ViewportLayout {
    pub fn label(self) -> &'static str {
        match self {
            Self::Single => "Single",
            Self::SplitHorizontal => "Split H",
            Self::SplitVertical => "Split V",
            Self::Quad => "Quad",
        }
    }

    pub fn pane_count(self) -> usize {
        match self {
            Self::Single => 1,
            Self::SplitHorizontal | Self::SplitVertical => 2,
            Self::Quad => 4,
        }
    }

    pub fn cycle(self) -> Self {
        match self {
            Self::Single => Self::SplitHorizontal,
            Self::SplitHorizontal => Self::SplitVertical,
            Self::SplitVertical => Self::Quad,
            Self::Quad => Self::Single,
        }
    }
}

/// A camera preset assigned to a viewport pane.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PanePreset {
    Front,
    Back,
    Right,
    Left,
    Top,
    Bottom,
    Perspective,
    Isometric,
    Custom,
}

impl PanePreset {
    pub fn label(self) -> &'static str {
        match self {
            Self::Front => "Front",
            Self::Back => "Back",
            Self::Right => "Right",
            Self::Left => "Left",
            Self::Top => "Top",
            Self::Bottom => "Bottom",
            Self::Perspective => "Perspective",
            Self::Isometric => "Isometric",
            Self::Custom => "Custom",
        }
    }

    /// Default quad layout presets.
    pub fn quad_defaults() -> [PanePreset; 4] {
        [Self::Front, Self::Right, Self::Top, Self::Perspective]
    }
}

/// A single viewport pane.
#[derive(Clone, Debug)]
pub struct ViewportPane {
    /// Camera preset for this pane.
    pub preset: PanePreset,
    /// Whether this pane is the active (focused) one.
    pub active: bool,
    /// Screen-space rect [x, y, w, h].
    pub rect: [f32; 4],
}

impl ViewportPane {
    pub fn new(preset: PanePreset) -> Self {
        Self {
            preset,
            active: false,
            rect: [0.0; 4],
        }
    }
}

/// The viewport splitter.
pub struct ViewportSplitter {
    /// Current layout mode.
    pub layout: ViewportLayout,
    /// Panes (up to 4).
    pub panes: Vec<ViewportPane>,
    /// Active pane index.
    pub active_pane: usize,
    /// Hovered pane index.
    pub hovered_pane: Option<usize>,
    /// Whether the splitter borders are being dragged.
    pub dragging: bool,
    /// Split ratio (0.0 to 1.0) for horizontal split.
    pub split_h: f32,
    /// Split ratio (0.0 to 1.0) for vertical split.
    pub split_v: f32,
    /// Border thickness.
    pub border: f32,
}

impl ViewportSplitter {
    pub fn new() -> Self {
        Self {
            layout: ViewportLayout::Single,
            panes: vec![ViewportPane::new(PanePreset::Perspective)],
            active_pane: 0,
            hovered_pane: None,
            dragging: false,
            split_h: 0.5,
            split_v: 0.5,
            border: 2.0,
        }
    }

    /// Set the layout and configure panes.
    pub fn set_layout(&mut self, layout: ViewportLayout) {
        self.layout = layout;
        self.panes.clear();
        match layout {
            ViewportLayout::Single => {
                self.panes.push(ViewportPane::new(PanePreset::Perspective));
            }
            ViewportLayout::SplitHorizontal => {
                self.panes.push(ViewportPane::new(PanePreset::Front));
                self.panes.push(ViewportPane::new(PanePreset::Perspective));
            }
            ViewportLayout::SplitVertical => {
                self.panes.push(ViewportPane::new(PanePreset::Top));
                self.panes.push(ViewportPane::new(PanePreset::Perspective));
            }
            ViewportLayout::Quad => {
                let presets = PanePreset::quad_defaults();
                for p in presets {
                    self.panes.push(ViewportPane::new(p));
                }
            }
        }
        self.active_pane = self.panes.len() - 1; // perspective is usually last
        if let Some(pane) = self.panes.get_mut(self.active_pane) {
            pane.active = true;
        }
    }

    /// Cycle to next layout.
    pub fn cycle_layout(&mut self) {
        self.set_layout(self.layout.cycle());
    }

    /// Resolve pane rects based on available area.
    pub fn resolve(&mut self, x: f32, y: f32, w: f32, h: f32) {
        let b = self.border;
        match self.layout {
            ViewportLayout::Single => {
                if let Some(pane) = self.panes.get_mut(0) {
                    pane.rect = [x, y, w, h];
                }
            }
            ViewportLayout::SplitHorizontal => {
                let split = w * self.split_h;
                if let Some(p) = self.panes.get_mut(0) {
                    p.rect = [x, y, split - b * 0.5, h];
                }
                if let Some(p) = self.panes.get_mut(1) {
                    p.rect = [x + split + b * 0.5, y, w - split - b * 0.5, h];
                }
            }
            ViewportLayout::SplitVertical => {
                let split = h * self.split_v;
                if let Some(p) = self.panes.get_mut(0) {
                    p.rect = [x, y, w, split - b * 0.5];
                }
                if let Some(p) = self.panes.get_mut(1) {
                    p.rect = [x, y + split + b * 0.5, w, h - split - b * 0.5];
                }
            }
            ViewportLayout::Quad => {
                let sh = w * self.split_h;
                let sv = h * self.split_v;
                let left_w = sh - b * 0.5;
                let right_w = w - sh - b * 0.5;
                let top_h = sv - b * 0.5;
                let bot_h = h - sv - b * 0.5;
                if let Some(p) = self.panes.get_mut(0) {
                    p.rect = [x, y, left_w, top_h]; // top-left
                }
                if let Some(p) = self.panes.get_mut(1) {
                    p.rect = [x + sh + b * 0.5, y, right_w, top_h]; // top-right
                }
                if let Some(p) = self.panes.get_mut(2) {
                    p.rect = [x, y + sv + b * 0.5, left_w, bot_h]; // bottom-left
                }
                if let Some(p) = self.panes.get_mut(3) {
                    p.rect = [x + sh + b * 0.5, y + sv + b * 0.5, right_w, bot_h]; // bottom-right
                }
            }
        }
    }

    /// Hit test: which pane is the mouse in?
    pub fn hit_test_pane(&self, mx: f32, my: f32) -> Option<usize> {
        for (i, pane) in self.panes.iter().enumerate() {
            let [px, py, pw, ph] = pane.rect;
            if mx >= px && mx < px + pw && my >= py && my < py + ph {
                return Some(i);
            }
        }
        None
    }

    /// Set active pane.
    pub fn set_active(&mut self, idx: usize) {
        for (i, pane) in self.panes.iter_mut().enumerate() {
            pane.active = i == idx;
        }
        self.active_pane = idx;
    }

    /// Draw the viewport splitter borders and pane labels.
    pub fn draw(
        &self,
        dl: &mut DrawList,
        x: f32, y: f32, w: f32, h: f32,
        border_color: [f32; 4],
        text_color: [f32; 4],
        accent_color: [f32; 4],
    ) {
        if self.layout == ViewportLayout::Single { return; }

        let b = self.border;

        // Draw split borders
        match self.layout {
            ViewportLayout::SplitHorizontal => {
                let split = x + w * self.split_h;
                dl.push_quad(split - b * 0.5, y, b, h, border_color);
            }
            ViewportLayout::SplitVertical => {
                let split = y + h * self.split_v;
                dl.push_quad(x, split - b * 0.5, w, b, border_color);
            }
            ViewportLayout::Quad => {
                let sh = x + w * self.split_h;
                let sv = y + h * self.split_v;
                dl.push_quad(sh - b * 0.5, y, b, h, border_color);
                dl.push_quad(x, sv - b * 0.5, w, b, border_color);
            }
            ViewportLayout::Single => {}
        }

        // Draw pane labels (top-left corner of each pane)
        for (i, pane) in self.panes.iter().enumerate() {
            let [px, py, _pw, _ph] = pane.rect;
            let label = pane.preset.label();
            let color = if pane.active { accent_color } else { text_color };

            // Background for label
            let lw = font::measure_text(label, 10.0, None);
            dl.push_quad(px + 2.0, py + 2.0, lw + 8.0, 14.0,
                [0.0, 0.0, 0.0, 0.5]);
            emit_text(dl, label, px + 6.0, py + 4.0, 10.0, color);

            // Active indicator
            if pane.active {
                dl.push_quad(px, py, 3.0, 14.0, accent_color);
            }

            let _ = i; // suppress unused
        }
    }
}

impl Default for ViewportSplitter {
    fn default() -> Self {
        Self::new()
    }
}

fn emit_text(dl: &mut DrawList, text: &str, x: f32, y: f32, size: f32, color: [f32; 4]) {
    let mut cx = x;
    for c in text.chars() {
        let params = font::CharQuadParams {
            c, x: cx, y, size, color, atlas: None,
        };
        cx += font::emit_char_quads(&params, &mut dl.vertices, &mut dl.indices);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_single() {
        let vs = ViewportSplitter::new();
        assert_eq!(vs.layout, ViewportLayout::Single);
        assert_eq!(vs.panes.len(), 1);
    }

    #[test]
    fn set_quad_layout() {
        let mut vs = ViewportSplitter::new();
        vs.set_layout(ViewportLayout::Quad);
        assert_eq!(vs.panes.len(), 4);
        assert_eq!(vs.panes[0].preset, PanePreset::Front);
        assert_eq!(vs.panes[3].preset, PanePreset::Perspective);
    }

    #[test]
    fn resolve_rects() {
        let mut vs = ViewportSplitter::new();
        vs.set_layout(ViewportLayout::SplitHorizontal);
        vs.resolve(0.0, 0.0, 800.0, 600.0);
        assert!(vs.panes[0].rect[2] > 0.0);
        assert!(vs.panes[1].rect[2] > 0.0);
        // Both widths should approximately sum to 800
        let total_w = vs.panes[0].rect[2] + vs.panes[1].rect[2] + vs.border;
        assert!((total_w - 800.0).abs() < 1.0);
    }

    #[test]
    fn cycle_layout() {
        let mut vs = ViewportSplitter::new();
        vs.cycle_layout();
        assert_eq!(vs.layout, ViewportLayout::SplitHorizontal);
        vs.cycle_layout();
        assert_eq!(vs.layout, ViewportLayout::SplitVertical);
        vs.cycle_layout();
        assert_eq!(vs.layout, ViewportLayout::Quad);
        vs.cycle_layout();
        assert_eq!(vs.layout, ViewportLayout::Single);
    }

    #[test]
    fn hit_test_pane() {
        let mut vs = ViewportSplitter::new();
        vs.set_layout(ViewportLayout::SplitHorizontal);
        vs.resolve(0.0, 0.0, 800.0, 600.0);
        assert_eq!(vs.hit_test_pane(100.0, 300.0), Some(0)); // left pane
        assert_eq!(vs.hit_test_pane(600.0, 300.0), Some(1)); // right pane
    }
}
