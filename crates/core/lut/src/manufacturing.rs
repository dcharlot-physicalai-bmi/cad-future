//! Manufacturing constraint lookup — DFM rules by process × material.
//!
//! Every constraint is a boolean pass/fail check backed by a table lookup.
//! Zero computation. Pure graph traversal.
//!
//! Data sources: Machinery's Handbook (31st ed.), Protolabs design guidelines,
//! Xometry manufacturing standards, Formlabs design guide, HP MJF design guide,
//! EOS DMLS application notes, SME Sheet Metal Handbook, NADCA product specification
//! standards, ASM Handbook Vol. 15, Investment Casting Institute standards.

use physical_units::*;

// ---------------------------------------------------------------------------
// Enumerations
// ---------------------------------------------------------------------------

/// Manufacturing process identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Process {
    CncMill3Ax,
    CncMill5Ax,
    CncTurn,
    InjectionMold,
    SheetMetal,
    DieCasting,
    Fdm,
    /// FDM with specific nozzle size — 0.2 mm.
    Fdm02,
    /// FDM with specific nozzle size — 0.4 mm (most common).
    Fdm04,
    /// FDM with specific nozzle size — 0.6 mm.
    Fdm06,
    /// FDM with specific nozzle size — 0.8 mm.
    Fdm08,
    Sla,
    /// DLP — similar to SLA but with projected UV layer cure.
    Dlp,
    Sls,
    Mjf,
    Dmls,
    LaserCut,
    WaterjetCut,
    Edm,
    InvestmentCast,
    Forging,
}

/// Material class for manufacturing constraint lookup.
/// Coarser than MaterialCategory — DFM rules group by machinability class.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MaterialClass {
    Aluminum,
    MildSteel,
    Stainless,
    Titanium,
    CopperBrass,
    NickelAlloy,
    CastIron,
    ToolSteel,
    Plastic,
    Nylon,
    PEEK,
    Composite,
    Rubber,
    // --- Specific AM polymers ---
    Pla,
    Abs,
    Petg,
    Pa, // Polyamide (nylon family, generic)
    Pc,
    Tpu,
    // --- Specific resin types ---
    ResinStandard,
    ResinTough,
    ResinFlexible,
    // --- Specific SLS powders ---
    Pa12,
    Pa11,
    // --- Die-casting alloys ---
    AlA380,
    AlA356,
    ZincZamak3,
    ZincZamak5,
    Magnesium,
    // --- Injection molding polymers ---
    Pp,
    Pe,
    Pom,
}

// ---------------------------------------------------------------------------
// Core constraint record — expanded
// ---------------------------------------------------------------------------

/// Complete manufacturing constraint record for a process × material class.
///
/// Fields set to 0.0 or `Length::mm(0.0)` / `Angle::deg(0.0)` mean "not
/// applicable" for this process × material combination.
#[derive(Debug, Clone, Copy)]
pub struct ManufacturingConstraint {
    pub process: Process,
    pub material_class: MaterialClass,
    pub min_wall_thickness: Length,
    /// Maximum wall thickness before sink marks / warpage (injection molding).
    /// 0 = no upper limit constraint.
    pub max_wall_thickness: Length,
    pub min_hole_diameter: Length,
    pub min_corner_radius: Length,
    /// Maximum pocket depth as a multiple of cutter diameter (0.0 = N/A).
    pub max_pocket_depth_ratio: f64,
    /// Maximum depth-to-width ratio for pockets / ribs.
    pub max_depth_to_width_ratio: f64,
    /// Minimum required draft angle for mold/cast features.
    pub draft_angle_min: Angle,
    /// Minimum bend radius expressed as a factor × material thickness.
    /// Relevant only for sheet metal. 0.0 = N/A.
    pub min_bend_radius_factor: f64,
    /// Maximum aspect ratio for thin standing features (height/thickness).
    /// 0.0 = N/A.
    pub max_aspect_ratio: f64,
    /// Standard (±) achievable tolerance in production.
    pub tolerance_standard: Length,
    /// Precision (±) tolerance with care / inspection.
    pub tolerance_precision: Length,
    /// Achievable surface finish Ra (µm). 0.0 = not characterized.
    pub surface_finish_ra_um: f64,
    /// Maximum blind hole depth as a multiple of diameter (0.0 = through only).
    pub max_hole_depth_ratio: f64,
    pub source: &'static str,
}

// ---------------------------------------------------------------------------
// Helper macro to reduce boilerplate — fills defaults for N/A fields
// ---------------------------------------------------------------------------

/// Create a `ManufacturingConstraint` with sensible defaults for unused fields.
macro_rules! mfg {
    (
        $proc:expr, $mat:expr,
        wall_min=$wmin:expr $(, wall_max=$wmax:expr)?
        $(, hole_min=$hmin:expr)?
        , corner_r=$cr:expr
        $(, pocket_depth=$pd:expr)?
        $(, depth_width=$dw:expr)?
        $(, draft=$draft:expr)?
        $(, bend_factor=$bf:expr)?
        $(, aspect=$ar:expr)?
        , tol_std=$ts:expr, tol_prec=$tp:expr
        $(, ra=$ra:expr)?
        $(, hole_depth=$hd:expr)?
        , src=$src:expr
    ) => {
        ManufacturingConstraint {
            process: $proc,
            material_class: $mat,
            min_wall_thickness: Length::mm($wmin),
            max_wall_thickness: Length::mm(mfg!(@opt $($wmax)?)),
            min_hole_diameter: Length::mm(mfg!(@opt $($hmin)?)),
            min_corner_radius: Length::mm($cr),
            max_pocket_depth_ratio: mfg!(@opt $($pd)?),
            max_depth_to_width_ratio: mfg!(@opt $($dw)?),
            draft_angle_min: Angle::deg(mfg!(@opt $($draft)?)),
            min_bend_radius_factor: mfg!(@opt $($bf)?),
            max_aspect_ratio: mfg!(@opt $($ar)?),
            tolerance_standard: Length::mm($ts),
            tolerance_precision: Length::mm($tp),
            surface_finish_ra_um: mfg!(@opt $($ra)?),
            max_hole_depth_ratio: mfg!(@opt $($hd)?),
            source: $src,
        }
    };
    (@opt $v:expr) => { $v };
    (@opt) => { 0.0 };
}

// ---------------------------------------------------------------------------
// CONSTRAINTS — comprehensive process × material matrix
// ---------------------------------------------------------------------------

