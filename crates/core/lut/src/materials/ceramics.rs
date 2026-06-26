//! Technical ceramic material property lookup tables.
//!
//! Structural and engineering ceramics for high-temperature, wear, and
//! corrosion applications. Very high stiffness, hardness, and melting points.
//! Brittle — no distinct yield point; yield ≈ ultimate for design purposes.
//! Machinability is very low (diamond grinding required).
//!
//!
//! Sources: CeramTec datasheets, CoorsTek, ASM Handbook Vol 4 (Ceramics),
//! MatWeb, NIST Structural Ceramics Database.

use physical_units::*;
use super::{Material, MaterialCategory, HardnessScale};

pub static CERAMICS: &[Material] = &[
    // -----------------------------------------------------------------------
    // Alumina (Aluminum Oxide, Al₂O₃)
    // -----------------------------------------------------------------------

    // High-purity alumina, 99.9%. Premium substrate and optical ceramic.
    // Source: CoorsTek AD-999, Kyocera A-479, ASM Handbook Vol 4
    Material {
        id: "Al2O3-999",
        name: "Alumina (Al₂O₃ 99.9%, high purity)",
        category: MaterialCategory::Ceramic,
        density: Density::kg_m3(3980.0),
        yield_strength: Pressure::mpa(380.0),             // flexural strength
        ultimate_tensile: Pressure::mpa(380.0),
        elastic_modulus: Pressure::gpa(400.0),
        poissons_ratio: Dimensionless::ratio(0.22),
        thermal_conductivity: ThermalConductivity::w_mk(35.0),
        cte: CTE::um_mk(8.0),
        specific_heat: SpecificHeat::j_kgk(880.0),
        melting_point: Temperature::celsius(2072.0),
        hardness: 1800.0,
        hardness_scale: HardnessScale::Vickers,
        fatigue_endurance: Pressure::mpa(170.0),           // ~45% UTS
        machinability_index: 12.0,
        source: "CoorsTek AD-999, Kyocera A-479, ASM Handbook Vol 4",
    },

    // High-purity alumina, 99.5%. Premium structural ceramic.
    // Source: CeramTec Rubalit 710, CoorsTek AD-995, MatWeb
    Material {
        id: "Al2O3-995",
        name: "Alumina (Al₂O₃ 99.5%)",
        category: MaterialCategory::Ceramic,
        density: Density::kg_m3(3900.0),
        yield_strength: Pressure::mpa(310.0),     // flexural / no distinct yield
        ultimate_tensile: Pressure::mpa(310.0),    // tensile strength ≈ yield (brittle)
        elastic_modulus: Pressure::gpa(380.0),
        poissons_ratio: Dimensionless::ratio(0.22),
        thermal_conductivity: ThermalConductivity::w_mk(30.0),
        cte: CTE::um_mk(8.1),
        specific_heat: SpecificHeat::j_kgk(880.0),
        melting_point: Temperature::celsius(2050.0),
        hardness: 1700.0,                          // HV (Vickers)
        hardness_scale: HardnessScale::Vickers,
        fatigue_endurance: Pressure::mpa(140.0),    // ~45% UTS
        machinability_index: 15.0,                  // diamond grinding only
        source: "CeramTec Rubalit 710, CoorsTek AD-995, MatWeb",
    },

    // 96% alumina — more economical, slightly lower properties.
    // Source: CoorsTek AD-96, ASM Handbook Vol 4
    Material {
        id: "Al2O3-96",
        name: "Alumina (Al₂O₃ 96%)",
        category: MaterialCategory::Ceramic,
        density: Density::kg_m3(3720.0),
        yield_strength: Pressure::mpa(260.0),
        ultimate_tensile: Pressure::mpa(260.0),
        elastic_modulus: Pressure::gpa(340.0),
        poissons_ratio: Dimensionless::ratio(0.22),
        thermal_conductivity: ThermalConductivity::w_mk(24.0),
        cte: CTE::um_mk(8.2),
        specific_heat: SpecificHeat::j_kgk(880.0),
        melting_point: Temperature::celsius(1900.0),
        hardness: 1400.0,
        hardness_scale: HardnessScale::Vickers,
        fatigue_endurance: Pressure::mpa(115.0),
        machinability_index: 18.0,
        source: "CoorsTek AD-96, ASM Handbook Vol 4",
    },

    // 92% alumina — general purpose, widely available grade.
    // Source: CoorsTek AD-92, CeramTec Alotec 92, ASM Handbook Vol 4
    Material {
        id: "Al2O3-92",
        name: "Alumina (Al₂O₃ 92%, general purpose)",
        category: MaterialCategory::Ceramic,
        density: Density::kg_m3(3600.0),
        yield_strength: Pressure::mpa(220.0),
        ultimate_tensile: Pressure::mpa(220.0),
        elastic_modulus: Pressure::gpa(300.0),
        poissons_ratio: Dimensionless::ratio(0.22),
        thermal_conductivity: ThermalConductivity::w_mk(18.0),
        cte: CTE::um_mk(8.5),
        specific_heat: SpecificHeat::j_kgk(880.0),
        melting_point: Temperature::celsius(1800.0),
        hardness: 1100.0,
        hardness_scale: HardnessScale::Vickers,
        fatigue_endurance: Pressure::mpa(95.0),
        machinability_index: 20.0,
        source: "CoorsTek AD-92, CeramTec Alotec 92, ASM Handbook Vol 4",
    },

    // -----------------------------------------------------------------------
    // Silicon Carbide (SiC)
    // -----------------------------------------------------------------------

    // Sintered alpha-SiC. Extreme hardness and thermal conductivity.
    // Source: Saint-Gobain Hexoloy SA, CoorsTek SC-30, MatWeb
    Material {
        id: "SiC",
        name: "Silicon Carbide (SiC, sintered)",
        category: MaterialCategory::Ceramic,
        density: Density::kg_m3(3100.0),
        yield_strength: Pressure::mpa(390.0),
        ultimate_tensile: Pressure::mpa(390.0),
        elastic_modulus: Pressure::gpa(410.0),
        poissons_ratio: Dimensionless::ratio(0.17),
        thermal_conductivity: ThermalConductivity::w_mk(120.0),
        cte: CTE::um_mk(4.0),
        specific_heat: SpecificHeat::j_kgk(680.0),
        melting_point: Temperature::celsius(2730.0),   // decomposes, sublimes
        hardness: 2600.0,
        hardness_scale: HardnessScale::Vickers,
        fatigue_endurance: Pressure::mpa(175.0),
        machinability_index: 10.0,
        source: "Saint-Gobain Hexoloy SA, CoorsTek SC-30, MatWeb",
    },

    // -----------------------------------------------------------------------
    // Silicon Nitride (Si₃N₄)
    // -----------------------------------------------------------------------

    // Hot-pressed Si₃N₄ (HPSN). Highest density and strength grade.
    // Source: CeramTec SN-80, CoorsTek Ceralloy 147, ASM Handbook Vol 4
    Material {
        id: "Si3N4",
        name: "Silicon Nitride (Si₃N₄, hot-pressed)",
        category: MaterialCategory::Ceramic,
        density: Density::kg_m3(3200.0),
        yield_strength: Pressure::mpa(700.0),     // flexural strength (high for ceramic)
        ultimate_tensile: Pressure::mpa(700.0),
        elastic_modulus: Pressure::gpa(310.0),
        poissons_ratio: Dimensionless::ratio(0.27),
        thermal_conductivity: ThermalConductivity::w_mk(28.0),
        cte: CTE::um_mk(3.2),
        specific_heat: SpecificHeat::j_kgk(710.0),
        melting_point: Temperature::celsius(1900.0),   // decomposes
        hardness: 1550.0,
        hardness_scale: HardnessScale::Vickers,
        fatigue_endurance: Pressure::mpa(320.0),        // ~46% UTS
        machinability_index: 12.0,
        source: "CeramTec SN-80, CoorsTek Ceralloy 147, ASM Handbook Vol 4",
    },

    // Reaction-bonded Si₃N₄ (RBSN). Lower density due to residual porosity; near-net-shape.
    // Source: CoorsTek RBSN, ASM Handbook Vol 4, Richerson "Modern Ceramic Engineering"
    Material {
        id: "Si3N4-RB",
        name: "Silicon Nitride (Si₃N₄, reaction-bonded)",
        category: MaterialCategory::Ceramic,
        density: Density::kg_m3(2500.0),            // ~80% theoretical density
        yield_strength: Pressure::mpa(250.0),
        ultimate_tensile: Pressure::mpa(250.0),
        elastic_modulus: Pressure::gpa(160.0),       // lower E due to porosity
        poissons_ratio: Dimensionless::ratio(0.25),
        thermal_conductivity: ThermalConductivity::w_mk(14.0),
        cte: CTE::um_mk(3.0),
        specific_heat: SpecificHeat::j_kgk(710.0),
        melting_point: Temperature::celsius(1850.0),
        hardness: 900.0,
        hardness_scale: HardnessScale::Vickers,
        fatigue_endurance: Pressure::mpa(110.0),
        machinability_index: 18.0,                   // more porous, slightly easier to grind
        source: "CoorsTek RBSN, ASM Handbook Vol 4, Richerson Modern Ceramic Engineering",
    },

    // -----------------------------------------------------------------------
    // Zirconia (Zirconium Dioxide, ZrO₂)
    // -----------------------------------------------------------------------

    // Tetragonal Zirconia Polycrystal (TZP) — 3 mol% Y₂O₃ stabilized.
    // Highest flexural strength among structural ceramics. Transformation toughening.
    // Source: CeramTec TZP-A, Tosoh TZ-3Y, MatWeb
    Material {
        id: "ZrO2-TZP",
        name: "Zirconia (ZrO₂ TZP, 3Y)",
        category: MaterialCategory::Ceramic,
        density: Density::kg_m3(6050.0),
        yield_strength: Pressure::mpa(1000.0),    // flexural, transformation toughened
        ultimate_tensile: Pressure::mpa(1000.0),
        elastic_modulus: Pressure::gpa(210.0),
        poissons_ratio: Dimensionless::ratio(0.30),
        thermal_conductivity: ThermalConductivity::w_mk(2.5),
        cte: CTE::um_mk(10.5),
        specific_heat: SpecificHeat::j_kgk(450.0),
        melting_point: Temperature::celsius(2715.0),
        hardness: 1300.0,
        hardness_scale: HardnessScale::Vickers,
        fatigue_endurance: Pressure::mpa(450.0),        // ~45% UTS
        machinability_index: 20.0,                      // easier than SiC/B4C
        source: "CeramTec TZP-A, Tosoh TZ-3Y, MatWeb",
    },

    // Partially Stabilized Zirconia (PSZ) — MgO stabilized.
    // Source: CoorsTek ZPZ, MatWeb, ASM Handbook Vol 4
    Material {
        id: "ZrO2-PSZ",
        name: "Zirconia (ZrO₂ PSZ, Mg-stabilized)",
        category: MaterialCategory::Ceramic,
        density: Density::kg_m3(5740.0),
        yield_strength: Pressure::mpa(600.0),
        ultimate_tensile: Pressure::mpa(600.0),
        elastic_modulus: Pressure::gpa(200.0),
        poissons_ratio: Dimensionless::ratio(0.31),
        thermal_conductivity: ThermalConductivity::w_mk(2.2),
        cte: CTE::um_mk(10.0),
        specific_heat: SpecificHeat::j_kgk(460.0),
        melting_point: Temperature::celsius(2715.0),
        hardness: 1100.0,
        hardness_scale: HardnessScale::Vickers,
        fatigue_endurance: Pressure::mpa(270.0),
        machinability_index: 22.0,
        source: "CoorsTek ZPZ, MatWeb, ASM Handbook Vol 4",
    },

    // -----------------------------------------------------------------------
    // Boron Carbide (B₄C)
    // -----------------------------------------------------------------------

    // Third hardest material known. Used in armor and abrasives.
    // Source: H.C. Starck datasheets, Saint-Gobain Norbide, MatWeb
    Material {
        id: "B4C",
        name: "Boron Carbide (B₄C)",
        category: MaterialCategory::Ceramic,
        density: Density::kg_m3(2520.0),
        yield_strength: Pressure::mpa(350.0),
        ultimate_tensile: Pressure::mpa(350.0),
        elastic_modulus: Pressure::gpa(450.0),
        poissons_ratio: Dimensionless::ratio(0.17),
        thermal_conductivity: ThermalConductivity::w_mk(30.0),
        cte: CTE::um_mk(5.6),
        specific_heat: SpecificHeat::j_kgk(950.0),
        melting_point: Temperature::celsius(2445.0),
        hardness: 3000.0,
        hardness_scale: HardnessScale::Vickers,
        fatigue_endurance: Pressure::mpa(155.0),         // ~44% UTS
        machinability_index: 10.0,                       // extremely difficult
        source: "H.C. Starck datasheets, Saint-Gobain Norbide, MatWeb",
    },

    // -----------------------------------------------------------------------
    // Tungsten Carbide (WC)
    // -----------------------------------------------------------------------

    // WC-6%Co cemented carbide (hardmetal). High hardness, excellent wear resistance.
    // Source: Sandvik Coromant H10F, ASM Handbook Vol 4, Kennametal datasheets
    Material {
        id: "WC-6Co",
        name: "Tungsten Carbide (WC-6%Co cemented carbide)",
        category: MaterialCategory::Ceramic,
        density: Density::kg_m3(14900.0),
        yield_strength: Pressure::mpa(2400.0),           // compressive; tensile ~700 MPa
        ultimate_tensile: Pressure::mpa(2400.0),
        elastic_modulus: Pressure::gpa(620.0),
        poissons_ratio: Dimensionless::ratio(0.22),
        thermal_conductivity: ThermalConductivity::w_mk(100.0),
        cte: CTE::um_mk(5.5),
        specific_heat: SpecificHeat::j_kgk(240.0),
        melting_point: Temperature::celsius(2870.0),      // WC melts; Co binder ~1495 °C
        hardness: 1700.0,
        hardness_scale: HardnessScale::Vickers,
        fatigue_endurance: Pressure::mpa(1050.0),
        machinability_index: 8.0,                        // EDM or grinding only
        source: "Sandvik H10F, ASM Handbook Vol 4, Kennametal datasheets",
    },

    // WC-10%Co cemented carbide. Higher Co binder = better toughness, slightly lower hardness.
    // Source: Sandvik Coromant H13A, ASM Handbook Vol 4, Kennametal datasheets
    Material {
        id: "WC-10Co",
        name: "Tungsten Carbide (WC-10%Co cemented carbide)",
        category: MaterialCategory::Ceramic,
        density: Density::kg_m3(14500.0),
        yield_strength: Pressure::mpa(2000.0),
        ultimate_tensile: Pressure::mpa(2000.0),
        elastic_modulus: Pressure::gpa(580.0),
        poissons_ratio: Dimensionless::ratio(0.23),
        thermal_conductivity: ThermalConductivity::w_mk(85.0),
        cte: CTE::um_mk(6.0),
        specific_heat: SpecificHeat::j_kgk(260.0),
        melting_point: Temperature::celsius(2870.0),
        hardness: 1350.0,
        hardness_scale: HardnessScale::Vickers,
        fatigue_endurance: Pressure::mpa(900.0),
        machinability_index: 9.0,
        source: "Sandvik H13A, ASM Handbook Vol 4, Kennametal datasheets",
    },

    // -----------------------------------------------------------------------
    // Boron Nitride (BN)
    // -----------------------------------------------------------------------

    // Hexagonal BN (hBN) — "white graphite". Excellent machinability, lubricating properties.
    // Source: Saint-Gobain Boron Nitride, Momentive HBR, ASM Handbook Vol 4
    Material {
        id: "BN-hex",
        name: "Boron Nitride (hBN, hexagonal)",
        category: MaterialCategory::Ceramic,
        density: Density::kg_m3(2100.0),
        yield_strength: Pressure::mpa(50.0),              // low; layered structure
        ultimate_tensile: Pressure::mpa(50.0),
        elastic_modulus: Pressure::gpa(20.0),              // in-plane; out-of-plane ~5 GPa
        poissons_ratio: Dimensionless::ratio(0.12),
        thermal_conductivity: ThermalConductivity::w_mk(60.0),  // in-plane; out-of-plane ~2 W/mK
        cte: CTE::um_mk(1.5),                             // in-plane; out-of-plane ~40
        specific_heat: SpecificHeat::j_kgk(800.0),
        melting_point: Temperature::celsius(2973.0),       // sublimes in vacuum; oxidizes ~850 °C in air
        hardness: 100.0,
        hardness_scale: HardnessScale::Vickers,
        fatigue_endurance: Pressure::mpa(20.0),
        machinability_index: 55.0,                        // machinable with carbide tooling
        source: "Saint-Gobain Boron Nitride, Momentive HBR, ASM Handbook Vol 4",
    },

    // Cubic BN (cBN) — second hardest material after diamond. Used in cutting tools.
    // Source: Sumitomo BN300, ASM Handbook Vol 4, Showa Denko datasheets
    Material {
        id: "BN-cubic",
        name: "Boron Nitride (cBN, cubic)",
        category: MaterialCategory::Ceramic,
        density: Density::kg_m3(3480.0),
        yield_strength: Pressure::mpa(700.0),
        ultimate_tensile: Pressure::mpa(700.0),
        elastic_modulus: Pressure::gpa(850.0),
        poissons_ratio: Dimensionless::ratio(0.12),
        thermal_conductivity: ThermalConductivity::w_mk(1300.0), // highest among ceramics
        cte: CTE::um_mk(1.2),
        specific_heat: SpecificHeat::j_kgk(750.0),
        melting_point: Temperature::celsius(3000.0),       // converts to hBN above ~1500 °C at 1 atm
        hardness: 4700.0,
        hardness_scale: HardnessScale::Vickers,
        fatigue_endurance: Pressure::mpa(315.0),
        machinability_index: 5.0,                         // requires diamond grinding
        source: "Sumitomo BN300, ASM Handbook Vol 4, Showa Denko datasheets",
    },

    // -----------------------------------------------------------------------
    // Aluminum Nitride (AlN)
    // -----------------------------------------------------------------------

    // Excellent thermal conductivity for a ceramic. Electronics substrate material.
    // Source: Tokuyama Shapal, CoorsTek AlN, MatWeb
    Material {
        id: "AlN",
        name: "Aluminum Nitride (AlN)",
        category: MaterialCategory::Ceramic,
        density: Density::kg_m3(3260.0),
        yield_strength: Pressure::mpa(300.0),
        ultimate_tensile: Pressure::mpa(300.0),
        elastic_modulus: Pressure::gpa(330.0),
        poissons_ratio: Dimensionless::ratio(0.24),
        thermal_conductivity: ThermalConductivity::w_mk(170.0),   // very high for ceramic
        cte: CTE::um_mk(4.6),
        specific_heat: SpecificHeat::j_kgk(740.0),
        melting_point: Temperature::celsius(2200.0),               // decomposes in air
        hardness: 1100.0,
        hardness_scale: HardnessScale::Vickers,
        fatigue_endurance: Pressure::mpa(135.0),
        machinability_index: 15.0,
        source: "Tokuyama Shapal, CoorsTek AlN, MatWeb",
    },

    // -----------------------------------------------------------------------
    // Cordierite (2MgO·2Al₂O₃·5SiO₂)
    // -----------------------------------------------------------------------

    // Very low CTE — used in kiln furniture, catalyst supports, heat exchangers.
    // Source: Corning Celcor, NGK cordierite, ASM Handbook Vol 4
    Material {
        id: "Cordierite",
        name: "Cordierite (2MgO·2Al₂O₃·5SiO₂)",
        category: MaterialCategory::Ceramic,
        density: Density::kg_m3(2100.0),
        yield_strength: Pressure::mpa(60.0),               // flexural ~100-130 MPa; tensile lower
        ultimate_tensile: Pressure::mpa(60.0),
        elastic_modulus: Pressure::gpa(120.0),
        poissons_ratio: Dimensionless::ratio(0.25),
        thermal_conductivity: ThermalConductivity::w_mk(2.5),
        cte: CTE::um_mk(1.5),                             // extremely low; ~1.0–2.0 depending on direction
        specific_heat: SpecificHeat::j_kgk(780.0),
        melting_point: Temperature::celsius(1460.0),
        hardness: 650.0,
        hardness_scale: HardnessScale::Vickers,
        fatigue_endurance: Pressure::mpa(25.0),
        machinability_index: 25.0,
        source: "Corning Celcor, NGK cordierite, ASM Handbook Vol 4",
    },

    // -----------------------------------------------------------------------
    // Steatite (MgO·SiO₂)
    // -----------------------------------------------------------------------

    // Talc-based ceramic; excellent dielectric, good machinability vs other ceramics.
    // Source: CeramTec L5, Quartz & Silice, ASM Handbook Vol 4
    Material {
        id: "Steatite",
        name: "Steatite (MgO·SiO₂, electrical grade)",
        category: MaterialCategory::Ceramic,
        density: Density::kg_m3(2700.0),
        yield_strength: Pressure::mpa(140.0),
        ultimate_tensile: Pressure::mpa(140.0),
        elastic_modulus: Pressure::gpa(95.0),
        poissons_ratio: Dimensionless::ratio(0.26),
        thermal_conductivity: ThermalConductivity::w_mk(2.5),
        cte: CTE::um_mk(8.0),
        specific_heat: SpecificHeat::j_kgk(840.0),
        melting_point: Temperature::celsius(1550.0),
        hardness: 600.0,
        hardness_scale: HardnessScale::Vickers,
        fatigue_endurance: Pressure::mpa(55.0),
        machinability_index: 28.0,
        source: "CeramTec L5, ASM Handbook Vol 4",
    },

    // -----------------------------------------------------------------------
    // Porcelain (electrical grade)
    // -----------------------------------------------------------------------

    // Feldspathic porcelain. Classic insulator for HV bushings, spark plugs.
    // Source: NGK Insulators, ASM Handbook Vol 4, Kingery "Introduction to Ceramics"
    Material {
        id: "Porcelain-Elec",
        name: "Porcelain (electrical grade)",
        category: MaterialCategory::Ceramic,
        density: Density::kg_m3(2400.0),
        yield_strength: Pressure::mpa(65.0),
        ultimate_tensile: Pressure::mpa(65.0),
        elastic_modulus: Pressure::gpa(70.0),
        poissons_ratio: Dimensionless::ratio(0.25),
        thermal_conductivity: ThermalConductivity::w_mk(1.7),
        cte: CTE::um_mk(5.5),
        specific_heat: SpecificHeat::j_kgk(800.0),
        melting_point: Temperature::celsius(1400.0),
        hardness: 600.0,
        hardness_scale: HardnessScale::Vickers,
        fatigue_endurance: Pressure::mpa(28.0),
        machinability_index: 22.0,
        source: "NGK Insulators, ASM Handbook Vol 4, Kingery Introduction to Ceramics",
    },

    // -----------------------------------------------------------------------
    // Macor (machinable glass-ceramic)
    // -----------------------------------------------------------------------

    // Corning Macor. Can be machined to tight tolerances with standard carbide tooling.
    // Fluorphlogopite mica in glass matrix. Zero porosity.
    // Source: Corning Macor datasheet, ASM Handbook Vol 4
    Material {
        id: "Macor",
        name: "Macor (machinable glass-ceramic)",
        category: MaterialCategory::Ceramic,
        density: Density::kg_m3(2520.0),
        yield_strength: Pressure::mpa(94.0),               // flexural strength
        ultimate_tensile: Pressure::mpa(94.0),
        elastic_modulus: Pressure::gpa(67.0),
        poissons_ratio: Dimensionless::ratio(0.29),
        thermal_conductivity: ThermalConductivity::w_mk(1.46),
        cte: CTE::um_mk(9.3),
        specific_heat: SpecificHeat::j_kgk(790.0),
        melting_point: Temperature::celsius(1000.0),        // softening point; use limit ~800 °C
        hardness: 250.0,
        hardness_scale: HardnessScale::Vickers,
        fatigue_endurance: Pressure::mpa(38.0),
        machinability_index: 45.0,                         // machinable with carbide, best with sharp tooling
        source: "Corning Macor datasheet, ASM Handbook Vol 4",
    },

    // -----------------------------------------------------------------------
    // Pyrolytic Graphite (PG)
    // -----------------------------------------------------------------------

    // Chemical vapor deposited, highly oriented pyrolytic graphite (HOPG).
    // Extreme anisotropy: in-plane properties listed. Used in thermal management.
    // Source: Momentive PG-2, GrafTech POCO PG, ASM Handbook Vol 4
    Material {
        id: "PyroGraphite",
        name: "Pyrolytic Graphite (HOPG, in-plane)",
        category: MaterialCategory::Ceramic,
        density: Density::kg_m3(2200.0),
        yield_strength: Pressure::mpa(40.0),               // in-plane tensile (brittle cleavage)
        ultimate_tensile: Pressure::mpa(40.0),
        elastic_modulus: Pressure::gpa(700.0),              // in-plane; c-axis ~35 GPa
        poissons_ratio: Dimensionless::ratio(0.16),         // in-plane
        thermal_conductivity: ThermalConductivity::w_mk(700.0), // in-plane; c-axis ~5 W/mK
        cte: CTE::um_mk(-1.0),                             // in-plane negative; c-axis ~28 µm/mK
        specific_heat: SpecificHeat::j_kgk(720.0),
        melting_point: Temperature::celsius(3600.0),        // sublimes at ~3700 °C in vacuum
        hardness: 35.0,
        hardness_scale: HardnessScale::Vickers,
        fatigue_endurance: Pressure::mpa(15.0),
        machinability_index: 30.0,                         // cleaves easily in-plane; brittle
        source: "Momentive PG-2, GrafTech POCO PG, ASM Handbook Vol 4",
    },

    // -----------------------------------------------------------------------
    // Alumina (additional grades)
    // -----------------------------------------------------------------------

    // 85% alumina — budget structural grade with higher silica content.
    // Source: CoorsTek AD-85, ASM Handbook Vol 4
    Material {
        id: "Al2O3-85",
        name: "Alumina (Al₂O₃ 85%, budget structural)",
        category: MaterialCategory::Ceramic,
        density: Density::kg_m3(3400.0),
        yield_strength: Pressure::mpa(190.0),
        ultimate_tensile: Pressure::mpa(190.0),
        elastic_modulus: Pressure::gpa(260.0),
        poissons_ratio: Dimensionless::ratio(0.22),
        thermal_conductivity: ThermalConductivity::w_mk(14.0),
        cte: CTE::um_mk(8.8),
        specific_heat: SpecificHeat::j_kgk(880.0),
        melting_point: Temperature::celsius(1700.0),
        hardness: 900.0,
        hardness_scale: HardnessScale::Vickers,
        fatigue_endurance: Pressure::mpa(85.0),
        machinability_index: 22.0,
        source: "CoorsTek AD-85, ASM Handbook Vol 4",
    },

    // Alumina toughened zirconia (ATZ) — 80% Al₂O₃ + 20% ZrO₂.
    // Source: CeramTec ATZ, MatWeb
    Material {
        id: "ATZ",
        name: "Alumina Toughened Zirconia (ATZ, 80/20)",
        category: MaterialCategory::Ceramic,
        density: Density::kg_m3(4370.0),
        yield_strength: Pressure::mpa(800.0),
        ultimate_tensile: Pressure::mpa(800.0),
        elastic_modulus: Pressure::gpa(340.0),
        poissons_ratio: Dimensionless::ratio(0.26),
        thermal_conductivity: ThermalConductivity::w_mk(20.0),
        cte: CTE::um_mk(8.5),
        specific_heat: SpecificHeat::j_kgk(700.0),
        melting_point: Temperature::celsius(1950.0),
        hardness: 1700.0,
        hardness_scale: HardnessScale::Vickers,
        fatigue_endurance: Pressure::mpa(360.0),
        machinability_index: 16.0,
        source: "CeramTec ATZ datasheet, MatWeb",
    },

    // Zirconia toughened alumina (ZTA) — 75% Al₂O₃ + 25% ZrO₂.
    // Source: CoorsTek ZTA, MatWeb
    Material {
        id: "ZTA",
        name: "Zirconia Toughened Alumina (ZTA, 75/25)",
        category: MaterialCategory::Ceramic,
        density: Density::kg_m3(4200.0),
        yield_strength: Pressure::mpa(600.0),
        ultimate_tensile: Pressure::mpa(600.0),
        elastic_modulus: Pressure::gpa(310.0),
        poissons_ratio: Dimensionless::ratio(0.27),
        thermal_conductivity: ThermalConductivity::w_mk(18.0),
        cte: CTE::um_mk(8.8),
        specific_heat: SpecificHeat::j_kgk(750.0),
        melting_point: Temperature::celsius(1900.0),
        hardness: 1550.0,
        hardness_scale: HardnessScale::Vickers,
        fatigue_endurance: Pressure::mpa(270.0),
        machinability_index: 18.0,
        source: "CoorsTek ZTA datasheet, MatWeb",
    },

    // -----------------------------------------------------------------------
    // Silicon Nitride (additional grades)
    // -----------------------------------------------------------------------

    // Gas-pressure sintered Si₃N₄ (GPS). Good balance of strength and thermal shock.
    // Source: Kyocera SN-235P, ASM Handbook Vol 4
    Material {
        id: "Si3N4-GPS",
        name: "Silicon Nitride (Si₃N₄, gas-pressure sintered)",
        category: MaterialCategory::Ceramic,
        density: Density::kg_m3(3250.0),
        yield_strength: Pressure::mpa(900.0),
        ultimate_tensile: Pressure::mpa(900.0),
        elastic_modulus: Pressure::gpa(310.0),
        poissons_ratio: Dimensionless::ratio(0.27),
        thermal_conductivity: ThermalConductivity::w_mk(85.0), // high-TC grade for power electronics substrates
        cte: CTE::um_mk(3.0),
        specific_heat: SpecificHeat::j_kgk(710.0),
        melting_point: Temperature::celsius(1900.0),
        hardness: 1500.0,
        hardness_scale: HardnessScale::Vickers,
        fatigue_endurance: Pressure::mpa(400.0),
        machinability_index: 12.0,
        source: "Kyocera SN-235P, ASM Handbook Vol 4",
    },

    // Si₃N₄ bearing-grade (fully dense HIP). Rolling contact fatigue applications.
    // Source: CeramTec SN-BB, Toshiba TSN-03NH, MatWeb
    Material {
        id: "Si3N4-Bearing",
        name: "Silicon Nitride (Si₃N₄, bearing-grade HIP)",
        category: MaterialCategory::Ceramic,
        density: Density::kg_m3(3230.0),
        yield_strength: Pressure::mpa(800.0),
        ultimate_tensile: Pressure::mpa(800.0),
        elastic_modulus: Pressure::gpa(315.0),
        poissons_ratio: Dimensionless::ratio(0.27),
        thermal_conductivity: ThermalConductivity::w_mk(30.0),
        cte: CTE::um_mk(3.2),
        specific_heat: SpecificHeat::j_kgk(710.0),
        melting_point: Temperature::celsius(1900.0),
        hardness: 1600.0,
        hardness_scale: HardnessScale::Vickers,
        fatigue_endurance: Pressure::mpa(400.0),
        machinability_index: 10.0,
        source: "CeramTec SN-BB, Toshiba TSN-03NH, MatWeb",
    },

    // -----------------------------------------------------------------------
    // Boron Carbide (additional)
    // -----------------------------------------------------------------------

    // B₄C hot-pressed, high-density armor grade.
    // Source: Ceradyne / 3M B₄C armor datasheet, MatWeb
    Material {
        id: "B4C-HP",
        name: "Boron Carbide (B₄C, hot-pressed armor grade)",
        category: MaterialCategory::Ceramic,
        density: Density::kg_m3(2510.0),
        yield_strength: Pressure::mpa(400.0),
        ultimate_tensile: Pressure::mpa(400.0),
        elastic_modulus: Pressure::gpa(460.0),
        poissons_ratio: Dimensionless::ratio(0.17),
        thermal_conductivity: ThermalConductivity::w_mk(35.0),
        cte: CTE::um_mk(5.5),
        specific_heat: SpecificHeat::j_kgk(950.0),
        melting_point: Temperature::celsius(2445.0),
        hardness: 3200.0,
        hardness_scale: HardnessScale::Vickers,
        fatigue_endurance: Pressure::mpa(180.0),
        machinability_index: 8.0,
        source: "Ceradyne / 3M B4C armor datasheet, MatWeb",
    },

    // -----------------------------------------------------------------------
    // Tungsten Carbide (additional grades)
    // -----------------------------------------------------------------------

    // WC-15%Co — very tough grade for mining, stamping.
    // Source: Sandvik Coromant H20, Kennametal, ASM Handbook Vol 4
    Material {
        id: "WC-15Co",
        name: "Tungsten Carbide (WC-15%Co, tough grade)",
        category: MaterialCategory::Ceramic,
        density: Density::kg_m3(14100.0),
        yield_strength: Pressure::mpa(1600.0),
        ultimate_tensile: Pressure::mpa(1600.0),
        elastic_modulus: Pressure::gpa(540.0),
        poissons_ratio: Dimensionless::ratio(0.24),
        thermal_conductivity: ThermalConductivity::w_mk(70.0),
        cte: CTE::um_mk(6.5),
        specific_heat: SpecificHeat::j_kgk(280.0),
        melting_point: Temperature::celsius(2870.0),
        hardness: 1100.0,
        hardness_scale: HardnessScale::Vickers,
        fatigue_endurance: Pressure::mpa(700.0),
        machinability_index: 10.0,
        source: "Sandvik H20, Kennametal datasheets, ASM Handbook Vol 4",
    },

    // WC-3%Co — ultra-hard grade for wear parts.
    // Source: Sandvik Coromant H05, ASM Handbook Vol 4
    Material {
        id: "WC-3Co",
        name: "Tungsten Carbide (WC-3%Co, ultra-hard wear grade)",
        category: MaterialCategory::Ceramic,
        density: Density::kg_m3(15300.0),
        yield_strength: Pressure::mpa(2800.0),
        ultimate_tensile: Pressure::mpa(2800.0),
        elastic_modulus: Pressure::gpa(650.0),
        poissons_ratio: Dimensionless::ratio(0.21),
        thermal_conductivity: ThermalConductivity::w_mk(110.0),
        cte: CTE::um_mk(5.0),
        specific_heat: SpecificHeat::j_kgk(220.0),
        melting_point: Temperature::celsius(2870.0),
        hardness: 2000.0,
        hardness_scale: HardnessScale::Vickers,
        fatigue_endurance: Pressure::mpa(1200.0),
        machinability_index: 6.0,
        source: "Sandvik H05, ASM Handbook Vol 4",
    },

    // -----------------------------------------------------------------------
    // Technical Porcelain
    // -----------------------------------------------------------------------

    // Hard porcelain (dental / laboratory grade). High alumina content.
    // Source: Kuraray Noritake, Ivoclar Vivadent, ASM Handbook Vol 4
    Material {
        id: "Porcelain-Hard",
        name: "Porcelain (hard, dental/laboratory grade)",
        category: MaterialCategory::Ceramic,
        density: Density::kg_m3(2500.0),
        yield_strength: Pressure::mpa(90.0),
        ultimate_tensile: Pressure::mpa(90.0),
        elastic_modulus: Pressure::gpa(80.0),
        poissons_ratio: Dimensionless::ratio(0.25),
        thermal_conductivity: ThermalConductivity::w_mk(1.5),
        cte: CTE::um_mk(6.0),
        specific_heat: SpecificHeat::j_kgk(800.0),
        melting_point: Temperature::celsius(1450.0),
        hardness: 700.0,
        hardness_scale: HardnessScale::Vickers,
        fatigue_endurance: Pressure::mpa(38.0),
        machinability_index: 20.0,
        source: "Kuraray Noritake, Ivoclar Vivadent, ASM Handbook Vol 4",
    },

    // -----------------------------------------------------------------------
    // Piezoelectric Ceramics (PZT)
    // -----------------------------------------------------------------------

    // PZT-5A (soft piezoelectric). Sensors, actuators.
    // Source: CTS Corporation / Morgan Technical Ceramics PZT-5A datasheet
    Material {
        id: "PZT-5A",
        name: "PZT-5A (Lead Zirconate Titanate, soft piezoelectric)",
        category: MaterialCategory::Ceramic,
        density: Density::kg_m3(7750.0),
        yield_strength: Pressure::mpa(75.0),  // tensile, brittle
        ultimate_tensile: Pressure::mpa(75.0),
        elastic_modulus: Pressure::gpa(60.0),
        poissons_ratio: Dimensionless::ratio(0.31),
        thermal_conductivity: ThermalConductivity::w_mk(1.8),
        cte: CTE::um_mk(4.0),
        specific_heat: SpecificHeat::j_kgk(420.0),
        melting_point: Temperature::celsius(350.0), // Curie temperature; melts ~1350 °C
        hardness: 500.0,
        hardness_scale: HardnessScale::Vickers,
        fatigue_endurance: Pressure::mpa(30.0),
        machinability_index: 18.0,
        source: "CTS Corp PZT-5A datasheet, Morgan Technical Ceramics",
    },

    // PZT-4 (hard piezoelectric). High-power ultrasonic transducers.
    // Source: CTS Corporation / PI Ceramic PZT-4 datasheet
    Material {
        id: "PZT-4",
        name: "PZT-4 (Lead Zirconate Titanate, hard piezoelectric)",
        category: MaterialCategory::Ceramic,
        density: Density::kg_m3(7600.0),
        yield_strength: Pressure::mpa(80.0),
        ultimate_tensile: Pressure::mpa(80.0),
        elastic_modulus: Pressure::gpa(70.0),
        poissons_ratio: Dimensionless::ratio(0.31),
        thermal_conductivity: ThermalConductivity::w_mk(1.5),
        cte: CTE::um_mk(3.5),
        specific_heat: SpecificHeat::j_kgk(420.0),
        melting_point: Temperature::celsius(328.0), // Curie temperature
        hardness: 550.0,
        hardness_scale: HardnessScale::Vickers,
        fatigue_endurance: Pressure::mpa(35.0),
        machinability_index: 18.0,
        source: "CTS Corp PZT-4 datasheet, PI Ceramic",
    },

    // -----------------------------------------------------------------------
    // Bioceramics
    // -----------------------------------------------------------------------

    // Hydroxyapatite (HA). Biocompatible, bone implant coating material.
    // Source: Himed Inc HA datasheet, ASM Handbook Vol 4
    Material {
        id: "HA",
        name: "Hydroxyapatite (Ca₁₀(PO₄)₆(OH)₂, bioceramic)",
        category: MaterialCategory::Ceramic,
        density: Density::kg_m3(3160.0),
        yield_strength: Pressure::mpa(40.0),   // sintered HA, flexural
        ultimate_tensile: Pressure::mpa(40.0),
        elastic_modulus: Pressure::gpa(80.0),
        poissons_ratio: Dimensionless::ratio(0.27),
        thermal_conductivity: ThermalConductivity::w_mk(1.3),
        cte: CTE::um_mk(11.0),
        specific_heat: SpecificHeat::j_kgk(700.0),
        melting_point: Temperature::celsius(1614.0), // decomposes above ~1000 °C
        hardness: 600.0,
        hardness_scale: HardnessScale::Vickers,
        fatigue_endurance: Pressure::mpa(15.0),
        machinability_index: 25.0,
        source: "Himed Inc HA datasheet, ASM Handbook Vol 4",
    },

    // TCP (Tricalcium Phosphate, beta phase). Resorbable bioceramic.
    // Source: CAM Bioceramics beta-TCP datasheet, MatWeb
    Material {
        id: "TCP",
        name: "Beta-TCP (β-Ca₃(PO₄)₂, resorbable bioceramic)",
        category: MaterialCategory::Ceramic,
        density: Density::kg_m3(3070.0),
        yield_strength: Pressure::mpa(25.0),
        ultimate_tensile: Pressure::mpa(25.0),
        elastic_modulus: Pressure::gpa(33.0),
        poissons_ratio: Dimensionless::ratio(0.27),
        thermal_conductivity: ThermalConductivity::w_mk(1.0),
        cte: CTE::um_mk(12.0),
        specific_heat: SpecificHeat::j_kgk(700.0),
        melting_point: Temperature::celsius(1391.0), // phase transition to alpha-TCP
        hardness: 400.0,
        hardness_scale: HardnessScale::Vickers,
        fatigue_endurance: Pressure::mpa(10.0),
        machinability_index: 28.0,
        source: "CAM Bioceramics beta-TCP datasheet, MatWeb",
    },

    // -----------------------------------------------------------------------
    // Glass-Ceramics
    // -----------------------------------------------------------------------

    // Lithium Disilicate glass-ceramic (IPS e.max). Dental / high-strength.
    // Source: Ivoclar Vivadent IPS e.max CAD datasheet
    Material {
        id: "LiDiSi",
        name: "Lithium Disilicate Glass-Ceramic (IPS e.max)",
        category: MaterialCategory::Ceramic,
        density: Density::kg_m3(2500.0),
        yield_strength: Pressure::mpa(360.0),
        ultimate_tensile: Pressure::mpa(360.0),
        elastic_modulus: Pressure::gpa(95.0),
        poissons_ratio: Dimensionless::ratio(0.23),
        thermal_conductivity: ThermalConductivity::w_mk(2.5),
        cte: CTE::um_mk(10.2),
        specific_heat: SpecificHeat::j_kgk(800.0),
        melting_point: Temperature::celsius(920.0),  // firing temperature
        hardness: 580.0,
        hardness_scale: HardnessScale::Vickers,
        fatigue_endurance: Pressure::mpa(150.0),
        machinability_index: 30.0,
        source: "Ivoclar Vivadent IPS e.max CAD datasheet",
    },

    // Zerodur (Schott glass-ceramic). Near-zero CTE. Telescope mirrors, laser optics.
    // Source: Schott Zerodur datasheet
    Material {
        id: "Zerodur",
        name: "Zerodur (glass-ceramic, near-zero CTE)",
        category: MaterialCategory::Ceramic,
        density: Density::kg_m3(2530.0),
        yield_strength: Pressure::mpa(50.0),   // flexural ~50 MPa
        ultimate_tensile: Pressure::mpa(50.0),
        elastic_modulus: Pressure::gpa(91.0),
        poissons_ratio: Dimensionless::ratio(0.24),
        thermal_conductivity: ThermalConductivity::w_mk(1.46),
        cte: CTE::um_mk(0.02),  // virtually zero, 0 ± 0.1 µm/mK class
        specific_heat: SpecificHeat::j_kgk(821.0),
        melting_point: Temperature::celsius(700.0), // softening point; Tg ~660 °C
        hardness: 620.0,
        hardness_scale: HardnessScale::Vickers,
        fatigue_endurance: Pressure::mpa(20.0),
        machinability_index: 35.0,
        source: "Schott Zerodur datasheet",
    },

    // -----------------------------------------------------------------------
    // Sapphire (Single Crystal Alumina)
    // -----------------------------------------------------------------------

    // Source: Kyocera sapphire datasheet, GT Advanced Technologies
    Material {
        id: "Sapphire",
        name: "Sapphire (single crystal Al₂O₃)",
        category: MaterialCategory::Ceramic,
        density: Density::kg_m3(3980.0),
        yield_strength: Pressure::mpa(400.0),  // flexural, c-axis
        ultimate_tensile: Pressure::mpa(400.0),
        elastic_modulus: Pressure::gpa(435.0),
        poissons_ratio: Dimensionless::ratio(0.27),
        thermal_conductivity: ThermalConductivity::w_mk(46.0),
        cte: CTE::um_mk(5.3),  // a-axis at 25 °C; c-axis ~5.0
        specific_heat: SpecificHeat::j_kgk(750.0),
        melting_point: Temperature::celsius(2053.0),
        hardness: 2000.0,
        hardness_scale: HardnessScale::Vickers,
        fatigue_endurance: Pressure::mpa(180.0),
        machinability_index: 8.0,
        source: "Kyocera sapphire datasheet, GT Advanced Technologies",
    },

    // -----------------------------------------------------------------------
    // Fused Silica / Quartz
    // -----------------------------------------------------------------------

    // Fused silica (amorphous SiO₂). Ultra-low CTE, excellent optical properties.
    // Source: Heraeus Suprasil, Corning 7980, MatWeb
    Material {
        id: "FusedSilica",
        name: "Fused Silica (amorphous SiO₂)",
        category: MaterialCategory::Ceramic,
        density: Density::kg_m3(2200.0),
        yield_strength: Pressure::mpa(50.0),
        ultimate_tensile: Pressure::mpa(50.0),
        elastic_modulus: Pressure::gpa(73.0),
        poissons_ratio: Dimensionless::ratio(0.17),
        thermal_conductivity: ThermalConductivity::w_mk(1.38),
        cte: CTE::um_mk(0.55),  // very low
        specific_heat: SpecificHeat::j_kgk(740.0),
        melting_point: Temperature::celsius(1710.0), // softening ~1100 °C
        hardness: 550.0,
        hardness_scale: HardnessScale::Vickers,
        fatigue_endurance: Pressure::mpa(20.0),
        machinability_index: 30.0,
        source: "Heraeus Suprasil, Corning 7980, MatWeb",
    },

    // Crystalline quartz (alpha-SiO₂). Piezoelectric, optical.
    // Source: ASM Handbook Vol 4, MatWeb
    Material {
        id: "Quartz",
        name: "Quartz (crystalline α-SiO₂)",
        category: MaterialCategory::Ceramic,
        density: Density::kg_m3(2650.0),
        yield_strength: Pressure::mpa(50.0),
        ultimate_tensile: Pressure::mpa(50.0),
        elastic_modulus: Pressure::gpa(97.0),
        poissons_ratio: Dimensionless::ratio(0.17),
        thermal_conductivity: ThermalConductivity::w_mk(7.7),
        cte: CTE::um_mk(7.1),   // parallel to c-axis; perpendicular ~13.2
        specific_heat: SpecificHeat::j_kgk(730.0),
        melting_point: Temperature::celsius(1723.0),
        hardness: 1100.0,
        hardness_scale: HardnessScale::Vickers,
        fatigue_endurance: Pressure::mpa(20.0),
        machinability_index: 22.0,
        source: "ASM Handbook Vol 4, MatWeb",
    },

    // -----------------------------------------------------------------------
    // Mullite
    // -----------------------------------------------------------------------

    // 3Al₂O₃·2SiO₂. Excellent thermal shock resistance, kiln furniture.
    // Source: Kyocera mullite datasheet, ASM Handbook Vol 4
    Material {
        id: "Mullite",
        name: "Mullite (3Al₂O₃·2SiO₂)",
        category: MaterialCategory::Ceramic,
        density: Density::kg_m3(3160.0),
        yield_strength: Pressure::mpa(180.0),
        ultimate_tensile: Pressure::mpa(180.0),
        elastic_modulus: Pressure::gpa(230.0),
        poissons_ratio: Dimensionless::ratio(0.25),
        thermal_conductivity: ThermalConductivity::w_mk(6.0),
        cte: CTE::um_mk(5.3),
        specific_heat: SpecificHeat::j_kgk(850.0),
        melting_point: Temperature::celsius(1830.0),
        hardness: 1100.0,
        hardness_scale: HardnessScale::Vickers,
        fatigue_endurance: Pressure::mpa(80.0),
        machinability_index: 18.0,
        source: "Kyocera mullite datasheet, ASM Handbook Vol 4",
    },

    // -----------------------------------------------------------------------
    // Sialon
    // -----------------------------------------------------------------------

    // Si₃N₄-based ceramics with Al and O substitution. Cutting tool inserts.
    // Source: Kennametal Kyon sialon datasheet, ASM Handbook Vol 4
    Material {
        id: "Sialon",
        name: "Sialon (Si-Al-O-N, cutting grade)",
        category: MaterialCategory::Ceramic,
        density: Density::kg_m3(3250.0),
        yield_strength: Pressure::mpa(800.0),
        ultimate_tensile: Pressure::mpa(800.0),
        elastic_modulus: Pressure::gpa(290.0),
        poissons_ratio: Dimensionless::ratio(0.27),
        thermal_conductivity: ThermalConductivity::w_mk(20.0),
        cte: CTE::um_mk(3.3),
        specific_heat: SpecificHeat::j_kgk(700.0),
        melting_point: Temperature::celsius(1800.0),
        hardness: 1600.0,
        hardness_scale: HardnessScale::Vickers,
        fatigue_endurance: Pressure::mpa(360.0),
        machinability_index: 10.0,
        source: "Kennametal Kyon sialon datasheet, ASM Handbook Vol 4",
    },

    // -----------------------------------------------------------------------
    // Titanium Diboride (TiB₂)
    // -----------------------------------------------------------------------

    // Extremely hard ceramic, used in armor, wear parts, Al smelter cathodes.
    // Source: Momentive (GE) TiB₂ datasheet, ASM Handbook Vol 4
    Material {
        id: "TiB2",
        name: "Titanium Diboride (TiB₂)",
        category: MaterialCategory::Ceramic,
        density: Density::kg_m3(4520.0),
        yield_strength: Pressure::mpa(370.0),
        ultimate_tensile: Pressure::mpa(370.0),
        elastic_modulus: Pressure::gpa(560.0),
        poissons_ratio: Dimensionless::ratio(0.11),
        thermal_conductivity: ThermalConductivity::w_mk(65.0),
        cte: CTE::um_mk(7.4),
        specific_heat: SpecificHeat::j_kgk(630.0),
        melting_point: Temperature::celsius(3225.0),
        hardness: 2500.0,
        hardness_scale: HardnessScale::Vickers,
        fatigue_endurance: Pressure::mpa(165.0),
        machinability_index: 8.0,
        source: "Momentive TiB2 datasheet, ASM Handbook Vol 4",
    },

    // -----------------------------------------------------------------------
    // Silicon Carbide (additional)
    // -----------------------------------------------------------------------

    // Reaction-bonded SiC (RBSiC / SiSiC). Near-net-shape, contains free Si.
    // Source: Saint-Gobain Hexoloy SE, CoorsTek SC-RB, MatWeb
    Material {
        id: "SiC-RB",
        name: "Silicon Carbide (SiC, reaction-bonded / SiSiC)",
        category: MaterialCategory::Ceramic,
        density: Density::kg_m3(3050.0),
        yield_strength: Pressure::mpa(300.0),
        ultimate_tensile: Pressure::mpa(300.0),
        elastic_modulus: Pressure::gpa(380.0),
        poissons_ratio: Dimensionless::ratio(0.18),
        thermal_conductivity: ThermalConductivity::w_mk(150.0),
        cte: CTE::um_mk(4.3),
        specific_heat: SpecificHeat::j_kgk(680.0),
        melting_point: Temperature::celsius(1410.0), // limited by free Si melting
        hardness: 2200.0,
        hardness_scale: HardnessScale::Vickers,
        fatigue_endurance: Pressure::mpa(135.0),
        machinability_index: 12.0,
        source: "Saint-Gobain Hexoloy SE, CoorsTek SC-RB, MatWeb",
    },
];
