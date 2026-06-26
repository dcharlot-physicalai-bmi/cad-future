//! GD&T panel — geometric dimensioning and tolerancing feature control frames.
//!
//! Inspired by SolidWorks DimXpert, CATIA FTA, and ASME Y14.5.
//! Provides UI for creating and editing feature control frames with
//! geometric tolerance symbols, datum references, and material conditions.

use crate::draw::DrawList;
use crate::font;

/// Geometric characteristic symbol.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GdtCharacteristic {
    Straightness,
    Flatness,
    Circularity,
    Cylindricity,
    LineProfile,
    SurfaceProfile,
    Parallelism,
    Perpendicularity,
    Angularity,
    Position,
    Concentricity,
    Symmetry,
    CircularRunout,
    TotalRunout,
}

impl GdtCharacteristic {
    pub fn symbol(self) -> &'static str {
        match self {
            Self::Straightness => "-",
            Self::Flatness => "//",
            Self::Circularity => "O",
            Self::Cylindricity => "())",
            Self::LineProfile => "^",
            Self::SurfaceProfile => "^^",
            Self::Parallelism => "||",
            Self::Perpendicularity => "_|_",
            Self::Angularity => "<",
            Self::Position => "(+)",
            Self::Concentricity => "@@",
            Self::Symmetry => "=|=",
            Self::CircularRunout => "/",
            Self::TotalRunout => "//",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Straightness => "Straightness",
            Self::Flatness => "Flatness",
            Self::Circularity => "Circularity",
            Self::Cylindricity => "Cylindricity",
            Self::LineProfile => "Profile of a Line",
            Self::SurfaceProfile => "Profile of a Surface",
            Self::Parallelism => "Parallelism",
            Self::Perpendicularity => "Perpendicularity",
            Self::Angularity => "Angularity",
            Self::Position => "Position",
            Self::Concentricity => "Concentricity",
            Self::Symmetry => "Symmetry",
            Self::CircularRunout => "Circular Runout",
            Self::TotalRunout => "Total Runout",
        }
    }

    /// Whether this characteristic requires datum references.
    pub fn needs_datum(self) -> bool {
        match self {
            Self::Straightness | Self::Flatness | Self::Circularity | Self::Cylindricity => false,
            _ => true,
        }
    }

    pub fn all() -> &'static [Self] {
        &[
            Self::Straightness, Self::Flatness, Self::Circularity, Self::Cylindricity,
            Self::LineProfile, Self::SurfaceProfile, Self::Parallelism, Self::Perpendicularity,
            Self::Angularity, Self::Position, Self::Concentricity, Self::Symmetry,
            Self::CircularRunout, Self::TotalRunout,
        ]
    }
}

/// Material condition modifier.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MaterialCondition {
    None,
    /// Maximum Material Condition (M).
    MMC,
    /// Least Material Condition (L).
    LMC,
    /// Regardless of Feature Size.
    RFS,
}

impl MaterialCondition {
    pub fn symbol(self) -> &'static str {
        match self {
            Self::None => "",
            Self::MMC => "(M)",
            Self::LMC => "(L)",
            Self::RFS => "(S)",
        }
    }
}

/// A feature control frame.
#[derive(Clone, Debug)]
pub struct FeatureControlFrame {
    /// Geometric characteristic.
    pub characteristic: GdtCharacteristic,
    /// Tolerance value (mm).
    pub tolerance: f64,
    /// Material condition modifier on tolerance.
    pub modifier: MaterialCondition,
    /// Primary datum reference (e.g., "A").
    pub datum_a: String,
    /// Secondary datum reference.
    pub datum_b: String,
    /// Tertiary datum reference.
    pub datum_c: String,
    /// Screen position [x, y] for display.
    pub screen_pos: [f32; 2],
    /// Whether this FCF is selected.
    pub selected: bool,
}

impl FeatureControlFrame {
    pub fn new(characteristic: GdtCharacteristic, tolerance: f64) -> Self {
        Self {
            characteristic,
            tolerance,
            modifier: MaterialCondition::None,
            datum_a: String::new(),
            datum_b: String::new(),
            datum_c: String::new(),
            screen_pos: [0.0, 0.0],
            selected: false,
        }
    }

    pub fn with_datums(mut self, a: &str, b: &str, c: &str) -> Self {
        self.datum_a = a.to_string();
        self.datum_b = b.to_string();
        self.datum_c = c.to_string();
        self
    }