pub static CONSTRAINTS: &[ManufacturingConstraint] = &[
    // ===================================================================
    // CNC Mill 3-axis (8)
    // ===================================================================
    mfg!(Process::CncMill3Ax, MaterialClass::Aluminum,
        wall_min=1.0, hole_min=1.0, corner_r=0.5,
        pocket_depth=4.0, depth_width=4.0, aspect=15.0,
        tol_std=0.13, tol_prec=0.025, ra=1.6, hole_depth=4.0,
        src="Machinery's Handbook 31ed; Protolabs CNC guidelines"),
    mfg!(Process::CncMill3Ax, MaterialClass::MildSteel,
        wall_min=0.8, hole_min=1.5, corner_r=0.8,
        pocket_depth=3.0, depth_width=3.0, aspect=12.0,
        tol_std=0.13, tol_prec=0.025, ra=1.6, hole_depth=4.0,
        src="Machinery's Handbook 31ed"),
    mfg!(Process::CncMill3Ax, MaterialClass::Stainless,
        wall_min=0.8, hole_min=1.5, corner_r=1.0,
        pocket_depth=2.5, depth_width=2.5, aspect=10.0,
        tol_std=0.13, tol_prec=0.025, ra=1.6, hole_depth=3.0,
        src="Machinery's Handbook 31ed; Xometry stainless guidelines"),
    mfg!(Process::CncMill3Ax, MaterialClass::Titanium,
        wall_min=1.0, hole_min=2.0, corner_r=1.0,
        pocket_depth=2.0, depth_width=2.0, aspect=8.0,
        tol_std=0.13, tol_prec=0.025, ra=3.2, hole_depth=3.0,
        src="Machinery's Handbook 31ed; Kennametal Ti machining guide"),
    mfg!(Process::CncMill3Ax, MaterialClass::CopperBrass,
        wall_min=0.8, hole_min=1.0, corner_r=0.5,
        pocket_depth=4.0, depth_width=4.0, aspect=15.0,
        tol_std=0.13, tol_prec=0.025, ra=0.8, hole_depth=4.0,
        src="Machinery's Handbook 31ed"),
    mfg!(Process::CncMill3Ax, MaterialClass::NickelAlloy,
        wall_min=1.0, hole_min=2.0, corner_r=1.0,
        pocket_depth=2.0, depth_width=2.0, aspect=8.0,
        tol_std=0.13, tol_prec=0.025, ra=3.2, hole_depth=2.5,
        src="Special Metals Inconel machining guide; Machinery's Handbook 31ed"),
    mfg!(Process::CncMill3Ax, MaterialClass::ToolSteel,
        wall_min=1.0, hole_min=2.0, corner_r=1.0,
        pocket_depth=2.5, depth_width=2.5, aspect=10.0,
        tol_std=0.13, tol_prec=0.025, ra=1.6, hole_depth=3.0,
        src="Machinery's Handbook 31ed; Uddeholm tool steel machining"),
    mfg!(Process::CncMill3Ax, MaterialClass::Plastic,
        wall_min=0.5, hole_min=1.0, corner_r=0.5,
        pocket_depth=6.0, depth_width=6.0, aspect=20.0,
        tol_std=0.13, tol_prec=0.05, ra=0.8, hole_depth=6.0,
        src="Protolabs CNC guidelines"),

    // ===================================================================
    // CNC Mill 5-axis (5)
    // ===================================================================
    mfg!(Process::CncMill5Ax, MaterialClass::Aluminum,
        wall_min=0.5, hole_min=0.8, corner_r=0.5,
        pocket_depth=5.0, depth_width=5.0, aspect=20.0,
        tol_std=0.10, tol_prec=0.013, ra=0.8, hole_depth=5.0,
        src="Machinery's Handbook 31ed; 5-axis machining handbook"),
    mfg!(Process::CncMill5Ax, MaterialClass::MildSteel,
        wall_min=0.8, hole_min=1.0, corner_r=0.8,
        pocket_depth=4.0, depth_width=4.0, aspect=15.0,
        tol_std=0.10, tol_prec=0.013, ra=1.6, hole_depth=5.0,
        src="Machinery's Handbook 31ed"),
    mfg!(Process::CncMill5Ax, MaterialClass::Stainless,
        wall_min=0.8, hole_min=1.0, corner_r=0.8,
        pocket_depth=3.5, depth_width=3.5, aspect=12.0,
        tol_std=0.10, tol_prec=0.013, ra=1.6, hole_depth=4.0,
        src="Machinery's Handbook 31ed; Xometry 5-axis guidelines"),
    mfg!(Process::CncMill5Ax, MaterialClass::Titanium,
        wall_min=0.8, hole_min=1.5, corner_r=0.8,
        pocket_depth=3.0, depth_width=3.0, aspect=10.0,
        tol_std=0.13, tol_prec=0.025, ra=3.2, hole_depth=4.0,
        src="Kennametal Ti machining guide; Machinery's Handbook 31ed"),
    mfg!(Process::CncMill5Ax, MaterialClass::NickelAlloy,
        wall_min=1.0, hole_min=2.0, corner_r=1.0,
        pocket_depth=2.5, depth_width=2.5, aspect=8.0,
        tol_std=0.13, tol_prec=0.025, ra=3.2, hole_depth=3.0,
        src="Special Metals Inconel machining guide"),

    // ===================================================================
    // CNC Turning (7)
    // ===================================================================
    mfg!(Process::CncTurn, MaterialClass::Aluminum,
        wall_min=0.5, hole_min=1.0, corner_r=0.2,
        aspect=20.0,
        tol_std=0.05, tol_prec=0.013, ra=0.8, hole_depth=5.0,
        src="Machinery's Handbook 31ed; Protolabs turning guidelines"),
    mfg!(Process::CncTurn, MaterialClass::MildSteel,
        wall_min=0.5, hole_min=1.5, corner_r=0.2,
        aspect=16.0,
        tol_std=0.05, tol_prec=0.013, ra=1.6, hole_depth=5.0,
        src="Machinery's Handbook 31ed"),
    mfg!(Process::CncTurn, MaterialClass::Stainless,
        wall_min=0.5, hole_min=1.5, corner_r=0.2,
        aspect=12.0,
        tol_std=0.05, tol_prec=0.013, ra=1.6, hole_depth=4.0,
        src="Machinery's Handbook 31ed"),
    mfg!(Process::CncTurn, MaterialClass::Titanium,
        wall_min=0.8, hole_min=2.0, corner_r=0.3,
        aspect=10.0,
        tol_std=0.08, tol_prec=0.025, ra=3.2, hole_depth=4.0,
        src="Kennametal Ti machining guide; Machinery's Handbook 31ed"),
    mfg!(Process::CncTurn, MaterialClass::CopperBrass,
        wall_min=0.5, hole_min=1.0, corner_r=0.2,
        aspect=24.0,
        tol_std=0.05, tol_prec=0.013, ra=0.8, hole_depth=5.0,
        src="Machinery's Handbook 31ed"),
    mfg!(Process::CncTurn, MaterialClass::NickelAlloy,
        wall_min=1.0, hole_min=2.0, corner_r=0.3,
        aspect=8.0,
        tol_std=0.08, tol_prec=0.025, ra=3.2, hole_depth=3.0,
        src="Special Metals Inconel machining guide"),
    mfg!(Process::CncTurn, MaterialClass::Plastic,
        wall_min=0.5, hole_min=1.0, corner_r=0.2,
        aspect=20.0,
        tol_std=0.13, tol_prec=0.05, ra=0.8, hole_depth=5.0,
        src="Protolabs CNC plastic guidelines"),

    // ===================================================================
    // FDM generic (3 — backward compat)
    // ===================================================================
    mfg!(Process::Fdm, MaterialClass::Plastic,
        wall_min=0.8, hole_min=2.0, corner_r=0.4,
        pocket_depth=10.0, aspect=8.0,
        tol_std=0.5, tol_prec=0.2, ra=15.0, hole_depth=10.0,
        src="Prusa/Bambu Lab guidelines; Protolabs AM guidelines"),
    mfg!(Process::Fdm, MaterialClass::Nylon,
        wall_min=1.0, hole_min=2.5, corner_r=0.4,
        pocket_depth=10.0, aspect=6.0,
        tol_std=0.5, tol_prec=0.2, ra=15.0, hole_depth=10.0,
        src="Bambu Lab nylon guide; Markforged design guide"),
    mfg!(Process::Fdm, MaterialClass::PEEK,
        wall_min=1.2, hole_min=2.5, corner_r=0.5,
        pocket_depth=8.0, aspect=5.0,
        tol_std=0.5, tol_prec=0.25, ra=20.0, hole_depth=8.0,
        src="Apium PEEK printing guide; 3DXTECH PEEK guidelines"),

    // ===================================================================
    // FDM 0.4 mm nozzle — per filament (6)
    // ===================================================================
    mfg!(Process::Fdm04, MaterialClass::Pla,
        wall_min=0.8, hole_min=2.0, corner_r=0.4,
        aspect=8.0,
        tol_std=0.5, tol_prec=0.2, ra=12.0,
        src="Prusa Research PLA guide; Bambu Lab PLA profile"),
    mfg!(Process::Fdm04, MaterialClass::Abs,
        wall_min=1.0, hole_min=2.0, corner_r=0.4,
        aspect=6.0,
        tol_std=0.5, tol_prec=0.2, ra=14.0,
        src="Stratasys ABS design guide; Prusa ABS profile"),
    mfg!(Process::Fdm04, MaterialClass::Petg,
        wall_min=0.8, hole_min=2.0, corner_r=0.4,
        aspect=8.0,
        tol_std=0.5, tol_prec=0.2, ra=13.0,
        src="Prusament PETG datasheet; Bambu Lab PETG profile"),
    mfg!(Process::Fdm04, MaterialClass::Pa,
        wall_min=1.0, hole_min=2.5, corner_r=0.4,
        aspect=6.0,
        tol_std=0.5, tol_prec=0.25, ra=15.0,
        src="Taulman Nylon 645 guide; Markforged Onyx design guide"),
    mfg!(Process::Fdm04, MaterialClass::Pc,
        wall_min=1.0, hole_min=2.5, corner_r=0.4,
        aspect=6.0,
        tol_std=0.5, tol_prec=0.25, ra=14.0,
        src="Polymaker PolyMax PC guide; Stratasys PC design guide"),
    mfg!(Process::Fdm04, MaterialClass::Tpu,
        wall_min=1.2, hole_min=3.0, corner_r=0.5,
        aspect=4.0,
        tol_std=0.8, tol_prec=0.4, ra=20.0,
        src="Ninjatek TPU design guide; Bambu Lab TPU profile"),

    // ===================================================================
    // FDM 0.2 mm nozzle — per filament (6)
    // Finer resolution but slower, tighter constraints
    // ===================================================================
    mfg!(Process::Fdm02, MaterialClass::Pla,
        wall_min=0.4, hole_min=1.0, corner_r=0.2,
        aspect=6.0,
        tol_std=0.3, tol_prec=0.15, ra=8.0,
        src="Prusa Research 0.2mm nozzle guide"),
    mfg!(Process::Fdm02, MaterialClass::Abs,
        wall_min=0.5, hole_min=1.2, corner_r=0.2,
        aspect=5.0,
        tol_std=0.3, tol_prec=0.15, ra=10.0,
        src="Stratasys micro-nozzle ABS settings"),
    mfg!(Process::Fdm02, MaterialClass::Petg,
        wall_min=0.4, hole_min=1.0, corner_r=0.2,
        aspect=6.0,
        tol_std=0.3, tol_prec=0.15, ra=9.0,
        src="Prusament PETG 0.2mm nozzle datasheet"),
    mfg!(Process::Fdm02, MaterialClass::Pa,
        wall_min=0.6, hole_min=1.5, corner_r=0.3,
        aspect=4.0,
        tol_std=0.4, tol_prec=0.2, ra=12.0,
        src="Taulman Nylon 0.2mm nozzle guide"),
    mfg!(Process::Fdm02, MaterialClass::Pc,
        wall_min=0.6, hole_min=1.5, corner_r=0.3,
        aspect=4.0,
        tol_std=0.4, tol_prec=0.2, ra=11.0,
        src="Polymaker PC 0.2mm nozzle settings"),
    mfg!(Process::Fdm02, MaterialClass::Tpu,
        wall_min=0.8, hole_min=2.0, corner_r=0.4,
        aspect=3.0,
        tol_std=0.6, tol_prec=0.3, ra=16.0,
        src="Ninjatek TPU micro-nozzle guide"),

    // ===================================================================
    // FDM 0.6 mm nozzle — per filament (6)
    // ===================================================================
    mfg!(Process::Fdm06, MaterialClass::Pla,
        wall_min=1.2, hole_min=2.5, corner_r=0.6,
        aspect=10.0,
        tol_std=0.6, tol_prec=0.3, ra=18.0,
        src="Prusa Research 0.6mm nozzle guide"),
    mfg!(Process::Fdm06, MaterialClass::Abs,
        wall_min=1.4, hole_min=2.5, corner_r=0.6,
        aspect=8.0,
        tol_std=0.6, tol_prec=0.3, ra=20.0,
        src="Stratasys ABS 0.6mm nozzle settings"),
    mfg!(Process::Fdm06, MaterialClass::Petg,
        wall_min=1.2, hole_min=2.5, corner_r=0.6,
        aspect=10.0,
        tol_std=0.6, tol_prec=0.3, ra=19.0,
        src="Prusament PETG 0.6mm datasheet"),
    mfg!(Process::Fdm06, MaterialClass::Pa,
        wall_min=1.5, hole_min=3.0, corner_r=0.6,
        aspect=6.0,
        tol_std=0.7, tol_prec=0.35, ra=22.0,
        src="Taulman Nylon 0.6mm nozzle guide"),
    mfg!(Process::Fdm06, MaterialClass::Pc,
        wall_min=1.5, hole_min=3.0, corner_r=0.6,
        aspect=6.0,
        tol_std=0.7, tol_prec=0.35, ra=20.0,
        src="Polymaker PC 0.6mm settings"),
    mfg!(Process::Fdm06, MaterialClass::Tpu,
        wall_min=1.8, hole_min=3.5, corner_r=0.8,
        aspect=5.0,
        tol_std=1.0, tol_prec=0.5, ra=25.0,
        src="Ninjatek TPU 0.6mm nozzle guide"),

    // ===================================================================
    // FDM 0.8 mm nozzle — per filament (6)
    // ===================================================================
    mfg!(Process::Fdm08, MaterialClass::Pla,
        wall_min=1.6, hole_min=3.0, corner_r=0.8,
        aspect=12.0,
        tol_std=0.8, tol_prec=0.4, ra=22.0,
        src="Prusa Research 0.8mm nozzle guide"),
    mfg!(Process::Fdm08, MaterialClass::Abs,
        wall_min=1.8, hole_min=3.0, corner_r=0.8,
        aspect=10.0,
        tol_std=0.8, tol_prec=0.4, ra=24.0,
        src="Stratasys ABS 0.8mm nozzle settings"),
    mfg!(Process::Fdm08, MaterialClass::Petg,
        wall_min=1.6, hole_min=3.0, corner_r=0.8,
        aspect=12.0,
        tol_std=0.8, tol_prec=0.4, ra=23.0,
        src="Prusament PETG 0.8mm datasheet"),
    mfg!(Process::Fdm08, MaterialClass::Pa,
        wall_min=2.0, hole_min=3.5, corner_r=0.8,
        aspect=8.0,
        tol_std=0.9, tol_prec=0.5, ra=25.0,
        src="Taulman Nylon 0.8mm nozzle guide"),
    mfg!(Process::Fdm08, MaterialClass::Pc,
        wall_min=2.0, hole_min=3.5, corner_r=0.8,
        aspect=8.0,
        tol_std=0.9, tol_prec=0.5, ra=24.0,
        src="Polymaker PC 0.8mm settings"),
    mfg!(Process::Fdm08, MaterialClass::Tpu,
        wall_min=2.4, hole_min=4.0, corner_r=1.0,
        aspect=6.0,
        tol_std=1.2, tol_prec=0.6, ra=28.0,
        src="Ninjatek TPU 0.8mm nozzle guide"),

    // ===================================================================
    // SLA (3)
    // ===================================================================
    mfg!(Process::Sla, MaterialClass::ResinStandard,
        wall_min=0.5, hole_min=0.5, corner_r=0.2,
        pocket_depth=10.0, aspect=8.0,
        tol_std=0.15, tol_prec=0.05, ra=2.5, hole_depth=10.0,
        src="Formlabs Form 3+ design guide"),
    mfg!(Process::Sla, MaterialClass::ResinTough,
        wall_min=0.6, hole_min=0.5, corner_r=0.2,
        pocket_depth=10.0, aspect=8.0,
        tol_std=0.15, tol_prec=0.05, ra=3.0, hole_depth=10.0,
        src="Formlabs Tough 2000 datasheet"),
    mfg!(Process::Sla, MaterialClass::ResinFlexible,
        wall_min=1.0, hole_min=1.0, corner_r=0.3,
        pocket_depth=8.0, aspect=4.0,
        tol_std=0.20, tol_prec=0.10, ra=5.0, hole_depth=8.0,
        src="Formlabs Flexible 80A datasheet"),
    // Backward compat — generic Plastic entry
    mfg!(Process::Sla, MaterialClass::Plastic,
        wall_min=0.5, hole_min=0.5, corner_r=0.2,
        pocket_depth=10.0, aspect=8.0,
        tol_std=0.15, tol_prec=0.05, ra=2.5, hole_depth=10.0,
        src="Formlabs design guidelines"),

    // ===================================================================
    // DLP (3)
    // ===================================================================
    mfg!(Process::Dlp, MaterialClass::ResinStandard,
        wall_min=0.5, hole_min=0.5, corner_r=0.15,
        pocket_depth=10.0, aspect=8.0,
        tol_std=0.10, tol_prec=0.05, ra=2.0, hole_depth=10.0,
        src="Anycubic Photon design guide; ELEGOO DLP guidelines"),
    mfg!(Process::Dlp, MaterialClass::ResinTough,
        wall_min=0.6, hole_min=0.5, corner_r=0.2,
        pocket_depth=10.0, aspect=8.0,
        tol_std=0.12, tol_prec=0.05, ra=2.5, hole_depth=10.0,
        src="ELEGOO engineering resin guide"),
    mfg!(Process::Dlp, MaterialClass::ResinFlexible,
        wall_min=1.0, hole_min=1.0, corner_r=0.3,
        pocket_depth=8.0, aspect=4.0,
        tol_std=0.20, tol_prec=0.10, ra=5.0, hole_depth=8.0,
        src="Phrozen flexible resin design guide"),

    // ===================================================================
    // SLS (3)
    // ===================================================================
    mfg!(Process::Sls, MaterialClass::Pa12,
        wall_min=0.7, hole_min=1.5, corner_r=0.5,
        pocket_depth=10.0, aspect=10.0,
        tol_std=0.30, tol_prec=0.15, ra=9.0, hole_depth=10.0,
        src="EOS Formiga P110 PA2200 (PA12) process guide"),
    mfg!(Process::Sls, MaterialClass::Pa11,
        wall_min=0.7, hole_min=1.5, corner_r=0.5,
        pocket_depth=10.0, aspect=10.0,
        tol_std=0.30, tol_prec=0.15, ra=10.0, hole_depth=10.0,
        src="EOS PA1101 (PA11) process guide"),
    mfg!(Process::Sls, MaterialClass::Tpu,
        wall_min=1.0, hole_min=2.0, corner_r=0.5,
        pocket_depth=8.0, aspect=6.0,
        tol_std=0.40, tol_prec=0.20, ra=12.0, hole_depth=8.0,
        src="EOS TPU 1301 process guide"),
    // Backward compat — generic Nylon/Plastic entries
    mfg!(Process::Sls, MaterialClass::Nylon,
        wall_min=0.7, hole_min=1.5, corner_r=0.5,
        pocket_depth=10.0, aspect=10.0,
        tol_std=0.30, tol_prec=0.15, ra=9.0, hole_depth=10.0,
        src="EOS Formiga P110 process guide; Protolabs SLS guidelines"),
    mfg!(Process::Sls, MaterialClass::Plastic,
        wall_min=0.8, hole_min=1.5, corner_r=0.5,
        pocket_depth=10.0, aspect=10.0,
        tol_std=0.30, tol_prec=0.15, ra=10.0, hole_depth=10.0,
        src="EOS SLS application notes"),

    // ===================================================================
    // MJF (3)
    // ===================================================================
    mfg!(Process::Mjf, MaterialClass::Pa12,
        wall_min=0.5, hole_min=1.0, corner_r=0.3,
        pocket_depth=10.0, aspect=12.0,
        tol_std=0.30, tol_prec=0.10, ra=8.0, hole_depth=10.0,
        src="HP Multi Jet Fusion PA12 design guide 2023"),
    mfg!(Process::Mjf, MaterialClass::Pa11,
        wall_min=0.5, hole_min=1.0, corner_r=0.3,
        pocket_depth=10.0, aspect=12.0,
        tol_std=0.30, tol_prec=0.10, ra=9.0, hole_depth=10.0,
        src="HP Multi Jet Fusion PA11 design guide 2023"),
    mfg!(Process::Mjf, MaterialClass::Tpu,
        wall_min=0.8, hole_min=1.5, corner_r=0.4,
        pocket_depth=8.0, aspect=8.0,
        tol_std=0.40, tol_prec=0.15, ra=12.0, hole_depth=8.0,
        src="HP MJF TPU design guide"),
    // Backward compat
    mfg!(Process::Mjf, MaterialClass::Nylon,
        wall_min=0.5, hole_min=1.0, corner_r=0.3,
        pocket_depth=10.0, aspect=12.0,
        tol_std=0.30, tol_prec=0.10, ra=8.0, hole_depth=10.0,
        src="HP Multi Jet Fusion design guide 2023"),

    // ===================================================================
    // Sheet Metal (5)
    // ===================================================================
    mfg!(Process::SheetMetal, MaterialClass::Aluminum,
        wall_min=0.5, hole_min=1.0, corner_r=0.8,
        bend_factor=1.5, aspect=20.0,
        tol_std=0.25, tol_prec=0.10, ra=1.6,
        src="SME Sheet Metal Handbook; Protolabs sheet metal guidelines"),
    mfg!(Process::SheetMetal, MaterialClass::MildSteel,
        wall_min=0.46, hole_min=1.0, corner_r=1.0,
        bend_factor=1.0, aspect=20.0,
        tol_std=0.25, tol_prec=0.10, ra=1.6,
        src="SME Sheet Metal Handbook; Machinery's Handbook 31ed"),
    mfg!(Process::SheetMetal, MaterialClass::Stainless,
        wall_min=0.5, hole_min=1.0, corner_r=1.5,
        bend_factor=1.5, aspect=15.0,
        tol_std=0.25, tol_prec=0.10, ra=1.6,
        src="SME Sheet Metal Handbook; Xometry sheet metal guidelines"),
    mfg!(Process::SheetMetal, MaterialClass::Titanium,
        wall_min=0.6, hole_min=1.5, corner_r=2.0,
        bend_factor=3.0, aspect=10.0,
        tol_std=0.38, tol_prec=0.15, ra=3.2,
        src="SME Sheet Metal Handbook; TIMET Ti forming guide"),
    mfg!(Process::SheetMetal, MaterialClass::CopperBrass,
        wall_min=0.4, hole_min=0.8, corner_r=0.8,
        bend_factor=1.0, aspect=20.0,
        tol_std=0.25, tol_prec=0.10, ra=1.6,
        src="SME Sheet Metal Handbook"),

    // ===================================================================
    // Die Casting (5) — expanded with specific alloys
    // ===================================================================
    mfg!(Process::DieCasting, MaterialClass::AlA380,
        wall_min=1.0, hole_min=2.0, corner_r=0.8,
        depth_width=4.0, draft=1.0, aspect=12.0,
        tol_std=0.20, tol_prec=0.08, ra=1.6, hole_depth=4.0,
        src="NADCA Product Specification Standards; Machinery's Handbook 31ed"),
    mfg!(Process::DieCasting, MaterialClass::AlA356,
        wall_min=1.5, hole_min=2.5, corner_r=1.0,
        depth_width=4.0, draft=1.0, aspect=10.0,
        tol_std=0.25, tol_prec=0.10, ra=3.2, hole_depth=4.0,
        src="NADCA Product Specification Standards; ASM Handbook Vol. 15"),
    mfg!(Process::DieCasting, MaterialClass::ZincZamak3,
        wall_min=0.6, hole_min=1.0, corner_r=0.5,
        depth_width=6.0, draft=0.5, aspect=15.0,
        tol_std=0.10, tol_prec=0.05, ra=0.8, hole_depth=5.0,
        src="NADCA Zinc die casting guide; Dynacast design guidelines"),
    mfg!(Process::DieCasting, MaterialClass::ZincZamak5,
        wall_min=0.6, hole_min=1.0, corner_r=0.5,
        depth_width=6.0, draft=0.5, aspect=15.0,
        tol_std=0.10, tol_prec=0.05, ra=0.8, hole_depth=5.0,
        src="NADCA Zinc die casting guide; Dynacast design guidelines"),
    mfg!(Process::DieCasting, MaterialClass::Magnesium,
        wall_min=1.0, hole_min=2.0, corner_r=0.8,
        depth_width=4.0, draft=1.5, aspect=10.0,
        tol_std=0.15, tol_prec=0.08, ra=1.6, hole_depth=4.0,
        src="NADCA Mg die casting guide; Meridian Magnesium design manual"),
    // Backward compat
    mfg!(Process::DieCasting, MaterialClass::Aluminum,
        wall_min=1.0, hole_min=2.0, corner_r=0.8,
        depth_width=4.0, draft=1.0, aspect=12.0,
        tol_std=0.20, tol_prec=0.08, ra=1.6, hole_depth=4.0,
        src="NADCA Product Specification Standards; Machinery's Handbook 31ed"),
    mfg!(Process::DieCasting, MaterialClass::Plastic,
        wall_min=1.5, hole_min=2.0, corner_r=1.0,
        depth_width=3.0, draft=2.0, aspect=8.0,
        tol_std=0.25, tol_prec=0.10, ra=3.2, hole_depth=3.0,
        src="NADCA guidelines adapted for zinc/plastic die casting"),

    // ===================================================================
    // Injection Molding (8) — expanded with specific polymers
    // ===================================================================
    mfg!(Process::InjectionMold, MaterialClass::Abs,
        wall_min=1.2, wall_max=3.5, hole_min=1.0, corner_r=0.5,
        depth_width=4.0, draft=1.0, aspect=10.0,
        tol_std=0.20, tol_prec=0.05, ra=0.4, hole_depth=4.0,
        src="BASF Terluran ABS molding guide; Protomold guidelines"),
    mfg!(Process::InjectionMold, MaterialClass::Pp,
        wall_min=0.8, wall_max=3.8, hole_min=1.0, corner_r=0.5,
        depth_width=4.0, draft=1.0, aspect=10.0,
        tol_std=0.20, tol_prec=0.08, ra=0.2, hole_depth=4.0,
        src="LyondellBasell PP molding guide"),
    mfg!(Process::InjectionMold, MaterialClass::Pe,
        wall_min=1.0, wall_max=5.0, hole_min=1.0, corner_r=0.5,
        depth_width=4.0, draft=0.5, aspect=10.0,
        tol_std=0.25, tol_prec=0.10, ra=0.4, hole_depth=4.0,
        src="ExxonMobil HDPE molding guide"),
    mfg!(Process::InjectionMold, MaterialClass::Pa,
        wall_min=0.8, wall_max=3.0, hole_min=1.0, corner_r=0.5,
        depth_width=4.0, draft=0.5, aspect=12.0,
        tol_std=0.25, tol_prec=0.08, ra=0.4, hole_depth=4.0,
        src="DuPont Zytel Nylon molding guide"),
    mfg!(Process::InjectionMold, MaterialClass::Pc,
        wall_min=1.0, wall_max=4.0, hole_min=1.0, corner_r=0.5,
        depth_width=4.0, draft=1.0, aspect=10.0,
        tol_std=0.20, tol_prec=0.05, ra=0.2, hole_depth=4.0,
        src="Covestro Makrolon PC molding guide"),
    mfg!(Process::InjectionMold, MaterialClass::Pom,
        wall_min=0.8, wall_max=3.0, hole_min=1.0, corner_r=0.5,
        depth_width=4.0, draft=0.5, aspect=12.0,
        tol_std=0.20, tol_prec=0.05, ra=0.2, hole_depth=4.0,
        src="DuPont Delrin POM molding guide"),
    // Backward compat
    mfg!(Process::InjectionMold, MaterialClass::Plastic,
        wall_min=1.0, wall_max=4.0, hole_min=1.0, corner_r=0.5,
        depth_width=4.0, draft=1.0, aspect=10.0,
        tol_std=0.20, tol_prec=0.05, ra=0.4, hole_depth=4.0,
        src="Protomold design guidelines; Machinery's Handbook 31ed"),
    mfg!(Process::InjectionMold, MaterialClass::Nylon,
        wall_min=0.8, wall_max=3.0, hole_min=1.0, corner_r=0.5,
        depth_width=4.0, draft=1.0, aspect=12.0,
        tol_std=0.25, tol_prec=0.08, ra=0.4, hole_depth=4.0,
        src="DuPont nylon molding guide; Protolabs guidelines"),
    mfg!(Process::InjectionMold, MaterialClass::PEEK,
        wall_min=1.0, wall_max=4.0, hole_min=1.0, corner_r=0.5,
        depth_width=3.0, draft=1.5, aspect=8.0,
        tol_std=0.25, tol_prec=0.08, ra=0.4, hole_depth=4.0,
        src="Victrex PEEK processing guide"),

    // ===================================================================
    // Investment Casting (5) — expanded
    // ===================================================================
    mfg!(Process::InvestmentCast, MaterialClass::Stainless,
        wall_min=1.5, hole_min=3.0, corner_r=1.0,
        draft=0.5, aspect=8.0,
        tol_std=0.25, tol_prec=0.10, ra=3.2, hole_depth=3.0,
        src="Investment Casting Institute (ICI) standards; Hitchiner casting guide"),
    mfg!(Process::InvestmentCast, MaterialClass::Aluminum,
        wall_min=1.5, hole_min=3.0, corner_r=0.8,
        draft=0.5, aspect=8.0,
        tol_std=0.25, tol_prec=0.10, ra=3.2, hole_depth=3.0,
        src="Investment Casting Institute (ICI) standards"),
    mfg!(Process::InvestmentCast, MaterialClass::NickelAlloy,
        wall_min=2.0, hole_min=4.0, corner_r=1.5,
        draft=0.5, aspect=6.0,
        tol_std=0.38, tol_prec=0.15, ra=6.3, hole_depth=2.5,
        src="Investment Casting Institute (ICI) standards; PCC Airfoils guide"),
    mfg!(Process::InvestmentCast, MaterialClass::Titanium,
        wall_min=2.0, hole_min=4.0, corner_r=1.5,
        draft=1.0, aspect=6.0,
        tol_std=0.38, tol_prec=0.15, ra=6.3, hole_depth=2.5,
        src="Precision Castparts Ti investment casting guide"),
    mfg!(Process::InvestmentCast, MaterialClass::MildSteel,
        wall_min=1.5, hole_min=3.0, corner_r=1.0,
        draft=0.5, aspect=8.0,
        tol_std=0.25, tol_prec=0.10, ra=3.2, hole_depth=3.0,
        src="Investment Casting Institute (ICI) standards"),

    // ===================================================================
    // Forging (4)
    // ===================================================================
    mfg!(Process::Forging, MaterialClass::Aluminum,
        wall_min=3.0, hole_min=6.0, corner_r=3.0,
        depth_width=3.0, draft=3.0, aspect=6.0,
        tol_std=0.50, tol_prec=0.25, ra=3.2,
        src="Forging Industry Association handbook; Machinery's Handbook 31ed"),
    mfg!(Process::Forging, MaterialClass::MildSteel,
        wall_min=3.0, hole_min=6.0, corner_r=3.0,
        depth_width=3.0, draft=5.0, aspect=6.0,
        tol_std=0.50, tol_prec=0.25, ra=6.3,
        src="Forging Industry Association handbook; Machinery's Handbook 31ed"),
    mfg!(Process::Forging, MaterialClass::Stainless,
        wall_min=4.0, hole_min=8.0, corner_r=5.0,
        depth_width=2.5, draft=5.0, aspect=5.0,
        tol_std=0.75, tol_prec=0.38, ra=6.3,
        src="Forging Industry Association handbook"),
    mfg!(Process::Forging, MaterialClass::Titanium,
        wall_min=5.0, hole_min=10.0, corner_r=6.0,
        depth_width=2.0, draft=7.0, aspect=4.0,
        tol_std=1.00, tol_prec=0.50, ra=6.3,
        src="TIMET Ti forging guide; Wyman-Gordon forging design manual"),

    // ===================================================================
    // DMLS (4)
    // ===================================================================
    mfg!(Process::Dmls, MaterialClass::Aluminum,
        wall_min=0.4, hole_min=0.5, corner_r=0.2,
        pocket_depth=10.0, aspect=10.0,
        tol_std=0.20, tol_prec=0.05, ra=10.0, hole_depth=8.0,
        src="EOS AlSi10Mg process guide; Protolabs DMLS guidelines"),
    mfg!(Process::Dmls, MaterialClass::MildSteel,
        wall_min=0.4, hole_min=0.5, corner_r=0.2,
        pocket_depth=10.0, aspect=10.0,
        tol_std=0.20, tol_prec=0.05, ra=8.0, hole_depth=8.0,
        src="EOS 316L/17-4 process guide"),
    mfg!(Process::Dmls, MaterialClass::Stainless,
        wall_min=0.4, hole_min=0.5, corner_r=0.2,
        pocket_depth=10.0, aspect=10.0,
        tol_std=0.20, tol_prec=0.05, ra=8.0, hole_depth=8.0,
        src="EOS 316L process guide; Renishaw AM400 guidelines"),
    mfg!(Process::Dmls, MaterialClass::Titanium,
        wall_min=0.4, hole_min=0.5, corner_r=0.2,
        pocket_depth=10.0, aspect=10.0,
        tol_std=0.20, tol_prec=0.05, ra=6.0, hole_depth=8.0,
        src="EOS Ti64 process guide; Renishaw Ti AM guidelines"),

    // ===================================================================
    // Laser Cut (3)
    // ===================================================================
    mfg!(Process::LaserCut, MaterialClass::Aluminum,
        wall_min=0.5, hole_min=1.5, corner_r=0.2,
        tol_std=0.15, tol_prec=0.05, ra=3.2,
        src="Trumpf laser cutting handbook; Protolabs laser guidelines"),
    mfg!(Process::LaserCut, MaterialClass::MildSteel,
        wall_min=0.5, hole_min=1.0, corner_r=0.2,
        tol_std=0.15, tol_prec=0.05, ra=3.2,
        src="Trumpf laser cutting handbook; Machinery's Handbook 31ed"),
    mfg!(Process::LaserCut, MaterialClass::Stainless,
        wall_min=0.5, hole_min=1.0, corner_r=0.2,
        tol_std=0.15, tol_prec=0.05, ra=3.2,
        src="Trumpf laser cutting handbook"),

    // ===================================================================
    // Waterjet Cut (5)
    // ===================================================================
    mfg!(Process::WaterjetCut, MaterialClass::Aluminum,
        wall_min=1.0, hole_min=3.0, corner_r=0.4,
        tol_std=0.25, tol_prec=0.08, ra=6.3,
        src="OMAX waterjet design guide; Protolabs waterjet guidelines"),
    mfg!(Process::WaterjetCut, MaterialClass::MildSteel,
        wall_min=1.0, hole_min=3.0, corner_r=0.4,
        tol_std=0.25, tol_prec=0.08, ra=6.3,
        src="OMAX waterjet design guide"),
    mfg!(Process::WaterjetCut, MaterialClass::Stainless,
        wall_min=1.0, hole_min=3.0, corner_r=0.4,
        tol_std=0.25, tol_prec=0.08, ra=6.3,
        src="OMAX waterjet design guide"),
    mfg!(Process::WaterjetCut, MaterialClass::Titanium,
        wall_min=1.0, hole_min=3.0, corner_r=0.5,
        tol_std=0.30, tol_prec=0.10, ra=6.3,
        src="OMAX waterjet design guide; TIMET waterjet guide"),
    mfg!(Process::WaterjetCut, MaterialClass::Composite,
        wall_min=1.0, hole_min=3.0, corner_r=0.5,
        tol_std=0.38, tol_prec=0.13, ra=6.3,
        src="OMAX composite cutting guide; SME composite fabrication"),

    // ===================================================================
    // EDM (3)
    // ===================================================================
    mfg!(Process::Edm, MaterialClass::ToolSteel,
        wall_min=0.1, hole_min=0.2, corner_r=0.05,
        pocket_depth=20.0,
        tol_std=0.025, tol_prec=0.005, ra=0.4, hole_depth=20.0,
        src="Machinery's Handbook 31ed; Agie Charmilles EDM guide"),
    mfg!(Process::Edm, MaterialClass::Stainless,
        wall_min=0.1, hole_min=0.2, corner_r=0.05,
        pocket_depth=15.0,
        tol_std=0.025, tol_prec=0.005, ra=0.4, hole_depth=15.0,
        src="Machinery's Handbook 31ed; Mitsubishi EDM guide"),
    mfg!(Process::Edm, MaterialClass::Titanium,
        wall_min=0.1, hole_min=0.2, corner_r=0.05,
        pocket_depth=15.0,
        tol_std=0.025, tol_prec=0.005, ra=0.8, hole_depth=15.0,
        src="Machinery's Handbook 31ed; GF Machining EDM guide"),
];

