//! `physical-bom` — Bill of Materials engine.
//!
//! Part numbering, hierarchical BOM structure, where-used analysis,
//! impact analysis (what breaks if I change this part), and BOM comparison.

use serde::{Serialize, Deserialize};
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Part Number System
// ---------------------------------------------------------------------------

/// A configurable part numbering scheme.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartNumberConfig {
    /// Prefix by category: "MECH-", "ELEC-", "SW-"
    pub category_prefixes: HashMap<String, String>,
    /// Next sequential number per category.
    pub next_numbers: HashMap<String, u32>,
    /// Number of digits (zero-padded).
    pub digits: usize,
    /// Revision scheme: "A-Z" or "1,2,3"
    pub revision_scheme: RevisionScheme,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RevisionScheme { Alpha, Numeric }

impl PartNumberConfig {
    pub fn default_mechanical() -> Self {
        let mut prefixes = HashMap::new();
        prefixes.insert("mechanical".into(), "MECH-".into());
        prefixes.insert("electrical".into(), "ELEC-".into());
        prefixes.insert("software".into(), "SW-".into());
        prefixes.insert("fastener".into(), "STD-".into());
        prefixes.insert("assembly".into(), "ASSY-".into());
        Self {
            category_prefixes: prefixes,
            next_numbers: HashMap::new(),
            digits: 5,
            revision_scheme: RevisionScheme::Alpha,
        }
    }

    /// Generate the next part number for a category.
    pub fn next_part_number(&mut self, category: &str) -> String {
        let prefix = self.category_prefixes.get(category)
            .cloned().unwrap_or_else(|| format!("{}-", category.to_uppercase()));
        let num = self.next_numbers.entry(category.into()).or_insert(1);
        let pn = format!("{}{:0>width$}", prefix, num, width = self.digits);
        *num += 1;
        pn
    }

    /// Generate a revision string.
    pub fn revision_string(&self, rev: u32) -> String {
        match self.revision_scheme {
            RevisionScheme::Alpha => {
                if rev == 0 { "-".into() }
                else {
                    let c = (b'A' + ((rev - 1) % 26) as u8) as char;
                    format!("{c}")
                }
            }
            RevisionScheme::Numeric => format!("{rev}"),
        }
    }
}

// ---------------------------------------------------------------------------
// BOM Types
// ---------------------------------------------------------------------------

/// A single item in the BOM.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BomItem {
    pub part_number: String,
    pub name: String,
    pub description: String,
    pub revision: String,
    pub quantity: f64,
    pub unit: String,    // "each", "kg", "m"
    pub material: Option<String>,
    pub category: String,
    pub children: Vec<BomItem>, // sub-components (for assemblies)
    pub properties: HashMap<String, String>,
}

impl BomItem {
    pub fn part(pn: &str, name: &str, qty: f64) -> Self {
        Self {
            part_number: pn.into(), name: name.into(), description: String::new(),
            revision: "A".into(), quantity: qty, unit: "each".into(),
            material: None, category: "mechanical".into(),
            children: Vec::new(), properties: HashMap::new(),
        }
    }

    pub fn assembly(pn: &str, name: &str, children: Vec<BomItem>) -> Self {
        Self {
            part_number: pn.into(), name: name.into(), description: String::new(),
            revision: "A".into(), quantity: 1.0, unit: "each".into(),
            material: None, category: "assembly".into(),
            children, properties: HashMap::new(),
        }
    }

    /// Count total unique parts in this item and its children.
    pub fn unique_part_count(&self) -> usize {
        let mut parts = std::collections::HashSet::new();
        self.collect_parts(&mut parts);
        parts.len()
    }

    fn collect_parts(&self, parts: &mut std::collections::HashSet<String>) {
        parts.insert(self.part_number.clone());
        for child in &self.children {
            child.collect_parts(parts);
        }
    }

    /// Total quantity of all items (flattened).
    pub fn total_quantity(&self) -> f64 {
        let mut total = self.quantity;
        for child in &self.children {
            total += child.total_quantity() * self.quantity;
        }
        total
    }

    /// Find all items matching a part number (where-used).
    pub fn where_used(&self, target_pn: &str) -> Vec<String> {
        let mut parents = Vec::new();
        for child in &self.children {
            if child.part_number == target_pn {
                parents.push(self.part_number.clone());
            }
            parents.extend(child.where_used(target_pn));
        }
        parents
    }

    /// Impact analysis: what assemblies are affected if this part changes?
    pub fn impact_analysis(&self, changed_pn: &str) -> Vec<String> {
        let mut affected = Vec::new();
        if self.children.iter().any(|c| c.part_number == changed_pn) {
            affected.push(self.part_number.clone());
        }
        for child in &self.children {
            affected.extend(child.impact_analysis(changed_pn));
        }
        affected
    }

    /// Flatten the BOM into a list of (part_number, name, total_qty) tuples.
    pub fn flatten(&self) -> Vec<(String, String, f64)> {
        let mut flat: HashMap<String, (String, f64)> = HashMap::new();
        self.flatten_into(&mut flat, 1.0);
        flat.into_iter().map(|(pn, (name, qty))| (pn, name, qty)).collect()
    }

    fn flatten_into(&self, flat: &mut HashMap<String, (String, f64)>, parent_qty: f64) {
        let entry = flat.entry(self.part_number.clone())
            .or_insert((self.name.clone(), 0.0));
        entry.1 += self.quantity * parent_qty;
        for child in &self.children {
            child.flatten_into(flat, self.quantity * parent_qty);
        }
    }
}

