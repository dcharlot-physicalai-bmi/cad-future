//! Composite material property lookup tables.
//!
//! Fiber-reinforced polymer (FRP) composites for structural engineering.
//! Properties are fiber-direction (0°) unless otherwise noted.
//! Typical fiber volume fraction Vf = 60% for prepreg layups.
//!
//! Sources: MIL-HDBK-17, CMH-17, Hexcel/Toray datasheets, ASM Handbook Vol 21.

use physical_units::*;
use super::{Material, MaterialCategory, HardnessScale};

pub static COMPOSITES: &[Material] = &[
    // -----------------------------------------------------------------------
    // Carbon Fiber / Epoxy systems
    // -----------------------------------------------------------------------

    // T700/epoxy class, unidirectional tape, Vf~60%, 0° fiber direction.
    // Source: Toray T700S datasheet, MIL-HDBK-17 Vol 2
    Material {
        id: "CF-Uni-0",
        name: "Carbon Fiber / Epoxy Unidirectional (0°)",
        category: MaterialCategory::Composite,
        density: Density::kg_m3(1580.0),
        yield_strength: Pressure::mpa(1500.0),       // onset of fiber breakage (no distinct yield)
        ultimate_tensile: Pressure::mpa(2410.0),      // 0° tensile strength
        elastic_modulus: Pressure::gpa(135.0),         // 0° longitudinal modulus
        poissons_ratio: Dimensionless::ratio(0.30),
        thermal_conductivity: ThermalConductivity::w_mk(5.0),   // fiber direction
        cte: CTE::um_mk(-0.1),                        // near-zero / slightly negative in fiber dir
        specific_heat: SpecificHeat::j_kgk(900.0),
        melting_point: Temperature::celsius(180.0),    // epoxy Tg (decomposition ~300 °C)
        hardness: 70.0,                                // Brinell equivalent, indicative only
        hardness_scale: HardnessScale::Brinell,
        fatigue_endurance: Pressure::mpa(1200.0),      // ~50% UTS, R=-1, 10^6 cycles
        machinability_index: 45.0,                     // abrasive to tooling
        source: "Toray T700S datasheet, MIL-HDBK-17 Vol 2",
    },

    // Same laminate tested in 90° (transverse) direction — matrix-dominated.
    // Source: MIL-HDBK-17 Vol 2, CMH-17
    Material {
        id: "CF-Uni-90",
        name: "Carbon Fiber / Epoxy Unidirectional (90°)",
        category: MaterialCategory::Composite,
        density: Density::kg_m3(1580.0),
        yield_strength: Pressure::mpa(40.0),           // transverse, matrix-dominated
        ultimate_tensile: Pressure::mpa(50.0),          // 90° tensile
        elastic_modulus: Pressure::gpa(8.5),            // transverse modulus
        poissons_ratio: Dimensionless::ratio(0.025),    // ν_21
        thermal_conductivity: ThermalConductivity::w_mk(0.8),   // transverse
        cte: CTE::um_mk(28.0),                         // transverse CTE, matrix-dominated
        specific_heat: SpecificHeat::j_kgk(900.0),
        melting_point: Temperature::celsius(180.0),
        hardness: 70.0,
        hardness_scale: HardnessScale::Brinell,
        fatigue_endurance: Pressure::mpa(20.0),         // ~40% UTS transverse
        machinability_index: 45.0,
        source: "MIL-HDBK-17 Vol 2, CMH-17",
    },

    // Carbon/epoxy biaxial 0/90 layup, symmetric balanced laminate.
    // Source: ASM Handbook Vol 21, CMH-17 Vol 2 Section 4
    Material {
        id: "CF-Biaxial-0-90",
        name: "Carbon Fiber / Epoxy Biaxial (0°/90°)",
        category: MaterialCategory::Composite,
        density: Density::kg_m3(1580.0),
        yield_strength: Pressure::mpa(600.0),
        ultimate_tensile: Pressure::mpa(760.0),         // in-plane, balanced laminate
        elastic_modulus: Pressure::gpa(70.0),            // half-power rule: (E0 + E90) / 2
        poissons_ratio: Dimensionless::ratio(0.05),
        thermal_conductivity: ThermalConductivity::w_mk(2.5),
        cte: CTE::um_mk(1.5),                           // balanced, low CTE
        specific_heat: SpecificHeat::j_kgk(900.0),
        melting_point: Temperature::celsius(180.0),
        hardness: 68.0,
        hardness_scale: HardnessScale::Brinell,
        fatigue_endurance: Pressure::mpa(340.0),
        machinability_index: 45.0,
        source: "ASM Handbook Vol 21, CMH-17 Vol 2",
    },

    // Carbon/epoxy ±45 biaxial — shear-dominated, commonly used for torsion.
    // Source: ASM Handbook Vol 21, Herakovich "Mechanics of Fibrous Composites"
    Material {
        id: "CF-Biaxial-pm45",
        name: "Carbon Fiber / Epoxy Biaxial (±45°)",
        category: MaterialCategory::Composite,
        density: Density::kg_m3(1580.0),
        yield_strength: Pressure::mpa(100.0),           // matrix-dominated in tension
        ultimate_tensile: Pressure::mpa(140.0),
        elastic_modulus: Pressure::gpa(17.0),            // in-plane; shear modulus G12 ~ 42 GPa
        poissons_ratio: Dimensionless::ratio(0.75),      // high ν for ±45
        thermal_conductivity: ThermalConductivity::w_mk(2.5),
        cte: CTE::um_mk(16.0),                          // matrix-dominated CTE
        specific_heat: SpecificHeat::j_kgk(900.0),
        melting_point: Temperature::celsius(180.0),
        hardness: 65.0,
        hardness_scale: HardnessScale::Brinell,
        fatigue_endurance: Pressure::mpa(55.0),
        machinability_index: 45.0,
        source: "ASM Handbook Vol 21, Herakovich Mechanics of Fibrous Composites",
    },

    // 2×2 twill or 5-harness satin woven fabric, balanced 0/90.
    // Source: Hexcel HexTow AS4 woven datasheet, CMH-17
    Material {
        id: "CF-Woven",
        name: "Carbon Fiber / Epoxy Woven (balanced 0/90)",
        category: MaterialCategory::Composite,
        density: Density::kg_m3(1570.0),
        yield_strength: Pressure::mpa(500.0),
        ultimate_tensile: Pressure::mpa(620.0),         // warp direction
        elastic_modulus: Pressure::gpa(70.0),            // warp = weft for balanced weave
        poissons_ratio: Dimensionless::ratio(0.06),
        thermal_conductivity: ThermalConductivity::w_mk(3.5),
        cte: CTE::um_mk(2.1),                           // in-plane
        specific_heat: SpecificHeat::j_kgk(900.0),
        melting_point: Temperature::celsius(180.0),
        hardness: 65.0,
        hardness_scale: HardnessScale::Brinell,
        fatigue_endurance: Pressure::mpa(280.0),
        machinability_index: 45.0,
        source: "Hexcel HexTow AS4 woven datasheet, CMH-17",
    },

    // Quasi-isotropic layup [0/±45/90]s, T300/5208 class.
    // Source: MIL-HDBK-17, Tsai "Composites Design" 4th ed.
    Material {
        id: "CF-QI",
        name: "Carbon Fiber / Epoxy Quasi-Isotropic [0/±45/90]s",
        category: MaterialCategory::Composite,
        density: Density::kg_m3(1580.0),
        yield_strength: Pressure::mpa(350.0),
        ultimate_tensile: Pressure::mpa(565.0),
        elastic_modulus: Pressure::gpa(46.0),            // isotropic in-plane
        poissons_ratio: Dimensionless::ratio(0.31),
        thermal_conductivity: ThermalConductivity::w_mk(3.0),
        cte: CTE::um_mk(2.4),                           // quasi-isotropic in-plane
        specific_heat: SpecificHeat::j_kgk(900.0),
        melting_point: Temperature::celsius(180.0),
        hardness: 65.0,
        hardness_scale: HardnessScale::Brinell,
        fatigue_endurance: Pressure::mpa(240.0),
        machinability_index: 45.0,
        source: "MIL-HDBK-17, Tsai Composites Design 4th ed.",
    },

    // Carbon fiber with PEEK thermoplastic matrix — aerospace grade (APC-2 class), unidirectional.
    // Source: Solvay APC-2 datasheet, CMH-17
    Material {
        id: "CF-PEEK",
        name: "Carbon Fiber / PEEK (APC-2 aerospace)",
        category: MaterialCategory::Composite,
        density: Density::kg_m3(1600.0),
        yield_strength: Pressure::mpa(1600.0),
        ultimate_tensile: Pressure::mpa(2130.0),         // 0° unidirectional
        elastic_modulus: Pressure::gpa(134.0),
        poissons_ratio: Dimensionless::ratio(0.30),
        thermal_conductivity: ThermalConductivity::w_mk(5.2),
        cte: CTE::um_mk(0.2),
        specific_heat: SpecificHeat::j_kgk(930.0),
        melting_point: Temperature::celsius(343.0),       // PEEK Tm (Tg ~143 °C)
        hardness: 75.0,
        hardness_scale: HardnessScale::Brinell,
        fatigue_endurance: Pressure::mpa(1100.0),
        machinability_index: 40.0,
        source: "Solvay APC-2 datasheet, CMH-17",
    },

    // Carbon/BMI (bismaleimide) unidirectional. High-temperature use to 232 °C wet.
    // Source: Cytec 5250-4 BMI datasheet, ASM Handbook Vol 21, CMH-17
    Material {
        id: "CF-BMI",
        name: "Carbon Fiber / BMI (bismaleimide, high-temp)",
        category: MaterialCategory::Composite,
        density: Density::kg_m3(1590.0),
        yield_strength: Pressure::mpa(1400.0),
        ultimate_tensile: Pressure::mpa(2200.0),         // 0° unidirectional, RT
        elastic_modulus: Pressure::gpa(140.0),
        poissons_ratio: Dimensionless::ratio(0.30),
        thermal_conductivity: ThermalConductivity::w_mk(5.0),
        cte: CTE::um_mk(0.2),
        specific_heat: SpecificHeat::j_kgk(880.0),
        melting_point: Temperature::celsius(232.0),       // dry Tg (wet service limit ~200 °C)
        hardness: 72.0,
        hardness_scale: HardnessScale::Brinell,
        fatigue_endurance: Pressure::mpa(1050.0),
        machinability_index: 43.0,
        source: "Cytec 5250-4 BMI datasheet, ASM Handbook Vol 21, CMH-17",
    },

    // -----------------------------------------------------------------------
    // Glass Fiber / Epoxy systems
    // -----------------------------------------------------------------------

    // E-glass/epoxy unidirectional, Vf~60%, 0° direction.
    // Source: MIL-HDBK-17 Vol 2, Owens Corning datasheets
    Material {
        id: "GF-Uni",
        name: "Glass Fiber / Epoxy Unidirectional (E-glass, 0°)",
        category: MaterialCategory::Composite,
        density: Density::kg_m3(2000.0),
        yield_strength: Pressure::mpa(800.0),
        ultimate_tensile: Pressure::mpa(1080.0),
        elastic_modulus: Pressure::gpa(45.0),
        poissons_ratio: Dimensionless::ratio(0.28),
        thermal_conductivity: ThermalConductivity::w_mk(1.1),
        cte: CTE::um_mk(6.3),
        specific_heat: SpecificHeat::j_kgk(850.0),
        melting_point: Temperature::celsius(180.0),
        hardness: 55.0,
        hardness_scale: HardnessScale::Brinell,
        fatigue_endurance: Pressure::mpa(350.0),
        machinability_index: 55.0,
        source: "MIL-HDBK-17 Vol 2, Owens Corning",
    },

    // E-glass woven roving / epoxy, balanced weave.
    // Source: MIL-HDBK-17, JPS Composite Materials datasheets
    Material {
        id: "GF-Woven",
        name: "Glass Fiber / Epoxy Woven (E-glass, balanced)",
        category: MaterialCategory::Composite,
        density: Density::kg_m3(1950.0),
        yield_strength: Pressure::mpa(250.0),
        ultimate_tensile: Pressure::mpa(380.0),
        elastic_modulus: Pressure::gpa(25.0),
        poissons_ratio: Dimensionless::ratio(0.12),
        thermal_conductivity: ThermalConductivity::w_mk(0.9),
        cte: CTE::um_mk(10.0),
        specific_heat: SpecificHeat::j_kgk(850.0),
        melting_point: Temperature::celsius(180.0),
        hardness: 50.0,
        hardness_scale: HardnessScale::Brinell,
        fatigue_endurance: Pressure::mpa(150.0),
        machinability_index: 55.0,
        source: "MIL-HDBK-17, JPS Composite Materials",
    },

    // E-glass / polyester chopped strand mat (CSM), hand layup, Vf~25-30%. Random fibers.
    // Source: ASM Handbook Vol 21, Quinn Composites Design Manual
    Material {
        id: "GF-Poly-CSM",
        name: "Glass Fiber / Polyester Chopped Strand Mat (CSM)",
        category: MaterialCategory::Composite,
        density: Density::kg_m3(1600.0),
        yield_strength: Pressure::mpa(60.0),             // random fiber orientation
        ultimate_tensile: Pressure::mpa(90.0),
        elastic_modulus: Pressure::gpa(8.0),
        poissons_ratio: Dimensionless::ratio(0.30),
        thermal_conductivity: ThermalConductivity::w_mk(0.5),
        cte: CTE::um_mk(22.0),
        specific_heat: SpecificHeat::j_kgk(800.0),
        melting_point: Temperature::celsius(110.0),       // polyester HDT
        hardness: 38.0,
        hardness_scale: HardnessScale::Brinell,
        fatigue_endurance: Pressure::mpa(30.0),
        machinability_index: 62.0,
        source: "ASM Handbook Vol 21, Quinn Composites Design Manual",
    },

    // E-glass / polyester, hand layup, Vf~30-35%. Lower properties than prepreg.
    // Source: Composites Design Manual (Quinn), MatWeb
    Material {
        id: "EG-Poly",
        name: "E-Glass / Polyester (hand layup)",
        category: MaterialCategory::Composite,
        density: Density::kg_m3(1800.0),
        yield_strength: Pressure::mpa(80.0),
        ultimate_tensile: Pressure::mpa(130.0),
        elastic_modulus: Pressure::gpa(10.0),
        poissons_ratio: Dimensionless::ratio(0.25),
        thermal_conductivity: ThermalConductivity::w_mk(0.6),
        cte: CTE::um_mk(18.0),
        specific_heat: SpecificHeat::j_kgk(800.0),
        melting_point: Temperature::celsius(120.0),       // polyester HDT
        hardness: 40.0,
        hardness_scale: HardnessScale::Brinell,
        fatigue_endurance: Pressure::mpa(45.0),
        machinability_index: 60.0,
        source: "Quinn Composites Design Manual, MatWeb",
    },

    // E-glass / vinyl ester, marine grade, vacuum infusion, Vf~50%.
    // Source: ASM Handbook Vol 21, Hexion vinyl ester resin datasheets
    Material {
        id: "GF-VE-Marine",
        name: "Glass Fiber / Vinyl Ester (marine grade)",
        category: MaterialCategory::Composite,
        density: Density::kg_m3(1850.0),
        yield_strength: Pressure::mpa(180.0),
        ultimate_tensile: Pressure::mpa(270.0),           // woven fabric, quasi-isotropic
        elastic_modulus: Pressure::gpa(16.0),
        poissons_ratio: Dimensionless::ratio(0.25),
        thermal_conductivity: ThermalConductivity::w_mk(0.7),
        cte: CTE::um_mk(17.0),
        specific_heat: SpecificHeat::j_kgk(820.0),
        melting_point: Temperature::celsius(130.0),       // vinyl ester HDT (wet use limit ~65 °C)
        hardness: 45.0,
        hardness_scale: HardnessScale::Brinell,
        fatigue_endurance: Pressure::mpa(100.0),
        machinability_index: 58.0,
        source: "ASM Handbook Vol 21, Hexion vinyl ester datasheets",
    },

    // E-glass / phenolic, fire-resistant laminate (e.g. FR4 structural, aircraft interior).
    // Source: ASM Handbook Vol 21, Durite/Momentive phenolic resin datasheets
    Material {
        id: "GF-Phenolic",
        name: "Glass Fiber / Phenolic (fire-resistant)",
        category: MaterialCategory::Composite,
        density: Density::kg_m3(1850.0),
        yield_strength: Pressure::mpa(200.0),
        ultimate_tensile: Pressure::mpa(280.0),
        elastic_modulus: Pressure::gpa(18.0),
        poissons_ratio: Dimensionless::ratio(0.25),
        thermal_conductivity: ThermalConductivity::w_mk(0.5),
        cte: CTE::um_mk(14.0),
        specific_heat: SpecificHeat::j_kgk(800.0),
        melting_point: Temperature::celsius(200.0),       // phenolic Tg (chars rather than burns)
        hardness: 48.0,
        hardness_scale: HardnessScale::Brinell,
        fatigue_endurance: Pressure::mpa(90.0),
        machinability_index: 55.0,
        source: "ASM Handbook Vol 21, Momentive phenolic datasheets",
    },

    // S-2 glass / epoxy, Vf~60%. Higher strength than E-glass.
    // Source: AGY S-2 Glass datasheet, MIL-HDBK-17
    Material {
        id: "SG-Epoxy",
        name: "S-Glass / Epoxy Unidirectional",
        category: MaterialCategory::Composite,
        density: Density::kg_m3(2000.0),
        yield_strength: Pressure::mpa(1100.0),
        ultimate_tensile: Pressure::mpa(1600.0),
        elastic_modulus: Pressure::gpa(55.0),
        poissons_ratio: Dimensionless::ratio(0.28),
        thermal_conductivity: ThermalConductivity::w_mk(1.2),
        cte: CTE::um_mk(5.0),
        specific_heat: SpecificHeat::j_kgk(850.0),
        melting_point: Temperature::celsius(180.0),
        hardness: 60.0,
        hardness_scale: HardnessScale::Brinell,
        fatigue_endurance: Pressure::mpa(550.0),
        machinability_index: 50.0,
        source: "AGY S-2 Glass datasheet, MIL-HDBK-17",
    },

    // -----------------------------------------------------------------------
    // Aramid (Kevlar) systems
    // -----------------------------------------------------------------------

    // Kevlar 49 / epoxy, Vf~60%, 0° unidirectional.
    // Source: DuPont Kevlar 49 technical guide, MIL-HDBK-17
    Material {
        id: "K49-Epoxy",
        name: "Kevlar 49 / Epoxy Unidirectional",
        category: MaterialCategory::Composite,
        density: Density::kg_m3(1380.0),
        yield_strength: Pressure::mpa(1100.0),
        ultimate_tensile: Pressure::mpa(1400.0),
        elastic_modulus: Pressure::gpa(76.0),
        poissons_ratio: Dimensionless::ratio(0.34),
        thermal_conductivity: ThermalConductivity::w_mk(2.5),
        cte: CTE::um_mk(-2.0),                          // negative CTE in fiber direction
        specific_heat: SpecificHeat::j_kgk(1400.0),
        melting_point: Temperature::celsius(180.0),       // epoxy Tg, Kevlar decomposes ~450 °C
        hardness: 50.0,
        hardness_scale: HardnessScale::Brinell,
        fatigue_endurance: Pressure::mpa(600.0),
        machinability_index: 50.0,                       // fuzzes, needs special cutters
        source: "DuPont Kevlar 49 technical guide, MIL-HDBK-17",
    },

    // Aramid / epoxy woven fabric, balanced weave, Vf~55%.
    // Source: ASM Handbook Vol 21, DuPont Kevlar 49 technical guide
    Material {
        id: "Aramid-Woven",
        name: "Aramid / Epoxy Woven (balanced)",
        category: MaterialCategory::Composite,
        density: Density::kg_m3(1330.0),
        yield_strength: Pressure::mpa(350.0),
        ultimate_tensile: Pressure::mpa(480.0),
        elastic_modulus: Pressure::gpa(35.0),
        poissons_ratio: Dimensionless::ratio(0.15),
        thermal_conductivity: ThermalConductivity::w_mk(1.8),
        cte: CTE::um_mk(0.5),                           // balanced, near-zero
        specific_heat: SpecificHeat::j_kgk(1300.0),
        melting_point: Temperature::celsius(180.0),
        hardness: 45.0,
        hardness_scale: HardnessScale::Brinell,
        fatigue_endurance: Pressure::mpa(200.0),
        machinability_index: 48.0,
        source: "ASM Handbook Vol 21, DuPont Kevlar 49 technical guide",
    },

    // Kevlar 49 / carbon hybrid, 50/50 fiber ratio, woven fabric / epoxy.
    // Source: Hexcel hybrid fabric datasheets, CMH-17
    Material {
        id: "K49-CF-Hybrid",
        name: "Kevlar 49 / Epoxy Hybrid with Carbon (woven)",
        category: MaterialCategory::Composite,
        density: Density::kg_m3(1450.0),
        yield_strength: Pressure::mpa(450.0),
        ultimate_tensile: Pressure::mpa(620.0),
        elastic_modulus: Pressure::gpa(55.0),
        poissons_ratio: Dimensionless::ratio(0.20),
        thermal_conductivity: ThermalConductivity::w_mk(2.8),
        cte: CTE::um_mk(1.0),                           // hybrid balances negative CF + negative K49
        specific_heat: SpecificHeat::j_kgk(1100.0),
        melting_point: Temperature::celsius(180.0),
        hardness: 55.0,
        hardness_scale: HardnessScale::Brinell,
        fatigue_endurance: Pressure::mpa(270.0),
        machinability_index: 45.0,
        source: "Hexcel hybrid fabric datasheets, CMH-17",
    },

    // -----------------------------------------------------------------------
    // Basalt Fiber composite
    // -----------------------------------------------------------------------

    // Basalt fiber / epoxy, Vf~60%, unidirectional.
    // Source: Kamenny Vek / Mafic datasheets, Fiore et al. "Basalt fibre composites" review
    Material {
        id: "BF-Epoxy",
        name: "Basalt Fiber / Epoxy Unidirectional",
        category: MaterialCategory::Composite,
        density: Density::kg_m3(2100.0),
        yield_strength: Pressure::mpa(850.0),
        ultimate_tensile: Pressure::mpa(1150.0),
        elastic_modulus: Pressure::gpa(50.0),
        poissons_ratio: Dimensionless::ratio(0.26),
        thermal_conductivity: ThermalConductivity::w_mk(1.0),
        cte: CTE::um_mk(6.5),
        specific_heat: SpecificHeat::j_kgk(840.0),
        melting_point: Temperature::celsius(180.0),
        hardness: 55.0,
        hardness_scale: HardnessScale::Brinell,
        fatigue_endurance: Pressure::mpa(400.0),
        machinability_index: 50.0,
        source: "Kamenny Vek datasheets, Fiore et al. Composites Part B (2015)",
    },

    // -----------------------------------------------------------------------
    // Natural Fiber composites
    // -----------------------------------------------------------------------

    // Flax / epoxy unidirectional, Vf~40-45%. Low-density, sustainable alternative.
    // Source: ASM Handbook Vol 21, Bos et al. Composites Part A (2006), Shah review (2013)
    Material {
        id: "Flax-Epoxy",
        name: "Flax Fiber / Epoxy Unidirectional",
        category: MaterialCategory::Composite,
        density: Density::kg_m3(1250.0),
        yield_strength: Pressure::mpa(200.0),
        ultimate_tensile: Pressure::mpa(280.0),
        elastic_modulus: Pressure::gpa(28.0),
        poissons_ratio: Dimensionless::ratio(0.35),
        thermal_conductivity: ThermalConductivity::w_mk(0.6),
        cte: CTE::um_mk(8.0),
        specific_heat: SpecificHeat::j_kgk(1300.0),
        melting_point: Temperature::celsius(180.0),       // epoxy Tg; fiber chars >200 °C
        hardness: 30.0,
        hardness_scale: HardnessScale::Brinell,
        fatigue_endurance: Pressure::mpa(100.0),
        machinability_index: 65.0,
        source: "ASM Handbook Vol 21, Bos et al. Composites Part A (2006)",
    },

    // Hemp / epoxy, woven or UD, Vf~35-40%. Similar to flax but slightly lower stiffness.
    // Source: Wambua et al. Composites Science & Tech (2003), ASM Handbook Vol 21
    Material {
        id: "Hemp-Epoxy",
        name: "Hemp Fiber / Epoxy",
        category: MaterialCategory::Composite,
        density: Density::kg_m3(1220.0),
        yield_strength: Pressure::mpa(150.0),
        ultimate_tensile: Pressure::mpa(220.0),
        elastic_modulus: Pressure::gpa(18.0),
        poissons_ratio: Dimensionless::ratio(0.35),
        thermal_conductivity: ThermalConductivity::w_mk(0.5),
        cte: CTE::um_mk(10.0),
        specific_heat: SpecificHeat::j_kgk(1350.0),
        melting_point: Temperature::celsius(180.0),
        hardness: 28.0,
        hardness_scale: HardnessScale::Brinell,
        fatigue_endurance: Pressure::mpa(80.0),
        machinability_index: 68.0,
        source: "Wambua et al. Composites Sci & Tech 63 (2003), ASM Handbook Vol 21",
    },

    // -----------------------------------------------------------------------
    // Short Fiber Reinforced Thermoplastics (SFRTs)
    // -----------------------------------------------------------------------

    // Short carbon fiber / PA6 (polyamide 6), 30 wt% CF. Injection-moldable structural grade.
    // Source: BASF Ultramid® C3K, Toray Amilan® CF, ASM Handbook Vol 21
    Material {
        id: "CF-PA6-30",
        name: "Short Carbon Fiber / PA6 (30% fill, CF-PA6)",
        category: MaterialCategory::Composite,
        density: Density::kg_m3(1240.0),
        yield_strength: Pressure::mpa(175.0),
        ultimate_tensile: Pressure::mpa(200.0),
        elastic_modulus: Pressure::gpa(16.0),
        poissons_ratio: Dimensionless::ratio(0.38),
        thermal_conductivity: ThermalConductivity::w_mk(3.0),   // improved vs neat PA6
        cte: CTE::um_mk(20.0),                          // flow direction ~15, cross ~30
        specific_heat: SpecificHeat::j_kgk(1300.0),
        melting_point: Temperature::celsius(220.0),       // PA6 Tm (Tg ~50 °C dry)
        hardness: 85.0,
        hardness_scale: HardnessScale::Brinell,
        fatigue_endurance: Pressure::mpa(70.0),
        machinability_index: 60.0,
        source: "BASF Ultramid C3K, Toray Amilan CF, ASM Handbook Vol 21",
    },

    // Short glass fiber / PA66 (polyamide 66), 30 wt% GF. Workhorse engineering thermoplastic.
    // Source: DuPont Zytel 70G30L, ASM Handbook Vol 21, CAMPUS database
    Material {
        id: "GF-PA66-30",
        name: "Short Glass Fiber / PA66 (30% fill, GF-PA66)",
        category: MaterialCategory::Composite,
        density: Density::kg_m3(1380.0),
        yield_strength: Pressure::mpa(165.0),
        ultimate_tensile: Pressure::mpa(185.0),
        elastic_modulus: Pressure::gpa(9.5),
        poissons_ratio: Dimensionless::ratio(0.38),
        thermal_conductivity: ThermalConductivity::w_mk(0.5),
        cte: CTE::um_mk(25.0),                          // flow direction ~20, cross ~50
        specific_heat: SpecificHeat::j_kgk(1400.0),
        melting_point: Temperature::celsius(262.0),       // PA66 Tm (Tg ~70 °C dry)
        hardness: 80.0,
        hardness_scale: HardnessScale::Brinell,
        fatigue_endurance: Pressure::mpa(65.0),
        machinability_index: 65.0,
        source: "DuPont Zytel 70G30L, ASM Handbook Vol 21, CAMPUS database",
    },

    // Short glass fiber / PBT (polybutylene terephthalate), 30 wt% GF.
    // Source: BASF Ultradur® B4300 G6, ASM Handbook Vol 21, CAMPUS database
    Material {
        id: "GF-PBT-30",
        name: "Short Glass Fiber / PBT (30% fill, GF-PBT)",
        category: MaterialCategory::Composite,
        density: Density::kg_m3(1520.0),
        yield_strength: Pressure::mpa(130.0),
        ultimate_tensile: Pressure::mpa(145.0),
        elastic_modulus: Pressure::gpa(9.0),
        poissons_ratio: Dimensionless::ratio(0.38),
        thermal_conductivity: ThermalConductivity::w_mk(0.4),
        cte: CTE::um_mk(30.0),
        specific_heat: SpecificHeat::j_kgk(1300.0),
        melting_point: Temperature::celsius(225.0),       // PBT Tm (Tg ~50 °C)
        hardness: 75.0,
        hardness_scale: HardnessScale::Brinell,
        fatigue_endurance: Pressure::mpa(50.0),
        machinability_index: 63.0,
        source: "BASF Ultradur B4300 G6, ASM Handbook Vol 21, CAMPUS database",
    },

    // Short carbon fiber / PEEK, 30 wt% CF. High-performance injection-moldable grade.
    // Source: Victrex 150CA30, Solvay KetaSpire® KT-820 CF30, ASM Handbook Vol 21
    Material {
        id: "CF-PEEK-30",
        name: "Short Carbon Fiber / PEEK (30% fill, CF-PEEK)",
        category: MaterialCategory::Composite,
        density: Density::kg_m3(1440.0),
        yield_strength: Pressure::mpa(200.0),
        ultimate_tensile: Pressure::mpa(220.0),
        elastic_modulus: Pressure::gpa(16.0),
        poissons_ratio: Dimensionless::ratio(0.38),
        thermal_conductivity: ThermalConductivity::w_mk(3.5),
        cte: CTE::um_mk(18.0),
        specific_heat: SpecificHeat::j_kgk(950.0),
        melting_point: Temperature::celsius(343.0),       // PEEK Tm (Tg ~143 °C)
        hardness: 90.0,
        hardness_scale: HardnessScale::Brinell,
        fatigue_endurance: Pressure::mpa(80.0),
        machinability_index: 55.0,
        source: "Victrex 150CA30, Solvay KetaSpire KT-820 CF30, ASM Handbook Vol 21",
    },

    // -----------------------------------------------------------------------
    // Carbon Fiber / Epoxy — High Modulus & Ultra-High Modulus
    // -----------------------------------------------------------------------

    // M55J class high-modulus carbon fiber / epoxy, unidirectional, Vf~60%.
    // Source: Toray M55J datasheet, CMH-17
    Material {
        id: "CF-HM-Uni",
        name: "Carbon Fiber / Epoxy High Modulus (M55J, 0°)",
        category: MaterialCategory::Composite,
        density: Density::kg_m3(1650.0),
        yield_strength: Pressure::mpa(1400.0),
        ultimate_tensile: Pressure::mpa(1850.0),
        elastic_modulus: Pressure::gpa(300.0),
        poissons_ratio: Dimensionless::ratio(0.28),
        thermal_conductivity: ThermalConductivity::w_mk(70.0),
        cte: CTE::um_mk(-0.7),
        specific_heat: SpecificHeat::j_kgk(850.0),
        melting_point: Temperature::celsius(180.0),
        hardness: 75.0,
        hardness_scale: HardnessScale::Brinell,
        fatigue_endurance: Pressure::mpa(900.0),
        machinability_index: 40.0,
        source: "Toray M55J datasheet, CMH-17",
    },

    // M60J ultra-high-modulus carbon fiber / epoxy, unidirectional.
    // Source: Toray M60J datasheet, CMH-17
    Material {
        id: "CF-UHM-Uni",
        name: "Carbon Fiber / Epoxy Ultra-High Modulus (M60J, 0°)",
        category: MaterialCategory::Composite,
        density: Density::kg_m3(1700.0),
        yield_strength: Pressure::mpa(1100.0),
        ultimate_tensile: Pressure::mpa(1500.0),
        elastic_modulus: Pressure::gpa(390.0),
        poissons_ratio: Dimensionless::ratio(0.27),
        thermal_conductivity: ThermalConductivity::w_mk(120.0),
        cte: CTE::um_mk(-1.0),
        specific_heat: SpecificHeat::j_kgk(820.0),
        melting_point: Temperature::celsius(180.0),
        hardness: 78.0,
        hardness_scale: HardnessScale::Brinell,
        fatigue_endurance: Pressure::mpa(700.0),
        machinability_index: 38.0,
        source: "Toray M60J datasheet, CMH-17",
    },

    // T1100G intermediate-modulus, ultra-high-strength. Next-gen aerospace fiber.
    // Source: Toray T1100G datasheet
    Material {
        id: "CF-T1100-Uni",
        name: "Carbon Fiber / Epoxy T1100G (IM, ultra-high strength, 0°)",
        category: MaterialCategory::Composite,
        density: Density::kg_m3(1600.0),
        yield_strength: Pressure::mpa(2200.0),
        ultimate_tensile: Pressure::mpa(3000.0),
        elastic_modulus: Pressure::gpa(175.0),
        poissons_ratio: Dimensionless::ratio(0.30),
        thermal_conductivity: ThermalConductivity::w_mk(8.0),
        cte: CTE::um_mk(-0.3),
        specific_heat: SpecificHeat::j_kgk(900.0),
        melting_point: Temperature::celsius(180.0),
        hardness: 72.0,
        hardness_scale: HardnessScale::Brinell,
        fatigue_endurance: Pressure::mpa(1500.0),
        machinability_index: 42.0,
        source: "Toray T1100G datasheet",
    },

    // Spread-tow thin-ply carbon / epoxy, quasi-isotropic. Thinner plies improve first-ply failure.
    // Source: North Thin Ply Technology (NTPT) datasheets, Sihn et al. Composites Sci & Tech (2007)
    Material {
        id: "CF-SpreadTow-QI",
        name: "Carbon Fiber / Epoxy Spread-Tow Thin-Ply QI",
        category: MaterialCategory::Composite,
        density: Density::kg_m3(1560.0),
        yield_strength: Pressure::mpa(450.0),
        ultimate_tensile: Pressure::mpa(700.0),
        elastic_modulus: Pressure::gpa(48.0),
        poissons_ratio: Dimensionless::ratio(0.31),
        thermal_conductivity: ThermalConductivity::w_mk(3.5),
        cte: CTE::um_mk(2.0),
        specific_heat: SpecificHeat::j_kgk(900.0),
        melting_point: Temperature::celsius(180.0),
        hardness: 66.0,
        hardness_scale: HardnessScale::Brinell,
        fatigue_endurance: Pressure::mpa(320.0),
        machinability_index: 44.0,
        source: "NTPT datasheets, Sihn et al. Composites Sci & Tech 67 (2007)",
    },

    // Carbon / epoxy prepreg with cyanate ester matrix — very low moisture uptake, space-grade.
    // Source: Tencate RS-3 Cyanate Ester datasheet, CMH-17
    Material {
        id: "CF-CE",
        name: "Carbon Fiber / Cyanate Ester (space-grade, 0°)",
        category: MaterialCategory::Composite,
        density: Density::kg_m3(1580.0),
        yield_strength: Pressure::mpa(1450.0),
        ultimate_tensile: Pressure::mpa(2100.0),
        elastic_modulus: Pressure::gpa(135.0),
        poissons_ratio: Dimensionless::ratio(0.30),
        thermal_conductivity: ThermalConductivity::w_mk(5.5),
        cte: CTE::um_mk(-0.1),
        specific_heat: SpecificHeat::j_kgk(910.0),
        melting_point: Temperature::celsius(250.0), // cyanate ester Tg ~250 °C
        hardness: 72.0,
        hardness_scale: HardnessScale::Brinell,
        fatigue_endurance: Pressure::mpa(1050.0),
        machinability_index: 43.0,
        source: "Tencate RS-3 Cyanate Ester datasheet, CMH-17",
    },

    // Carbon / PPS thermoplastic, woven fabric. Recyclable, chemical-resistant.
    // Source: Tencate Cetex TC1100 PPS datasheet
    Material {
        id: "CF-PPS-Woven",
        name: "Carbon Fiber / PPS Woven (thermoplastic)",
        category: MaterialCategory::Composite,
        density: Density::kg_m3(1560.0),
        yield_strength: Pressure::mpa(500.0),
        ultimate_tensile: Pressure::mpa(650.0),
        elastic_modulus: Pressure::gpa(58.0),
        poissons_ratio: Dimensionless::ratio(0.08),
        thermal_conductivity: ThermalConductivity::w_mk(3.0),
        cte: CTE::um_mk(2.5),
        specific_heat: SpecificHeat::j_kgk(950.0),
        melting_point: Temperature::celsius(280.0), // PPS Tm
        hardness: 65.0,
        hardness_scale: HardnessScale::Brinell,
        fatigue_endurance: Pressure::mpa(280.0),
        machinability_index: 44.0,
        source: "Tencate Cetex TC1100 PPS datasheet",
    },

    // -----------------------------------------------------------------------
    // Aramid composites (additional)
    // -----------------------------------------------------------------------

    // Kevlar 29 / epoxy — lower modulus but excellent energy absorption. Armor / ballistic.
    // Source: DuPont Kevlar 29 technical guide, MIL-HDBK-17
    Material {
        id: "K29-Epoxy",
        name: "Kevlar 29 / Epoxy Unidirectional (ballistic)",
        category: MaterialCategory::Composite,
        density: Density::kg_m3(1350.0),
        yield_strength: Pressure::mpa(900.0),
        ultimate_tensile: Pressure::mpa(1200.0),
        elastic_modulus: Pressure::gpa(60.0),
        poissons_ratio: Dimensionless::ratio(0.35),
        thermal_conductivity: ThermalConductivity::w_mk(2.0),
        cte: CTE::um_mk(-3.0),
        specific_heat: SpecificHeat::j_kgk(1400.0),
        melting_point: Temperature::celsius(180.0),
        hardness: 48.0,
        hardness_scale: HardnessScale::Brinell,
        fatigue_endurance: Pressure::mpa(500.0),
        machinability_index: 48.0,
        source: "DuPont Kevlar 29 technical guide, MIL-HDBK-17",
    },

    // Twaron / epoxy woven (1000 series). European aramid equivalent.
    // Source: Teijin Twaron 1000 datasheet, ASM Handbook Vol 21
    Material {
        id: "Twaron-Woven",
        name: "Twaron / Epoxy Woven (1000 series)",
        category: MaterialCategory::Composite,
        density: Density::kg_m3(1340.0),
        yield_strength: Pressure::mpa(360.0),
        ultimate_tensile: Pressure::mpa(490.0),
        elastic_modulus: Pressure::gpa(36.0),
        poissons_ratio: Dimensionless::ratio(0.15),
        thermal_conductivity: ThermalConductivity::w_mk(1.7),
        cte: CTE::um_mk(0.5),
        specific_heat: SpecificHeat::j_kgk(1300.0),
        melting_point: Temperature::celsius(180.0),
        hardness: 44.0,
        hardness_scale: HardnessScale::Brinell,
        fatigue_endurance: Pressure::mpa(200.0),
        machinability_index: 48.0,
        source: "Teijin Twaron 1000 datasheet, ASM Handbook Vol 21",
    },

    // -----------------------------------------------------------------------
    // Natural Fiber composites (additional)
    // -----------------------------------------------------------------------

    // Jute / epoxy woven, Vf~35%. Automotive interior panels.
    // Source: Shah Composites Sci & Tech 73 (2013), ASM Handbook Vol 21
    Material {
        id: "Jute-Epoxy",
        name: "Jute Fiber / Epoxy Woven",
        category: MaterialCategory::Composite,
        density: Density::kg_m3(1200.0),
        yield_strength: Pressure::mpa(80.0),
        ultimate_tensile: Pressure::mpa(120.0),
        elastic_modulus: Pressure::gpa(12.0),
        poissons_ratio: Dimensionless::ratio(0.35),
        thermal_conductivity: ThermalConductivity::w_mk(0.4),
        cte: CTE::um_mk(12.0),
        specific_heat: SpecificHeat::j_kgk(1400.0),
        melting_point: Temperature::celsius(180.0),
        hardness: 28.0,
        hardness_scale: HardnessScale::Brinell,
        fatigue_endurance: Pressure::mpa(42.0),
        machinability_index: 68.0,
        source: "Shah Composites Sci & Tech 73 (2013), ASM Handbook Vol 21",
    },

    // Sisal / polypropylene injection-molded. Automotive under-body panels.
    // Source: Wambua et al. Composites Sci & Tech 63 (2003)
    Material {
        id: "Sisal-PP",
        name: "Sisal Fiber / PP (30% fill, injection-molded)",
        category: MaterialCategory::Composite,
        density: Density::kg_m3(1060.0),
        yield_strength: Pressure::mpa(35.0),
        ultimate_tensile: Pressure::mpa(55.0),
        elastic_modulus: Pressure::gpa(4.5),
        poissons_ratio: Dimensionless::ratio(0.38),
        thermal_conductivity: ThermalConductivity::w_mk(0.3),
        cte: CTE::um_mk(50.0),
        specific_heat: SpecificHeat::j_kgk(1600.0),
        melting_point: Temperature::celsius(165.0),
        hardness: 25.0,
        hardness_scale: HardnessScale::Brinell,
        fatigue_endurance: Pressure::mpa(18.0),
        machinability_index: 72.0,
        source: "Wambua et al. Composites Sci & Tech 63 (2003)",
    },

    // -----------------------------------------------------------------------
    // Metal Matrix Composites (MMC)
    // -----------------------------------------------------------------------

    // Al-SiC (aluminum with silicon carbide particles), 20 vol% SiC.
    // Source: Duralcan F3S.20S datasheet, ASM Handbook Vol 21
    Material {
        id: "Al-SiC-20",
        name: "Aluminum / SiC MMC (20 vol%, particulate)",
        category: MaterialCategory::Composite,
        density: Density::kg_m3(2770.0),
        yield_strength: Pressure::mpa(310.0),
        ultimate_tensile: Pressure::mpa(380.0),
        elastic_modulus: Pressure::gpa(100.0),
        poissons_ratio: Dimensionless::ratio(0.30),
        thermal_conductivity: ThermalConductivity::w_mk(170.0),
        cte: CTE::um_mk(16.0),
        specific_heat: SpecificHeat::j_kgk(850.0),
        melting_point: Temperature::celsius(620.0), // Al matrix solidus
        hardness: 120.0,
        hardness_scale: HardnessScale::Brinell,
        fatigue_endurance: Pressure::mpa(150.0),
        machinability_index: 30.0,
        source: "Duralcan F3S.20S datasheet, ASM Handbook Vol 21",
    },

    // Al-SiC 40 vol% — higher stiffness, electronics thermal management.
    // Source: CPS Technologies AlSiC datasheet, MatWeb
    Material {
        id: "Al-SiC-40",
        name: "Aluminum / SiC MMC (40 vol%, particulate)",
        category: MaterialCategory::Composite,
        density: Density::kg_m3(2900.0),
        yield_strength: Pressure::mpa(250.0),
        ultimate_tensile: Pressure::mpa(320.0),
        elastic_modulus: Pressure::gpa(145.0),
        poissons_ratio: Dimensionless::ratio(0.27),
        thermal_conductivity: ThermalConductivity::w_mk(190.0),
        cte: CTE::um_mk(10.5),
        specific_heat: SpecificHeat::j_kgk(800.0),
        melting_point: Temperature::celsius(620.0),
        hardness: 150.0,
        hardness_scale: HardnessScale::Brinell,
        fatigue_endurance: Pressure::mpa(130.0),
        machinability_index: 22.0,
        source: "CPS Technologies AlSiC datasheet, MatWeb",
    },

    // Ti-SiC (titanium matrix, continuous SiC fiber). Aerospace engine parts.
    // Source: DERA / Rolls-Royce Ti-6Al-4V/SCS-6 data, ASM Handbook Vol 21
    Material {
        id: "Ti-SiC",
        name: "Titanium / SiC MMC (35 vol%, continuous fiber)",
        category: MaterialCategory::Composite,
        density: Density::kg_m3(3900.0),
        yield_strength: Pressure::mpa(1400.0),
        ultimate_tensile: Pressure::mpa(1700.0),
        elastic_modulus: Pressure::gpa(210.0),
        poissons_ratio: Dimensionless::ratio(0.28),
        thermal_conductivity: ThermalConductivity::w_mk(18.0),
        cte: CTE::um_mk(6.0),
        specific_heat: SpecificHeat::j_kgk(580.0),
        melting_point: Temperature::celsius(1650.0), // Ti alloy melting
        hardness: 350.0,
        hardness_scale: HardnessScale::Brinell,
        fatigue_endurance: Pressure::mpa(700.0),
        machinability_index: 15.0,
        source: "DERA Ti-6Al-4V/SCS-6 data, ASM Handbook Vol 21",
    },

    // Al-Al2O3 (aluminum oxide particle reinforced), Saffil short fiber.
    // Source: 3M Nextel 610 / Al MMC data, ASM Handbook Vol 21
    Material {
        id: "Al-Al2O3",
        name: "Aluminum / Al₂O₃ MMC (20 vol%, Saffil fiber)",
        category: MaterialCategory::Composite,
        density: Density::kg_m3(2850.0),
        yield_strength: Pressure::mpa(280.0),
        ultimate_tensile: Pressure::mpa(340.0),
        elastic_modulus: Pressure::gpa(95.0),
        poissons_ratio: Dimensionless::ratio(0.31),
        thermal_conductivity: ThermalConductivity::w_mk(150.0),
        cte: CTE::um_mk(17.0),
        specific_heat: SpecificHeat::j_kgk(870.0),
        melting_point: Temperature::celsius(620.0),
        hardness: 110.0,
        hardness_scale: HardnessScale::Brinell,
        fatigue_endurance: Pressure::mpa(130.0),
        machinability_index: 28.0,
        source: "3M Nextel 610 / Al MMC data, ASM Handbook Vol 21",
    },

    // -----------------------------------------------------------------------
    // Ceramic Matrix Composites (CMC)
    // -----------------------------------------------------------------------

    // SiC/SiC (silicon carbide fiber in SiC matrix). Jet engine hot section.
    // Source: GE CMCS (Hi-Nicalon S fiber), ASM Handbook Vol 21
    Material {
        id: "SiC-SiC",
        name: "SiC/SiC Ceramic Matrix Composite",
        category: MaterialCategory::Composite,
        density: Density::kg_m3(2500.0),
        yield_strength: Pressure::mpa(250.0),
        ultimate_tensile: Pressure::mpa(310.0),
        elastic_modulus: Pressure::gpa(230.0),
        poissons_ratio: Dimensionless::ratio(0.18),
        thermal_conductivity: ThermalConductivity::w_mk(15.0),
        cte: CTE::um_mk(4.5),
        specific_heat: SpecificHeat::j_kgk(700.0),
        melting_point: Temperature::celsius(1400.0), // max continuous use temp
        hardness: 2000.0,
        hardness_scale: HardnessScale::Vickers,
        fatigue_endurance: Pressure::mpa(150.0),
        machinability_index: 8.0,
        source: "GE CMC Hi-Nicalon S data, ASM Handbook Vol 21",
    },

    // Oxide/Oxide CMC (Nextel 720 fiber in alumina-mullite matrix). No oxidation issue.
    // Source: 3M Nextel 720 / COI Ceramics data, ASM Handbook Vol 21
    Material {
        id: "Ox-Ox-CMC",
        name: "Oxide/Oxide CMC (Nextel 720 / alumina matrix)",
        category: MaterialCategory::Composite,
        density: Density::kg_m3(2800.0),
        yield_strength: Pressure::mpa(170.0),
        ultimate_tensile: Pressure::mpa(210.0),
        elastic_modulus: Pressure::gpa(80.0),
        poissons_ratio: Dimensionless::ratio(0.22),
        thermal_conductivity: ThermalConductivity::w_mk(3.0),
        cte: CTE::um_mk(6.0),
        specific_heat: SpecificHeat::j_kgk(800.0),
        melting_point: Temperature::celsius(1200.0), // max use temp
        hardness: 800.0,
        hardness_scale: HardnessScale::Vickers,
        fatigue_endurance: Pressure::mpa(90.0),
        machinability_index: 10.0,
        source: "3M Nextel 720 data, COI Ceramics, ASM Handbook Vol 21",
    },

    // C/SiC (carbon fiber in SiC matrix). Brake discs, rocket nozzles.
    // Source: SGL Carbon SIGRASIC datasheet, DLR C/SiC data
    Material {
        id: "C-SiC",
        name: "C/SiC Ceramic Matrix Composite (brake grade)",
        category: MaterialCategory::Composite,
        density: Density::kg_m3(1900.0),
        yield_strength: Pressure::mpa(200.0),
        ultimate_tensile: Pressure::mpa(250.0),
        elastic_modulus: Pressure::gpa(60.0),
        poissons_ratio: Dimensionless::ratio(0.15),
        thermal_conductivity: ThermalConductivity::w_mk(25.0),
        cte: CTE::um_mk(2.5),
        specific_heat: SpecificHeat::j_kgk(750.0),
        melting_point: Temperature::celsius(1600.0), // max use in oxidizing atmosphere
        hardness: 1200.0,
        hardness_scale: HardnessScale::Vickers,
        fatigue_endurance: Pressure::mpa(120.0),
        machinability_index: 8.0,
        source: "SGL Carbon SIGRASIC datasheet, DLR C/SiC data",
    },

    // C/C (carbon/carbon). Extreme temperature. Re-entry vehicles, brake discs.
    // Source: SGL Carbon SIGRABOND datasheet, ASM Handbook Vol 21
    Material {
        id: "C-C",
        name: "C/C Carbon-Carbon Composite",
        category: MaterialCategory::Composite,
        density: Density::kg_m3(1700.0),
        yield_strength: Pressure::mpa(100.0),
        ultimate_tensile: Pressure::mpa(180.0),
        elastic_modulus: Pressure::gpa(70.0),
        poissons_ratio: Dimensionless::ratio(0.10),
        thermal_conductivity: ThermalConductivity::w_mk(100.0),
        cte: CTE::um_mk(1.0),
        specific_heat: SpecificHeat::j_kgk(710.0),
        melting_point: Temperature::celsius(3000.0), // sublimes; inert atmosphere only above 500 °C
        hardness: 500.0,
        hardness_scale: HardnessScale::Vickers,
        fatigue_endurance: Pressure::mpa(80.0),
        machinability_index: 12.0,
        source: "SGL Carbon SIGRABOND datasheet, ASM Handbook Vol 21",
    },

    // -----------------------------------------------------------------------
    // Sandwich Core Materials
    // -----------------------------------------------------------------------

    // Nomex honeycomb core (aramid paper), 48 kg/m3. Aircraft floors, fairings.
    // Source: Hexcel HRH-10 datasheet, MIL-C-81986
    Material {
        id: "Nomex-HC-48",
        name: "Nomex Honeycomb Core (48 kg/m3, 3.2mm cell)",
        category: MaterialCategory::Composite,
        density: Density::kg_m3(48.0),
        yield_strength: Pressure::mpa(1.2),      // bare compressive strength
        ultimate_tensile: Pressure::mpa(1.5),
        elastic_modulus: Pressure::gpa(0.070),
        poissons_ratio: Dimensionless::ratio(0.30),
        thermal_conductivity: ThermalConductivity::w_mk(0.040),
        cte: CTE::um_mk(3.0),
        specific_heat: SpecificHeat::j_kgk(1200.0),
        melting_point: Temperature::celsius(180.0), // max service temp (phenolic coated)
        hardness: 5.0,
        hardness_scale: HardnessScale::Brinell,
        fatigue_endurance: Pressure::mpa(0.5),
        machinability_index: 75.0,
        source: "Hexcel HRH-10 datasheet, MIL-C-81986",
    },

    // Nomex honeycomb core, 96 kg/m3 — higher density for higher loads.
    // Source: Hexcel HRH-10 datasheet
    Material {
        id: "Nomex-HC-96",
        name: "Nomex Honeycomb Core (96 kg/m3, 3.2mm cell)",
        category: MaterialCategory::Composite,
        density: Density::kg_m3(96.0),
        yield_strength: Pressure::mpa(3.5),
        ultimate_tensile: Pressure::mpa(4.2),
        elastic_modulus: Pressure::gpa(0.200),
        poissons_ratio: Dimensionless::ratio(0.30),
        thermal_conductivity: ThermalConductivity::w_mk(0.050),
        cte: CTE::um_mk(3.0),
        specific_heat: SpecificHeat::j_kgk(1200.0),
        melting_point: Temperature::celsius(180.0),
        hardness: 8.0,
        hardness_scale: HardnessScale::Brinell,
        fatigue_endurance: Pressure::mpa(1.5),
        machinability_index: 72.0,
        source: "Hexcel HRH-10 datasheet",
    },

    // Aluminum honeycomb core (5052 alloy), 72 kg/m3. Stiffest core option.
    // Source: Hexcel CR III datasheet, MIL-C-7438
    Material {
        id: "Al-HC-72",
        name: "Aluminum Honeycomb Core (5052, 72 kg/m3, 6.4mm cell)",
        category: MaterialCategory::Composite,
        density: Density::kg_m3(72.0),
        yield_strength: Pressure::mpa(3.1),
        ultimate_tensile: Pressure::mpa(4.1),
        elastic_modulus: Pressure::gpa(0.700),
        poissons_ratio: Dimensionless::ratio(0.33),
        thermal_conductivity: ThermalConductivity::w_mk(3.0),
        cte: CTE::um_mk(23.0),
        specific_heat: SpecificHeat::j_kgk(900.0),
        melting_point: Temperature::celsius(180.0), // limited by adhesive, not aluminum
        hardness: 10.0,
        hardness_scale: HardnessScale::Brinell,
        fatigue_endurance: Pressure::mpa(1.5),
        machinability_index: 68.0,
        source: "Hexcel CR III datasheet, MIL-C-7438",
    },

    // PVC structural foam core (Divinycell H80). Marine and wind energy.
    // Source: DIAB Divinycell H80 datasheet
    Material {
        id: "PVC-Core-H80",
        name: "PVC Foam Core (Divinycell H80, 80 kg/m3)",
        category: MaterialCategory::Composite,
        density: Density::kg_m3(80.0),
        yield_strength: Pressure::mpa(1.4),
        ultimate_tensile: Pressure::mpa(2.5),
        elastic_modulus: Pressure::gpa(0.090),
        poissons_ratio: Dimensionless::ratio(0.32),
        thermal_conductivity: ThermalConductivity::w_mk(0.033),
        cte: CTE::um_mk(35.0),
        specific_heat: SpecificHeat::j_kgk(1200.0),
        melting_point: Temperature::celsius(70.0), // service temp limit
        hardness: 5.0,
        hardness_scale: HardnessScale::Brinell,
        fatigue_endurance: Pressure::mpa(0.6),
        machinability_index: 85.0,
        source: "DIAB Divinycell H80 datasheet",
    },

    // PVC foam core, higher density grade H130.
    // Source: DIAB Divinycell H130 datasheet
    Material {
        id: "PVC-Core-H130",
        name: "PVC Foam Core (Divinycell H130, 130 kg/m3)",
        category: MaterialCategory::Composite,
        density: Density::kg_m3(130.0),
        yield_strength: Pressure::mpa(2.8),
        ultimate_tensile: Pressure::mpa(4.0),
        elastic_modulus: Pressure::gpa(0.170),
        poissons_ratio: Dimensionless::ratio(0.32),
        thermal_conductivity: ThermalConductivity::w_mk(0.038),
        cte: CTE::um_mk(35.0),
        specific_heat: SpecificHeat::j_kgk(1200.0),
        melting_point: Temperature::celsius(70.0),
        hardness: 8.0,
        hardness_scale: HardnessScale::Brinell,
        fatigue_endurance: Pressure::mpa(1.2),
        machinability_index: 82.0,
        source: "DIAB Divinycell H130 datasheet",
    },

    // Balsa wood end-grain core. Highest shear strength per density for cores.
    // Source: DIAB ProBalsa datasheet, Gurit Balsaflex
    Material {
        id: "Balsa-Core",
        name: "Balsa End-Grain Core (150 kg/m3)",
        category: MaterialCategory::Composite,
        density: Density::kg_m3(150.0),
        yield_strength: Pressure::mpa(5.5),       // compressive, L direction
        ultimate_tensile: Pressure::mpa(8.0),
        elastic_modulus: Pressure::gpa(0.300),
        poissons_ratio: Dimensionless::ratio(0.30),
        thermal_conductivity: ThermalConductivity::w_mk(0.050),
        cte: CTE::um_mk(5.0),
        specific_heat: SpecificHeat::j_kgk(2900.0),
        melting_point: Temperature::celsius(150.0), // resin cure limit; wood chars ~200 °C
        hardness: 12.0,
        hardness_scale: HardnessScale::Brinell,
        fatigue_endurance: Pressure::mpa(2.5),
        machinability_index: 80.0,
        source: "DIAB ProBalsa datasheet, Gurit Balsaflex",
    },

    // -----------------------------------------------------------------------
    // UHMWPE Fiber Composites
    // -----------------------------------------------------------------------

    // Dyneema / polyethylene UD cross-ply (Dyneema HB80). Ballistic armor.
    // Source: DSM Dyneema HB80 datasheet
    Material {
        id: "UHMWPE-UD",
        name: "UHMWPE / PE Unidirectional Cross-Ply (Dyneema HB80)",
        category: MaterialCategory::Composite,
        density: Density::kg_m3(970.0),
        yield_strength: Pressure::mpa(600.0),
        ultimate_tensile: Pressure::mpa(800.0),
        elastic_modulus: Pressure::gpa(60.0),
        poissons_ratio: Dimensionless::ratio(0.30),
        thermal_conductivity: ThermalConductivity::w_mk(0.6),
        cte: CTE::um_mk(-12.0),  // negative CTE in fiber direction
        specific_heat: SpecificHeat::j_kgk(1850.0),
        melting_point: Temperature::celsius(145.0), // PE fiber melting
        hardness: 40.0,
        hardness_scale: HardnessScale::Brinell,
        fatigue_endurance: Pressure::mpa(320.0),
        machinability_index: 55.0,
        source: "DSM Dyneema HB80 datasheet",
    },

    // -----------------------------------------------------------------------
    // High-Temperature Glass Fiber
    // -----------------------------------------------------------------------

    // R-glass / epoxy unidirectional. Higher temperature capability than E-glass.
    // Source: Owens Corning R-glass datasheet, MIL-HDBK-17
    Material {
        id: "RG-Epoxy",
        name: "R-Glass / Epoxy Unidirectional",
        category: MaterialCategory::Composite,
        density: Density::kg_m3(2050.0),
        yield_strength: Pressure::mpa(950.0),
        ultimate_tensile: Pressure::mpa(1350.0),
        elastic_modulus: Pressure::gpa(52.0),
        poissons_ratio: Dimensionless::ratio(0.28),
        thermal_conductivity: ThermalConductivity::w_mk(1.2),
        cte: CTE::um_mk(5.5),
        specific_heat: SpecificHeat::j_kgk(850.0),
        melting_point: Temperature::celsius(180.0),
        hardness: 58.0,
        hardness_scale: HardnessScale::Brinell,
        fatigue_endurance: Pressure::mpa(450.0),
        machinability_index: 52.0,
        source: "Owens Corning R-glass datasheet, MIL-HDBK-17",
    },
];