// ---------------------------------------------------------------------------
// FDM per-material constraints (legacy table, kept for backward compat)
// ---------------------------------------------------------------------------

/// FDM filament-specific constraint record.
#[derive(Debug, Clone, Copy)]
pub struct FdmConstraint {
    pub material_name: &'static str,
    pub min_wall_mm: f64,
    pub min_hole_mm: f64,
    /// Maximum overhang angle from vertical before support required (degrees).
    pub overhang_angle_deg: f64,
    /// Maximum unsupported bridge span (mm).
    pub bridge_length_mm: f64,
    /// Recommended heated bed temperature (°C).
    pub bed_adhesion_temp_c: f64,
    /// Typical nozzle temperature range mid-point (°C).
    pub nozzle_temp_c: f64,
    pub source: &'static str,
}

pub static FDM_CONSTRAINTS: &[FdmConstraint] = &[
    FdmConstraint {
        material_name: "PLA",
        min_wall_mm: 0.8,
        min_hole_mm: 2.0,
        overhang_angle_deg: 45.0,
        bridge_length_mm: 50.0,
        bed_adhesion_temp_c: 60.0,
        nozzle_temp_c: 210.0,
        source: "Prusa Research PLA guide; Bambu Lab PLA settings",
    },
    FdmConstraint {
        material_name: "ABS",
        min_wall_mm: 1.0,
        min_hole_mm: 2.0,
        overhang_angle_deg: 45.0,
        bridge_length_mm: 40.0,
        bed_adhesion_temp_c: 100.0,
        nozzle_temp_c: 240.0,
        source: "Stratasys ABS design guide; Prusa ABS settings",
    },
    FdmConstraint {
        material_name: "PETG",
        min_wall_mm: 0.8,
        min_hole_mm: 2.0,
        overhang_angle_deg: 45.0,
        bridge_length_mm: 40.0,
        bed_adhesion_temp_c: 70.0,
        nozzle_temp_c: 235.0,
        source: "Prusament PETG datasheet; Bambu Lab PETG settings",
    },
    FdmConstraint {
        material_name: "Nylon",
        min_wall_mm: 1.0,
        min_hole_mm: 2.5,
        overhang_angle_deg: 40.0,
        bridge_length_mm: 30.0,
        bed_adhesion_temp_c: 70.0,
        nozzle_temp_c: 260.0,
        source: "Taulman Nylon 645 guide; Markforged Onyx design guide",
    },
    FdmConstraint {
        material_name: "TPU",
        min_wall_mm: 1.2,
        min_hole_mm: 3.0,
        overhang_angle_deg: 30.0,
        bridge_length_mm: 20.0,
        bed_adhesion_temp_c: 40.0,
        nozzle_temp_c: 225.0,
        source: "Ninjatek TPU design guide; Bambu Lab TPU settings",
    },
    FdmConstraint {
        material_name: "PC",
        min_wall_mm: 1.0,
        min_hole_mm: 2.5,
        overhang_angle_deg: 40.0,
        bridge_length_mm: 30.0,
        bed_adhesion_temp_c: 110.0,
        nozzle_temp_c: 270.0,
        source: "Polymaker PolyMax PC guide; Stratasys PC design guide",
    },
    FdmConstraint {
        material_name: "PEEK",
        min_wall_mm: 1.2,
        min_hole_mm: 2.5,
        overhang_angle_deg: 40.0,
        bridge_length_mm: 25.0,
        bed_adhesion_temp_c: 120.0,
        nozzle_temp_c: 380.0,
        source: "Apium PEEK printing guide; 3DXTECH PEEK settings",
    },
    FdmConstraint {
        material_name: "ASA",
        min_wall_mm: 1.0,
        min_hole_mm: 2.0,
        overhang_angle_deg: 45.0,
        bridge_length_mm: 40.0,
        bed_adhesion_temp_c: 100.0,
        nozzle_temp_c: 245.0,
        source: "Prusa ASA guide; Bambu Lab ASA settings",
    },
];

