//! Reference geometry — planes, axes, and coordinate systems creation dialog.
//!
//! Inspired by SolidWorks Reference Geometry, Fusion 360 Construction,
//! and CATIA Wireframe. Provides UI for creating datum planes, axes,
//! and points with various definition methods.

use crate::draw::DrawList;
use crate::font;

/// Type of reference geometry to create.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RefGeomType {
    Plane,
    Axis,
    Point,
    CoordinateSystem,
}

impl RefGeomType {
    pub fn label(self) -> &'static str {
        match self {
            Self::Plane => "Plane",
            Self::Axis => "Axis",
            Self::Point => "Point",
            Self::CoordinateSystem => "CSYS",
        }
    }

    pub fn icon(self) -> &'static str {
        match self {
            Self::Plane => "[]",
            Self::Axis => "|",
            Self::Point => ".",
            Self::CoordinateSystem => "+",
        }
    }
}

/// How the reference geometry is defined.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlaneDefinition {
    Offset,
    ThreePoints,
    ThroughEdge,
    Parallel,
    Perpendicular,
    Tangent,
    MidPlane,
}

impl PlaneDefinition {
    pub fn label(self) -> &'static str {
        match self {
            Self::Offset => "Offset from Plane",
            Self::ThreePoints => "Three Points",
            Self::ThroughEdge => "Through Edge",
            Self::Parallel => "Parallel to Plane",
            Self::Perpendicular => "Perpendicular",
            Self::Tangent => "Tangent to Face",
            Self::MidPlane => "Mid Plane",
        }
    }
}

/// An existing reference geometry item.
#[derive(Clone, Debug)]
pub struct RefGeomItem {
    /// Unique ID.
    pub id: u64,
    /// Display name.
    pub name: String,
    /// Type.
    pub kind: RefGeomType,
    /// Whether it's visible in the viewport.
    pub visible: bool,
    /// Color for display.
    pub color: [f32; 4],
}

impl RefGeomItem {
    pub fn new(id: u64, name: &str, kind: RefGeomType) -> Self {
        let color = match kind {
            RefGeomType::Plane => [0.2, 0.6, 0.9, 0.3],
            RefGeomType::Axis => [0.9, 0.3, 0.3, 0.6],
            RefGeomType::Point => [0.9, 0.9, 0.2, 0.8],
            RefGeomType::CoordinateSystem => [0.3, 0.9, 0.3, 0.6],
        };
        Self {
            id,
            name: name.to_string(),
            kind,
            visible: true,
            color,
        }
    }
}

/// The reference geometry dialog/panel.
pub struct ReferenceGeometry {
    /// Whether the dialog is open.
    pub visible: bool,
    /// Current creation type.
    pub create_type: RefGeomType,
    /// Plane definition method.
    pub plane_def: PlaneDefinition,
    /// Offset distance (for offset plane).
    pub offset_distance: f32,
    /// Whether to flip direction.
    pub flip: bool,
    /// Existing reference geometry items.
    pub items: Vec<RefGeomItem>,
    /// Selected item index.
    pub selected: Option<usize>,
    /// Hovered item index.
    pub hovered: Option<usize>,
    /// Panel width.
    pub width: f32,
    /// Next auto ID.
    next_id: u64,
    /// Creation step (0 = choose type, 1 = define, 2 = confirm).
    pub step: u8,
}

impl ReferenceGeometry {
    pub fn new() -> Self {
        Self {
            visible: false,
            create_type: RefGeomType::Plane,
            plane_def: PlaneDefinition::Offset,
            offset_distance: 10.0,
            flip: false,
            items: Vec::new(),
            selected: None,
            hovered: None,
            width: 240.0,
            next_id: 1,
            step: 0,
        }
    }

    /// Open the dialog for creating a specific type.
    pub fn open(&mut self, kind: RefGeomType) {
        self.visible = true;
        self.create_type = kind;
        self.step = 1;
    }

    /// Close the dialog.
    pub fn close(&mut self) {
        self.visible = false;
        self.step = 0;
    }

    /// Confirm creation and add to items list.
    pub fn confirm(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        let name = format!("{}{}", self.create_type.label(), id);
        self.items.push(RefGeomItem::new(id, &name, self.create_type));
        self.close();
        id
    }

    /// Toggle visibility of an item.
    pub fn toggle_item_visibility(&mut self, idx: usize) {
        if let Some(item) = self.items.get_mut(idx) {
            item.visible = !item.visible;
        }
    }

    /// Remove an item.
    pub fn remove_item(&mut self, idx: usize) -> Option<RefGeomItem> {
        if idx < self.items.len() { Some(self.items.remove(idx)) } else { None }
    }

    /// Draw the reference geometry dialog.
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

        let panel_h = 200.0;

        // Background
        dl.push_quad(panel_x, panel_y, self.width, panel_h, bg_color);
        let border = [bg_color[0] + 0.1, bg_color[1] + 0.1, bg_color[2] + 0.1, 0.8];
        dl.push_quad(panel_x, panel_y, self.width, 1.0, border);
        dl.push_quad(panel_x, panel_y + panel_h - 1.0, self.width, 1.0, border);
        dl.push_quad(panel_x, panel_y, 1.0, panel_h, border);
        dl.push_quad(panel_x + self.width - 1.0, panel_y, 1.0, panel_h, border);

