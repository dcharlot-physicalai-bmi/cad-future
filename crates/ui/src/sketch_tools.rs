//! Sketch tools palette — 2D sketch creation tools.
//!
//! Inspired by SolidWorks Sketch toolbar, Fusion 360 Sketch palette,
//! and Onshape Sketch tools. Provides tool selection for line, arc,
//! circle, rectangle, spline, offset, trim, mirror, and construction geometry.

use crate::draw::DrawList;
use crate::font;

/// Available sketch tools.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SketchTool {
    Select,
    Line,
    CenterLine,
    Rectangle,
    CenterRectangle,
    Circle,
    CenterPointArc,
    ThreePointArc,
    Ellipse,
    Spline,
    Point,
    Polygon,
    Slot,
    Offset,
    Trim,
    Extend,
    Mirror,
    FilletSketch,
    ChamferSketch,
    Text,
}

impl SketchTool {
    pub fn label(self) -> &'static str {
        match self {
            Self::Select => "Select",
            Self::Line => "Line",
            Self::CenterLine => "Center Line",
            Self::Rectangle => "Rectangle",
            Self::CenterRectangle => "Center Rect",
            Self::Circle => "Circle",
            Self::CenterPointArc => "Arc (Center)",
            Self::ThreePointArc => "Arc (3-Point)",
            Self::Ellipse => "Ellipse",
            Self::Spline => "Spline",
            Self::Point => "Point",
            Self::Polygon => "Polygon",
            Self::Slot => "Slot",
            Self::Offset => "Offset",
            Self::Trim => "Trim",
            Self::Extend => "Extend",
            Self::Mirror => "Mirror",
            Self::FilletSketch => "Fillet",
            Self::ChamferSketch => "Chamfer",
            Self::Text => "Text",
        }
    }

    pub fn icon(self) -> &'static str {
        match self {
            Self::Select => "^",
            Self::Line => "/",
            Self::CenterLine => "|",
            Self::Rectangle => "[]",
            Self::CenterRectangle => "[+]",
            Self::Circle => "O",
            Self::CenterPointArc => "(",
            Self::ThreePointArc => "~",
            Self::Ellipse => "0",
            Self::Spline => "S",
            Self::Point => ".",
            Self::Polygon => "<>",
            Self::Slot => "=",
            Self::Offset => ">>",
            Self::Trim => "X",
            Self::Extend => "->",
            Self::Mirror => "M",
            Self::FilletSketch => "R",
            Self::ChamferSketch => "C",
            Self::Text => "T",
        }
    }

    pub fn shortcut(self) -> &'static str {
        match self {
            Self::Line => "L",
            Self::Rectangle => "R",
            Self::Circle => "C",
            Self::Trim => "T",
            Self::Offset => "O",
            Self::Mirror => "Ctrl+M",
            _ => "",
        }
    }

    /// Draw tools (geometry creation).
    pub fn draw_tools() -> &'static [Self] {
        &[
            Self::Line, Self::CenterLine, Self::Rectangle, Self::CenterRectangle,
            Self::Circle, Self::CenterPointArc, Self::ThreePointArc, Self::Ellipse,
            Self::Spline, Self::Point, Self::Polygon, Self::Slot,
        ]
    }

    /// Modify tools (edit existing geometry).
    pub fn modify_tools() -> &'static [Self] {
        &[Self::Offset, Self::Trim, Self::Extend, Self::Mirror, Self::FilletSketch, Self::ChamferSketch]
    }
}

/// The sketch tools palette.
pub struct SketchTools {
    /// Whether the palette is visible (active sketch mode).
    pub visible: bool,
    /// Currently active tool.
    pub active_tool: SketchTool,
    /// Whether construction mode is on (dashed lines).
    pub construction_mode: bool,
    /// Hovered tool index in draw section.
    pub hovered_draw: Option<usize>,
    /// Hovered tool index in modify section.
    pub hovered_modify: Option<usize>,
    /// Button size.
    pub button_size: f32,
    /// Columns in grid.
    pub columns: usize,
}

impl SketchTools {
    pub fn new() -> Self {
        Self {
            visible: false,
            active_tool: SketchTool::Select,
            construction_mode: false,
            hovered_draw: None,
            hovered_modify: None,
            button_size: 32.0,
            columns: 4,
        }
    }

