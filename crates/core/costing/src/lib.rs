//! `physical-costing` — Real-time manufacturing cost estimation.
//!
//! Inspired by MecAgent's live pricing. Estimates cost, lead time, and
//! process steps for CNC, 3D printing, sheet metal, casting, and injection molding.

use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostEstimate {
    pub material_cost_usd: f64,
    pub machining_cost_usd: f64,
    pub setup_cost_usd: f64,
    pub finishing_cost_usd: f64,
    pub total_cost_usd: f64,
    pub lead_time_days: f64,
    pub quantity: u32,
    pub process: String,
    pub breakdown: Vec<CostLineItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostLineItem {
    pub description: String,
    pub cost_usd: f64,
}

fn material_cost_per_kg(id: &str) -> f64 {
    let l = id.to_lowercase();
    if l.contains("titanium") || l.contains("ti-6al") { 25.0 }
    else if l.contains("7075") { 5.0 }
    else if l.contains("6061") || l.contains("aluminum") { 3.0 }
    else if l.contains("copper") { 8.0 }
    else if l.contains("stainless") { 4.0 }
    else if l.contains("inconel") { 30.0 }
    else if l.contains("steel") || l.contains("1018") { 1.5 }
    else if l.contains("abs") || l.contains("pla") { 20.0 }
    else if l.contains("peek") { 300.0 }
    else { 3.0 }
}

fn density(id: &str) -> f64 {
    physical_lut::materials::lookup(id).map(|m| m.density.value()).unwrap_or(2700.0)
}

fn qty_discount(q: u32) -> f64 {
    match q { 0..=1 => 1.0, 2..=9 => 0.9, 10..=49 => 0.7, 50..=99 => 0.55, 100..=499 => 0.45, 500..=999 => 0.35, _ => 0.30 }
}

pub fn estimate_cnc_cost(vol_mm3: f64, mat_id: &str, complexity: f64, qty: u32) -> CostEstimate {
    let mass = density(mat_id) * vol_mm3 * 1e-9;
    let mat = mass * material_cost_per_kg(mat_id) * 1.3;
    let rate = 85.0 + complexity * 15.0;
    let hours = (vol_mm3 / 50_000.0).max(0.25) * complexity * 0.5;
    let mach = hours * rate;
    let setup = 75.0 + complexity * 25.0;
    let finish = if complexity > 2.0 { 25.0 } else { 10.0 };
    let pu = (mat + mach + finish) * qty_discount(qty);
    let total = pu * qty as f64 + setup;
    CostEstimate {
        material_cost_usd: mat * qty as f64, machining_cost_usd: mach * qty as f64,
        setup_cost_usd: setup, finishing_cost_usd: finish * qty as f64,
        total_cost_usd: total, lead_time_days: 3.0 + (qty as f64 / 10.0).ceil().min(15.0),
        quantity: qty, process: "CNC 3-axis".into(),
        breakdown: vec![
            CostLineItem { description: "Material".into(), cost_usd: mat },
            CostLineItem { description: "Machining".into(), cost_usd: mach },
            CostLineItem { description: "Setup".into(), cost_usd: setup },
            CostLineItem { description: "Finishing".into(), cost_usd: finish },
        ],
    }
}

pub fn estimate_3dprint_cost(vol_mm3: f64, mat_id: &str, process: &str, qty: u32) -> CostEstimate {
    let cm3 = vol_mm3 / 1000.0;
    let (rate, base, name) = match process {
        "fdm" => (0.10, 5.0, "FDM"), "sla" => (0.30, 10.0, "SLA"),
        "sls" => (0.50, 15.0, "SLS"), "dmls" | "slm" => (2.50, 50.0, "DMLS"),
        _ => (0.15, 8.0, "FDM"),
    };
    let mass = density(mat_id) * vol_mm3 * 1e-9;
    let mat = mass * material_cost_per_kg(mat_id);
    let print = cm3 * rate;
    let finish = if process == "fdm" { 5.0 } else { 15.0 };
    let pu = (mat + print + finish) * qty_discount(qty);
    CostEstimate {
        material_cost_usd: mat * qty as f64, machining_cost_usd: print * qty as f64,
        setup_cost_usd: base, finishing_cost_usd: finish * qty as f64,
        total_cost_usd: pu * qty as f64 + base,
        lead_time_days: 1.0 + (cm3 * 0.5 * qty as f64 / 24.0).ceil().min(14.0),
        quantity: qty, process: name.into(),
        breakdown: vec![
            CostLineItem { description: "Material".into(), cost_usd: mat },
            CostLineItem { description: "Print".into(), cost_usd: print },
            CostLineItem { description: "Post-process".into(), cost_usd: finish },
        ],
    }
}

pub fn estimate_sheetmetal_cost(area_mm2: f64, _thick: f64, bends: u32, mat_id: &str, qty: u32) -> CostEstimate {
    let vol = area_mm2 * _thick;
    let mass = density(mat_id) * vol * 1e-9;
    let mat = mass * material_cost_per_kg(mat_id) * 1.15;
    let cut = area_mm2 * 0.00005;
    let bend_cost = bends as f64 * 8.0;
    let setup = 50.0 + bends as f64 * 10.0;
    let pu = (mat + cut + bend_cost + 10.0) * qty_discount(qty);
    CostEstimate {
        material_cost_usd: mat * qty as f64, machining_cost_usd: (cut + bend_cost) * qty as f64,
        setup_cost_usd: setup, finishing_cost_usd: 10.0 * qty as f64,
        total_cost_usd: pu * qty as f64 + setup,
        lead_time_days: 2.0 + (qty as f64 / 50.0).ceil().min(10.0),
        quantity: qty, process: "Sheet metal".into(),
        breakdown: vec![
            CostLineItem { description: "Material".into(), cost_usd: mat },
            CostLineItem { description: "Laser cut".into(), cost_usd: cut },
            CostLineItem { description: format!("{bends} bends"), cost_usd: bend_cost },
        ],
    }
}

pub fn estimate_casting_cost(vol_mm3: f64, mat_id: &str, complexity: f64, qty: u32) -> CostEstimate {
    let mass = density(mat_id) * vol_mm3 * 1e-9;
    let mat = mass * material_cost_per_kg(mat_id);
    let tooling = 500.0 + complexity * 500.0;
    let per_part = mat * 1.2 + complexity * 5.0 + 15.0;
    let pu = per_part * qty_discount(qty);
    CostEstimate {
        material_cost_usd: mat * qty as f64, machining_cost_usd: (per_part - mat - 15.0) * qty as f64,
        setup_cost_usd: tooling, finishing_cost_usd: 15.0 * qty as f64,
        total_cost_usd: pu * qty as f64 + tooling,
        lead_time_days: 10.0 + (qty as f64 / 100.0).ceil().min(20.0),
        quantity: qty, process: "Casting".into(),
        breakdown: vec![
            CostLineItem { description: "Tooling".into(), cost_usd: tooling },
            CostLineItem { description: "Material + casting".into(), cost_usd: per_part },
        ],
    }
}

pub fn estimate_injection_mold_cost(vol_mm3: f64, mat_id: &str, qty: u32) -> CostEstimate {
    let mass = density(mat_id) * vol_mm3 * 1e-9;
    let mat = mass * material_cost_per_kg(mat_id);
    let mold = 5_000.0 + vol_mm3 * 0.001;
    let per_part = (mat + 0.50) * qty_discount(qty);
    CostEstimate {
        material_cost_usd: mat * qty as f64, machining_cost_usd: 0.50 * qty as f64,
        setup_cost_usd: mold, finishing_cost_usd: 0.0,
        total_cost_usd: per_part * qty as f64 + mold,
        lead_time_days: 21.0 + (qty as f64 / 1000.0).ceil().min(14.0),
        quantity: qty, process: "Injection molding".into(),
        breakdown: vec![
            CostLineItem { description: "Mold".into(), cost_usd: mold },
            CostLineItem { description: "Per shot".into(), cost_usd: mat + 0.50 },
        ],
    }
}

pub fn cheapest_process(vol_mm3: f64, mat_id: &str, qty: u32) -> CostEstimate {
    [estimate_cnc_cost(vol_mm3, mat_id, 2.0, qty),
     estimate_3dprint_cost(vol_mm3, mat_id, "fdm", qty),
     estimate_3dprint_cost(vol_mm3, mat_id, "sla", qty)]
        .into_iter().min_by(|a, b| a.total_cost_usd.partial_cmp(&b.total_cost_usd).unwrap()).unwrap()
}

pub fn quantity_break_analysis(vol_mm3: f64, mat_id: &str, qtys: &[u32]) -> Vec<CostEstimate> {
    qtys.iter().map(|&q| cheapest_process(vol_mm3, mat_id, q)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test] fn cnc_basic() { let e = estimate_cnc_cost(50_000.0, "6061-T6", 2.0, 1); assert!(e.total_cost_usd > 0.0); }
    #[test] fn cnc_qty_discount() {
        let s = estimate_cnc_cost(50_000.0, "6061-T6", 2.0, 1);
        let h = estimate_cnc_cost(50_000.0, "6061-T6", 2.0, 100);
        assert!(h.total_cost_usd / 100.0 < s.total_cost_usd);
    }
    #[test] fn ti_more_than_al() {
        let al = estimate_cnc_cost(50_000.0, "6061-T6", 2.0, 1);
        let ti = estimate_cnc_cost(50_000.0, "Ti-6Al-4V", 2.0, 1);
        assert!(ti.material_cost_usd > al.material_cost_usd);
    }
    #[test] fn fdm_cost() { let e = estimate_3dprint_cost(10_000.0, "ABS", "fdm", 1); assert!(e.total_cost_usd > 0.0); }
    #[test] fn dmls_more_than_fdm() {
        let f = estimate_3dprint_cost(10_000.0, "6061-T6", "fdm", 1);
        let d = estimate_3dprint_cost(10_000.0, "6061-T6", "dmls", 1);
        assert!(d.total_cost_usd > f.total_cost_usd);
    }
    #[test] fn sheetmetal() { let e = estimate_sheetmetal_cost(10_000.0, 1.5, 3, "1018-CD", 10); assert!(e.total_cost_usd > 0.0); }
    #[test] fn more_bends_more_cost() {
        let a = estimate_sheetmetal_cost(10_000.0, 1.5, 1, "1018-CD", 1);
        let b = estimate_sheetmetal_cost(10_000.0, 1.5, 8, "1018-CD", 1);
        assert!(b.total_cost_usd > a.total_cost_usd);
    }
    #[test] fn casting() { let e = estimate_casting_cost(200_000.0, "6061-T6", 3.0, 50); assert!(e.setup_cost_usd > 500.0); }
    #[test] fn injection_mold() {
        let e = estimate_injection_mold_cost(5_000.0, "ABS", 1000);
        assert!(e.setup_cost_usd > 5000.0);
        assert!(e.total_cost_usd / 1000.0 < 20.0);
    }
    #[test] fn cheapest_selects() { let e = cheapest_process(20_000.0, "6061-T6", 1); assert!(!e.process.is_empty()); }
    #[test] fn qty_break() {
        let r = quantity_break_analysis(20_000.0, "6061-T6", &[1, 10, 100]);
        assert_eq!(r.len(), 3);
        assert!(r[2].total_cost_usd / 100.0 < r[0].total_cost_usd);
    }
    #[test] fn breakdown_items() { let e = estimate_cnc_cost(30_000.0, "6061-T6", 1.0, 1); assert!(!e.breakdown.is_empty()); }
}