    pub fn with_modifier(mut self, m: MaterialCondition) -> Self {
        self.modifier = m;
        self
    }

    /// Format the tolerance value.
    pub fn format_tolerance(&self) -> String {
        if self.tolerance < 0.01 {
            format!("{:.4}", self.tolerance)
        } else if self.tolerance < 1.0 {
            format!("{:.3}", self.tolerance)
        } else {
            format!("{:.2}", self.tolerance)
        }
    }

    /// Full text representation of the FCF.
    pub fn to_string_repr(&self) -> String {
        let mut s = format!("{} {} {}", self.characteristic.symbol(),
            self.format_tolerance(), self.modifier.symbol());
        if !self.datum_a.is_empty() {
            s += &format!(" | {}", self.datum_a);
        }
        if !self.datum_b.is_empty() {
            s += &format!(" | {}", self.datum_b);
        }
        if !self.datum_c.is_empty() {
            s += &format!(" | {}", self.datum_c);
        }
        s
    }
}

/// The GD&T panel.
pub struct GdtPanel {
    /// Whether the panel is visible.
    pub visible: bool,
    /// All feature control frames.
    pub frames: Vec<FeatureControlFrame>,
    /// Selected FCF index.
    pub selected: Option<usize>,
    /// Hovered FCF index.
    pub hovered: Option<usize>,
    /// Whether we're in creation mode.
    pub creating: bool,
    /// Current characteristic for creation.
    pub create_char: GdtCharacteristic,
    /// Panel width.
    pub width: f32,
}

impl GdtPanel {
    pub fn new() -> Self {
        Self {
            visible: false,
            frames: Vec::new(),
            selected: None,
            hovered: None,
            creating: false,
            create_char: GdtCharacteristic::Position,
            width: 280.0,
        }
    }

    pub fn toggle(&mut self) {
        self.visible = !self.visible;
    }

    pub fn add(&mut self, fcf: FeatureControlFrame) {
        self.frames.push(fcf);
    }

    pub fn remove(&mut self, idx: usize) -> Option<FeatureControlFrame> {
        if idx < self.frames.len() { Some(self.frames.remove(idx)) } else { None }
    }

    pub fn begin_create(&mut self, char: GdtCharacteristic) {
        self.creating = true;
        self.create_char = char;
    }

    pub fn cancel_create(&mut self) {
        self.creating = false;
    }

    /// Draw the GD&T panel.
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

        let row_h = 32.0;
        let header_h = 28.0;
        let rows = self.frames.len().min(8);
        let panel_h = header_h + rows as f32 * row_h + 8.0;

        // Background
        dl.push_quad(panel_x, panel_y, self.width, panel_h, bg_color);
        let border = [bg_color[0] + 0.1, bg_color[1] + 0.1, bg_color[2] + 0.1, 0.8];
        dl.push_quad(panel_x, panel_y, self.width, 1.0, border);

        let muted = [text_color[0] * 0.5, text_color[1] * 0.5, text_color[2] * 0.5, text_color[3]];

        // Title
        emit_text(dl, "GD&T", panel_x + 8.0, panel_y + 7.0, 11.0, text_color);
        let count = format!("{}", self.frames.len());
        let cw = font::measure_text(&count, 9.0, None);
        emit_text(dl, &count, panel_x + self.width - cw - 8.0, panel_y + 9.0, 9.0, muted);

