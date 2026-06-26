//! Bill of Materials (BOM) — assembly parts table with quantities, materials, mass.
//!
//! Inspired by SolidWorks Bill of Materials, Fusion 360 BOM, and
//! CATIA Assembly BOM. Displays a sortable, filterable table of parts
//! with part numbers, descriptions, quantities, materials, and mass.

use crate::draw::DrawList;
use crate::font;

/// Sort column for BOM table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BomSortColumn {
    Item,
    PartNumber,
    Description,
    Quantity,
    Material,
    Mass,
}

impl BomSortColumn {
    pub fn label(self) -> &'static str {
        match self {
            Self::Item => "#",
            Self::PartNumber => "Part Number",
            Self::Description => "Description",
            Self::Quantity => "Qty",
            Self::Material => "Material",
            Self::Mass => "Mass",
        }
    }
}

/// A single BOM row.
#[derive(Clone, Debug)]
pub struct BomRow {
    /// Item number (auto-assigned or manual).
    pub item: u32,
    /// Part number (e.g., "PN-001-A").
    pub part_number: String,
    /// Description.
    pub description: String,
    /// Quantity in the assembly.
    pub quantity: u32,
    /// Material name (e.g., "6061-T6").
    pub material: String,
    /// Mass per unit (grams).
    pub unit_mass: f64,
    /// Whether this row is selected.
    pub selected: bool,
}

impl BomRow {
    pub fn new(item: u32, part_number: &str, description: &str, qty: u32) -> Self {
        Self {
            item,
            part_number: part_number.to_string(),
            description: description.to_string(),
            quantity: qty,
            material: String::new(),
            unit_mass: 0.0,
            selected: false,
        }
    }

    pub fn with_material(mut self, material: &str, unit_mass: f64) -> Self {
        self.material = material.to_string();
        self.unit_mass = unit_mass;
        self
    }

    /// Total mass for this row (unit_mass * quantity).
    pub fn total_mass(&self) -> f64 {
        self.unit_mass * self.quantity as f64
    }
}

/// The BOM table.
pub struct BomTable {
    /// All rows.
    pub rows: Vec<BomRow>,
    /// Whether the table is visible.
    pub visible: bool,
    /// Sort column.
    pub sort_by: BomSortColumn,
    /// Sort ascending.
    pub sort_asc: bool,
    /// Search/filter query.
    pub search: String,
    /// Selected row index.
    pub selected: Option<usize>,
    /// Hovered row index.
    pub hovered: Option<usize>,
    /// Table width.
    pub width: f32,
    /// Row height.
    pub row_height: f32,
    /// Scroll offset.
    pub scroll_offset: usize,
    /// Maximum visible rows.
    pub max_visible: usize,
    /// Next auto-item number.
    next_item: u32,
}

impl BomTable {
    pub fn new() -> Self {
        Self {
            rows: Vec::new(),
            visible: false,
            sort_by: BomSortColumn::Item,
            sort_asc: true,
            search: String::new(),
            selected: None,
            hovered: None,
            width: 600.0,
            row_height: 24.0,
            scroll_offset: 0,
            max_visible: 20,
            next_item: 1,
        }
    }

    /// Add a row and auto-assign item number.
    pub fn add(&mut self, mut row: BomRow) -> u32 {
        let item = self.next_item;
        row.item = item;
        self.next_item += 1;
        self.rows.push(row);
        item
    }

    /// Remove a row by index.
    pub fn remove(&mut self, idx: usize) -> Option<BomRow> {
        if idx < self.rows.len() {
            Some(self.rows.remove(idx))
        } else {
            None
        }
    }

    /// Toggle visibility.
    pub fn toggle(&mut self) {
        self.visible = !self.visible;
    }

    /// Total unique parts.
    pub fn unique_parts(&self) -> usize {
        self.rows.len()
    }

    /// Total quantity (sum of all quantities).
    pub fn total_quantity(&self) -> u32 {
        self.rows.iter().map(|r| r.quantity).sum()
    }

    /// Total mass (sum of all row total masses).
    pub fn total_mass(&self) -> f64 {
        self.rows.iter().map(|r| r.total_mass()).sum()
    }

    /// Get filtered rows based on search query.
    pub fn filtered(&self) -> Vec<(usize, &BomRow)> {
        self.rows.iter().enumerate()
            .filter(|(_, r)| {
                if self.search.is_empty() { return true; }
                let q = self.search.to_lowercase();
                r.part_number.to_lowercase().contains(&q)
                    || r.description.to_lowercase().contains(&q)
                    || r.material.to_lowercase().contains(&q)
            })
            .collect()
    }