    /// Activate the sketch palette.
    pub fn activate(&mut self) {
        self.visible = true;
        self.active_tool = SketchTool::Line;
    }

    /// Deactivate the sketch palette.
    pub fn deactivate(&mut self) {
        self.visible = false;
        self.active_tool = SketchTool::Select;
    }

    /// Set the active tool.
    pub fn set_tool(&mut self, tool: SketchTool) {
        self.active_tool = tool;
    }

    /// Hit test the draw tools grid. Returns tool index.
    pub fn hit_test_draw(&self, mx: f32, my: f32, panel_x: f32, panel_y: f32) -> Option<usize> {
        if !self.visible { return None; }
        let tools = SketchTool::draw_tools();
        let gap = 2.0;
        let header_h = 40.0;
        for (i, _) in tools.iter().enumerate() {
            let col = i % self.columns;
            let row = i / self.columns;
            let bx = panel_x + col as f32 * (self.button_size + gap);
            let by = panel_y + header_h + row as f32 * (self.button_size + gap);
            if mx >= bx && mx < bx + self.button_size && my >= by && my < by + self.button_size {
                return Some(i);
            }
        }
        None
    }

    /// Hit test modify tools. Returns tool index.
    pub fn hit_test_modify(&self, mx: f32, my: f32, panel_x: f32, panel_y: f32) -> Option<usize> {
        if !self.visible { return None; }
        let draw_tools = SketchTool::draw_tools();
        let draw_rows = (draw_tools.len() + self.columns - 1) / self.columns;
        let gap = 2.0;
        let header_h = 40.0;
        let modify_y = panel_y + header_h + draw_rows as f32 * (self.button_size + gap) + 24.0;

        let modify_tools = SketchTool::modify_tools();
        for (i, _) in modify_tools.iter().enumerate() {
            let col = i % self.columns;
            let row = i / self.columns;
            let bx = panel_x + col as f32 * (self.button_size + gap);
            let by = modify_y + row as f32 * (self.button_size + gap);
            if mx >= bx && mx < bx + self.button_size && my >= by && my < by + self.button_size {
                return Some(i);
            }
        }
        None
    }