        let muted = [text_color[0] * 0.5, text_color[1] * 0.5, text_color[2] * 0.5, text_color[3]];

        // Title
        let title = format!("Create {}", self.create_type.label());
        emit_text(dl, &title, panel_x + 8.0, panel_y + 6.0, 11.0, text_color);

        // Type buttons
        let btn_y = panel_y + 28.0;
        let types = [RefGeomType::Plane, RefGeomType::Axis, RefGeomType::Point, RefGeomType::CoordinateSystem];
        let mut bx = panel_x + 8.0;
        for t in types {
            let is_active = self.create_type == t;
            let btn_bg = if is_active { accent_color } else { [0.3, 0.3, 0.3, 0.4] };
            let lbl = t.label();
            let lw = font::measure_text(lbl, 9.0, None);
            dl.push_quad(bx, btn_y, lw + 8.0, 18.0, btn_bg);
            let tc = if is_active { [1.0, 1.0, 1.0, 1.0] } else { text_color };
            emit_text(dl, lbl, bx + 4.0, btn_y + 3.0, 9.0, tc);
            bx += lw + 12.0;
        }

        // Definition method (for planes)
        if self.create_type == RefGeomType::Plane {
            emit_text(dl, "Method:", panel_x + 8.0, panel_y + 56.0, 8.0, muted);
            emit_text(dl, self.plane_def.label(), panel_x + 52.0, panel_y + 56.0, 9.0, text_color);

            // Offset distance
            if self.plane_def == PlaneDefinition::Offset {
                emit_text(dl, "Distance:", panel_x + 8.0, panel_y + 76.0, 8.0, muted);
                let dist = format!("{:.1} mm", self.offset_distance);
                emit_text(dl, &dist, panel_x + 60.0, panel_y + 76.0, 9.0, text_color);

                // Flip
                let flip_bg = if self.flip { accent_color } else { [0.3, 0.3, 0.3, 0.5] };
                dl.push_quad(panel_x + 8.0, panel_y + 94.0, 12.0, 12.0, flip_bg);
                emit_text(dl, "Flip direction", panel_x + 26.0, panel_y + 95.0, 8.0, text_color);
            }
        }

        // Selection prompt
        let prompt = match self.step {
            1 => "Select reference face or plane...",
            2 => "Adjust parameters, then confirm",
            _ => "",
        };
        if !prompt.is_empty() {
            emit_text(dl, prompt, panel_x + 8.0, panel_y + 120.0, 8.0, [accent_color[0], accent_color[1], accent_color[2], 0.8]);
        }

        // OK / Cancel buttons
        let btn_ok_y = panel_y + panel_h - 30.0;
        dl.push_quad(panel_x + 8.0, btn_ok_y, 60.0, 22.0, accent_color);
        emit_text(dl, "OK", panel_x + 28.0, btn_ok_y + 5.0, 10.0, [1.0, 1.0, 1.0, 1.0]);

        dl.push_quad(panel_x + 76.0, btn_ok_y, 60.0, 22.0, [0.4, 0.4, 0.4, 0.6]);
        emit_text(dl, "Cancel", panel_x + 86.0, btn_ok_y + 5.0, 10.0, text_color);

        // Existing items count
        if !self.items.is_empty() {
            let count = format!("{} items", self.items.len());
            let cw = font::measure_text(&count, 8.0, None);
            emit_text(dl, &count, panel_x + self.width - cw - 8.0, panel_y + panel_h - 24.0, 8.0, muted);
        }
    }
}

impl Default for ReferenceGeometry {
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
    fn open_and_close() {
        let mut rg = ReferenceGeometry::new();
        rg.open(RefGeomType::Plane);
        assert!(rg.visible);
        assert_eq!(rg.create_type, RefGeomType::Plane);
        rg.close();
        assert!(!rg.visible);
    }

    #[test]
    fn confirm_creates_item() {
        let mut rg = ReferenceGeometry::new();
        rg.open(RefGeomType::Axis);
        let id = rg.confirm();
        assert!(id > 0);
        assert_eq!(rg.items.len(), 1);
        assert_eq!(rg.items[0].kind, RefGeomType::Axis);
    }

    #[test]
    fn toggle_item_visibility() {
        let mut rg = ReferenceGeometry::new();
        rg.items.push(RefGeomItem::new(1, "Plane1", RefGeomType::Plane));
        assert!(rg.items[0].visible);
        rg.toggle_item_visibility(0);
        assert!(!rg.items[0].visible);
    }

    #[test]
    fn plane_definitions() {
        let defs = [
            PlaneDefinition::Offset, PlaneDefinition::ThreePoints,
            PlaneDefinition::ThroughEdge, PlaneDefinition::Parallel,
            PlaneDefinition::Perpendicular, PlaneDefinition::Tangent,
            PlaneDefinition::MidPlane,
        ];
        for d in defs {
            assert!(!d.label().is_empty());
        }
    }

    #[test]
    fn ref_geom_types() {
        let types = [RefGeomType::Plane, RefGeomType::Axis, RefGeomType::Point, RefGeomType::CoordinateSystem];
        for t in types {
            assert!(!t.label().is_empty());
            assert!(!t.icon().is_empty());
        }
    }
}