// ---------------------------------------------------------------------------
// SLA per-resin constraints
// ---------------------------------------------------------------------------

/// SLA resin-specific constraint record.
#[derive(Debug, Clone, Copy)]
pub struct SlaConstraint {
    pub resin_type: &'static str,
    pub min_wall_mm: f64,
    pub min_hole_mm: f64,
    pub min_feature_mm: f64,
    /// Recommended drain hole diameter for hollow parts (mm).
    pub drain_hole_diameter_mm: f64,
    /// Volumetric cure shrinkage (%).
    pub cure_shrinkage_pct: f64,
    pub source: &'static str,
}

pub static SLA_CONSTRAINTS: &[SlaConstraint] = &[
    SlaConstraint {
        resin_type: "Standard",
        min_wall_mm: 0.5,
        min_hole_mm: 0.5,
        min_feature_mm: 0.3,
        drain_hole_diameter_mm: 3.5,
        cure_shrinkage_pct: 3.5,
        source: "Formlabs Form 3 design guide",
    },
    SlaConstraint {
        resin_type: "Tough",
        min_wall_mm: 0.6,
        min_hole_mm: 0.5,
        min_feature_mm: 0.3,
        drain_hole_diameter_mm: 3.5,
        cure_shrinkage_pct: 3.0,
        source: "Formlabs Tough 2000/1500 datasheet",
    },
    SlaConstraint {
        resin_type: "Flexible",
        min_wall_mm: 1.0,
        min_hole_mm: 1.0,
        min_feature_mm: 0.5,
        drain_hole_diameter_mm: 4.0,
        cure_shrinkage_pct: 2.0,
        source: "Formlabs Flexible 80A datasheet",
    },
    SlaConstraint {
        resin_type: "Castable",
        min_wall_mm: 0.5,
        min_hole_mm: 0.5,
        min_feature_mm: 0.2,
        drain_hole_diameter_mm: 3.0,
        cure_shrinkage_pct: 5.0,
        source: "Formlabs Castable Wax resin guide",
    },
    SlaConstraint {
        resin_type: "Dental",
        min_wall_mm: 0.4,
        min_hole_mm: 0.4,
        min_feature_mm: 0.2,
        drain_hole_diameter_mm: 2.0,
        cure_shrinkage_pct: 2.5,
        source: "Formlabs Dental SG resin guide",
    },
];

// ---------------------------------------------------------------------------
// Sheet Metal per material+thickness constraints
// ---------------------------------------------------------------------------

/// Sheet metal bend constraint for a specific material and nominal thickness.
#[derive(Debug, Clone, Copy)]
pub struct SheetMetalConstraint {
    pub material_class: MaterialClass,
    /// Nominal sheet thickness (mm).
    pub thickness_mm: f64,
    /// Minimum inside bend radius (mm) — typically 1×t for steel, 1.5×t for Ti.
    pub min_bend_radius_mm: f64,
    /// Minimum flange height after bend (mm).
    pub min_flange_mm: f64,
    /// Neutral axis k-factor for this material/thickness.
    pub k_factor: f64,
    /// Minimum hole-to-edge distance (mm).
    pub min_hole_to_edge_mm: f64,
    /// Whether grain direction significantly affects bend quality.
    pub grain_effect: bool,
    pub source: &'static str,
}