        // FCF rows
        for (i, fcf) in self.frames.iter().enumerate().take(rows) {
            let ry = panel_y + header_h + i as f32 * row_h;

            let is_sel = self.selected == Some(i);
            let is_hov = self.hovered == Some(i);

            if is_sel {
                dl.push_quad(panel_x, ry, self.width, row_h,
                    [accent_color[0] * 0.2, accent_color[1] * 0.2, accent_color[2] * 0.2, 0.4]);
            } else if is_hov {
                dl.push_quad(panel_x, ry, self.width, row_h, [1.0, 1.0, 1.0, 0.05]);
            }

            // Feature control frame box
            let frame_x = panel_x + 8.0;
            let frame_w = self.width - 16.0;
            dl.push_quad(frame_x, ry + 2.0, frame_w, row_h - 4.0, [0.0, 0.0, 0.0, 0.15]);
            dl.push_quad(frame_x, ry + 2.0, frame_w, 1.0, border);
            dl.push_quad(frame_x, ry + row_h - 2.0, frame_w, 1.0, border);
            dl.push_quad(frame_x, ry + 2.0, 1.0, row_h - 4.0, border);
            dl.push_quad(frame_x + frame_w, ry + 2.0, 1.0, row_h - 4.0, border);

            // Symbol cell
            let sym = fcf.characteristic.symbol();
            emit_text(dl, sym, frame_x + 4.0, ry + 8.0, 10.0, accent_color);

            // Tolerance cell
            let cell_x = frame_x + 40.0;
            dl.push_quad(cell_x, ry + 2.0, 1.0, row_h - 4.0, border);
            let tol = fcf.format_tolerance();
            emit_text(dl, &tol, cell_x + 4.0, ry + 8.0, 9.0, text_color);

            // Modifier
            if fcf.modifier != MaterialCondition::None {
                let mod_str = fcf.modifier.symbol();
                emit_text(dl, mod_str, cell_x + 50.0, ry + 8.0, 8.0, muted);
            }

            // Datum cells
            let datum_x = cell_x + 70.0;
            dl.push_quad(datum_x, ry + 2.0, 1.0, row_h - 4.0, border);
            if !fcf.datum_a.is_empty() {
                emit_text(dl, &fcf.datum_a, datum_x + 4.0, ry + 8.0, 10.0, text_color);
            }
            if !fcf.datum_b.is_empty() {
                let dx2 = datum_x + 24.0;
                dl.push_quad(dx2, ry + 2.0, 1.0, row_h - 4.0, border);
                emit_text(dl, &fcf.datum_b, dx2 + 4.0, ry + 8.0, 10.0, text_color);
            }
            if !fcf.datum_c.is_empty() {
                let dx3 = datum_x + 48.0;
                dl.push_quad(dx3, ry + 2.0, 1.0, row_h - 4.0, border);
                emit_text(dl, &fcf.datum_c, dx3 + 4.0, ry + 8.0, 10.0, text_color);
            }
        }
    }

    /// Draw on-canvas FCF annotations.
    pub fn draw_annotations(
        &self,
        dl: &mut DrawList,
        text_color: [f32; 4],
        accent_color: [f32; 4],
    ) {
        for fcf in &self.frames {
            let [sx, sy] = fcf.screen_pos;
            if sx == 0.0 && sy == 0.0 { continue; }

            let text = fcf.to_string_repr();
            let tw = font::measure_text(&text, 9.0, None);

            // Background
            dl.push_quad(sx - 2.0, sy - 1.0, tw + 4.0, 14.0, [0.0, 0.0, 0.0, 0.7]);
            // Border
            let bc = if fcf.selected { accent_color } else { [0.5, 0.5, 0.5, 0.5] };
            dl.push_quad(sx - 2.0, sy - 1.0, tw + 4.0, 1.0, bc);
            dl.push_quad(sx - 2.0, sy + 13.0, tw + 4.0, 1.0, bc);

            emit_text(dl, &text, sx, sy + 1.0, 9.0, text_color);
        }
    }
}

impl Default for GdtPanel {
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
    fn create_fcf() {
        let fcf = FeatureControlFrame::new(GdtCharacteristic::Position, 0.05)
            .with_datums("A", "B", "")
            .with_modifier(MaterialCondition::MMC);
        assert_eq!(fcf.characteristic, GdtCharacteristic::Position);
        assert!(!fcf.datum_a.is_empty());
        assert!(fcf.datum_c.is_empty());
    }

    #[test]
    fn format_tolerance() {
        let fcf = FeatureControlFrame::new(GdtCharacteristic::Flatness, 0.005);
        assert!(fcf.format_tolerance().contains("0.0050"));
    }

    #[test]
    fn datum_required() {
        assert!(!GdtCharacteristic::Flatness.needs_datum());
        assert!(GdtCharacteristic::Position.needs_datum());
        assert!(GdtCharacteristic::Parallelism.needs_datum());
    }

    #[test]
    fn all_characteristics() {
        let all = GdtCharacteristic::all();
        assert_eq!(all.len(), 14);
        for c in all {
            assert!(!c.symbol().is_empty());
            assert!(!c.label().is_empty());
        }
    }

    #[test]
    fn panel_add_and_remove() {
        let mut panel = GdtPanel::new();
        panel.add(FeatureControlFrame::new(GdtCharacteristic::Flatness, 0.01));
        panel.add(FeatureControlFrame::new(GdtCharacteristic::Position, 0.1));
        assert_eq!(panel.frames.len(), 2);
        panel.remove(0);
        assert_eq!(panel.frames.len(), 1);
    }
}