    /// Draw the sketch tools palette.
    pub fn draw(
        &self,
        dl: &mut DrawList,
        panel_x: f32,
        panel_y: f32,
        bg_color: [f32; 4],
        text_color: [f32; 4],
        accent_color: [f32; 4],
    ) {
        if !self.visible { return; }

        let gap = 2.0;
        let header_h = 40.0;
        let draw_tools = SketchTool::draw_tools();
        let modify_tools = SketchTool::modify_tools();
        let draw_rows = (draw_tools.len() + self.columns - 1) / self.columns;
        let modify_rows = (modify_tools.len() + self.columns - 1) / self.columns;
        let panel_w = self.columns as f32 * (self.button_size + gap);
        let panel_h = header_h + draw_rows as f32 * (self.button_size + gap) + 24.0
            + modify_rows as f32 * (self.button_size + gap) + 8.0;

        // Background
        dl.push_quad(panel_x, panel_y, panel_w, panel_h, bg_color);

        let muted = [text_color[0] * 0.5, text_color[1] * 0.5, text_color[2] * 0.5, text_color[3]];

        // Title
        emit_text(dl, "Sketch", panel_x + 4.0, panel_y + 4.0, 11.0, text_color);

        // Active tool label
        emit_text(dl, self.active_tool.label(), panel_x + 4.0, panel_y + 20.0, 8.0, accent_color);

        // Construction mode toggle
        let cmode_x = panel_x + panel_w - 40.0;
        let cmode_bg = if self.construction_mode { accent_color } else { [0.3, 0.3, 0.3, 0.5] };
        dl.push_quad(cmode_x, panel_y + 18.0, 36.0, 14.0, cmode_bg);
        emit_text(dl, "Const", cmode_x + 4.0, panel_y + 20.0, 8.0,
            if self.construction_mode { [1.0, 1.0, 1.0, 1.0] } else { text_color });

        // ── Draw tools section ──
        emit_text(dl, "Draw", panel_x + 4.0, panel_y + header_h - 12.0, 7.0, muted);

        for (i, tool) in draw_tools.iter().enumerate() {
            let col = i % self.columns;
            let row = i / self.columns;
            let bx = panel_x + col as f32 * (self.button_size + gap);
            let by = panel_y + header_h + row as f32 * (self.button_size + gap);

            let is_active = self.active_tool == *tool;
            let is_hov = self.hovered_draw == Some(i);

            let btn_bg = if is_active {
                accent_color
            } else if is_hov {
                [0.35, 0.35, 0.35, 0.6]
            } else {
                [0.25, 0.25, 0.25, 0.5]
            };
            dl.push_quad(bx, by, self.button_size, self.button_size, btn_bg);

            let tc = if is_active { [1.0, 1.0, 1.0, 1.0] } else { text_color };
            emit_text(dl, tool.icon(), bx + 8.0, by + 6.0, 12.0, tc);

            // Tool label (small, below icon)
            let lbl = tool.label();
            let short_lbl = if lbl.len() > 6 { &lbl[..6] } else { lbl };
            emit_text(dl, short_lbl, bx + 2.0, by + self.button_size - 10.0, 6.0,
                if is_active { [1.0, 1.0, 1.0, 0.7] } else { muted });
        }

        // ── Modify tools section ──
        let modify_y = panel_y + header_h + draw_rows as f32 * (self.button_size + gap);
        emit_text(dl, "Modify", panel_x + 4.0, modify_y + 8.0, 7.0, muted);

        let modify_start_y = modify_y + 24.0;
        for (i, tool) in modify_tools.iter().enumerate() {
            let col = i % self.columns;
            let row = i / self.columns;
            let bx = panel_x + col as f32 * (self.button_size + gap);
            let by = modify_start_y + row as f32 * (self.button_size + gap);

            let is_active = self.active_tool == *tool;
            let is_hov = self.hovered_modify == Some(i);

            let btn_bg = if is_active {
                accent_color
            } else if is_hov {
                [0.35, 0.35, 0.35, 0.6]
            } else {
                [0.25, 0.25, 0.25, 0.5]
            };
            dl.push_quad(bx, by, self.button_size, self.button_size, btn_bg);

            let tc = if is_active { [1.0, 1.0, 1.0, 1.0] } else { text_color };
            emit_text(dl, tool.icon(), bx + 8.0, by + 6.0, 12.0, tc);

            let lbl = tool.label();
            let short_lbl = if lbl.len() > 6 { &lbl[..6] } else { lbl };
            emit_text(dl, short_lbl, bx + 2.0, by + self.button_size - 10.0, 6.0,
                if is_active { [1.0, 1.0, 1.0, 0.7] } else { muted });
        }
    }
}

impl Default for SketchTools {
    fn default() -> Self { Self::new() }
}

fn emit_text(dl: &mut DrawList, text: &str, x: f32, y: f32, size: f32, color: [f32; 4]) {
    let mut cx = x;
    for c in text.chars() {
        let params = font::CharQuadParams { c, x: cx, y, size, color, atlas: None };
        cx += font::emit_char_quads(&params, &mut dl.vertices, &mut dl.indices);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn activate_deactivate() {
        let mut st = SketchTools::new();
        assert!(!st.visible);
        st.activate();
        assert!(st.visible);
        assert_eq!(st.active_tool, SketchTool::Line);
        st.deactivate();
        assert!(!st.visible);
    }

    #[test]
    fn draw_tools_count() {
        assert!(SketchTool::draw_tools().len() >= 10);
    }

    #[test]
    fn modify_tools_count() {
        assert!(SketchTool::modify_tools().len() >= 5);
    }

    #[test]
    fn all_tools_have_labels() {
        for t in SketchTool::draw_tools() {
            assert!(!t.label().is_empty());
            assert!(!t.icon().is_empty());
        }
        for t in SketchTool::modify_tools() {
            assert!(!t.label().is_empty());
            assert!(!t.icon().is_empty());
        }
    }

    #[test]
    fn construction_toggle() {
        let mut st = SketchTools::new();
        assert!(!st.construction_mode);
        st.construction_mode = true;
        assert!(st.construction_mode);
    }
}
