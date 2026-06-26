use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::LazyLock;

// ---------------------------------------------------------------------------
// Core types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StandardPart {
    pub part_number: String,
    pub description: String,
    pub category: String,
    pub dimensions: HashMap<String, f64>,
    pub material: String,
    pub unit_price_usd: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FastenerSet {
    pub bolt: StandardPart,
    pub nut: StandardPart,
    pub washer: StandardPart,
    pub total_price_usd: f64,
}

// ---------------------------------------------------------------------------
// Built-in catalog
// ---------------------------------------------------------------------------

static CATALOG: LazyLock<Vec<StandardPart>> = LazyLock::new(build_catalog);

fn dims(pairs: &[(&str, f64)]) -> HashMap<String, f64> {
    pairs.iter().map(|(k, v)| (k.to_string(), *v)).collect()
}

fn build_catalog() -> Vec<StandardPart> {
    let mut parts = Vec::new();

    // -- Metric socket head cap screws (M3-M24) --
    let shcs_data: &[(f64, f64, f64, f64, &str, &str, f64)] = &[
        //  thread, length, head_d, head_h, hex,  pn,           price
        (3.0,  10.0, 5.5,  3.0, "2.5", "91290A111", 0.08),
        (3.0,  20.0, 5.5,  3.0, "2.5", "91290A123", 0.09),
        (4.0,  10.0, 7.0,  4.0, "3",   "91290A143", 0.09),
        (4.0,  20.0, 7.0,  4.0, "3",   "91290A155", 0.10),
        (5.0,  16.0, 8.5,  5.0, "4",   "91290A228", 0.12),
        (5.0,  25.0, 8.5,  5.0, "4",   "91290A235", 0.14),
        (6.0,  16.0, 10.0, 6.0, "5",   "91290A318", 0.15),
        (6.0,  30.0, 10.0, 6.0, "5",   "91290A335", 0.18),
        (8.0,  20.0, 13.0, 8.0, "6",   "91290A420", 0.25),
        (8.0,  40.0, 13.0, 8.0, "6",   "91290A445", 0.32),
        (10.0, 25.0, 16.0, 10.0,"8",   "91290A527", 0.42),
        (10.0, 50.0, 16.0, 10.0,"8",   "91290A553", 0.55),
        (12.0, 30.0, 18.0, 12.0,"10",  "91290A637", 0.65),
        (12.0, 60.0, 18.0, 12.0,"10",  "91290A661", 0.85),
        (16.0, 40.0, 24.0, 16.0,"14",  "91290A748", 1.45),
        (16.0, 70.0, 24.0, 16.0,"14",  "91290A772", 1.90),
        (20.0, 50.0, 30.0, 20.0,"17",  "91290A854", 2.60),
        (20.0, 80.0, 30.0, 20.0,"17",  "91290A878", 3.40),
        (24.0, 60.0, 36.0, 24.0,"19",  "91290A964", 4.20),
        (24.0, 100.0,36.0, 24.0,"19",  "91290A988", 5.50),
    ];
    for &(thread, length, head_d, head_h, hex, pn, price) in shcs_data {
        parts.push(StandardPart {
            part_number: pn.into(),
            description: format!("M{thread}x{length} Socket Head Cap Screw, Class 12.9"),
            category: "screw".into(),
            dimensions: dims(&[
                ("thread_size", thread),
                ("length_mm", length),
                ("head_diameter", head_d),
                ("head_height", head_h),
                ("hex_size", hex.parse::<f64>().unwrap_or(0.0)),
            ]),
            material: "Alloy Steel".into(),
            unit_price_usd: price,
        });
    }

    // -- Metric hex nuts (M3-M24) --
    let nut_data: &[(f64, f64, f64, &str, f64)] = &[
        (3.0,  5.5,  2.4,  "90592A085", 0.04),
        (4.0,  7.0,  3.2,  "90592A090", 0.05),
        (5.0,  8.0,  4.7,  "90592A095", 0.06),
        (6.0,  10.0, 5.2,  "90592A100", 0.07),
        (8.0,  13.0, 6.8,  "90592A105", 0.10),
        (10.0, 16.0, 8.4,  "90592A110", 0.14),
        (12.0, 18.0, 10.8, "90592A115", 0.20),
        (16.0, 24.0, 14.8, "90592A120", 0.35),
        (20.0, 30.0, 18.0, "90592A125", 0.55),
        (24.0, 36.0, 21.5, "90592A130", 0.80),
    ];
    for &(thread, waf, height, pn, price) in nut_data {
        parts.push(StandardPart {
            part_number: pn.into(),
            description: format!("M{thread} Hex Nut, Class 8"),
            category: "nut".into(),
            dimensions: dims(&[
                ("thread_size", thread),
                ("width_across_flats", waf),
                ("height", height),
            ]),
            material: "Steel".into(),
            unit_price_usd: price,
        });
    }

    // -- Metric washers (M3-M24) --
    let washer_data: &[(f64, f64, f64, f64, &str, f64)] = &[
        (3.0,  3.2,  7.0,  0.5,  "93475A210", 0.03),
        (4.0,  4.3,  9.0,  0.8,  "93475A215", 0.03),
        (5.0,  5.3,  10.0, 1.0,  "93475A220", 0.04),
        (6.0,  6.4,  12.0, 1.6,  "93475A225", 0.04),
        (8.0,  8.4,  16.0, 1.6,  "93475A230", 0.05),
        (10.0, 10.5, 20.0, 2.0,  "93475A235", 0.07),
        (12.0, 13.0, 24.0, 2.5,  "93475A240", 0.09),
        (16.0, 17.0, 30.0, 3.0,  "93475A245", 0.14),
        (20.0, 21.0, 37.0, 3.0,  "93475A250", 0.20),
        (24.0, 25.0, 44.0, 4.0,  "93475A255", 0.28),
    ];
    for &(nom, id, od, thick, pn, price) in washer_data {
        parts.push(StandardPart {
            part_number: pn.into(),
            description: format!("M{nom} Flat Washer, DIN 125"),
            category: "washer".into(),
            dimensions: dims(&[
                ("nominal_size", nom),
                ("inner_diameter", id),
                ("outer_diameter", od),
                ("thickness", thick),
            ]),
            material: "Steel".into(),
            unit_price_usd: price,
        });
    }

    // -- Bearings (608, 6001-6010) --
    let bearing_data: &[(&str, f64, f64, f64, &str, &str, f64)] = &[
        ("6681K21",  8.0,  22.0, 7.0,  "deep_groove",     "608",  3.50),
        ("6681K31",  12.0, 28.0, 8.0,  "deep_groove",     "6001", 4.20),
        ("6681K32",  15.0, 32.0, 9.0,  "deep_groove",     "6002", 4.50),
        ("6681K33",  17.0, 35.0, 10.0, "deep_groove",     "6003", 4.80),
        ("6681K34",  20.0, 42.0, 12.0, "deep_groove",     "6004", 5.20),
        ("6681K35",  25.0, 47.0, 12.0, "deep_groove",     "6005", 5.60),
        ("6681K36",  30.0, 55.0, 13.0, "deep_groove",     "6006", 6.20),
        ("6681K37",  35.0, 62.0, 14.0, "deep_groove",     "6007", 7.00),
        ("6681K38",  40.0, 68.0, 15.0, "deep_groove",     "6008", 7.80),
        ("6681K39",  45.0, 75.0, 16.0, "deep_groove",     "6009", 8.50),
        ("6681K40",  50.0, 80.0, 16.0, "deep_groove",     "6010", 9.20),
        ("5908K11",  15.0, 35.0, 11.0, "angular_contact", "7002", 12.50),
        ("5908K12",  20.0, 42.0, 12.0, "angular_contact", "7004", 14.00),
    ];
    for &(pn, bore, od, width, bearing_type, designation, price) in bearing_data {
        parts.push(StandardPart {
            part_number: pn.into(),
            description: format!("{designation} Bearing, {bearing_type}"),
            category: "bearing".into(),
            dimensions: dims(&[
                ("bore_mm", bore),
                ("od_mm", od),
                ("width_mm", width),
            ]),
            material: "Chrome Steel".into(),
            unit_price_usd: price,
        });
    }

    // -- Dowel pins --
    let dowel_data: &[(f64, f64, &str, f64)] = &[
        (3.0,  10.0, "90145A101", 0.60),
        (3.0,  20.0, "90145A111", 0.70),
        (4.0,  16.0, "90145A201", 0.75),
        (4.0,  25.0, "90145A211", 0.85),
        (5.0,  20.0, "90145A301", 0.90),
        (5.0,  30.0, "90145A311", 1.00),
        (6.0,  24.0, "90145A401", 1.10),
        (6.0,  40.0, "90145A421", 1.30),
        (8.0,  30.0, "90145A501", 1.50),
        (8.0,  50.0, "90145A521", 1.80),
        (10.0, 40.0, "90145A601", 2.10),
        (10.0, 60.0, "90145A621", 2.50),
    ];
    for &(dia, length, pn, price) in dowel_data {
        parts.push(StandardPart {
            part_number: pn.into(),
            description: format!("{dia}mm x {length}mm Dowel Pin"),
            category: "dowel_pin".into(),
            dimensions: dims(&[("diameter", dia), ("length", length)]),
            material: "Alloy Steel".into(),
            unit_price_usd: price,
        });
    }

    // -- O-rings (AS568 common sizes) --
    let oring_data: &[(&str, f64, f64, &str, f64)] = &[
        ("9452K11",  1.07,  1.27, "AS568-001", 0.15),
        ("9452K13",  1.42,  1.52, "AS568-002", 0.15),
        ("9452K15",  1.78,  1.78, "AS568-003", 0.16),
        ("9452K17",  2.57,  1.78, "AS568-005", 0.16),
        ("9452K21",  4.34,  1.78, "AS568-010", 0.17),
        ("9452K31",  7.65,  1.78, "AS568-015", 0.18),
        ("9452K41",  12.37, 1.78, "AS568-020", 0.20),
        ("9452K51",  18.72, 2.62, "AS568-025", 0.25),
        ("9452K61",  25.07, 2.62, "AS568-030", 0.30),
        ("9452K71",  34.65, 2.62, "AS568-035", 0.35),
        ("9452K81",  44.04, 3.53, "AS568-040", 0.45),
        ("9452K91",  53.57, 3.53, "AS568-045", 0.55),
    ];
    for &(pn, id, cs, designation, price) in oring_data {
        parts.push(StandardPart {
            part_number: pn.into(),
            description: format!("O-Ring {designation}, Buna-N"),
            category: "o_ring".into(),
            dimensions: dims(&[("inner_diameter", id), ("cross_section", cs)]),
            material: "Buna-N (NBR)".into(),
            unit_price_usd: price,
        });
    }

    parts
}