pub static SHEET_METAL_CONSTRAINTS: &[SheetMetalConstraint] = &[
    SheetMetalConstraint {
        material_class: MaterialClass::MildSteel,
        thickness_mm: 1.0,
        min_bend_radius_mm: 1.0,
        min_flange_mm: 6.0,
        k_factor: 0.44,
        min_hole_to_edge_mm: 2.0,
        grain_effect: true,
        source: "SME Sheet Metal Handbook; Machinery's Handbook 31ed",
    },
    SheetMetalConstraint {
        material_class: MaterialClass::MildSteel,
        thickness_mm: 2.0,
        min_bend_radius_mm: 2.0,
        min_flange_mm: 8.0,
        k_factor: 0.44,
        min_hole_to_edge_mm: 3.5,
        grain_effect: true,
        source: "SME Sheet Metal Handbook; Machinery's Handbook 31ed",
    },
    SheetMetalConstraint {
        material_class: MaterialClass::MildSteel,
        thickness_mm: 3.0,
        min_bend_radius_mm: 3.0,
        min_flange_mm: 10.0,
        k_factor: 0.44,
        min_hole_to_edge_mm: 5.0,
        grain_effect: true,
        source: "SME Sheet Metal Handbook",
    },
    SheetMetalConstraint {
        material_class: MaterialClass::Aluminum,
        thickness_mm: 1.0,
        min_bend_radius_mm: 1.5,
        min_flange_mm: 6.0,
        k_factor: 0.40,
        min_hole_to_edge_mm: 2.0,
        grain_effect: true,
        source: "SME Sheet Metal Handbook; Protolabs aluminum sheet guidelines",
    },
    SheetMetalConstraint {
        material_class: MaterialClass::Aluminum,
        thickness_mm: 2.0,
        min_bend_radius_mm: 2.5,
        min_flange_mm: 8.0,
        k_factor: 0.40,
        min_hole_to_edge_mm: 3.5,
        grain_effect: true,
        source: "SME Sheet Metal Handbook",
    },
    SheetMetalConstraint {
        material_class: MaterialClass::Stainless,
        thickness_mm: 1.0,
        min_bend_radius_mm: 1.5,
        min_flange_mm: 6.0,
        k_factor: 0.47,
        min_hole_to_edge_mm: 2.0,
        grain_effect: true,
        source: "SME Sheet Metal Handbook; Xometry stainless sheet guidelines",
    },
    SheetMetalConstraint {
        material_class: MaterialClass::Stainless,
        thickness_mm: 2.0,
        min_bend_radius_mm: 3.0,
        min_flange_mm: 9.0,
        k_factor: 0.47,
        min_hole_to_edge_mm: 3.5,
        grain_effect: true,
        source: "SME Sheet Metal Handbook",
    },
    SheetMetalConstraint {
        material_class: MaterialClass::Titanium,
        thickness_mm: 1.0,
        min_bend_radius_mm: 2.0,
        min_flange_mm: 8.0,
        k_factor: 0.45,
        min_hole_to_edge_mm: 2.5,
        grain_effect: true,
        source: "TIMET Ti forming guide; SME Sheet Metal Handbook",
    },
    SheetMetalConstraint {
        material_class: MaterialClass::CopperBrass,
        thickness_mm: 1.0,
        min_bend_radius_mm: 1.0,
        min_flange_mm: 5.0,
        k_factor: 0.42,
        min_hole_to_edge_mm: 2.0,
        grain_effect: false,
        source: "SME Sheet Metal Handbook",
    },
    SheetMetalConstraint {
        material_class: MaterialClass::CopperBrass,
        thickness_mm: 2.0,
        min_bend_radius_mm: 2.0,
        min_flange_mm: 7.0,
        k_factor: 0.42,
        min_hole_to_edge_mm: 3.5,
        grain_effect: false,
        source: "SME Sheet Metal Handbook",
    },
];

// ---------------------------------------------------------------------------
// Injection Molding per-polymer constraints (legacy table)
// ---------------------------------------------------------------------------

/// Injection molding polymer-specific constraint record.
#[derive(Debug, Clone, Copy)]
pub struct InjectionMoldConstraint {
    pub polymer_name: &'static str,
    pub wall_min_mm: f64,
    pub wall_max_mm: f64,
    pub draft_angle_deg: f64,
    pub gate_diameter_mm: f64,
    /// Linear shrinkage (%).
    pub shrinkage_pct: f64,
    /// Maximum flow length : wall thickness ratio before jetting risk.
    pub max_flow_length_ratio: f64,
    pub source: &'static str,
}

pub static INJECTION_MOLD_CONSTRAINTS: &[InjectionMoldConstraint] = &[
    InjectionMoldConstraint {
        polymer_name: "ABS",
        wall_min_mm: 1.2,
        wall_max_mm: 3.5,
        draft_angle_deg: 1.0,
        gate_diameter_mm: 1.5,
        shrinkage_pct: 0.6,
        max_flow_length_ratio: 200.0,
        source: "BASF Terluran ABS molding guide; Protomold guidelines",
    },
    InjectionMoldConstraint {
        polymer_name: "Polypropylene",
        wall_min_mm: 0.8,
        wall_max_mm: 3.8,
        draft_angle_deg: 1.0,
        gate_diameter_mm: 1.5,
        shrinkage_pct: 1.5,
        max_flow_length_ratio: 250.0,
        source: "LyondellBasell PP molding guide",
    },
    InjectionMoldConstraint {
        polymer_name: "Nylon-66",
        wall_min_mm: 0.8,
        wall_max_mm: 3.0,
        draft_angle_deg: 0.5,
        gate_diameter_mm: 1.0,
        shrinkage_pct: 1.5,
        max_flow_length_ratio: 200.0,
        source: "DuPont Zytel Nylon molding guide",
    },
    InjectionMoldConstraint {
        polymer_name: "PEEK",
        wall_min_mm: 1.0,
        wall_max_mm: 4.0,
        draft_angle_deg: 1.5,
        gate_diameter_mm: 1.0,
        shrinkage_pct: 0.5,
        max_flow_length_ratio: 100.0,
        source: "Victrex PEEK injection molding guide",
    },
    InjectionMoldConstraint {
        polymer_name: "POM (Acetal)",
        wall_min_mm: 0.8,
        wall_max_mm: 3.0,
        draft_angle_deg: 0.5,
        gate_diameter_mm: 1.0,
        shrinkage_pct: 2.0,
        max_flow_length_ratio: 200.0,
        source: "DuPont Delrin POM molding guide",
    },
    InjectionMoldConstraint {
        polymer_name: "PC",
        wall_min_mm: 1.0,
        wall_max_mm: 4.0,
        draft_angle_deg: 1.0,
        gate_diameter_mm: 1.5,
        shrinkage_pct: 0.6,
        max_flow_length_ratio: 175.0,
        source: "Covestro Makrolon PC molding guide",
    },
    InjectionMoldConstraint {
        polymer_name: "PPS",
        wall_min_mm: 0.8,
        wall_max_mm: 4.5,
        draft_angle_deg: 1.0,
        gate_diameter_mm: 1.0,
        shrinkage_pct: 0.4,
        max_flow_length_ratio: 150.0,
        source: "Solvay Ryton PPS molding guide",
    },
    InjectionMoldConstraint {
        polymer_name: "HDPE",
        wall_min_mm: 1.0,
        wall_max_mm: 5.0,
        draft_angle_deg: 0.5,
        gate_diameter_mm: 2.0,
        shrinkage_pct: 2.5,
        max_flow_length_ratio: 300.0,
        source: "ExxonMobil HDPE molding guide",
    },
];

// ---------------------------------------------------------------------------
// Cutting (Laser + Waterjet) per-material constraints
// ---------------------------------------------------------------------------

/// Laser/waterjet cutting constraint for a specific process and material.
#[derive(Debug, Clone, Copy)]
pub struct CuttingConstraint {
    pub process: Process,
    pub material_class: MaterialClass,
    pub min_kerf_mm: f64,
    pub min_hole_mm: f64,
    /// Maximum sheet thickness achievable with good quality (mm).
    pub max_thickness_mm: f64,
    /// Heat affected zone width (mm); 0.0 for waterjet.
    pub heat_affected_zone_mm: f64,
    pub source: &'static str,
}

pub static CUTTING_CONSTRAINTS: &[CuttingConstraint] = &[
    // Laser
    CuttingConstraint {
        process: Process::LaserCut,
        material_class: MaterialClass::MildSteel,
        min_kerf_mm: 0.1,
        min_hole_mm: 1.0,
        max_thickness_mm: 25.0,
        heat_affected_zone_mm: 0.3,
        source: "Trumpf TruLaser 5030 spec; Machinery's Handbook 31ed",
    },
    CuttingConstraint {
        process: Process::LaserCut,
        material_class: MaterialClass::Stainless,
        min_kerf_mm: 0.1,
        min_hole_mm: 1.0,
        max_thickness_mm: 20.0,
        heat_affected_zone_mm: 0.25,
        source: "Trumpf TruLaser 5030 spec",
    },
    CuttingConstraint {
        process: Process::LaserCut,
        material_class: MaterialClass::Aluminum,
        min_kerf_mm: 0.15,
        min_hole_mm: 1.5,
        max_thickness_mm: 15.0,
        heat_affected_zone_mm: 0.4,
        source: "Trumpf fiber laser Al cutting guide",
    },
    CuttingConstraint {
        process: Process::LaserCut,
        material_class: MaterialClass::CopperBrass,
        min_kerf_mm: 0.15,
        min_hole_mm: 1.5,
        max_thickness_mm: 6.0,
        heat_affected_zone_mm: 0.5,
        source: "Trumpf fiber laser Cu/Brass cutting note",
    },
    CuttingConstraint {
        process: Process::LaserCut,
        material_class: MaterialClass::Titanium,
        min_kerf_mm: 0.1,
        min_hole_mm: 1.0,
        max_thickness_mm: 10.0,
        heat_affected_zone_mm: 0.3,
        source: "TIMET laser cutting guide",
    },
    // Waterjet
    CuttingConstraint {
        process: Process::WaterjetCut,
        material_class: MaterialClass::MildSteel,
        min_kerf_mm: 0.9,
        min_hole_mm: 3.0,
        max_thickness_mm: 150.0,
        heat_affected_zone_mm: 0.0,
        source: "OMAX design guide; Flow International waterjet spec",
    },
    CuttingConstraint {
        process: Process::WaterjetCut,
        material_class: MaterialClass::Stainless,
        min_kerf_mm: 0.9,
        min_hole_mm: 3.0,
        max_thickness_mm: 100.0,
        heat_affected_zone_mm: 0.0,
        source: "OMAX design guide",
    },
    CuttingConstraint {
        process: Process::WaterjetCut,
        material_class: MaterialClass::Aluminum,
        min_kerf_mm: 0.9,
        min_hole_mm: 3.0,
        max_thickness_mm: 200.0,
        heat_affected_zone_mm: 0.0,
        source: "OMAX design guide",
    },
    CuttingConstraint {
        process: Process::WaterjetCut,
        material_class: MaterialClass::Titanium,
        min_kerf_mm: 1.0,
        min_hole_mm: 3.0,
        max_thickness_mm: 75.0,
        heat_affected_zone_mm: 0.0,
        source: "OMAX Ti waterjet guide",
    },
    CuttingConstraint {
        process: Process::WaterjetCut,
        material_class: MaterialClass::Composite,
        min_kerf_mm: 1.0,
        min_hole_mm: 3.0,
        max_thickness_mm: 50.0,
        heat_affected_zone_mm: 0.0,
        source: "OMAX composite cutting guide",
    },
    CuttingConstraint {
        process: Process::WaterjetCut,
        material_class: MaterialClass::CopperBrass,
        min_kerf_mm: 0.9,
        min_hole_mm: 3.0,
        max_thickness_mm: 75.0,
        heat_affected_zone_mm: 0.0,
        source: "OMAX design guide",
    },
    CuttingConstraint {
        process: Process::WaterjetCut,
        material_class: MaterialClass::Plastic,
        min_kerf_mm: 1.0,
        min_hole_mm: 3.0,
        max_thickness_mm: 150.0,
        heat_affected_zone_mm: 0.0,
        source: "OMAX design guide",
    },
];

// ---------------------------------------------------------------------------
// CNC Turning per-material constraints
// ---------------------------------------------------------------------------

/// Turning-specific constraint record per material class.
#[derive(Debug, Clone, Copy)]
pub struct TurningConstraint {
    pub material_class: MaterialClass,
    /// Minimum bore diameter achievable on a CNC lathe (mm).
    pub min_bore_diameter_mm: f64,
    /// Maximum length-to-diameter ratio before chatter risk.
    pub max_length_to_diameter: f64,
    /// Minimum groove width (mm).
    pub min_groove_width_mm: f64,
    pub source: &'static str,
}

pub static TURNING_CONSTRAINTS: &[TurningConstraint] = &[
    TurningConstraint {
        material_class: MaterialClass::Aluminum,
        min_bore_diameter_mm: 2.0,
        max_length_to_diameter: 10.0,
        min_groove_width_mm: 1.5,
        source: "Machinery's Handbook 31ed; Sandvik turning guide",
    },
    TurningConstraint {
        material_class: MaterialClass::MildSteel,
        min_bore_diameter_mm: 2.0,
        max_length_to_diameter: 8.0,
        min_groove_width_mm: 1.5,
        source: "Machinery's Handbook 31ed; Sandvik turning guide",
    },
    TurningConstraint {
        material_class: MaterialClass::Stainless,
        min_bore_diameter_mm: 2.5,
        max_length_to_diameter: 6.0,
        min_groove_width_mm: 2.0,
        source: "Machinery's Handbook 31ed; Iscar stainless turning guide",
    },
    TurningConstraint {
        material_class: MaterialClass::Titanium,
        min_bore_diameter_mm: 3.0,
        max_length_to_diameter: 5.0,
        min_groove_width_mm: 2.0,
        source: "Kennametal Ti turning guide; Machinery's Handbook 31ed",
    },
    TurningConstraint {
        material_class: MaterialClass::CopperBrass,
        min_bore_diameter_mm: 1.5,
        max_length_to_diameter: 12.0,
        min_groove_width_mm: 1.0,
        source: "Machinery's Handbook 31ed",
    },
    TurningConstraint {
        material_class: MaterialClass::NickelAlloy,
        min_bore_diameter_mm: 3.0,
        max_length_to_diameter: 4.0,
        min_groove_width_mm: 2.5,
        source: "Special Metals turning guide; Machinery's Handbook 31ed",
    },
];

// ---------------------------------------------------------------------------
// DMLS per-material constraints
// ---------------------------------------------------------------------------

/// DMLS/SLM metal additive manufacturing constraint per material.
#[derive(Debug, Clone, Copy)]
pub struct DmlsConstraint {
    pub material_class: MaterialClass,
    pub min_wall_mm: f64,
    pub min_hole_mm: f64,
    pub min_feature_mm: f64,
    /// Maximum overhang angle from horizontal before support required (degrees).
    pub support_angle_deg: f64,
    /// As-printed surface roughness Ra (µm).
    pub surface_finish_ra_um: f64,
    pub source: &'static str,
}

