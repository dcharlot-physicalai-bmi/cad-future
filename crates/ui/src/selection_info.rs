//! Selection info panel — mass properties, bounding box, and topology counts.
//!
//! Inspired by SolidWorks Mass Properties, Fusion 360 Inspect > Measure,
//! and Ansys SpaceClaim measure tool. Shows computed properties for the
//! currently selected object(s): volume, surface area, center of mass,
//! bounding box, face/edge/vertex counts, and material-based mass estimate.

use crate::draw::DrawList;
use crate::font;

/// Computed selection properties.
#[derive(Clone, Debug, Default)]
pub struct SelectionProperties {
    /// Object name.
    pub name: String,
    /// Bounding box min corner.
    pub bbox_min: [f32; 3],
    /// Bounding box max corner.
    pub bbox_max: [f32; 3],
    /// Estimated volume (m^3).
    pub volume: f32,
    /// Estimated surface area (m^2).
    pub surface_area: f32,
    /// Center of mass (world coords).
    pub center_of_mass: [f32; 3],
    /// Estimated mass (kg) based on density.
    pub mass: Option<f32>,
    /// Material name.
    pub material: String,
    /// Density (kg/m^3).
    pub density: Option<f32>,
    /// Face count.
    pub faces: u32,
    /// Edge count.
    pub edges: u32,
    /// Vertex count.
    pub vertices: u32,
    /// Triangle count (mesh).
    pub triangles: u32,
}

impl SelectionProperties {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            ..Default::default()
        }
    }

    /// Bounding box dimensions.
    pub fn bbox_size(&self) -> [f32; 3] {
        [
            self.bbox_max[0] - self.bbox_min[0],
            self.bbox_max[1] - self.bbox_min[1],
            self.bbox_max[2] - self.bbox_min[2],
        ]
    }

    /// Format volume with appropriate unit.
    pub fn format_volume(&self) -> String {
        if self.volume < 0.001 {
            format!("{:.2} mm\u{00B3}", self.volume * 1e9)
        } else {
            format!("{:.4} m\u{00B3}", self.volume)
        }
    }

    /// Format mass with appropriate unit.
    pub fn format_mass(&self) -> String {
        match self.mass {
            Some(m) if m < 1.0 => format!("{:.1} g", m * 1000.0),
            Some(m) => format!("{:.3} kg", m),
            None => "N/A".to_string(),
        }
    }
}

/// The selection info panel.
pub struct SelectionInfo {
    /// Whether the panel is visible.
    pub visible: bool,
    /// Current selection properties (None if nothing selected).
    pub properties: Option<SelectionProperties>,
    /// Panel width.
    pub width: f32,
    /// Panel position (anchored bottom-left, above status bar).
    pub anchor_x: f32,
    pub anchor_y: f32,
    /// Collapsed sections.
    pub section_collapsed: [bool; 4], // Geometry, Mass, Topology, Bounding Box
}

impl SelectionInfo {
    pub fn new() -> Self {
        Self {
            visible: false,
            properties: None,
            width: 240.0,
            anchor_x: 44.0,
            anchor_y: 0.0,
            section_collapsed: [false; 4],
        }
    }

    /// Show the panel with properties.
    pub fn show(&mut self, props: SelectionProperties) {
        self.visible = true;
        self.properties = Some(props);
    }

    /// Hide the panel.
    pub fn hide(&mut self) {
        self.visible = false;
        self.properties = None;
    }

    /// Toggle visibility.
    pub fn toggle(&mut self) {
        self.visible = !self.visible;
    }

    /// Toggle a section.
    pub fn toggle_section(&mut self, idx: usize) {
        if idx < 4 {
            self.section_collapsed[idx] = !self.section_collapsed[idx];
        }
    }

    /// Hit test section headers. Returns section index.
    pub fn hit_test_section(
        &self, mx: f32, my: f32,
        panel_x: f32, panel_y: f32,
    ) -> Option<usize> {
        if !self.visible || self.properties.is_none() { return None; }

        let header_h = 20.0;
        let row_h = 16.0;
        let title_h = 24.0;

        let sections = ["Geometry", "Mass Properties", "Topology", "Bounding Box"];
        let row_counts = [3, 3, 4, 3]; // rows per section when expanded

        let mut cy = panel_y + title_h;
        for (i, _) in sections.iter().enumerate() {
            if mx >= panel_x && mx < panel_x + self.width
                && my >= cy && my < cy + header_h
            {
                return Some(i);
            }
            cy += header_h;
            if !self.section_collapsed[i] {
                cy += row_counts[i] as f32 * row_h;
            }
        }
        None
    }