// ---------------------------------------------------------------------------
// Query API
// ---------------------------------------------------------------------------

/// Full-text keyword search across part descriptions, categories, and part numbers.
pub fn search_parts(query: &str) -> Vec<&'static StandardPart> {
    let lower = query.to_lowercase();
    let tokens: Vec<&str> = lower.split_whitespace().collect();
    CATALOG
        .iter()
        .filter(|p| {
            let haystack = format!(
                "{} {} {} {}",
                p.part_number.to_lowercase(),
                p.description.to_lowercase(),
                p.category.to_lowercase(),
                p.material.to_lowercase(),
            );
            tokens.iter().all(|tok| haystack.contains(tok))
        })
        .collect()
}

/// Look up a single part by exact part number.
pub fn lookup_part(part_number: &str) -> Option<&'static StandardPart> {
    CATALOG.iter().find(|p| p.part_number == part_number)
}

/// Find bolts (screws) whose thread size matches a given hole diameter in mm.
pub fn bolts_for_hole(diameter_mm: f64) -> Vec<&'static StandardPart> {
    CATALOG
        .iter()
        .filter(|p| {
            p.category == "screw"
                && p.dimensions
                    .get("thread_size")
                    .is_some_and(|t| (*t - diameter_mm).abs() < 0.01)
        })
        .collect()
}