pub static DMLS_CONSTRAINTS: &[DmlsConstraint] = &[
    DmlsConstraint {
        material_class: MaterialClass::Aluminum,
        min_wall_mm: 0.4,
        min_hole_mm: 0.5,
        min_feature_mm: 0.2,
        support_angle_deg: 45.0,
        surface_finish_ra_um: 10.0,
        source: "EOS AlSi10Mg process guide; Protolabs DMLS guidelines",
    },
    DmlsConstraint {
        material_class: MaterialClass::MildSteel,
        min_wall_mm: 0.4,
        min_hole_mm: 0.5,
        min_feature_mm: 0.2,
        support_angle_deg: 45.0,
        surface_finish_ra_um: 8.0,
        source: "EOS 316L process guide",
    },
    DmlsConstraint {
        material_class: MaterialClass::Stainless,
        min_wall_mm: 0.4,
        min_hole_mm: 0.5,
        min_feature_mm: 0.2,
        support_angle_deg: 45.0,
        surface_finish_ra_um: 8.0,
        source: "EOS 316L/17-4PH process guide; Renishaw AM400 guide",
    },
    DmlsConstraint {
        material_class: MaterialClass::Titanium,
        min_wall_mm: 0.4,
        min_hole_mm: 0.5,
        min_feature_mm: 0.2,
        support_angle_deg: 45.0,
        surface_finish_ra_um: 6.0,
        source: "EOS Ti64 process guide; Renishaw Ti AM guide",
    },
    DmlsConstraint {
        material_class: MaterialClass::NickelAlloy,
        min_wall_mm: 0.5,
        min_hole_mm: 0.8,
        min_feature_mm: 0.3,
        support_angle_deg: 45.0,
        surface_finish_ra_um: 12.0,
        source: "EOS IN718/625 process guide",
    },
    DmlsConstraint {
        material_class: MaterialClass::ToolSteel,
        min_wall_mm: 0.4,
        min_hole_mm: 0.5,
        min_feature_mm: 0.2,
        support_angle_deg: 45.0,
        surface_finish_ra_um: 6.0,
        source: "EOS MS1 Maraging Steel process guide",
    },
];

// ---------------------------------------------------------------------------
// K-factor lookup table
// ---------------------------------------------------------------------------

/// Bend allowance k-factor entry for a specific material, thickness, and R/t ratio.
#[derive(Debug, Clone, Copy)]
pub struct KFactorEntry {
    pub material_class: MaterialClass,
    /// Sheet thickness (mm).
    pub thickness_mm: f64,
    /// Inside bend radius / thickness ratio.
    pub bend_radius_over_t: f64,
    /// Neutral axis k-factor (0.0 to 0.5).
    pub k_factor: f64,
    pub source: &'static str,
}

pub static K_FACTORS: &[KFactorEntry] = &[
    // Aluminum 6061-T6
    KFactorEntry { material_class: MaterialClass::Aluminum, thickness_mm: 1.0, bend_radius_over_t: 0.5, k_factor: 0.38, source: "Machinery's Handbook 31ed Table 26-8" },
    KFactorEntry { material_class: MaterialClass::Aluminum, thickness_mm: 1.0, bend_radius_over_t: 1.0, k_factor: 0.41, source: "Machinery's Handbook 31ed Table 26-8" },
    KFactorEntry { material_class: MaterialClass::Aluminum, thickness_mm: 1.0, bend_radius_over_t: 2.0, k_factor: 0.43, source: "Machinery's Handbook 31ed Table 26-8" },
    KFactorEntry { material_class: MaterialClass::Aluminum, thickness_mm: 1.0, bend_radius_over_t: 3.0, k_factor: 0.45, source: "Machinery's Handbook 31ed Table 26-8" },
    KFactorEntry { material_class: MaterialClass::Aluminum, thickness_mm: 1.0, bend_radius_over_t: 5.0, k_factor: 0.46, source: "Machinery's Handbook 31ed Table 26-8" },
    KFactorEntry { material_class: MaterialClass::Aluminum, thickness_mm: 2.0, bend_radius_over_t: 0.5, k_factor: 0.36, source: "Machinery's Handbook 31ed Table 26-8" },
    KFactorEntry { material_class: MaterialClass::Aluminum, thickness_mm: 2.0, bend_radius_over_t: 1.0, k_factor: 0.40, source: "Machinery's Handbook 31ed Table 26-8" },
    KFactorEntry { material_class: MaterialClass::Aluminum, thickness_mm: 2.0, bend_radius_over_t: 2.0, k_factor: 0.42, source: "Machinery's Handbook 31ed Table 26-8" },
    KFactorEntry { material_class: MaterialClass::Aluminum, thickness_mm: 2.0, bend_radius_over_t: 3.0, k_factor: 0.44, source: "Machinery's Handbook 31ed Table 26-8" },
    KFactorEntry { material_class: MaterialClass::Aluminum, thickness_mm: 2.0, bend_radius_over_t: 5.0, k_factor: 0.46, source: "Machinery's Handbook 31ed Table 26-8" },
    // Mild Steel 1020
    KFactorEntry { material_class: MaterialClass::MildSteel, thickness_mm: 1.0, bend_radius_over_t: 0.5, k_factor: 0.42, source: "Machinery's Handbook 31ed Table 26-9" },
    KFactorEntry { material_class: MaterialClass::MildSteel, thickness_mm: 1.0, bend_radius_over_t: 1.0, k_factor: 0.44, source: "Machinery's Handbook 31ed Table 26-9" },
    KFactorEntry { material_class: MaterialClass::MildSteel, thickness_mm: 1.0, bend_radius_over_t: 2.0, k_factor: 0.46, source: "Machinery's Handbook 31ed Table 26-9" },
    KFactorEntry { material_class: MaterialClass::MildSteel, thickness_mm: 1.0, bend_radius_over_t: 3.0, k_factor: 0.47, source: "Machinery's Handbook 31ed Table 26-9" },
    KFactorEntry { material_class: MaterialClass::MildSteel, thickness_mm: 1.0, bend_radius_over_t: 5.0, k_factor: 0.48, source: "Machinery's Handbook 31ed Table 26-9" },
    KFactorEntry { material_class: MaterialClass::MildSteel, thickness_mm: 2.0, bend_radius_over_t: 0.5, k_factor: 0.41, source: "Machinery's Handbook 31ed Table 26-9" },
    KFactorEntry { material_class: MaterialClass::MildSteel, thickness_mm: 2.0, bend_radius_over_t: 1.0, k_factor: 0.44, source: "Machinery's Handbook 31ed Table 26-9" },
    KFactorEntry { material_class: MaterialClass::MildSteel, thickness_mm: 2.0, bend_radius_over_t: 2.0, k_factor: 0.46, source: "Machinery's Handbook 31ed Table 26-9" },
    KFactorEntry { material_class: MaterialClass::MildSteel, thickness_mm: 2.0, bend_radius_over_t: 3.0, k_factor: 0.47, source: "Machinery's Handbook 31ed Table 26-9" },
    KFactorEntry { material_class: MaterialClass::MildSteel, thickness_mm: 2.0, bend_radius_over_t: 5.0, k_factor: 0.48, source: "Machinery's Handbook 31ed Table 26-9" },
    // Stainless 304
    KFactorEntry { material_class: MaterialClass::Stainless, thickness_mm: 1.0, bend_radius_over_t: 0.5, k_factor: 0.44, source: "Machinery's Handbook 31ed Table 26-10" },
    KFactorEntry { material_class: MaterialClass::Stainless, thickness_mm: 1.0, bend_radius_over_t: 1.0, k_factor: 0.46, source: "Machinery's Handbook 31ed Table 26-10" },
    KFactorEntry { material_class: MaterialClass::Stainless, thickness_mm: 1.0, bend_radius_over_t: 2.0, k_factor: 0.47, source: "Machinery's Handbook 31ed Table 26-10" },
    KFactorEntry { material_class: MaterialClass::Stainless, thickness_mm: 1.0, bend_radius_over_t: 3.0, k_factor: 0.48, source: "Machinery's Handbook 31ed Table 26-10" },
    KFactorEntry { material_class: MaterialClass::Stainless, thickness_mm: 1.0, bend_radius_over_t: 5.0, k_factor: 0.50, source: "Machinery's Handbook 31ed Table 26-10" },
];

// ---------------------------------------------------------------------------
// Tool library
// ---------------------------------------------------------------------------

/// Carbide end mill tool library entry.
#[derive(Debug, Clone, Copy)]
pub struct ToolEntry {
    pub diameter_mm: f64,
    pub flutes: u32,
    pub material: &'static str,
    pub max_rpm: f64,
    /// Aluminum recommended surface footage (SFM).
    pub aluminum_sfm: f64,
    /// Carbon/mild steel recommended SFM.
    pub steel_sfm: f64,
    /// Stainless steel recommended SFM.
    pub stainless_sfm: f64,
    /// Titanium recommended SFM.
    pub titanium_sfm: f64,
    pub source: &'static str,
}

pub static TOOL_LIBRARY: &[ToolEntry] = &[
    ToolEntry { diameter_mm:  1.0, flutes: 2, material: "Solid Carbide", max_rpm: 60000.0, aluminum_sfm: 800.0, steel_sfm: 150.0, stainless_sfm: 100.0, titanium_sfm: 60.0, source: "Harvey Tool carbide end mill catalog; Machinery's Handbook 31ed" },
    ToolEntry { diameter_mm:  1.5, flutes: 2, material: "Solid Carbide", max_rpm: 40000.0, aluminum_sfm: 800.0, steel_sfm: 150.0, stainless_sfm: 100.0, titanium_sfm: 60.0, source: "Harvey Tool carbide end mill catalog" },
    ToolEntry { diameter_mm:  2.0, flutes: 2, material: "Solid Carbide", max_rpm: 30000.0, aluminum_sfm: 850.0, steel_sfm: 175.0, stainless_sfm: 110.0, titanium_sfm: 65.0, source: "Harvey Tool carbide end mill catalog" },
    ToolEntry { diameter_mm:  2.0, flutes: 4, material: "Solid Carbide", max_rpm: 30000.0, aluminum_sfm: 700.0, steel_sfm: 200.0, stainless_sfm: 130.0, titanium_sfm: 75.0, source: "Harvey Tool carbide end mill catalog" },
    ToolEntry { diameter_mm:  3.0, flutes: 2, material: "Solid Carbide", max_rpm: 24000.0, aluminum_sfm: 900.0, steel_sfm: 175.0, stainless_sfm: 120.0, titanium_sfm: 70.0, source: "Harvey Tool carbide end mill catalog" },
    ToolEntry { diameter_mm:  3.0, flutes: 4, material: "Solid Carbide", max_rpm: 24000.0, aluminum_sfm: 750.0, steel_sfm: 200.0, stainless_sfm: 140.0, titanium_sfm: 80.0, source: "Harvey Tool carbide end mill catalog" },
    ToolEntry { diameter_mm:  4.0, flutes: 2, material: "Solid Carbide", max_rpm: 20000.0, aluminum_sfm: 900.0, steel_sfm: 200.0, stainless_sfm: 130.0, titanium_sfm: 70.0, source: "Kennametal end mill catalog" },
    ToolEntry { diameter_mm:  4.0, flutes: 4, material: "Solid Carbide", max_rpm: 20000.0, aluminum_sfm: 750.0, steel_sfm: 220.0, stainless_sfm: 150.0, titanium_sfm: 85.0, source: "Kennametal end mill catalog" },
    ToolEntry { diameter_mm:  6.0, flutes: 2, material: "Solid Carbide", max_rpm: 15000.0, aluminum_sfm: 950.0, steel_sfm: 200.0, stainless_sfm: 130.0, titanium_sfm: 75.0, source: "Kennametal end mill catalog" },
    ToolEntry { diameter_mm:  6.0, flutes: 4, material: "Solid Carbide", max_rpm: 15000.0, aluminum_sfm: 800.0, steel_sfm: 250.0, stainless_sfm: 160.0, titanium_sfm: 90.0, source: "Kennametal end mill catalog" },
    ToolEntry { diameter_mm:  8.0, flutes: 4, material: "Solid Carbide", max_rpm: 12000.0, aluminum_sfm: 950.0, steel_sfm: 250.0, stainless_sfm: 160.0, titanium_sfm: 90.0, source: "Iscar end mill catalog" },
    ToolEntry { diameter_mm: 10.0, flutes: 4, material: "Solid Carbide", max_rpm: 10000.0, aluminum_sfm: 1000.0, steel_sfm: 280.0, stainless_sfm: 175.0, titanium_sfm: 95.0, source: "Iscar end mill catalog" },
    ToolEntry { diameter_mm: 12.0, flutes: 4, material: "Solid Carbide", max_rpm:  8000.0, aluminum_sfm: 1000.0, steel_sfm: 300.0, stainless_sfm: 185.0, titanium_sfm: 100.0, source: "Sandvik Coromant end mill catalog" },
    ToolEntry { diameter_mm: 16.0, flutes: 4, material: "Solid Carbide", max_rpm:  6000.0, aluminum_sfm: 1100.0, steel_sfm: 320.0, stainless_sfm: 200.0, titanium_sfm: 110.0, source: "Sandvik Coromant end mill catalog" },
    ToolEntry { diameter_mm: 20.0, flutes: 4, material: "Solid Carbide", max_rpm:  5000.0, aluminum_sfm: 1200.0, steel_sfm: 350.0, stainless_sfm: 220.0, titanium_sfm: 120.0, source: "Sandvik Coromant end mill catalog" },
    ToolEntry { diameter_mm: 25.0, flutes: 4, material: "Solid Carbide", max_rpm:  4000.0, aluminum_sfm: 1250.0, steel_sfm: 370.0, stainless_sfm: 230.0, titanium_sfm: 125.0, source: "Sandvik Coromant end mill catalog" },
    ToolEntry { diameter_mm: 32.0, flutes: 4, material: "Solid Carbide", max_rpm:  3000.0, aluminum_sfm: 1300.0, steel_sfm: 390.0, stainless_sfm: 240.0, titanium_sfm: 130.0, source: "Sandvik Coromant end mill catalog" },
];