    /// Sort rows by current column.
    pub fn sort(&mut self) {
        let asc = self.sort_asc;
        match self.sort_by {
            BomSortColumn::Item => self.rows.sort_by(|a, b| {
                if asc { a.item.cmp(&b.item) } else { b.item.cmp(&a.item) }
            }),
            BomSortColumn::PartNumber => self.rows.sort_by(|a, b| {
                let cmp = a.part_number.cmp(&b.part_number);
                if asc { cmp } else { cmp.reverse() }
            }),
            BomSortColumn::Description => self.rows.sort_by(|a, b| {
                let cmp = a.description.cmp(&b.description);
                if asc { cmp } else { cmp.reverse() }
            }),
            BomSortColumn::Quantity => self.rows.sort_by(|a, b| {
                if asc { a.quantity.cmp(&b.quantity) } else { b.quantity.cmp(&a.quantity) }
            }),
            BomSortColumn::Material => self.rows.sort_by(|a, b| {
                let cmp = a.material.cmp(&b.material);
                if asc { cmp } else { cmp.reverse() }
            }),
            BomSortColumn::Mass => self.rows.sort_by(|a, b| {
                let cmp = a.total_mass().partial_cmp(&b.total_mass()).unwrap_or(std::cmp::Ordering::Equal);
                if asc { cmp } else { cmp.reverse() }
            }),
        }
    }

    /// Set sort column (toggles direction if same column clicked).
    pub fn set_sort(&mut self, col: BomSortColumn) {
        if self.sort_by == col {
            self.sort_asc = !self.sort_asc;
        } else {
            self.sort_by = col;
            self.sort_asc = true;
        }
        self.sort();
    }

    /// Format mass for display.
    fn format_mass(grams: f64) -> String {
        if grams >= 1000.0 {
            format!("{:.2} kg", grams / 1000.0)
        } else {
            format!("{:.1} g", grams)
        }
    }

    /// Column widths.
    fn col_widths(&self) -> [f32; 6] {
        [40.0, 100.0, 160.0, 40.0, 100.0, 80.0]
    }

    /// Hit test: which row was clicked? Returns filtered index.
    pub fn hit_test_row(
        &self, mx: f32, my: f32,
        table_x: f32, table_y: f32,
    ) -> Option<usize> {
        if !self.visible { return None; }

        let header_h = 48.0;
        let ry = my - table_y - header_h;
        if ry < 0.0 || mx < table_x || mx > table_x + self.width { return None; }

        let row = (ry / self.row_height) as usize + self.scroll_offset;
        let filtered = self.filtered();
        if row < filtered.len() {
            Some(filtered[row].0) // return real index
        } else {
            None
        }
    }

