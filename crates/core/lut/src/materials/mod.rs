//! Material property lookup — organized by family, aggregated here.
//!
//! Every entry cites its source (ASM Handbook, MMPDS, MatWeb, CAMPUS).
//! All values in SI. Convert at display boundary only.

use physical_units::*;

/// Hardness scale identifier.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HardnessScale {
    Rockwell,
    Brinell,
    Vickers,
}

/// Material category for filtering.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MaterialCategory {
    Aluminum,
    Steel,
    Stainless,
    Titanium,
    Copper,
    Nickel,
    CastIron,
    ToolSteel,
    Magnesium,
    Zinc,
    Refractory,   // Tungsten, Molybdenum, TZM, WC-Co
    Cobalt,       // Stellite, CoCr biomedical alloys
    PreciousMetal, // Gold, Silver (electronics/plating reference)
    LowMeltingMetal, // Lead, Tin, solders
    Polymer,
    Composite,
    Ceramic,
}

/// Complete material property record.
#[derive(Debug, Clone, Copy)]
pub struct Material {
    pub id: &'static str,
    pub name: &'static str,
    pub category: MaterialCategory,
    pub density: Density,
    pub yield_strength: Pressure,
    pub ultimate_tensile: Pressure,
    pub elastic_modulus: Pressure,
    pub poissons_ratio: Dimensionless,
    pub thermal_conductivity: ThermalConductivity,
    pub cte: CTE,
    pub specific_heat: SpecificHeat,
    pub melting_point: Temperature,
    pub hardness: f64,
    pub hardness_scale: HardnessScale,
    pub fatigue_endurance: Pressure,
    pub machinability_index: f64,
    pub source: &'static str,
}

// Sub-modules with material data organized by family
pub mod metals;
pub mod polymers;
pub mod composites;
pub mod ceramics;

/// All materials across all categories. Searches here for universal lookup.
static ALL_TABLES: &[&[Material]] = &[
    metals::METALS,
    polymers::POLYMERS,
    composites::COMPOSITES,
    ceramics::CERAMICS,
];

/// O(n) scan across all material tables.
pub fn lookup(id: &str) -> Option<&'static Material> {
    for table in ALL_TABLES {
        if let Some(mat) = table.iter().find(|m| m.id == id) {
            return Some(mat);
        }
    }
    None
}

/// Filter materials by category across all tables.
pub fn by_category(category: MaterialCategory) -> impl Iterator<Item = &'static Material> {
    ALL_TABLES
        .iter()
        .flat_map(|table| table.iter())
        .filter(move |m| m.category == category)
}

/// Total number of materials in the database.
pub fn count() -> usize {
    ALL_TABLES.iter().map(|t| t.len()).sum()
}

/// Search materials by name substring (case-insensitive).
pub fn search(query: &str) -> impl Iterator<Item = &'static Material> {
    let query_lower = query.to_lowercase();
    ALL_TABLES
        .iter()
        .flat_map(|table| table.iter())
        .filter(move |m| {
            m.name.to_lowercase().contains(&query_lower)
                || m.id.to_lowercase().contains(&query_lower)
        })
}

/// Get all unique categories present in the database.
pub fn categories() -> impl Iterator<Item = MaterialCategory> {
    use MaterialCategory::*;
    [
        Aluminum, Steel, Stainless, Titanium, Copper, Nickel,
        CastIron, ToolSteel, Magnesium, Zinc,
        Refractory, Cobalt, PreciousMetal, LowMeltingMetal,
        Polymer, Composite, Ceramic,
    ]
    .into_iter()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lookup_6061_t6() {
        let mat = lookup("6061-T6").expect("6061-T6 must exist");
        assert_eq!(mat.category, MaterialCategory::Aluminum);
        assert!((mat.yield_strength.to_mpa() - 276.0).abs() < 0.1);
        assert!((mat.density.value() - 2710.0).abs() < 1.0);
    }

    #[test]
    fn lookup_7075_t6() {
        let mat = lookup("7075-T6").expect("7075-T6 must exist");
        assert!(mat.yield_strength > Pressure::mpa(500.0));
    }

    #[test]
    fn lookup_ti64() {
        let mat = lookup("Ti-6Al-4V").expect("Ti-6Al-4V must exist");
        assert_eq!(mat.category, MaterialCategory::Titanium);
        assert!(mat.yield_strength > Pressure::mpa(800.0));
    }

    #[test]
    fn missing_material_returns_none() {
        assert!(lookup("unobtainium").is_none());
    }

    #[test]
    fn filter_by_category() {
        let aluminums: Vec<_> = by_category(MaterialCategory::Aluminum).collect();
        assert!(aluminums.len() >= 3);
        assert!(aluminums.iter().all(|m| m.category == MaterialCategory::Aluminum));
    }

    #[test]
    fn material_count_substantial() {
        let total = count();
        assert!(total >= 100, "expected 100+ materials, got {total}");
    }

    #[test]
    fn search_finds_stainless() {
        let results: Vec<_> = search("stainless").collect();
        assert!(!results.is_empty(), "search for 'stainless' should find results");
    }

    #[test]
    fn search_finds_by_id() {
        let results: Vec<_> = search("4140").collect();
        assert!(!results.is_empty(), "search for '4140' should find results");
    }

    #[test]
    fn all_materials_have_valid_density() {
        for table in ALL_TABLES {
            for m in *table {
                assert!(m.density.value() > 0.0, "material {} has zero density", m.id);
            }
        }
    }

    #[test]
    fn all_materials_have_valid_yield() {
        for table in ALL_TABLES {
            for m in *table {
                assert!(m.yield_strength.value() > 0.0, "material {} has zero yield", m.id);
            }
        }
    }

    #[test]
    fn all_materials_have_source() {
        for table in ALL_TABLES {
            for m in *table {
                assert!(!m.source.is_empty(), "material {} has no source", m.id);
            }
        }
    }

    #[test]
    fn no_duplicate_ids() {
        let mut seen = std::collections::HashSet::new();
        for table in ALL_TABLES {
            for m in *table {
                assert!(seen.insert(m.id), "duplicate material ID: {}", m.id);
            }
        }
    }

    #[test]
    fn polymers_exist() {
        let polymers: Vec<_> = by_category(MaterialCategory::Polymer).collect();
        assert!(polymers.len() >= 20, "expected 20+ polymers, got {}", polymers.len());
    }

    #[test]
    fn composites_exist() {
        let composites: Vec<_> = by_category(MaterialCategory::Composite).collect();
        assert!(composites.len() >= 8, "expected 8+ composites, got {}", composites.len());
    }

    #[test]
    fn ceramics_exist() {
        let ceramics: Vec<_> = by_category(MaterialCategory::Ceramic).collect();
        assert!(ceramics.len() >= 4, "expected 4+ ceramics, got {}", ceramics.len());
    }
}