// ---------------------------------------------------------------------------
// Utility arrays
// ---------------------------------------------------------------------------

/// Standard CNC cutter diameters (mm).
pub static STANDARD_CUTTER_DIAMETERS_MM: &[f64] = &[
    1.0, 1.5, 2.0, 3.0, 4.0, 5.0, 6.0, 8.0, 10.0, 12.0, 16.0, 20.0, 25.0, 32.0, 40.0,
];

/// Standard sheet metal gauges (mm thickness) — mild steel gauge series.
pub static SHEET_METAL_GAUGES_MM: &[f64] = &[
    0.46,  // 28 ga
    0.56,  // 26 ga
    0.64,  // 24 ga
    0.79,  // 22 ga
    0.95,  // 20 ga
    1.11,  // 18 ga
    1.27,  // 17 ga
    1.59,  // 16 ga
    1.91,  // 14 ga
    2.38,  // 13 ga
    3.18,  // 11 ga (1/8 in approx)
    4.55,  //  7 ga
    6.35,  // 1/4 in
    9.53,  // 3/8 in
    12.70, // 1/2 in
];

/// Common FDM nozzle diameters (mm).
pub static FDM_NOZZLE_DIAMETERS_MM: &[f64] = &[
    0.2, 0.25, 0.3, 0.4, 0.5, 0.6, 0.8, 1.0,
];

// ---------------------------------------------------------------------------
// Lookup functions
// ---------------------------------------------------------------------------

/// Lookup manufacturing constraints for a process × material class pair.
pub fn lookup(process: Process, material_class: MaterialClass) -> Option<&'static ManufacturingConstraint> {
    CONSTRAINTS
        .iter()
        .find(|c| c.process == process && c.material_class == material_class)
}

/// Lookup all constraints for a given process (any material).
pub fn lookup_by_process(process: Process) -> impl Iterator<Item = &'static ManufacturingConstraint> {
    CONSTRAINTS.iter().filter(move |c| c.process == process)
}

/// Lookup all constraints for a given material class (any process).
pub fn lookup_by_material(material_class: MaterialClass) -> impl Iterator<Item = &'static ManufacturingConstraint> {
    CONSTRAINTS.iter().filter(move |c| c.material_class == material_class)
}

/// Check if a wall thickness is valid for a given process and material.
pub fn check_wall_thickness(
    wall: Length,
    process: Process,
    material_class: MaterialClass,
) -> Option<bool> {
    lookup(process, material_class).map(|c| wall >= c.min_wall_thickness)
}

/// Check if a wall thickness would cause sink marks (too thick) for injection molding.
/// Returns `Some(true)` if the wall is within bounds, `Some(false)` if too thick.
/// Returns `None` if no constraint found or max_wall is 0 (no limit).
pub fn check_max_wall_thickness(
    wall: Length,
    process: Process,
    material_class: MaterialClass,
) -> Option<bool> {
    lookup(process, material_class).and_then(|c| {
        let max = c.max_wall_thickness.to_mm();
        if max > 0.0 {
            Some(wall.to_mm() <= max)
        } else {
            None
        }
    })
}

/// Check if a corner radius is valid for a given process and material.
pub fn check_corner_radius(
    radius: Length,
    process: Process,
    material_class: MaterialClass,
) -> Option<bool> {
    lookup(process, material_class).map(|c| radius >= c.min_corner_radius)
}

/// Check if a hole diameter is valid for a given process and material.
pub fn check_hole_diameter(
    diameter: Length,
    process: Process,
    material_class: MaterialClass,
) -> Option<bool> {
    lookup(process, material_class).and_then(|c| {
        let min = c.min_hole_diameter.to_mm();
        if min > 0.0 {
            Some(diameter.to_mm() >= min)
        } else {
            None
        }
    })
}

/// Check if an aspect ratio (height / thickness) is within limits.
pub fn check_aspect_ratio(
    aspect_ratio: f64,
    process: Process,
    material_class: MaterialClass,
) -> Option<bool> {
    lookup(process, material_class).and_then(|c| {
        if c.max_aspect_ratio > 0.0 {
            Some(aspect_ratio <= c.max_aspect_ratio)
        } else {
            None
        }
    })
}

/// Check if a draft angle meets the minimum for a process × material.
pub fn check_draft_angle(
    draft: Angle,
    process: Process,
    material_class: MaterialClass,
) -> Option<bool> {
    lookup(process, material_class).map(|c| draft >= c.draft_angle_min)
}

/// Get the achievable surface finish Ra (µm) for a process × material.
pub fn surface_finish_ra(
    process: Process,
    material_class: MaterialClass,
) -> Option<f64> {
    lookup(process, material_class).and_then(|c| {
        if c.surface_finish_ra_um > 0.0 {
            Some(c.surface_finish_ra_um)
        } else {
            None
        }
    })
}

/// Find the nearest standard cutter diameter >= `min_diameter_mm`.
pub fn nearest_cutter(min_diameter_mm: f64) -> Option<f64> {
    STANDARD_CUTTER_DIAMETERS_MM
        .iter()
        .copied()
        .find(|&d| d >= min_diameter_mm)
}

/// Lookup FDM constraints by filament material name (case-insensitive prefix match).
pub fn lookup_fdm(material_name: &str) -> Option<&'static FdmConstraint> {
    let name_lower = material_name.to_lowercase();
    FDM_CONSTRAINTS
        .iter()
        .find(|c| c.material_name.to_lowercase().starts_with(&name_lower))
}

/// Lookup SLA constraints by resin type name (case-insensitive prefix match).
pub fn lookup_sla(resin_type: &str) -> Option<&'static SlaConstraint> {
    let name_lower = resin_type.to_lowercase();
    SLA_CONSTRAINTS
        .iter()
        .find(|c| c.resin_type.to_lowercase().starts_with(&name_lower))
}

/// Lookup sheet metal bend constraints for a material and thickness.
/// Returns the entry with the closest thickness_mm.
pub fn lookup_sheet_metal(
    material_class: MaterialClass,
    thickness_mm: f64,
) -> Option<&'static SheetMetalConstraint> {
    SHEET_METAL_CONSTRAINTS
        .iter()
        .filter(|c| c.material_class == material_class)
        .min_by(|a, b| {
            let da = (a.thickness_mm - thickness_mm).abs();
            let db = (b.thickness_mm - thickness_mm).abs();
            da.partial_cmp(&db).unwrap()
        })
}

/// Lookup injection molding constraints by polymer name (case-insensitive prefix match).
pub fn lookup_injection_mold(polymer_name: &str) -> Option<&'static InjectionMoldConstraint> {
    let name_lower = polymer_name.to_lowercase();
    INJECTION_MOLD_CONSTRAINTS
        .iter()
        .find(|c| c.polymer_name.to_lowercase().starts_with(&name_lower))
}

/// Lookup cutting constraints for a process × material class pair.
pub fn lookup_cutting(
    process: Process,
    material_class: MaterialClass,
) -> Option<&'static CuttingConstraint> {
    CUTTING_CONSTRAINTS
        .iter()
        .find(|c| c.process == process && c.material_class == material_class)
}

/// Lookup turning constraints for a material class.
pub fn lookup_turning(material_class: MaterialClass) -> Option<&'static TurningConstraint> {
    TURNING_CONSTRAINTS
        .iter()
        .find(|c| c.material_class == material_class)
}

/// Lookup DMLS constraints for a material class.
pub fn lookup_dmls(material_class: MaterialClass) -> Option<&'static DmlsConstraint> {
    DMLS_CONSTRAINTS
        .iter()
        .find(|c| c.material_class == material_class)
}

/// Lookup the k-factor for a material class and bend geometry.
/// Finds the entry with matching material and closest (thickness, R/t) pair.
pub fn lookup_k_factor(
    material_class: MaterialClass,
    thickness_mm: f64,
    bend_radius_over_t: f64,
) -> Option<&'static KFactorEntry> {
    K_FACTORS
        .iter()
        .filter(|e| e.material_class == material_class)
        .min_by(|a, b| {
            let da = (a.thickness_mm - thickness_mm).abs()
                + (a.bend_radius_over_t - bend_radius_over_t).abs();
            let db = (b.thickness_mm - thickness_mm).abs()
                + (b.bend_radius_over_t - bend_radius_over_t).abs();
            da.partial_cmp(&db).unwrap()
        })
}

/// Lookup the best-match carbide tool for a given minimum diameter.
/// Returns the tool with the smallest diameter >= `min_diameter_mm` and the
/// specified flute count. Falls back to any flute count if no match.
pub fn lookup_tool(min_diameter_mm: f64, preferred_flutes: u32) -> Option<&'static ToolEntry> {
    // First try: exact flute count
    let result = TOOL_LIBRARY
        .iter()
        .filter(|t| t.diameter_mm >= min_diameter_mm && t.flutes == preferred_flutes)
        .min_by(|a, b| a.diameter_mm.partial_cmp(&b.diameter_mm).unwrap());
    if result.is_some() {
        return result;
    }
    // Fallback: any flute count
    TOOL_LIBRARY
        .iter()
        .filter(|t| t.diameter_mm >= min_diameter_mm)
        .min_by(|a, b| a.diameter_mm.partial_cmp(&b.diameter_mm).unwrap())
}

/// Map an FDM nozzle diameter to the appropriate nozzle-specific process.
/// Returns `None` if the nozzle diameter doesn't match a known size.
pub fn fdm_process_for_nozzle(nozzle_mm: f64) -> Option<Process> {
    const EPS: f64 = 0.01;
    if (nozzle_mm - 0.2).abs() < EPS {
        Some(Process::Fdm02)
    } else if (nozzle_mm - 0.4).abs() < EPS {
        Some(Process::Fdm04)
    } else if (nozzle_mm - 0.6).abs() < EPS {
        Some(Process::Fdm06)
    } else if (nozzle_mm - 0.8).abs() < EPS {
        Some(Process::Fdm08)
    } else {
        None
    }
}