    /// Draw the BOM table.
    pub fn draw(
        &self,
        dl: &mut DrawList,
        table_x: f32,
        table_y: f32,
        bg_color: [f32; 4],
        text_color: [f32; 4],
        accent_color: [f32; 4],
    ) {
        if !self.visible { return; }

        let header_h = 48.0;
        let col_header_h = 22.0;
        let filtered = self.filtered();
        let visible_rows = filtered.len().min(self.max_visible);
        let table_h = header_h + col_header_h + visible_rows as f32 * self.row_height + 28.0;

        // Shadow + background
        dl.push_quad(table_x + 3.0, table_y + 3.0, self.width, table_h, [0.0, 0.0, 0.0, 0.2]);
        dl.push_quad(table_x, table_y, self.width, table_h, bg_color);

        // Border
        let border = [bg_color[0] + 0.1, bg_color[1] + 0.1, bg_color[2] + 0.1, 0.8];
        dl.push_quad(table_x, table_y, self.width, 1.0, border);
        dl.push_quad(table_x, table_y + table_h - 1.0, self.width, 1.0, border);
        dl.push_quad(table_x, table_y, 1.0, table_h, border);
        dl.push_quad(table_x + self.width - 1.0, table_y, 1.0, table_h, border);

        // Title bar
        let title_bg = [bg_color[0] + 0.03, bg_color[1] + 0.03, bg_color[2] + 0.03, bg_color[3]];
        dl.push_quad(table_x, table_y, self.width, 24.0, title_bg);
        emit_text(dl, "Bill of Materials", table_x + 8.0, table_y + 5.0, 11.0, text_color);

        // Summary
        let muted = [text_color[0] * 0.5, text_color[1] * 0.5, text_color[2] * 0.5, text_color[3]];
        let summary = format!("{} parts | {} pcs | {}",
            self.unique_parts(), self.total_quantity(), Self::format_mass(self.total_mass()));
        emit_text(dl, &summary, table_x + 8.0, table_y + 24.0, 8.0, muted);

        // Column headers
        let ch_y = table_y + header_h;
        dl.push_quad(table_x, ch_y, self.width, col_header_h, title_bg);

        let cols = self.col_widths();
        let col_labels = [
            BomSortColumn::Item, BomSortColumn::PartNumber, BomSortColumn::Description,
            BomSortColumn::Quantity, BomSortColumn::Material, BomSortColumn::Mass,
        ];

        let mut cx = table_x + 4.0;
        for (i, col) in col_labels.iter().enumerate() {
            let is_sort = self.sort_by == *col;
            let color = if is_sort { accent_color } else { muted };
            let label = if is_sort {
                let arrow = if self.sort_asc { " ^" } else { " v" };
                format!("{}{}", col.label(), arrow)
            } else {
                col.label().to_string()
            };
            emit_text(dl, &label, cx, ch_y + 5.0, 9.0, color);
            cx += cols[i];
        }

        // Data rows
        let data_y = ch_y + col_header_h;
        for (vis_i, (real_i, row)) in filtered.iter().skip(self.scroll_offset).take(visible_rows).enumerate() {
            let ry = data_y + vis_i as f32 * self.row_height;

            let is_hov = self.hovered == Some(*real_i);
            let is_sel = self.selected == Some(*real_i);

            // Row background
            if is_sel {
                dl.push_quad(table_x, ry, self.width, self.row_height,
                    [accent_color[0] * 0.2, accent_color[1] * 0.2, accent_color[2] * 0.2, 0.4]);
            } else if is_hov {
                dl.push_quad(table_x, ry, self.width, self.row_height, [1.0, 1.0, 1.0, 0.04]);
            } else if vis_i % 2 == 1 {
                dl.push_quad(table_x, ry, self.width, self.row_height, [1.0, 1.0, 1.0, 0.02]);
            }

            let tc = if is_hov { text_color } else { [text_color[0] * 0.85, text_color[1] * 0.85, text_color[2] * 0.85, text_color[3]] };

            let mut cx = table_x + 4.0;
            // Item
            let item_str = format!("{}", row.item);
            emit_text(dl, &item_str, cx, ry + 5.0, 9.0, tc);
            cx += cols[0];
            // Part number
            emit_text(dl, &row.part_number, cx, ry + 5.0, 9.0, tc);
            cx += cols[1];
            // Description (truncate)
            let desc = if row.description.len() > 24 { &row.description[..24] } else { &row.description };
            emit_text(dl, desc, cx, ry + 5.0, 9.0, tc);
            cx += cols[2];
            // Quantity
            let qty_str = format!("{}", row.quantity);
            emit_text(dl, &qty_str, cx, ry + 5.0, 9.0, tc);
            cx += cols[3];
            // Material
            emit_text(dl, &row.material, cx, ry + 5.0, 9.0, tc);
            cx += cols[4];
            // Mass
            let mass_str = Self::format_mass(row.total_mass());
            emit_text(dl, &mass_str, cx, ry + 5.0, 9.0, tc);
        }

        // Footer total
        let footer_y = data_y + visible_rows as f32 * self.row_height;
        dl.push_quad(table_x, footer_y, self.width, 1.0, border);
        let total_str = format!("Total: {} | {}", self.total_quantity(), Self::format_mass(self.total_mass()));
        emit_text(dl, &total_str, table_x + 8.0, footer_y + 6.0, 9.0, text_color);
    }
}

impl Default for BomTable {
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
    fn add_and_totals() {
        let mut bom = BomTable::new();
        bom.add(BomRow::new(0, "PN-001", "Bracket", 4).with_material("6061-T6", 120.0));
        bom.add(BomRow::new(0, "PN-002", "Shaft", 2).with_material("4140", 340.0));
        assert_eq!(bom.unique_parts(), 2);
        assert_eq!(bom.total_quantity(), 6);
        assert!((bom.total_mass() - (4.0 * 120.0 + 2.0 * 340.0)).abs() < 0.1);
    }

    #[test]
    fn sort_by_quantity() {
        let mut bom = BomTable::new();
        bom.add(BomRow::new(0, "A", "Part A", 5));
        bom.add(BomRow::new(0, "B", "Part B", 1));
        bom.add(BomRow::new(0, "C", "Part C", 10));
        bom.set_sort(BomSortColumn::Quantity);
        assert_eq!(bom.rows[0].quantity, 1);
        assert_eq!(bom.rows[2].quantity, 10);
    }

    #[test]
    fn search_filter() {
        let mut bom = BomTable::new();
        bom.add(BomRow::new(0, "PN-001", "Bracket", 4).with_material("6061-T6", 120.0));
        bom.add(BomRow::new(0, "PN-002", "Shaft", 2).with_material("4140", 340.0));
        bom.search = "bracket".to_string();
        let filtered = bom.filtered();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].1.description, "Bracket");
    }

    #[test]
    fn format_mass_units() {
        assert!(BomTable::format_mass(500.0).contains("g"));
        assert!(BomTable::format_mass(2500.0).contains("kg"));
    }

    #[test]
    fn toggle_sort_direction() {
        let mut bom = BomTable::new();
        // Switch to a different column first
        bom.set_sort(BomSortColumn::Quantity);
        assert!(bom.sort_asc);
        // Same column again toggles direction
        bom.set_sort(BomSortColumn::Quantity);
        assert!(!bom.sort_asc);
    }
}
