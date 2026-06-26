//! AI Mate Dialog — modal for constraint creation, equation input, and AI suggestions.
//!
//! Provides a professional mate interface similar to SolidWorks Mate panel or
//! Fusion 360's Joint dialog. Includes an AI suggestion area that proposes
//! mate operations based on selected geometry.

use crate::draw::DrawList;
use crate::font;
use crate::widgets::TextInputState;

/// A suggested mate operation from the AI engine.
#[derive(Clone, Debug)]
pub struct MateSuggestion {
    pub label: String,
    pub description: String,
    pub op_id: &'static str,
    pub confidence: f32, // 0.0..1.0
}

/// Current step in the mate workflow.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MateStep {
    /// User needs to pick the first (moving) object.
    PickObjectA,
    /// User needs to pick the reference (fixed) object.
    PickObjectB,
    /// Both objects picked — choose mate type.
    ChooseMate,
    /// Editing parameters (distance, angle, etc.).
    EditParams,
}

/// The full mate dialog state.
pub struct MateDialog {
    pub visible: bool,
    pub step: MateStep,

    /// Index of the moving object.
    pub object_a: Option<usize>,
    pub object_a_name: String,
    /// Index of the reference object.
    pub object_b: Option<usize>,
    pub object_b_name: String,

    /// Available mate operations.
    pub operations: Vec<MateOpEntry>,
    /// Currently selected operation index.
    pub selected_op: usize,
    /// Hovered operation index.
    pub hovered_op: Option<usize>,

    /// AI-generated suggestions.
    pub suggestions: Vec<MateSuggestion>,
    pub selected_suggestion: Option<usize>,

    /// Parameter input (for offset distance, angle, etc.).
    pub param_input: TextInputState,
    pub param_label: String,

    /// Equation/expression input for parametric mates.
    pub equation_input: TextInputState,
    pub equation_visible: bool,

    /// Name for the constraint.
    pub constraint_name: TextInputState,
}

/// A mate operation available in the dialog.
#[derive(Clone, Debug)]
pub struct MateOpEntry {
    pub label: String,
    pub icon: &'static str,
    pub op_id: &'static str,
    pub needs_param: bool,
}

impl MateDialog {
    pub fn new() -> Self {
        Self {
            visible: false,
            step: MateStep::PickObjectA,
            object_a: None,
            object_a_name: String::new(),
            object_b: None,
            object_b_name: String::new(),
            operations: Self::default_operations(),
            selected_op: 0,
            hovered_op: None,
            suggestions: Vec::new(),
            selected_suggestion: None,
            param_input: TextInputState::new(""),
            param_label: "Distance".to_string(),
            equation_input: TextInputState::new(""),
            equation_visible: false,
            constraint_name: TextInputState::new("Mate1"),
        }
    }

    fn default_operations() -> Vec<MateOpEntry> {
        vec![
            MateOpEntry { label: "Stack On Top".into(), icon: "↑", op_id: "stack_top", needs_param: false },
            MateOpEntry { label: "Stack Below".into(), icon: "↓", op_id: "stack_below", needs_param: false },
            MateOpEntry { label: "Align X".into(), icon: "→", op_id: "align_x", needs_param: false },
            MateOpEntry { label: "Align Y".into(), icon: "↕", op_id: "align_y", needs_param: false },
            MateOpEntry { label: "Align Z".into(), icon: "⊙", op_id: "align_z", needs_param: false },
            MateOpEntry { label: "Flush +X".into(), icon: "⊢", op_id: "flush_px", needs_param: false },
            MateOpEntry { label: "Flush -X".into(), icon: "⊣", op_id: "flush_nx", needs_param: false },
            MateOpEntry { label: "Flush +Z".into(), icon: "⊤", op_id: "flush_pz", needs_param: false },
            MateOpEntry { label: "Flush -Z".into(), icon: "⊥", op_id: "flush_nz", needs_param: false },
            MateOpEntry { label: "Concentric".into(), icon: "◎", op_id: "concentric", needs_param: false },
            MateOpEntry { label: "Offset".into(), icon: "⇔", op_id: "offset", needs_param: true },
        ]
    }

    /// Open the dialog and start the mate workflow.
    pub fn open(&mut self) {
        self.visible = true;
        self.step = MateStep::PickObjectA;
        self.object_a = None;
        self.object_a_name.clear();
        self.object_b = None;
        self.object_b_name.clear();
        self.selected_op = 0;
        self.hovered_op = None;
        self.suggestions.clear();
        self.selected_suggestion = None;
        self.equation_visible = false;
        self.param_input = TextInputState::new("");
    }

    /// Close the dialog.
    pub fn close(&mut self) {
        self.visible = false;
    }