/// Count total entries in the main CONSTRAINTS table.
pub fn constraint_count() -> usize {
    CONSTRAINTS.len()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // --- Core lookup ---------------------------------------------------------

    #[test]
    fn cnc_aluminum_wall_pass() {
        assert_eq!(
            check_wall_thickness(Length::mm(1.5), Process::CncMill3Ax, MaterialClass::Aluminum),
            Some(true)
        );
    }

    #[test]
    fn cnc_aluminum_wall_fail() {
        assert_eq!(
            check_wall_thickness(Length::mm(0.5), Process::CncMill3Ax, MaterialClass::Aluminum),
            Some(false)
        );
    }

    #[test]
    fn cnc_stainless_corner_radius() {
        let c = lookup(Process::CncMill3Ax, MaterialClass::Stainless).unwrap();
        assert!(c.min_corner_radius.to_mm() >= 0.5);
        assert!(check_corner_radius(Length::mm(2.0), Process::CncMill3Ax, MaterialClass::Stainless) == Some(true));
        assert!(check_corner_radius(Length::mm(0.1), Process::CncMill3Ax, MaterialClass::Stainless) == Some(false));
    }

    #[test]
    fn injection_mold_draft_angle() {
        let c = lookup(Process::InjectionMold, MaterialClass::Plastic).unwrap();
        assert!(c.draft_angle_min.to_deg() >= 1.0);
    }

    #[test]
    fn injection_mold_nylon_exists() {
        let c = lookup(Process::InjectionMold, MaterialClass::Nylon);
        assert!(c.is_some());
        assert!(c.unwrap().draft_angle_min.to_deg() >= 1.0);
    }

    #[test]
    fn nearest_cutter_lookup() {
        assert_eq!(nearest_cutter(2.5), Some(3.0));
        assert_eq!(nearest_cutter(1.0), Some(1.0));
        assert_eq!(nearest_cutter(20.0), Some(20.0));
        assert_eq!(nearest_cutter(33.0), Some(40.0));
        assert_eq!(nearest_cutter(50.0), None);
    }

    #[test]
    fn missing_combo_returns_none() {
        // CNC 3-axis should not have Rubber
        assert!(lookup(Process::CncMill3Ax, MaterialClass::Rubber).is_none());
    }

    #[test]
    fn all_processes_covered() {
        let processes = [
            Process::CncMill3Ax, Process::CncMill5Ax, Process::CncTurn,
            Process::InjectionMold, Process::SheetMetal, Process::DieCasting,
            Process::Fdm, Process::Fdm02, Process::Fdm04, Process::Fdm06,
            Process::Fdm08, Process::Sla, Process::Dlp, Process::Sls,
            Process::Mjf, Process::Dmls, Process::LaserCut,
            Process::WaterjetCut, Process::Edm, Process::InvestmentCast,
            Process::Forging,
        ];
        for p in &processes {
            assert!(
                CONSTRAINTS.iter().any(|c| &c.process == p),
                "Process {:?} has no CONSTRAINTS entry", p
            );
        }
    }

    // --- Expanded struct fields -------------------------------------------------

    #[test]
    fn new_fields_populated_cnc_mill() {
        let c = lookup(Process::CncMill3Ax, MaterialClass::Aluminum).unwrap();
        assert!(c.min_hole_diameter.to_mm() > 0.0, "min_hole_diameter should be set");
        assert!(c.max_depth_to_width_ratio > 0.0, "max_depth_to_width_ratio should be set");
        assert!(c.max_aspect_ratio > 0.0, "max_aspect_ratio should be set");
        assert!(c.surface_finish_ra_um > 0.0, "surface_finish_ra_um should be set");
    }

    #[test]
    fn injection_mold_max_wall_thickness() {
        let c = lookup(Process::InjectionMold, MaterialClass::Abs).unwrap();
        assert!(c.max_wall_thickness.to_mm() > 0.0, "IM should have max wall");
        assert!(c.max_wall_thickness.to_mm() <= 5.0, "IM max wall should be reasonable");
    }

    #[test]
    fn sheet_metal_bend_radius_factor() {
        let c = lookup(Process::SheetMetal, MaterialClass::Aluminum).unwrap();
        assert!(c.min_bend_radius_factor > 0.0, "Sheet metal should have bend radius factor");
    }

    // --- FDM nozzle variants ------------------------------------------------

    #[test]
    fn fdm_04_pla_lookup() {
        let c = lookup(Process::Fdm04, MaterialClass::Pla).unwrap();
        assert!(c.min_wall_thickness.to_mm() >= 0.7);
        assert!(c.min_wall_thickness.to_mm() <= 1.0);
    }

    #[test]
    fn fdm_02_finer_than_04() {
        let c02 = lookup(Process::Fdm02, MaterialClass::Pla).unwrap();
        let c04 = lookup(Process::Fdm04, MaterialClass::Pla).unwrap();
        assert!(c02.min_wall_thickness < c04.min_wall_thickness,
            "0.2mm nozzle should allow thinner walls than 0.4mm");
        assert!(c02.tolerance_standard < c04.tolerance_standard,
            "0.2mm nozzle should achieve tighter tolerance");
    }

    #[test]
    fn fdm_08_coarser_than_04() {
        let c04 = lookup(Process::Fdm04, MaterialClass::Pla).unwrap();
        let c08 = lookup(Process::Fdm08, MaterialClass::Pla).unwrap();
        assert!(c08.min_wall_thickness > c04.min_wall_thickness,
            "0.8mm nozzle should require thicker walls than 0.4mm");
    }

    #[test]
    fn fdm_all_materials_covered_04() {
        let materials = [
            MaterialClass::Pla, MaterialClass::Abs, MaterialClass::Petg,
            MaterialClass::Pa, MaterialClass::Pc, MaterialClass::Tpu,
        ];
        for m in &materials {
            assert!(lookup(Process::Fdm04, *m).is_some(),
                "FDM 0.4mm missing material {:?}", m);
        }
    }

    // --- SLA / DLP resin variants -------------------------------------------

    #[test]
    fn sla_resin_types() {
        assert!(lookup(Process::Sla, MaterialClass::ResinStandard).is_some());
        assert!(lookup(Process::Sla, MaterialClass::ResinTough).is_some());
        assert!(lookup(Process::Sla, MaterialClass::ResinFlexible).is_some());
    }

    #[test]
    fn dlp_resin_types() {
        assert!(lookup(Process::Dlp, MaterialClass::ResinStandard).is_some());
        assert!(lookup(Process::Dlp, MaterialClass::ResinTough).is_some());
        assert!(lookup(Process::Dlp, MaterialClass::ResinFlexible).is_some());
    }

    // --- SLS / MJF ----------------------------------------------------------

    #[test]
    fn sls_powder_types() {
        assert!(lookup(Process::Sls, MaterialClass::Pa12).is_some());
        assert!(lookup(Process::Sls, MaterialClass::Pa11).is_some());
        assert!(lookup(Process::Sls, MaterialClass::Tpu).is_some());
    }

    #[test]
    fn mjf_powder_types() {
        assert!(lookup(Process::Mjf, MaterialClass::Pa12).is_some());
        assert!(lookup(Process::Mjf, MaterialClass::Pa11).is_some());
        assert!(lookup(Process::Mjf, MaterialClass::Tpu).is_some());
    }

    // --- Die casting --------------------------------------------------------

    #[test]
    fn die_casting_specific_alloys() {
        let a380 = lookup(Process::DieCasting, MaterialClass::AlA380).unwrap();
        let zamak3 = lookup(Process::DieCasting, MaterialClass::ZincZamak3).unwrap();
        let mg = lookup(Process::DieCasting, MaterialClass::Magnesium).unwrap();

        // Zinc allows thinner walls than aluminum
        assert!(zamak3.min_wall_thickness < a380.min_wall_thickness);
        // All should require draft
        assert!(a380.draft_angle_min.to_deg() >= 0.5);
        assert!(zamak3.draft_angle_min.to_deg() >= 0.5);
        assert!(mg.draft_angle_min.to_deg() >= 1.0);
    }

    // --- Injection molding per-polymer --------------------------------------

    #[test]
    fn injection_mold_specific_polymers() {
        let abs = lookup(Process::InjectionMold, MaterialClass::Abs).unwrap();
        let pp = lookup(Process::InjectionMold, MaterialClass::Pp).unwrap();
        let pe = lookup(Process::InjectionMold, MaterialClass::Pe).unwrap();
        let pa = lookup(Process::InjectionMold, MaterialClass::Pa).unwrap();
        let pc = lookup(Process::InjectionMold, MaterialClass::Pc).unwrap();
        let pom = lookup(Process::InjectionMold, MaterialClass::Pom).unwrap();

        // All should have max wall thickness
        assert!(abs.max_wall_thickness.to_mm() > 0.0);
        assert!(pp.max_wall_thickness.to_mm() > 0.0);
        assert!(pe.max_wall_thickness.to_mm() > 0.0);
        assert!(pa.max_wall_thickness.to_mm() > 0.0);
        assert!(pc.max_wall_thickness.to_mm() > 0.0);
        assert!(pom.max_wall_thickness.to_mm() > 0.0);

        // PP has lower min wall than ABS
        assert!(pp.min_wall_thickness <= abs.min_wall_thickness);
    }

    #[test]
    fn check_max_wall_injection_mold() {
        // 2mm wall in ABS should pass (max is 3.5mm)
        assert_eq!(
            check_max_wall_thickness(Length::mm(2.0), Process::InjectionMold, MaterialClass::Abs),
            Some(true)
        );
        // 5mm wall in ABS should fail (max is 3.5mm)
        assert_eq!(
            check_max_wall_thickness(Length::mm(5.0), Process::InjectionMold, MaterialClass::Abs),
            Some(false)
        );
    }

    // --- Investment casting --------------------------------------------------

    #[test]
    fn investment_casting_expanded() {
        assert!(lookup(Process::InvestmentCast, MaterialClass::Titanium).is_some());
        assert!(lookup(Process::InvestmentCast, MaterialClass::MildSteel).is_some());
    }

    // --- Forging ------------------------------------------------------------

    #[test]
    fn forging_constraints() {
        let al = lookup(Process::Forging, MaterialClass::Aluminum).unwrap();
        let ti = lookup(Process::Forging, MaterialClass::Titanium).unwrap();

        // Forging requires thick walls
        assert!(al.min_wall_thickness.to_mm() >= 3.0);
        // Titanium forging requires higher draft than aluminum
        assert!(ti.draft_angle_min.to_deg() > al.draft_angle_min.to_deg());
    }

    // --- New lookup functions -----------------------------------------------

    #[test]
    fn lookup_by_process_returns_multiple() {
        let cnc3_count = lookup_by_process(Process::CncMill3Ax).count();
        assert!(cnc3_count >= 6, "CNC 3-axis should have many material entries");
    }

    #[test]
    fn lookup_by_material_returns_multiple() {
        let al_count = lookup_by_material(MaterialClass::Aluminum).count();
        assert!(al_count >= 5, "Aluminum should appear in many processes");
    }

    #[test]
    fn check_hole_diameter_cnc() {
        assert_eq!(
            check_hole_diameter(Length::mm(2.0), Process::CncMill3Ax, MaterialClass::Aluminum),
            Some(true)
        );
        assert_eq!(
            check_hole_diameter(Length::mm(0.1), Process::CncMill3Ax, MaterialClass::Aluminum),
            Some(false)
        );
    }

    #[test]
    fn check_aspect_ratio_fdm() {
        assert_eq!(
            check_aspect_ratio(6.0, Process::Fdm04, MaterialClass::Pla),
            Some(true)
        );
        assert_eq!(
            check_aspect_ratio(20.0, Process::Fdm04, MaterialClass::Pla),
            Some(false)
        );
    }

    #[test]
    fn check_draft_angle_die_casting() {
        assert_eq!(
            check_draft_angle(Angle::deg(2.0), Process::DieCasting, MaterialClass::AlA380),
            Some(true)
        );
        assert_eq!(
            check_draft_angle(Angle::deg(0.1), Process::DieCasting, MaterialClass::AlA380),
            Some(false)
        );
    }

    #[test]
    fn surface_finish_ra_lookup() {
        let ra = surface_finish_ra(Process::CncMill3Ax, MaterialClass::Aluminum).unwrap();
        assert!(ra > 0.0 && ra < 10.0, "CNC aluminum Ra should be < 10 µm");

        let ra_fdm = surface_finish_ra(Process::Fdm04, MaterialClass::Pla).unwrap();
        assert!(ra_fdm > 10.0, "FDM Ra should be > 10 µm");
    }

    #[test]
    fn fdm_nozzle_process_mapping() {
        assert_eq!(fdm_process_for_nozzle(0.2), Some(Process::Fdm02));
        assert_eq!(fdm_process_for_nozzle(0.4), Some(Process::Fdm04));
        assert_eq!(fdm_process_for_nozzle(0.6), Some(Process::Fdm06));
        assert_eq!(fdm_process_for_nozzle(0.8), Some(Process::Fdm08));
        assert_eq!(fdm_process_for_nozzle(1.0), None);
    }

    #[test]
    fn constraint_count_at_least_100() {
        assert!(
            constraint_count() >= 100,
            "Expected at least 100 constraint entries, got {}",
            constraint_count()
        );
    }

    // --- Legacy table compat ------------------------------------------------

    #[test]
    fn fdm_pla_lookup() {
        let c = lookup_fdm("PLA").unwrap();
        assert_eq!(c.material_name, "PLA");
        assert!(c.overhang_angle_deg <= 45.0);
        assert!(c.nozzle_temp_c > 200.0);
    }

    #[test]
    fn fdm_peek_lookup() {
        let c = lookup_fdm("PEEK").unwrap();
        assert!(c.nozzle_temp_c >= 370.0, "PEEK nozzle temp should be >= 370C");
    }

    #[test]
    fn fdm_unknown_returns_none() {
        assert!(lookup_fdm("Unobtanium").is_none());
    }

    #[test]
    fn sla_standard_lookup() {
        let c = lookup_sla("Standard").unwrap();
        assert!(c.min_wall_mm <= 1.0);
        assert!(c.cure_shrinkage_pct > 0.0);
    }

    #[test]
    fn sheet_metal_mild_steel_1mm() {
        let c = lookup_sheet_metal(MaterialClass::MildSteel, 1.0).unwrap();
        assert!((c.thickness_mm - 1.0).abs() < 0.1);
        assert!(c.k_factor > 0.3 && c.k_factor < 0.6);
    }

    #[test]
    fn k_factor_aluminum_lookup() {
        let e = lookup_k_factor(MaterialClass::Aluminum, 1.0, 1.0).unwrap();
        assert!(e.k_factor > 0.3 && e.k_factor < 0.5);
    }

    #[test]
    fn k_factor_stainless_high_rt() {
        let e = lookup_k_factor(MaterialClass::Stainless, 1.0, 5.0).unwrap();
        assert!(e.k_factor >= 0.48);
    }

    #[test]
    fn tool_lookup_4mm_4flute() {
        let t = lookup_tool(4.0, 4).unwrap();
        assert_eq!(t.diameter_mm, 4.0);
        assert_eq!(t.flutes, 4);
        assert!(t.aluminum_sfm > 500.0);
    }

    #[test]
    fn tool_lookup_fallback_flutes() {
        let t = lookup_tool(25.0, 2);
        assert!(t.is_some());
        assert!(t.unwrap().diameter_mm >= 25.0);
    }

    #[test]
    fn tool_lookup_too_large_returns_none() {
        assert!(lookup_tool(100.0, 4).is_none());
    }

    #[test]
    fn dmls_titanium_lookup() {
        let c = lookup_dmls(MaterialClass::Titanium).unwrap();
        assert!(c.min_wall_mm <= 0.5);
        assert!(c.support_angle_deg <= 45.0);
    }

    #[test]
    fn turning_nickel_alloy_ltd() {
        let c = lookup_turning(MaterialClass::NickelAlloy).unwrap();
        assert!(c.max_length_to_diameter <= 5.0);
    }
}
