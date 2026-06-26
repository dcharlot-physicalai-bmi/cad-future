//! Workspace switcher — tab bar for switching between application modes.
//!
//! Inspired by Fusion 360 workspace selector and FreeCAD workbenches.
//! Each workspace reconfigures toolbar, panel content, and available tools.

use crate::draw::DrawList;
use crate::font;

/// Available workspace modes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkspaceMode {
    /// 3D modeling — solid/surface creation and editing.
    Design,
    /// 2D sketch — constrained 2D drawing on planes.
    Sketch,
    /// Visual rendering — materials, lighting, HDRI.
    Render,
    /// Physics simulation — FEA, CFD, thermal.
    Simulate,
    /// Manufacturing — toolpaths, DFM checks.
    Manufacture,
    /// 2D drawing — dimensioned orthographic views.
    Drawing,
}

impl WorkspaceMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Design => "Design",
            Self::Sketch => "Sketch",
            Self::Render => "Render",
            Self::Simulate => "Simulate",
            Self::Manufacture => "Manufacture",
            Self::Drawing => "Drawing",
        }
    }

    pub fn icon(self) -> &'static str {
        match self {
            Self::Design => "#",
            Self::Sketch => "/",
            Self::Render => "*",
            Self::Simulate => "~",
            Self::Drawing => "=",
            Self::Manufacture => "%",
        }
    }

    /// All available workspaces.
    pub fn all() -> &'static [WorkspaceMode] {
        &[
            Self::Design,
            Self::Sketch,
            Self::Render,
            Self::Simulate,
            Self::Manufacture,
            Self::Drawing,
        ]
    }

    /// Shortcut for this workspace (shown in UI).
    pub fn shortcut(self) -> &'static str {
        match self {
            Self::Design => "1",
            Self::Sketch => "2",
            Self::Render => "3",
            Self::Simulate => "4",
            Self::Manufacture => "5",
            Self::Drawing => "6",
        }
    }
}

/// The workspace switcher tab bar.
pub struct WorkspaceSwitcher {
    /// Current active workspace.
    pub active: WorkspaceMode,
    /// Hovered tab index.
    pub hovered: Option<usize>,
    /// Height of the switcher bar.
    pub height: f32,
    /// Whether the switcher is visible (can be hidden in focused modes).
    pub visible: bool,
    /// Transition animation progress (0.0 to 1.0).
    pub transition_t: f32,
    /// Previous workspace (for transition animation).
    pub prev_workspace: WorkspaceMode,
}

impl WorkspaceSwitcher {
    pub fn new() -> Self {
        Self {
            active: WorkspaceMode::Design,
            hovered: None,
            height: 28.0,
            visible: true,
            transition_t: 1.0,
            prev_workspace: WorkspaceMode::Design,
        }
    }

    /// Switch to a new workspace.
    pub fn set_workspace(&mut self, mode: WorkspaceMode) {
        if self.active != mode {
            self.prev_workspace = self.active;
            self.active = mode;
            self.transition_t = 0.0;
        }
    }

    /// Animate the transition. Call each frame.
    pub fn update(&mut self, dt: f32) {
        if self.transition_t < 1.0 {
            self.transition_t = (self.transition_t + dt * 5.0).min(1.0);
        }
    }

    /// Hit test: which tab was clicked? Returns index into WorkspaceMode::all().
    pub fn hit_test(&self, mx: f32, my: f32, bar_x: f32, bar_y: f32) -> Option<usize> {
        if !self.visible { return None; }
        if my < bar_y || my > bar_y + self.height { return None; }

        let tab_w = 90.0;
        let padding = 4.0;

        for (i, _mode) in WorkspaceMode::all().iter().enumerate() {
            let tx = bar_x + padding + i as f32 * (tab_w + 2.0);
            if mx >= tx && mx < tx + tab_w {
                return Some(i);
            }
        }
        None
    }