/// Suggest a complete fastener set (bolt + nut + washer) for a given hole diameter
/// and grip length.
///
/// Picks the shortest bolt whose length >= grip_length_mm, the matching nut, and
/// the matching washer.
pub fn suggest_fastener_set(hole_diameter_mm: f64, grip_length_mm: f64) -> Option<FastenerSet> {
    // Find the shortest suitable bolt
    let mut bolts = bolts_for_hole(hole_diameter_mm);
    bolts.sort_by(|a, b| {
        let la = a.dimensions.get("length_mm").unwrap_or(&0.0);
        let lb = b.dimensions.get("length_mm").unwrap_or(&0.0);
        la.partial_cmp(lb).unwrap_or(std::cmp::Ordering::Equal)
    });
    let bolt = bolts
        .iter()
        .find(|b| {
            b.dimensions
                .get("length_mm")
                .is_some_and(|l| *l >= grip_length_mm)
        })?;

    // Matching nut
    let nut = CATALOG.iter().find(|p| {
        p.category == "nut"
            && p.dimensions
                .get("thread_size")
                .is_some_and(|t| (*t - hole_diameter_mm).abs() < 0.01)
    })?;

    // Matching washer
    let washer = CATALOG.iter().find(|p| {
        p.category == "washer"
            && p.dimensions
                .get("nominal_size")
                .is_some_and(|t| (*t - hole_diameter_mm).abs() < 0.01)
    })?;

    let total = bolt.unit_price_usd + nut.unit_price_usd + washer.unit_price_usd;

    Some(FastenerSet {
        bolt: (*bolt).clone(),
        nut: nut.clone(),
        washer: washer.clone(),
        total_price_usd: total,
    })
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_has_all_categories() {
        let cats: Vec<&str> = vec!["screw", "nut", "washer", "bearing", "dowel_pin", "o_ring"];
        for cat in cats {
            assert!(
                CATALOG.iter().any(|p| p.category == cat),
                "missing category: {cat}"
            );
        }
    }

    #[test]
    fn catalog_part_numbers_unique() {
        let mut seen = std::collections::HashSet::new();
        for p in CATALOG.iter() {
            assert!(seen.insert(&p.part_number), "duplicate: {}", p.part_number);
        }
    }

    #[test]
    fn lookup_known_part() {
        let part = lookup_part("91290A111").unwrap();
        assert!(part.description.contains("M3"));
        assert_eq!(part.category, "screw");
    }

    #[test]
    fn lookup_missing_part() {
        assert!(lookup_part("DOES_NOT_EXIST").is_none());
    }

    #[test]
    fn search_screws() {
        let results = search_parts("socket head cap screw M8");
        assert!(!results.is_empty());
        for r in &results {
            assert_eq!(r.category, "screw");
            assert!(r.dimensions.get("thread_size").is_some_and(|t| (*t - 8.0).abs() < 0.01));
        }
    }

    #[test]
    fn search_bearings() {
        let results = search_parts("bearing 6005");
        assert_eq!(results.len(), 1);
        assert!(results[0].description.contains("6005"));
    }

    #[test]
    fn search_orings() {
        let results = search_parts("o-ring");
        assert!(results.len() >= 10);
    }

    #[test]
    fn bolts_for_m6_hole() {
        let bolts = bolts_for_hole(6.0);
        assert_eq!(bolts.len(), 2); // M6x16 and M6x30
        for b in &bolts {
            assert!(b.dimensions.get("thread_size").is_some_and(|t| (*t - 6.0).abs() < 0.01));
        }
    }

    #[test]
    fn suggest_fastener_set_m8() {
        let set = suggest_fastener_set(8.0, 25.0).unwrap();
        assert!(set.bolt.description.contains("M8"));
        assert!(set.nut.description.contains("M8"));
        assert!(set.washer.description.contains("M8"));
        assert!(set.bolt.dimensions["length_mm"] >= 25.0);
        assert!(set.total_price_usd > 0.0);
    }

    #[test]
    fn suggest_fastener_set_no_match() {
        // M99 does not exist
        assert!(suggest_fastener_set(99.0, 10.0).is_none());
    }

    #[test]
    fn part_serialization_roundtrip() {
        let part = lookup_part("90592A085").unwrap();
        let json = serde_json::to_string(part).unwrap();
        let deserialized: StandardPart = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.part_number, "90592A085");
    }

    #[test]
    fn dowel_pins_present() {
        let results = search_parts("dowel pin");
        assert!(results.len() >= 10);
    }
}