    /// Set object A (the moving part).
    pub fn set_object_a(&mut self, index: usize, name: &str) {
        self.object_a = Some(index);
        self.object_a_name = name.to_string();
        self.step = MateStep::PickObjectB;
    }

    /// Set object B (the reference part).
    pub fn set_object_b(&mut self, index: usize, name: &str) {
        self.object_b = Some(index);
        self.object_b_name = name.to_string();
        self.step = MateStep::ChooseMate;
        self.generate_suggestions();
    }

    /// Get the selected operation ID.
    pub fn selected_op_id(&self) -> &'static str {
        self.operations[self.selected_op].op_id
    }

    /// Whether the selected operation needs a parameter.
    pub fn needs_param(&self) -> bool {
        self.operations[self.selected_op].needs_param
    }

    /// Parse the parameter input value.
    pub fn param_value(&self) -> Option<f32> {
        self.param_input.text.parse().ok()
    }

    /// Toggle the equation/expression editor.
    pub fn toggle_equation(&mut self) {
        self.equation_visible = !self.equation_visible;
    }

    /// Generate AI suggestions based on the two selected objects.
    fn generate_suggestions(&mut self) {
        self.suggestions.clear();

        // Simple heuristic suggestions based on object names and positions
        let a = &self.object_a_name;
        let b = &self.object_b_name;

        // Always suggest stack and concentric as high confidence
        self.suggestions.push(MateSuggestion {
            label: format!("Stack {} on {}", a, b),
            description: "Place the moving part on top of the reference part".into(),
            op_id: "stack_top",
            confidence: 0.92,
        });

        self.suggestions.push(MateSuggestion {
            label: format!("Center {} with {}", a, b),
            description: "Align centers on XZ plane (concentric)".into(),
            op_id: "concentric",
            confidence: 0.85,
        });

        self.suggestions.push(MateSuggestion {
            label: format!("{} flush against {} (+X face)", a, b),
            description: "Place adjacent on the +X side".into(),
            op_id: "flush_px",
            confidence: 0.72,
        });

        // If one looks like a cylinder, suggest concentric higher
        if a.contains("Cyl") || b.contains("Cyl") || a.contains("Sphere") || b.contains("Sphere") {
            self.suggestions.push(MateSuggestion {
                label: "Concentric alignment (detected round shapes)".into(),
                description: "AI detected cylindrical/spherical geometry — concentric recommended".into(),
                op_id: "concentric",
                confidence: 0.95,
            });
        }

        // Sort by confidence descending
        self.suggestions.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap());
    }

    /// Hit-test mate operation list. Returns index if hit.
    pub fn hit_test_ops(&self, mx: f32, my: f32, panel_x: f32, panel_y: f32) -> Option<usize> {
        let ops_y = panel_y + 110.0;
        let item_h = 24.0;
        if mx < panel_x || mx > panel_x + 280.0 {
            return None;
        }
        for (i, _) in self.operations.iter().enumerate() {
            let iy = ops_y + i as f32 * item_h;
            if my >= iy && my < iy + item_h {
                return Some(i);
            }
        }
        None
    }

    /// Draw the mate dialog.
    pub fn draw(&self, draw: &mut DrawList, screen_w: f32, screen_h: f32) {
        if !self.visible {
            return;
        }

        let panel_w = 340.0;
        let panel_h = 520.0;
        let px = (screen_w - panel_w) * 0.5;
        let py = (screen_h - panel_h) * 0.5;

        // Backdrop
        draw.push_quad(0.0, 0.0, screen_w, screen_h, [0.0, 0.0, 0.0, 0.4]);

        // Panel
        draw.push_quad(px, py, panel_w, panel_h, [0.12, 0.12, 0.15, 0.97]);
        // Border
        draw.push_quad(px, py, panel_w, 1.0, [0.3, 0.4, 0.7, 0.8]);
        draw.push_quad(px, py + panel_h - 1.0, panel_w, 1.0, [0.3, 0.4, 0.7, 0.8]);
        draw.push_quad(px, py, 1.0, panel_h, [0.3, 0.4, 0.7, 0.8]);
        draw.push_quad(px + panel_w - 1.0, py, 1.0, panel_h, [0.3, 0.4, 0.7, 0.8]);

        // Title bar
        draw.push_quad(px, py, panel_w, 28.0, [0.15, 0.2, 0.35, 1.0]);
        self.draw_text(draw, "Mate / Constrain", px + 12.0, py + 7.0, 13.0, [1.0, 1.0, 1.0, 1.0]);

        // Step indicator
        let step_y = py + 34.0;
        let step_labels = ["1. Pick Part", "2. Pick Reference", "3. Choose Mate", "4. Parameters"];
        let active_step = match self.step {
            MateStep::PickObjectA => 0,
            MateStep::PickObjectB => 1,
            MateStep::ChooseMate => 2,
            MateStep::EditParams => 3,
        };
        let mut sx = px + 8.0;
        for (i, label) in step_labels.iter().enumerate() {
            let color = if i == active_step {
                [0.4, 0.7, 1.0, 1.0]
            } else if i < active_step {
                [0.3, 0.8, 0.4, 0.9]
            } else {
                [0.4, 0.4, 0.45, 0.6]
            };
            self.draw_text(draw, label, sx, step_y, 9.0, color);
            sx += font::measure_text(label, 9.0, None) + 8.0;
        }

        // Object A/B display
        let obj_y = py + 50.0;
        draw.push_quad(px + 8.0, obj_y, panel_w - 16.0, 50.0, [0.08, 0.08, 0.1, 0.8]);

        let a_label = if self.object_a.is_some() {
            format!("Part A: {} (#{}) ✓", self.object_a_name, self.object_a.unwrap())
        } else {
            "Part A: Click to select...".to_string()
        };
        let a_color = if self.object_a.is_some() {
            [0.4, 0.9, 0.5, 1.0]
        } else if self.step == MateStep::PickObjectA {
            [1.0, 0.8, 0.3, 1.0]
        } else {
            [0.5, 0.5, 0.5, 0.6]
        };
        self.draw_text(draw, &a_label, px + 16.0, obj_y + 8.0, 11.0, a_color);

        let b_label = if self.object_b.is_some() {
            format!("Part B: {} (#{}) ✓", self.object_b_name, self.object_b.unwrap())
        } else {
            "Part B: Click to select...".to_string()
        };
        let b_color = if self.object_b.is_some() {
            [0.4, 0.9, 0.5, 1.0]
        } else if self.step == MateStep::PickObjectB {
            [1.0, 0.8, 0.3, 1.0]
        } else {
            [0.5, 0.5, 0.5, 0.6]
        };
        self.draw_text(draw, &b_label, px + 16.0, obj_y + 28.0, 11.0, b_color);

        // Mate operation list
        if self.step == MateStep::ChooseMate || self.step == MateStep::EditParams {
            let ops_y = py + 110.0;
            self.draw_text(draw, "Mate Type:", px + 12.0, ops_y - 14.0, 10.0, [0.6, 0.6, 0.7, 1.0]);

            let item_h = 24.0;
            for (i, op) in self.operations.iter().enumerate() {
                let iy = ops_y + i as f32 * item_h;
                let is_sel = i == self.selected_op;
                let is_hover = self.hovered_op == Some(i);

                if is_sel {
                    draw.push_quad(px + 8.0, iy, panel_w - 16.0, item_h, [0.2, 0.35, 0.55, 0.8]);
                } else if is_hover {
                    draw.push_quad(px + 8.0, iy, panel_w - 16.0, item_h, [0.18, 0.18, 0.25, 0.5]);
                }

                let color = if is_sel {
                    [1.0, 1.0, 1.0, 1.0]
                } else {
                    [0.75, 0.75, 0.8, 0.9]
                };

                let label = format!("{}  {}", op.icon, op.label);
                self.draw_text(draw, &label, px + 16.0, iy + 6.0, 11.0, color);

                if op.needs_param {
                    self.draw_text(draw, "[param]", px + panel_w - 70.0, iy + 7.0, 9.0, [0.5, 0.5, 0.6, 0.7]);
                }
            }

            // AI Suggestions section
            let ai_y = ops_y + self.operations.len() as f32 * item_h + 12.0;
            draw.push_quad(px + 8.0, ai_y, panel_w - 16.0, 1.0, [0.3, 0.3, 0.4, 0.5]);
            self.draw_text(draw, "AI Suggestions", px + 12.0, ai_y + 6.0, 10.0, [0.5, 0.7, 1.0, 1.0]);

            let mut sy = ai_y + 22.0;
            for (i, sug) in self.suggestions.iter().enumerate().take(4) {
                let conf_pct = (sug.confidence * 100.0) as u32;
                let conf_color = if sug.confidence > 0.85 {
                    [0.3, 0.9, 0.4, 1.0]
                } else if sug.confidence > 0.7 {
                    [1.0, 0.8, 0.3, 1.0]
                } else {
                    [0.7, 0.5, 0.3, 0.8]
                };

                let is_sel = self.selected_suggestion == Some(i);
                if is_sel {
                    draw.push_quad(px + 8.0, sy, panel_w - 16.0, 32.0, [0.15, 0.25, 0.45, 0.6]);
                }

                let conf_label = format!("{}%", conf_pct);
                self.draw_text(draw, &conf_label, px + 16.0, sy + 4.0, 10.0, conf_color);
                self.draw_text(draw, &sug.label, px + 50.0, sy + 4.0, 10.0, [0.85, 0.85, 0.9, 1.0]);
                self.draw_text(draw, &sug.description, px + 50.0, sy + 18.0, 8.0, [0.5, 0.5, 0.55, 0.7]);

                sy += 34.0;
            }

            // Equation editor toggle
            let eq_y = sy + 8.0;
            draw.push_quad(px + 8.0, eq_y, panel_w - 16.0, 1.0, [0.3, 0.3, 0.4, 0.5]);
            self.draw_text(draw, "Equation / Parameter Expression", px + 12.0, eq_y + 6.0, 10.0, [0.6, 0.8, 0.6, 1.0]);

            if self.equation_visible {
                let eq_box_y = eq_y + 22.0;
                draw.push_quad(px + 12.0, eq_box_y, panel_w - 24.0, 24.0, [0.06, 0.06, 0.08, 0.9]);
                draw.push_quad(px + 12.0, eq_box_y, panel_w - 24.0, 1.0, [0.3, 0.5, 0.3, 0.6]);

                let eq_text = if self.equation_input.text.is_empty() {
                    "e.g., offset = partA.height + 0.5 * gap"
                } else {
                    &self.equation_input.text
                };
                let eq_color = if self.equation_input.text.is_empty() {
                    [0.4, 0.4, 0.45, 0.5]
                } else {
                    [0.8, 1.0, 0.8, 1.0]
                };
                self.draw_text(draw, eq_text, px + 18.0, eq_box_y + 6.0, 10.0, eq_color);
            }
        }

        // Footer — action buttons
        let footer_y = py + panel_h - 36.0;
        draw.push_quad(px, footer_y - 1.0, panel_w, 1.0, [0.25, 0.25, 0.3, 0.5]);

        // Apply button
        let apply_enabled = self.object_a.is_some() && self.object_b.is_some()
            && (self.step == MateStep::ChooseMate || self.step == MateStep::EditParams);
        let apply_color = if apply_enabled {
            [0.2, 0.45, 0.7, 1.0]
        } else {
            [0.15, 0.15, 0.2, 0.5]
        };
        draw.push_quad(px + panel_w - 160.0, footer_y + 4.0, 70.0, 24.0, apply_color);
        self.draw_text(draw, "Apply", px + panel_w - 145.0, footer_y + 10.0, 11.0, [1.0, 1.0, 1.0, 1.0]);

        // Cancel button
        draw.push_quad(px + panel_w - 82.0, footer_y + 4.0, 70.0, 24.0, [0.25, 0.15, 0.15, 0.8]);
        self.draw_text(draw, "Cancel", px + panel_w - 68.0, footer_y + 10.0, 11.0, [0.9, 0.7, 0.7, 1.0]);

        // Status line
        let status = match self.step {
            MateStep::PickObjectA => "Click an object to set as the moving part (Part A)",
            MateStep::PickObjectB => "Click a second object as the reference (Part B)",
            MateStep::ChooseMate => "Select a mate type, then Apply",
            MateStep::EditParams => "Enter parameter value, then Apply",
        };
        self.draw_text(draw, status, px + 12.0, footer_y - 14.0, 9.0, [0.6, 0.6, 0.65, 0.8]);
    }

    fn draw_text(&self, draw: &mut DrawList, text: &str, x: f32, y: f32, size: f32, color: [f32; 4]) {
        let mut cx = x;
        for c in text.chars() {
            let params = font::CharQuadParams {
                c, x: cx, y, size, color, atlas: None,
            };
            cx += font::emit_char_quads(&params, &mut draw.vertices, &mut draw.indices);
        }
    }
}

impl Default for MateDialog {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_sets_step() {
        let mut dialog = MateDialog::new();
        dialog.open();
        assert!(dialog.visible);
        assert_eq!(dialog.step, MateStep::PickObjectA);
    }

    #[test]
    fn set_objects_advances_step() {
        let mut dialog = MateDialog::new();
        dialog.open();
        dialog.set_object_a(0, "Cube");
        assert_eq!(dialog.step, MateStep::PickObjectB);
        dialog.set_object_b(1, "Cylinder");
        assert_eq!(dialog.step, MateStep::ChooseMate);
    }

    #[test]
    fn suggestions_generated() {
        let mut dialog = MateDialog::new();
        dialog.open();
        dialog.set_object_a(0, "Cube");
        dialog.set_object_b(1, "Cylinder");
        assert!(!dialog.suggestions.is_empty());
        // Cylinder detected — should have concentric suggestion
        assert!(dialog.suggestions.iter().any(|s| s.op_id == "concentric"));
    }

    #[test]
    fn close_resets() {
        let mut dialog = MateDialog::new();
        dialog.open();
        dialog.set_object_a(0, "Cube");
        dialog.close();
        assert!(!dialog.visible);
    }
}