    /// Draw the workspace switcher.
    pub fn draw(
        &self,
        dl: &mut DrawList,
        bar_x: f32,
        bar_y: f32,
        screen_w: f32,
        bg_color: [f32; 4],
        text_color: [f32; 4],
        accent_color: [f32; 4],
    ) {
        if !self.visible { return; }

        // Background strip
        dl.push_quad(bar_x, bar_y, screen_w, self.height, bg_color);

        // Bottom border
        let border = [bg_color[0] - 0.05, bg_color[1] - 0.05, bg_color[2] - 0.05, 1.0];
        dl.push_quad(bar_x, bar_y + self.height - 1.0, screen_w, 1.0, border);

        let tab_w = 90.0;
        let tab_h = self.height - 4.0;
        let padding = 4.0;

        for (i, mode) in WorkspaceMode::all().iter().enumerate() {
            let tx = bar_x + padding + i as f32 * (tab_w + 2.0);
            let ty = bar_y + 2.0;
            let is_active = *mode == self.active;
            let is_hovered = self.hovered == Some(i);

            // Tab background
            let tab_bg = if is_active {
                [accent_color[0], accent_color[1], accent_color[2], 0.25]
            } else if is_hovered {
                [bg_color[0] + 0.06, bg_color[1] + 0.06, bg_color[2] + 0.06, 1.0]
            } else {
                [0.0, 0.0, 0.0, 0.0] // transparent
            };
            dl.push_quad(tx, ty, tab_w, tab_h, tab_bg);

            // Active indicator (bottom accent line)
            if is_active {
                dl.push_quad(tx, bar_y + self.height - 3.0, tab_w, 3.0, accent_color);
            }

            // Icon + Label
            let label_color = if is_active {
                accent_color
            } else if is_hovered {
                text_color
            } else {
                [text_color[0] * 0.7, text_color[1] * 0.7, text_color[2] * 0.7, text_color[3]]
            };

            let icon = mode.icon();
            emit_text(dl, icon, tx + 6.0, ty + 6.0, 12.0, label_color);

            let label = mode.label();
            emit_text(dl, label, tx + 20.0, ty + 7.0, 11.0, label_color);
        }

        // "Workspace" label at far right
        let label_x = bar_x + padding + WorkspaceMode::all().len() as f32 * (tab_w + 2.0) + 12.0;
        let muted = [text_color[0] * 0.4, text_color[1] * 0.4, text_color[2] * 0.4, text_color[3]];
        emit_text(dl, "Workspace", label_x, bar_y + 8.0, 10.0, muted);
    }
}

impl Default for WorkspaceSwitcher {
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
    fn default_is_design() {
        let ws = WorkspaceSwitcher::new();
        assert_eq!(ws.active, WorkspaceMode::Design);
    }

    #[test]
    fn switch_workspace() {
        let mut ws = WorkspaceSwitcher::new();
        ws.set_workspace(WorkspaceMode::Sketch);
        assert_eq!(ws.active, WorkspaceMode::Sketch);
        assert_eq!(ws.prev_workspace, WorkspaceMode::Design);
        assert_eq!(ws.transition_t, 0.0);
    }

    #[test]
    fn hit_test_tabs() {
        let ws = WorkspaceSwitcher::new();
        // First tab: x=4..94
        assert_eq!(ws.hit_test(50.0, 10.0, 0.0, 0.0), Some(0));
        // Second tab: x=96..186
        assert_eq!(ws.hit_test(100.0, 10.0, 0.0, 0.0), Some(1));
        // Outside
        assert_eq!(ws.hit_test(50.0, 40.0, 0.0, 0.0), None);
    }

    #[test]
    fn transition_animates() {
        let mut ws = WorkspaceSwitcher::new();
        ws.set_workspace(WorkspaceMode::Render);
        assert!(ws.transition_t < 0.01);
        ws.update(0.5);
        assert!(ws.transition_t > 0.5);
    }

    #[test]
    fn all_modes_have_labels() {
        for mode in WorkspaceMode::all() {
            assert!(!mode.label().is_empty());
            assert!(!mode.icon().is_empty());
        }
    }
}