/// Compare two BOMs and find differences.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BomDiff {
    pub added: Vec<String>,    // part numbers added
    pub removed: Vec<String>,  // part numbers removed
    pub qty_changed: Vec<(String, f64, f64)>, // (pn, old_qty, new_qty)
}

pub fn diff_boms(old: &BomItem, new: &BomItem) -> BomDiff {
    let old_flat = old.flatten();
    let new_flat = new.flatten();
    let old_map: HashMap<String, f64> = old_flat.iter().map(|(pn, _, q)| (pn.clone(), *q)).collect();
    let new_map: HashMap<String, f64> = new_flat.iter().map(|(pn, _, q)| (pn.clone(), *q)).collect();

    let mut added = Vec::new();
    let mut removed = Vec::new();
    let mut qty_changed = Vec::new();

    for (pn, new_qty) in &new_map {
        match old_map.get(pn) {
            None => added.push(pn.clone()),
            Some(old_qty) if (old_qty - new_qty).abs() > 0.001 => {
                qty_changed.push((pn.clone(), *old_qty, *new_qty));
            }
            _ => {}
        }
    }
    for pn in old_map.keys() {
        if !new_map.contains_key(pn) { removed.push(pn.clone()); }
    }

    BomDiff { added, removed, qty_changed }
}

/// Export BOM to CSV string.
pub fn export_csv(bom: &BomItem) -> String {
    let mut csv = String::from("Part Number,Name,Quantity,Unit,Material,Revision\n");
    let flat = bom.flatten();
    for (pn, name, qty) in &flat {
        csv.push_str(&format!("{pn},{name},{qty},each,,A\n"));
    }
    csv
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_bom() -> BomItem {
        BomItem::assembly("ASSY-00001", "Widget Assembly", vec![
            BomItem::part("MECH-00001", "Base Plate", 1.0),
            BomItem::part("MECH-00002", "Bracket", 2.0),
            BomItem::part("STD-00001", "M8 Bolt", 4.0),
            BomItem::part("STD-00002", "M8 Nut", 4.0),
            BomItem::part("STD-00003", "M8 Washer", 8.0),
            BomItem::assembly("ASSY-00002", "Sub-Assembly", vec![
                BomItem::part("MECH-00003", "Shaft", 1.0),
                BomItem::part("STD-00004", "Bearing 6001", 2.0),
            ]),
        ])
    }

    #[test]
    fn part_number_generation() {
        let mut config = PartNumberConfig::default_mechanical();
        let pn1 = config.next_part_number("mechanical");
        let pn2 = config.next_part_number("mechanical");
        assert_eq!(pn1, "MECH-00001");
        assert_eq!(pn2, "MECH-00002");
    }

    #[test]
    fn part_number_categories() {
        let mut config = PartNumberConfig::default_mechanical();
        let m = config.next_part_number("mechanical");
        let e = config.next_part_number("electrical");
        assert!(m.starts_with("MECH-"));
        assert!(e.starts_with("ELEC-"));
    }

    #[test]
    fn revision_alpha() {
        let config = PartNumberConfig::default_mechanical();
        assert_eq!(config.revision_string(1), "A");
        assert_eq!(config.revision_string(2), "B");
        assert_eq!(config.revision_string(26), "Z");
    }

    #[test]
    fn unique_part_count() {
        let bom = sample_bom();
        assert_eq!(bom.unique_part_count(), 9); // top assy + 5 parts + sub-assy + shaft + bearing
    }

    #[test]
    fn where_used() {
        let bom = sample_bom();
        let parents = bom.where_used("STD-00001");
        assert!(parents.contains(&"ASSY-00001".to_string()));
    }

    #[test]
    fn impact_analysis() {
        let bom = sample_bom();
        let affected = bom.impact_analysis("MECH-00003"); // shaft in sub-assembly
        assert!(affected.contains(&"ASSY-00002".to_string()));
    }

    #[test]
    fn flatten_bom() {
        let bom = sample_bom();
        let flat = bom.flatten();
        assert!(flat.len() >= 7);
        // M8 bolts should have qty 4
        let bolts = flat.iter().find(|(pn, _, _)| pn == "STD-00001").unwrap();
        assert!((bolts.2 - 4.0).abs() < 0.01);
    }

    #[test]
    fn diff_boms_added() {
        let old = sample_bom();
        let mut new_bom = sample_bom();
        new_bom.children.push(BomItem::part("MECH-00004", "Cover", 1.0));
        let diff = diff_boms(&old, &new_bom);
        assert!(diff.added.contains(&"MECH-00004".to_string()));
    }

    #[test]
    fn diff_boms_removed() {
        let old = sample_bom();
        let mut new_bom = sample_bom();
        new_bom.children.retain(|c| c.part_number != "STD-00003");
        let diff = diff_boms(&old, &new_bom);
        assert!(diff.removed.contains(&"STD-00003".to_string()));
    }

    #[test]
    fn export_csv_has_header() {
        let bom = sample_bom();
        let csv = export_csv(&bom);
        assert!(csv.starts_with("Part Number,"));
        assert!(csv.contains("MECH-00001"));
    }

    #[test]
    fn empty_bom() {
        let bom = BomItem::part("MECH-00001", "Solo Part", 1.0);
        assert_eq!(bom.unique_part_count(), 1);
        assert!(bom.where_used("anything").is_empty());
    }
}