    /// Draw the selection info panel.
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
        let Some(props) = &self.properties else { return; };

        let header_h = 20.0;
        let row_h = 16.0;
        let title_h = 24.0;
        let pad = 8.0;

        let sections = ["Geometry", "Mass Properties", "Topology", "Bounding Box"];
        let row_counts: [usize; 4] = [3, 3, 4, 3];

        // Compute total height
        let mut total_h = title_h;
        for (i, _) in sections.iter().enumerate() {
            total_h += header_h;
            if !self.section_collapsed[i] {
                total_h += row_counts[i] as f32 * row_h;
            }
        }
        total_h += 4.0; // bottom padding

        // Shadow
        dl.push_quad(panel_x + 2.0, panel_y + 2.0, self.width, total_h, [0.0, 0.0, 0.0, 0.2]);

        // Background
        dl.push_quad(panel_x, panel_y, self.width, total_h, bg_color);

        // Border
        let border = [bg_color[0] + 0.1, bg_color[1] + 0.1, bg_color[2] + 0.1, 0.8];
        dl.push_quad(panel_x, panel_y, self.width, 1.0, border);
        dl.push_quad(panel_x, panel_y + total_h - 1.0, self.width, 1.0, border);
        dl.push_quad(panel_x, panel_y, 1.0, total_h, border);
        dl.push_quad(panel_x + self.width - 1.0, panel_y, 1.0, total_h, border);

        // Title
        emit_text(dl, &props.name, panel_x + pad, panel_y + 6.0, 12.0, accent_color);

        let mut cy = panel_y + title_h;
        let muted = [text_color[0] * 0.6, text_color[1] * 0.6, text_color[2] * 0.6, text_color[3]];

        // Section 0: Geometry
        {
            let collapsed = self.section_collapsed[0];
            let arrow = if collapsed { ">" } else { "v" };
            emit_text(dl, arrow, panel_x + pad, cy + 4.0, 10.0, muted);
            emit_text(dl, sections[0], panel_x + pad + 12.0, cy + 4.0, 11.0, text_color);
            dl.push_quad(panel_x + 4.0, cy + header_h - 1.0, self.width - 8.0, 1.0,
                [border[0], border[1], border[2], 0.3]);
            cy += header_h;

            if !collapsed {
                let size = props.bbox_size();
                self.draw_row(dl, panel_x, cy, "Volume", &props.format_volume(), text_color, muted);
                cy += row_h;
                self.draw_row(dl, panel_x, cy, "Surface", &format!("{:.3} m\u{00B2}", props.surface_area), text_color, muted);
                cy += row_h;
                self.draw_row(dl, panel_x, cy, "Dims",
                    &format!("{:.2} x {:.2} x {:.2}", size[0], size[1], size[2]),
                    text_color, muted);
                cy += row_h;
            }
        }

        // Section 1: Mass Properties
        {
            let collapsed = self.section_collapsed[1];
            let arrow = if collapsed { ">" } else { "v" };
            emit_text(dl, arrow, panel_x + pad, cy + 4.0, 10.0, muted);
            emit_text(dl, sections[1], panel_x + pad + 12.0, cy + 4.0, 11.0, text_color);
            dl.push_quad(panel_x + 4.0, cy + header_h - 1.0, self.width - 8.0, 1.0,
                [border[0], border[1], border[2], 0.3]);
            cy += header_h;

            if !collapsed {
                self.draw_row(dl, panel_x, cy, "Material", &props.material, text_color, muted);
                cy += row_h;
                self.draw_row(dl, panel_x, cy, "Density",
                    &match props.density { Some(d) => format!("{:.0} kg/m\u{00B3}", d), None => "N/A".into() },
                    text_color, muted);
                cy += row_h;
                self.draw_row(dl, panel_x, cy, "Mass", &props.format_mass(), text_color, muted);
                cy += row_h;
            }
        }

        // Section 2: Topology
        {
            let collapsed = self.section_collapsed[2];
            let arrow = if collapsed { ">" } else { "v" };
            emit_text(dl, arrow, panel_x + pad, cy + 4.0, 10.0, muted);
            emit_text(dl, sections[2], panel_x + pad + 12.0, cy + 4.0, 11.0, text_color);
            dl.push_quad(panel_x + 4.0, cy + header_h - 1.0, self.width - 8.0, 1.0,
                [border[0], border[1], border[2], 0.3]);
            cy += header_h;

            if !collapsed {
                self.draw_row(dl, panel_x, cy, "Faces", &format!("{}", props.faces), text_color, muted);
                cy += row_h;
                self.draw_row(dl, panel_x, cy, "Edges", &format!("{}", props.edges), text_color, muted);
                cy += row_h;
                self.draw_row(dl, panel_x, cy, "Vertices", &format!("{}", props.vertices), text_color, muted);
                cy += row_h;
                self.draw_row(dl, panel_x, cy, "Triangles", &format!("{}", props.triangles), text_color, muted);
                cy += row_h;
            }
        }

        // Section 3: Bounding Box
        {
            let collapsed = self.section_collapsed[3];
            let arrow = if collapsed { ">" } else { "v" };
            emit_text(dl, arrow, panel_x + pad, cy + 4.0, 10.0, muted);
            emit_text(dl, sections[3], panel_x + pad + 12.0, cy + 4.0, 11.0, text_color);
            dl.push_quad(panel_x + 4.0, cy + header_h - 1.0, self.width - 8.0, 1.0,
                [border[0], border[1], border[2], 0.3]);
            cy += header_h;

            if !collapsed {
                self.draw_row(dl, panel_x, cy, "Min",
                    &format!("({:.2}, {:.2}, {:.2})", props.bbox_min[0], props.bbox_min[1], props.bbox_min[2]),
                    text_color, muted);
                cy += row_h;
                self.draw_row(dl, panel_x, cy, "Max",
                    &format!("({:.2}, {:.2}, {:.2})", props.bbox_max[0], props.bbox_max[1], props.bbox_max[2]),
                    text_color, muted);
                cy += row_h;
                let com = props.center_of_mass;
                self.draw_row(dl, panel_x, cy, "CoM",
                    &format!("({:.2}, {:.2}, {:.2})", com[0], com[1], com[2]),
                    text_color, muted);
                // cy += row_h; // last row
            }
        }
    }

    fn draw_row(
        &self,
        dl: &mut DrawList,
        panel_x: f32, y: f32,
        label: &str, value: &str,
        text_color: [f32; 4], muted: [f32; 4],
    ) {
        let pad = 8.0;
        emit_text(dl, label, panel_x + pad + 4.0, y + 2.0, 10.0, muted);
        let value_x = panel_x + 90.0;
        emit_text(dl, value, value_x, y + 2.0, 10.0, text_color);
    }
}

impl Default for SelectionInfo {
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
    fn show_and_hide() {
        let mut si = SelectionInfo::new();
        assert!(!si.visible);
        si.show(SelectionProperties::new("Cube"));
        assert!(si.visible);
        assert_eq!(si.properties.as_ref().unwrap().name, "Cube");
        si.hide();
        assert!(!si.visible);
    }

    #[test]
    fn bbox_size() {
        let mut props = SelectionProperties::new("Box");
        props.bbox_min = [-1.0, 0.0, -1.0];
        props.bbox_max = [1.0, 2.0, 1.0];
        let size = props.bbox_size();
        assert!((size[0] - 2.0).abs() < 0.01);
        assert!((size[1] - 2.0).abs() < 0.01);
        assert!((size[2] - 2.0).abs() < 0.01);
    }

    #[test]
    fn format_volume_small() {
        let mut props = SelectionProperties::new("Tiny");
        props.volume = 0.000001; // 1 mm^3
        let fmt = props.format_volume();
        assert!(fmt.contains("mm"));
    }

    #[test]
    fn format_mass_grams() {
        let mut props = SelectionProperties::new("Light");
        props.mass = Some(0.5);
        let fmt = props.format_mass();
        assert!(fmt.contains("g"));
    }

    #[test]
    fn section_toggle() {
        let mut si = SelectionInfo::new();
        assert!(!si.section_collapsed[0]);
        si.toggle_section(0);
        assert!(si.section_collapsed[0]);
    }
}
