//! Engineering standards lookup tables.
//!
//! Contains accurate data from ISO 262, ISO 286, ASME B1.1, ASME B36.10,
//! ASTM E140, IEC 60228, and general machining references.
//!
//! LUT first. If the answer exists in a table, never compute it.

// ---------------------------------------------------------------------------
// 1. ISO 262 — Metric Threads (M1 through M100)
// ---------------------------------------------------------------------------
// Source: ISO 262:1998, ISO 724:1993
// Minor diameter (D1) and pitch diameter (D2) per ISO 724 formulae:
//   D2 = d - 0.6495 * P
//   D1 = d - 1.0825 * P
// Tensile stress area: As = (pi/4) * ((D2 + D1)/2)^2  per ISO 898-1

#[derive(Debug, Clone, Copy)]
pub struct MetricThread {
    pub designation: &'static str,
    pub nominal_diameter_mm: f64,
    pub coarse_pitch_mm: f64,
    pub fine_pitches_mm: &'static [f64],
    pub minor_diameter_mm: f64,
    pub pitch_diameter_mm: f64,
    pub tensile_stress_area_mm2: f64,
}

pub static METRIC_THREADS: &[MetricThread] = &[
    MetricThread { designation: "M1",   nominal_diameter_mm:   1.0,  coarse_pitch_mm: 0.25,  fine_pitches_mm: &[0.2],                              minor_diameter_mm:  0.729,  pitch_diameter_mm:  0.838,  tensile_stress_area_mm2:    0.460 },
    MetricThread { designation: "M1.2", nominal_diameter_mm:   1.2,  coarse_pitch_mm: 0.25,  fine_pitches_mm: &[0.2],                              minor_diameter_mm:  0.929,  pitch_diameter_mm:  1.038,  tensile_stress_area_mm2:    0.732 },
    MetricThread { designation: "M1.4", nominal_diameter_mm:   1.4,  coarse_pitch_mm: 0.3,   fine_pitches_mm: &[0.2],                              minor_diameter_mm:  1.075,  pitch_diameter_mm:  1.205,  tensile_stress_area_mm2:    0.983 },
    MetricThread { designation: "M1.6", nominal_diameter_mm:   1.6,  coarse_pitch_mm: 0.35,  fine_pitches_mm: &[0.2],                              minor_diameter_mm:  1.221,  pitch_diameter_mm:  1.373,  tensile_stress_area_mm2:    1.27 },
    MetricThread { designation: "M2",   nominal_diameter_mm:   2.0,  coarse_pitch_mm: 0.4,   fine_pitches_mm: &[0.25],                             minor_diameter_mm:  1.567,  pitch_diameter_mm:  1.740,  tensile_stress_area_mm2:    2.07 },
    MetricThread { designation: "M2.5", nominal_diameter_mm:   2.5,  coarse_pitch_mm: 0.45,  fine_pitches_mm: &[0.35],                             minor_diameter_mm:  2.013,  pitch_diameter_mm:  2.208,  tensile_stress_area_mm2:    3.39 },
    MetricThread { designation: "M3",   nominal_diameter_mm:   3.0,  coarse_pitch_mm: 0.5,   fine_pitches_mm: &[0.35],                             minor_diameter_mm:  2.459,  pitch_diameter_mm:  2.675,  tensile_stress_area_mm2:    5.03 },
    MetricThread { designation: "M3.5", nominal_diameter_mm:   3.5,  coarse_pitch_mm: 0.6,   fine_pitches_mm: &[0.35],                             minor_diameter_mm:  2.850,  pitch_diameter_mm:  3.110,  tensile_stress_area_mm2:    6.78 },
    MetricThread { designation: "M4",   nominal_diameter_mm:   4.0,  coarse_pitch_mm: 0.7,   fine_pitches_mm: &[0.5],                              minor_diameter_mm:  3.242,  pitch_diameter_mm:  3.545,  tensile_stress_area_mm2:    8.78 },
    MetricThread { designation: "M5",   nominal_diameter_mm:   5.0,  coarse_pitch_mm: 0.8,   fine_pitches_mm: &[0.5],                              minor_diameter_mm:  4.134,  pitch_diameter_mm:  4.480,  tensile_stress_area_mm2:   14.2 },
    MetricThread { designation: "M6",   nominal_diameter_mm:   6.0,  coarse_pitch_mm: 1.0,   fine_pitches_mm: &[0.75],                             minor_diameter_mm:  4.917,  pitch_diameter_mm:  5.350,  tensile_stress_area_mm2:   20.1 },
    MetricThread { designation: "M7",   nominal_diameter_mm:   7.0,  coarse_pitch_mm: 1.0,   fine_pitches_mm: &[0.75],                             minor_diameter_mm:  5.917,  pitch_diameter_mm:  6.350,  tensile_stress_area_mm2:   28.9 },
    MetricThread { designation: "M8",   nominal_diameter_mm:   8.0,  coarse_pitch_mm: 1.25,  fine_pitches_mm: &[1.0, 0.75],                        minor_diameter_mm:  6.647,  pitch_diameter_mm:  7.188,  tensile_stress_area_mm2:   36.6 },
    MetricThread { designation: "M10",  nominal_diameter_mm:  10.0,  coarse_pitch_mm: 1.5,   fine_pitches_mm: &[1.25, 1.0, 0.75],                  minor_diameter_mm:  8.376,  pitch_diameter_mm:  9.026,  tensile_stress_area_mm2:   58.0 },
    MetricThread { designation: "M12",  nominal_diameter_mm:  12.0,  coarse_pitch_mm: 1.75,  fine_pitches_mm: &[1.5, 1.25, 1.0],                   minor_diameter_mm: 10.106,  pitch_diameter_mm: 10.863,  tensile_stress_area_mm2:   84.3 },
    MetricThread { designation: "M14",  nominal_diameter_mm:  14.0,  coarse_pitch_mm: 2.0,   fine_pitches_mm: &[1.5, 1.25, 1.0],                   minor_diameter_mm: 11.835,  pitch_diameter_mm: 12.701,  tensile_stress_area_mm2:  115.0 },
    MetricThread { designation: "M16",  nominal_diameter_mm:  16.0,  coarse_pitch_mm: 2.0,   fine_pitches_mm: &[1.5, 1.0],                         minor_diameter_mm: 13.835,  pitch_diameter_mm: 14.701,  tensile_stress_area_mm2:  157.0 },
    MetricThread { designation: "M18",  nominal_diameter_mm:  18.0,  coarse_pitch_mm: 2.5,   fine_pitches_mm: &[2.0, 1.5, 1.0],                    minor_diameter_mm: 15.294,  pitch_diameter_mm: 16.376,  tensile_stress_area_mm2:  192.0 },
    MetricThread { designation: "M20",  nominal_diameter_mm:  20.0,  coarse_pitch_mm: 2.5,   fine_pitches_mm: &[2.0, 1.5, 1.0],                    minor_diameter_mm: 17.294,  pitch_diameter_mm: 18.376,  tensile_stress_area_mm2:  245.0 },
    MetricThread { designation: "M22",  nominal_diameter_mm:  22.0,  coarse_pitch_mm: 2.5,   fine_pitches_mm: &[2.0, 1.5, 1.0],                    minor_diameter_mm: 19.294,  pitch_diameter_mm: 20.376,  tensile_stress_area_mm2:  303.0 },
    MetricThread { designation: "M24",  nominal_diameter_mm:  24.0,  coarse_pitch_mm: 3.0,   fine_pitches_mm: &[2.0, 1.5, 1.0],                    minor_diameter_mm: 20.752,  pitch_diameter_mm: 22.051,  tensile_stress_area_mm2:  353.0 },
    MetricThread { designation: "M27",  nominal_diameter_mm:  27.0,  coarse_pitch_mm: 3.0,   fine_pitches_mm: &[2.0, 1.5, 1.0],                    minor_diameter_mm: 23.752,  pitch_diameter_mm: 25.051,  tensile_stress_area_mm2:  459.0 },
    MetricThread { designation: "M30",  nominal_diameter_mm:  30.0,  coarse_pitch_mm: 3.5,   fine_pitches_mm: &[3.0, 2.0, 1.5],                    minor_diameter_mm: 26.211,  pitch_diameter_mm: 27.727,  tensile_stress_area_mm2:  561.0 },
    MetricThread { designation: "M33",  nominal_diameter_mm:  33.0,  coarse_pitch_mm: 3.5,   fine_pitches_mm: &[3.0, 2.0, 1.5],                    minor_diameter_mm: 29.211,  pitch_diameter_mm: 30.727,  tensile_stress_area_mm2:  694.0 },
    MetricThread { designation: "M36",  nominal_diameter_mm:  36.0,  coarse_pitch_mm: 4.0,   fine_pitches_mm: &[3.0, 2.0, 1.5],                    minor_diameter_mm: 31.670,  pitch_diameter_mm: 33.402,  tensile_stress_area_mm2:  817.0 },
    MetricThread { designation: "M39",  nominal_diameter_mm:  39.0,  coarse_pitch_mm: 4.0,   fine_pitches_mm: &[3.0, 2.0, 1.5],                    minor_diameter_mm: 34.670,  pitch_diameter_mm: 36.402,  tensile_stress_area_mm2:  976.0 },
    MetricThread { designation: "M42",  nominal_diameter_mm:  42.0,  coarse_pitch_mm: 4.5,   fine_pitches_mm: &[4.0, 3.0, 2.0, 1.5],              minor_diameter_mm: 37.129,  pitch_diameter_mm: 39.077,  tensile_stress_area_mm2: 1120.0 },
    MetricThread { designation: "M45",  nominal_diameter_mm:  45.0,  coarse_pitch_mm: 4.5,   fine_pitches_mm: &[4.0, 3.0, 2.0, 1.5],              minor_diameter_mm: 40.129,  pitch_diameter_mm: 42.077,  tensile_stress_area_mm2: 1310.0 },
    MetricThread { designation: "M48",  nominal_diameter_mm:  48.0,  coarse_pitch_mm: 5.0,   fine_pitches_mm: &[4.0, 3.0, 2.0, 1.5],              minor_diameter_mm: 42.587,  pitch_diameter_mm: 44.752,  tensile_stress_area_mm2: 1470.0 },
    MetricThread { designation: "M52",  nominal_diameter_mm:  52.0,  coarse_pitch_mm: 5.0,   fine_pitches_mm: &[4.0, 3.0, 2.0, 1.5],              minor_diameter_mm: 46.587,  pitch_diameter_mm: 48.752,  tensile_stress_area_mm2: 1760.0 },
    MetricThread { designation: "M56",  nominal_diameter_mm:  56.0,  coarse_pitch_mm: 5.5,   fine_pitches_mm: &[4.0, 3.0, 2.0, 1.5],              minor_diameter_mm: 50.046,  pitch_diameter_mm: 52.428,  tensile_stress_area_mm2: 2030.0 },
    MetricThread { designation: "M60",  nominal_diameter_mm:  60.0,  coarse_pitch_mm: 5.5,   fine_pitches_mm: &[4.0, 3.0, 2.0, 1.5],              minor_diameter_mm: 54.046,  pitch_diameter_mm: 56.428,  tensile_stress_area_mm2: 2360.0 },
    MetricThread { designation: "M64",  nominal_diameter_mm:  64.0,  coarse_pitch_mm: 6.0,   fine_pitches_mm: &[4.0, 3.0, 2.0, 1.5],              minor_diameter_mm: 57.505,  pitch_diameter_mm: 60.103,  tensile_stress_area_mm2: 2680.0 },
    MetricThread { designation: "M68",  nominal_diameter_mm:  68.0,  coarse_pitch_mm: 6.0,   fine_pitches_mm: &[4.0, 3.0, 2.0, 1.5],              minor_diameter_mm: 61.505,  pitch_diameter_mm: 64.103,  tensile_stress_area_mm2: 3060.0 },
    MetricThread { designation: "M72",  nominal_diameter_mm:  72.0,  coarse_pitch_mm: 6.0,   fine_pitches_mm: &[4.0, 3.0, 2.0, 1.5],              minor_diameter_mm: 65.505,  pitch_diameter_mm: 68.103,  tensile_stress_area_mm2: 3460.0 },
    MetricThread { designation: "M76",  nominal_diameter_mm:  76.0,  coarse_pitch_mm: 6.0,   fine_pitches_mm: &[4.0, 3.0, 2.0, 1.5],              minor_diameter_mm: 69.505,  pitch_diameter_mm: 72.103,  tensile_stress_area_mm2: 3880.0 },
    MetricThread { designation: "M80",  nominal_diameter_mm:  80.0,  coarse_pitch_mm: 6.0,   fine_pitches_mm: &[4.0, 3.0, 2.0, 1.5],              minor_diameter_mm: 73.505,  pitch_diameter_mm: 76.103,  tensile_stress_area_mm2: 4340.0 },
    MetricThread { designation: "M90",  nominal_diameter_mm:  90.0,  coarse_pitch_mm: 6.0,   fine_pitches_mm: &[4.0, 3.0, 2.0],                   minor_diameter_mm: 83.505,  pitch_diameter_mm: 86.103,  tensile_stress_area_mm2: 5590.0 },
    MetricThread { designation: "M100", nominal_diameter_mm: 100.0,  coarse_pitch_mm: 6.0,   fine_pitches_mm: &[4.0, 3.0, 2.0],                   minor_diameter_mm: 93.505,  pitch_diameter_mm: 96.103,  tensile_stress_area_mm2: 6990.0 },
];

// ---------------------------------------------------------------------------
// 2. UNC / UNF Threads — ASME B1.1
// ---------------------------------------------------------------------------
// Source: ASME B1.1-2019, Unified Inch Screw Threads
// D2 = d - 0.6495/TPI, D1 = d - 1.0825/TPI
// As = (pi/4)*((D2+D1)/2)^2

#[derive(Debug, Clone, Copy)]
pub struct UncThread {
    pub designation: &'static str,
    pub nominal_diameter_inch: f64,
    pub tpi: f64,
    pub minor_diameter_inch: f64,
    pub pitch_diameter_inch: f64,
    pub tensile_stress_area_in2: f64,
}

pub static UNC_THREADS: &[UncThread] = &[
    UncThread { designation: "#0-80",     nominal_diameter_inch: 0.0600, tpi: 80.0,  minor_diameter_inch: 0.0447, pitch_diameter_inch: 0.0519, tensile_stress_area_in2: 0.00180 },
    UncThread { designation: "#1-64",     nominal_diameter_inch: 0.0730, tpi: 64.0,  minor_diameter_inch: 0.0561, pitch_diameter_inch: 0.0629, tensile_stress_area_in2: 0.00263 },
    UncThread { designation: "#2-56",     nominal_diameter_inch: 0.0860, tpi: 56.0,  minor_diameter_inch: 0.0667, pitch_diameter_inch: 0.0744, tensile_stress_area_in2: 0.00370 },
    UncThread { designation: "#3-48",     nominal_diameter_inch: 0.0990, tpi: 48.0,  minor_diameter_inch: 0.0764, pitch_diameter_inch: 0.0855, tensile_stress_area_in2: 0.00487 },
    UncThread { designation: "#4-40",     nominal_diameter_inch: 0.1120, tpi: 40.0,  minor_diameter_inch: 0.0849, pitch_diameter_inch: 0.0958, tensile_stress_area_in2: 0.00604 },
    UncThread { designation: "#5-40",     nominal_diameter_inch: 0.1250, tpi: 40.0,  minor_diameter_inch: 0.0979, pitch_diameter_inch: 0.1088, tensile_stress_area_in2: 0.00796 },
    UncThread { designation: "#6-32",     nominal_diameter_inch: 0.1380, tpi: 32.0,  minor_diameter_inch: 0.1042, pitch_diameter_inch: 0.1177, tensile_stress_area_in2: 0.00909 },
    UncThread { designation: "#8-32",     nominal_diameter_inch: 0.1640, tpi: 32.0,  minor_diameter_inch: 0.1302, pitch_diameter_inch: 0.1437, tensile_stress_area_in2: 0.01400 },
    UncThread { designation: "#10-24",    nominal_diameter_inch: 0.1900, tpi: 24.0,  minor_diameter_inch: 0.1449, pitch_diameter_inch: 0.1629, tensile_stress_area_in2: 0.01750 },
    UncThread { designation: "#10-32",    nominal_diameter_inch: 0.1900, tpi: 32.0,  minor_diameter_inch: 0.1562, pitch_diameter_inch: 0.1697, tensile_stress_area_in2: 0.02000 },
    UncThread { designation: "1/4-20",    nominal_diameter_inch: 0.2500, tpi: 20.0,  minor_diameter_inch: 0.1959, pitch_diameter_inch: 0.2175, tensile_stress_area_in2: 0.03180 },
    UncThread { designation: "5/16-18",   nominal_diameter_inch: 0.3125, tpi: 18.0,  minor_diameter_inch: 0.2524, pitch_diameter_inch: 0.2764, tensile_stress_area_in2: 0.05240 },
    UncThread { designation: "3/8-16",    nominal_diameter_inch: 0.3750, tpi: 16.0,  minor_diameter_inch: 0.3073, pitch_diameter_inch: 0.3344, tensile_stress_area_in2: 0.07750 },
    UncThread { designation: "7/16-14",   nominal_diameter_inch: 0.4375, tpi: 14.0,  minor_diameter_inch: 0.3602, pitch_diameter_inch: 0.3911, tensile_stress_area_in2: 0.1063  },
    UncThread { designation: "1/2-13",    nominal_diameter_inch: 0.5000, tpi: 13.0,  minor_diameter_inch: 0.4167, pitch_diameter_inch: 0.4500, tensile_stress_area_in2: 0.1419  },
    UncThread { designation: "9/16-12",   nominal_diameter_inch: 0.5625, tpi: 12.0,  minor_diameter_inch: 0.4723, pitch_diameter_inch: 0.5084, tensile_stress_area_in2: 0.1820  },
    UncThread { designation: "5/8-11",    nominal_diameter_inch: 0.6250, tpi: 11.0,  minor_diameter_inch: 0.5266, pitch_diameter_inch: 0.5660, tensile_stress_area_in2: 0.2260  },
    UncThread { designation: "3/4-10",    nominal_diameter_inch: 0.7500, tpi: 10.0,  minor_diameter_inch: 0.6417, pitch_diameter_inch: 0.6850, tensile_stress_area_in2: 0.3340  },
    UncThread { designation: "7/8-9",     nominal_diameter_inch: 0.8750, tpi:  9.0,  minor_diameter_inch: 0.7547, pitch_diameter_inch: 0.8028, tensile_stress_area_in2: 0.4620  },
    UncThread { designation: "1\"-8",     nominal_diameter_inch: 1.0000, tpi:  8.0,  minor_diameter_inch: 0.8647, pitch_diameter_inch: 0.9188, tensile_stress_area_in2: 0.6060  },
    UncThread { designation: "1-1/8\"-7", nominal_diameter_inch: 1.1250, tpi:  7.0,  minor_diameter_inch: 0.9704, pitch_diameter_inch: 1.0322, tensile_stress_area_in2: 0.7630  },
    UncThread { designation: "1-1/4\"-7", nominal_diameter_inch: 1.2500, tpi:  7.0,  minor_diameter_inch: 1.0954, pitch_diameter_inch: 1.1572, tensile_stress_area_in2: 0.9690  },
    UncThread { designation: "1-1/2\"-6", nominal_diameter_inch: 1.5000, tpi:  6.0,  minor_diameter_inch: 1.3196, pitch_diameter_inch: 1.3917, tensile_stress_area_in2: 1.4050  },
    UncThread { designation: "1-3/4\"-5", nominal_diameter_inch: 1.7500, tpi:  5.0,  minor_diameter_inch: 1.5335, pitch_diameter_inch: 1.6201, tensile_stress_area_in2: 1.9000  },
    UncThread { designation: "2\"-4.5",   nominal_diameter_inch: 2.0000, tpi:  4.5,  minor_diameter_inch: 1.7594, pitch_diameter_inch: 1.8557, tensile_stress_area_in2: 2.5000  },
];

// ---------------------------------------------------------------------------
// 3. ISO 286 — IT Tolerance Grades
// ---------------------------------------------------------------------------
// Source: ISO 286-1:2010, Table 1
// Tolerances in micrometres for nominal size ranges (mm):
//   [0-3, 3-6, 6-10, 10-18, 18-30, 30-50, 50-80, 80-120, 120-180, 180-250, 250-315, 315-400, 400-500]

#[derive(Debug, Clone, Copy)]
pub struct ItGrade {
    pub grade: u8,
    /// Tolerance in micrometres per nominal size range.
    /// Index 0: 0..3mm, 1: 3..6mm, 2: 6..10mm, 3: 10..18mm, 4: 18..30mm,
    /// 5: 30..50mm, 6: 50..80mm, 7: 80..120mm, 8: 120..180mm, 9: 180..250mm,
    /// 10: 250..315mm, 11: 315..400mm, 12: 400..500mm
    pub tolerances_um: [f64; 13],
}

pub static IT_GRADES: &[ItGrade] = &[
    ItGrade { grade: 5,  tolerances_um: [  4.0,   5.0,   6.0,   8.0,   9.0,  11.0,  13.0,  15.0,  18.0,  20.0,  23.0,  25.0,  27.0] },
    ItGrade { grade: 6,  tolerances_um: [  6.0,   8.0,   9.0,  11.0,  13.0,  16.0,  19.0,  22.0,  25.0,  29.0,  32.0,  36.0,  40.0] },
    ItGrade { grade: 7,  tolerances_um: [ 10.0,  12.0,  15.0,  18.0,  21.0,  25.0,  30.0,  35.0,  40.0,  46.0,  52.0,  57.0,  63.0] },
    ItGrade { grade: 8,  tolerances_um: [ 14.0,  18.0,  22.0,  27.0,  33.0,  39.0,  46.0,  54.0,  63.0,  72.0,  81.0,  89.0,  97.0] },
    ItGrade { grade: 9,  tolerances_um: [ 25.0,  30.0,  36.0,  43.0,  52.0,  62.0,  74.0,  87.0, 100.0, 115.0, 130.0, 140.0, 155.0] },
    ItGrade { grade: 10, tolerances_um: [ 40.0,  48.0,  58.0,  70.0,  84.0, 100.0, 120.0, 140.0, 160.0, 185.0, 210.0, 230.0, 250.0] },
    ItGrade { grade: 11, tolerances_um: [ 60.0,  75.0,  90.0, 110.0, 130.0, 160.0, 190.0, 220.0, 250.0, 290.0, 320.0, 360.0, 400.0] },
    ItGrade { grade: 12, tolerances_um: [100.0, 120.0, 150.0, 180.0, 210.0, 250.0, 300.0, 350.0, 400.0, 460.0, 520.0, 570.0, 630.0] },
    ItGrade { grade: 13, tolerances_um: [140.0, 180.0, 220.0, 270.0, 330.0, 390.0, 460.0, 540.0, 630.0, 720.0, 810.0, 890.0, 970.0] },
    ItGrade { grade: 14, tolerances_um: [250.0, 300.0, 360.0, 430.0, 520.0, 620.0, 740.0, 870.0, 1000.0, 1150.0, 1300.0, 1400.0, 1550.0] },
    ItGrade { grade: 15, tolerances_um: [400.0, 480.0, 580.0, 700.0, 840.0, 1000.0, 1200.0, 1400.0, 1600.0, 1850.0, 2100.0, 2300.0, 2500.0] },
    ItGrade { grade: 16, tolerances_um: [600.0, 750.0, 900.0, 1100.0, 1300.0, 1600.0, 1900.0, 2200.0, 2500.0, 2900.0, 3200.0, 3600.0, 4000.0] },
];

// ---------------------------------------------------------------------------
// 4. Common Fit Pairs — ISO 286
// ---------------------------------------------------------------------------
// Source: ISO 286-1:2010, commonly referenced fit combinations

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FitType {
    Clearance,
    Transition,
    Interference,
}

#[derive(Debug, Clone, Copy)]
pub struct ToleranceFit {
    pub designation: &'static str,
    pub fit_type: FitType,
    pub description: &'static str,
    pub typical_use: &'static str,
}

pub static TOLERANCE_FITS: &[ToleranceFit] = &[
    ToleranceFit { designation: "H11/c11", fit_type: FitType::Clearance,    description: "Extra-loose running fit",       typical_use: "Agricultural machinery, rough pivots, large clearance assemblies" },
    ToleranceFit { designation: "H9/d9",   fit_type: FitType::Clearance,    description: "Free running fit",             typical_use: "Shafts in pillow blocks, general-purpose bearings, gearbox shafts" },
    ToleranceFit { designation: "H8/e8",   fit_type: FitType::Clearance,    description: "Close running fit",            typical_use: "Crankshaft journals, precision spindles in loose bearings" },
    ToleranceFit { designation: "H8/f7",   fit_type: FitType::Clearance,    description: "Loose running fit",            typical_use: "General-purpose running fit, machine tool slides" },
    ToleranceFit { designation: "H7/g6",   fit_type: FitType::Clearance,    description: "Sliding fit",                  typical_use: "Spigot and location fits, sliding gears, piston in cylinder" },
    ToleranceFit { designation: "H7/h6",   fit_type: FitType::Clearance,    description: "Locational clearance fit",     typical_use: "Location of stationary parts, snug fit assembled by hand" },
    ToleranceFit { designation: "H6/g5",   fit_type: FitType::Clearance,    description: "Precision sliding fit",        typical_use: "Gauge making, precision instrument spindles" },
    ToleranceFit { designation: "H6/h5",   fit_type: FitType::Clearance,    description: "Precision locational fit",     typical_use: "Precision location, jig bushings, precision assemblies" },
    ToleranceFit { designation: "H7/js6",  fit_type: FitType::Transition,   description: "Snug fit (transition)",        typical_use: "Light keyed shaft, locating ring on shaft, small interference possible" },
    ToleranceFit { designation: "H7/k6",   fit_type: FitType::Transition,   description: "Locational transition fit",    typical_use: "Keyed assemblies, hub on shaft, coupling halves" },
    ToleranceFit { designation: "H7/m6",   fit_type: FitType::Transition,   description: "Medium transition fit",        typical_use: "Gear on shaft with key, impeller on shaft" },
    ToleranceFit { designation: "H7/n6",   fit_type: FitType::Interference, description: "Locational interference fit",  typical_use: "Light press fit for dowel pins, tight keyed connections" },
    ToleranceFit { designation: "H7/p6",   fit_type: FitType::Interference, description: "Light press fit",              typical_use: "Press-fit bushings, bearing races in housings, semi-permanent assemblies" },
    ToleranceFit { designation: "H7/r6",   fit_type: FitType::Interference, description: "Medium press fit",             typical_use: "Bearing inner race on shaft, gear on shaft without key" },
    ToleranceFit { designation: "H7/s6",   fit_type: FitType::Interference, description: "Heavy press fit",              typical_use: "Permanent assemblies, bronze ring in wheel hub, railway wheel on axle" },
    ToleranceFit { designation: "H7/u6",   fit_type: FitType::Interference, description: "Force fit",                    typical_use: "Parts permanently joined, shrink fits for heavy-duty applications" },
];

// ---------------------------------------------------------------------------
// 5. Surface Finish — Ra roughness by process
// ---------------------------------------------------------------------------
// Source: Machinery's Handbook 31st ed., Oberg et al.; various manufacturing
// handbooks. Values are typical Ra in micrometres (um).

#[derive(Debug, Clone, Copy)]
pub struct SurfaceFinish {
    pub process: &'static str,
    pub ra_typical_um: f64,
    pub ra_range_um: (f64, f64),
    pub rz_typical_um: f64,
}

/// Approximate Rz from Ra: Rz ~ 4..6 * Ra depending on process.
/// The typical factor 4.5 is used for conventional machining,
/// 5.0 for casting/forging, 4.0 for finishing operations.
pub static SURFACE_FINISHES: &[SurfaceFinish] = &[
    // Conventional machining
    SurfaceFinish { process: "Turning (rough)",         ra_typical_um: 6.3,    ra_range_um: (3.2,   12.5),   rz_typical_um: 28.0   },
    SurfaceFinish { process: "Turning (finish)",        ra_typical_um: 1.6,    ra_range_um: (0.4,    3.2),   rz_typical_um:  7.2   },
    SurfaceFinish { process: "Milling (rough)",         ra_typical_um: 6.3,    ra_range_um: (3.2,   12.5),   rz_typical_um: 28.0   },
    SurfaceFinish { process: "Milling (finish)",        ra_typical_um: 1.6,    ra_range_um: (0.8,    3.2),   rz_typical_um:  7.2   },
    SurfaceFinish { process: "Drilling",                ra_typical_um: 3.2,    ra_range_um: (1.6,    6.3),   rz_typical_um: 14.4   },
    SurfaceFinish { process: "Reaming",                 ra_typical_um: 1.6,    ra_range_um: (0.4,    3.2),   rz_typical_um:  7.2   },
    SurfaceFinish { process: "Boring",                  ra_typical_um: 1.6,    ra_range_um: (0.4,    3.2),   rz_typical_um:  7.2   },
    SurfaceFinish { process: "Broaching",               ra_typical_um: 1.6,    ra_range_um: (0.4,    3.2),   rz_typical_um:  7.2   },
    // Finishing processes
    SurfaceFinish { process: "Grinding",                ra_typical_um: 0.4,    ra_range_um: (0.1,    1.6),   rz_typical_um:  1.6   },
    SurfaceFinish { process: "Lapping",                 ra_typical_um: 0.05,   ra_range_um: (0.012,  0.4),   rz_typical_um:  0.2   },
    SurfaceFinish { process: "Polishing",               ra_typical_um: 0.05,   ra_range_um: (0.006,  0.1),   rz_typical_um:  0.2   },
    SurfaceFinish { process: "Honing",                  ra_typical_um: 0.2,    ra_range_um: (0.05,   0.8),   rz_typical_um:  0.8   },
    SurfaceFinish { process: "Superfinishing",          ra_typical_um: 0.025,  ra_range_um: (0.006,  0.05),  rz_typical_um:  0.1   },
    // Non-traditional machining
    SurfaceFinish { process: "EDM (wire)",              ra_typical_um: 1.6,    ra_range_um: (0.4,    3.2),   rz_typical_um:  8.0   },
    SurfaceFinish { process: "EDM (sinker)",            ra_typical_um: 3.2,    ra_range_um: (0.8,    6.3),   rz_typical_um: 16.0   },
    // Casting & forming
    SurfaceFinish { process: "Sand casting (as-cast)",  ra_typical_um: 25.0,   ra_range_um: (12.5,  50.0),   rz_typical_um: 125.0  },
    SurfaceFinish { process: "Die casting",             ra_typical_um: 1.6,    ra_range_um: (0.8,    3.2),   rz_typical_um:   8.0  },
    SurfaceFinish { process: "Investment casting",      ra_typical_um: 3.2,    ra_range_um: (1.6,    6.3),   rz_typical_um:  16.0  },
    SurfaceFinish { process: "As-forged",               ra_typical_um: 6.3,    ra_range_um: (3.2,   25.0),   rz_typical_um:  31.5  },
    // Additive manufacturing
    SurfaceFinish { process: "FDM 3D printing",         ra_typical_um: 15.0,   ra_range_um: (8.0,   35.0),   rz_typical_um:  75.0  },
    SurfaceFinish { process: "SLA 3D printing",         ra_typical_um: 3.0,    ra_range_um: (1.0,    8.0),   rz_typical_um:  15.0  },
    SurfaceFinish { process: "SLS 3D printing",         ra_typical_um: 12.0,   ra_range_um: (6.0,   20.0),   rz_typical_um:  60.0  },
    SurfaceFinish { process: "MJF 3D printing",         ra_typical_um: 8.0,    ra_range_um: (4.0,   15.0),   rz_typical_um:  40.0  },
    SurfaceFinish { process: "DMLS/SLM 3D printing",    ra_typical_um: 10.0,   ra_range_um: (5.0,   20.0),   rz_typical_um:  50.0  },
    // Rolling
    SurfaceFinish { process: "Hot rolling",             ra_typical_um: 25.0,   ra_range_um: (12.5,  50.0),   rz_typical_um: 125.0  },
    SurfaceFinish { process: "Cold rolling",            ra_typical_um: 1.6,    ra_range_um: (0.8,    3.2),   rz_typical_um:   8.0  },
    SurfaceFinish { process: "Extrusion",               ra_typical_um: 1.6,    ra_range_um: (0.8,    3.2),   rz_typical_um:   8.0  },
];

// ---------------------------------------------------------------------------
// 6. Hardness Conversion — ASTM E140
// ---------------------------------------------------------------------------
// Source: ASTM E140-12b(2019)e1 Standard Hardness Conversion Tables
// for Metals Relationship Among Brinell Hardness, Vickers Hardness,
// Rockwell Hardness, Superficial Hardness, Knoop Hardness, Scleroscope
// Hardness, and Leeb Hardness.

#[derive(Debug, Clone, Copy)]
pub struct HardnessConversion {
    pub hrc: f64,
    pub hrb: Option<f64>,
    pub hv: f64,
    pub hb: f64,
}

pub static HARDNESS_CONVERSIONS: &[HardnessConversion] = &[
    // HRB range (HRC not applicable)
    HardnessConversion { hrc:  0.0, hrb: Some(60.0),  hv: 107.0, hb: 105.0 },
    HardnessConversion { hrc:  0.0, hrb: Some(65.0),  hv: 114.0, hb: 111.0 },
    HardnessConversion { hrc:  0.0, hrb: Some(70.0),  hv: 120.0, hb: 118.0 },
    HardnessConversion { hrc:  0.0, hrb: Some(75.0),  hv: 128.0, hb: 126.0 },
    HardnessConversion { hrc:  0.0, hrb: Some(80.0),  hv: 137.0, hb: 135.0 },
    HardnessConversion { hrc:  0.0, hrb: Some(85.0),  hv: 147.0, hb: 143.0 },
    HardnessConversion { hrc:  0.0, hrb: Some(90.0),  hv: 158.0, hb: 156.0 },
    HardnessConversion { hrc:  0.0, hrb: Some(92.0),  hv: 163.0, hb: 162.0 },
    HardnessConversion { hrc:  0.0, hrb: Some(94.0),  hv: 170.0, hb: 167.0 },
    HardnessConversion { hrc:  0.0, hrb: Some(96.0),  hv: 176.0, hb: 173.0 },
    HardnessConversion { hrc:  0.0, hrb: Some(98.0),  hv: 183.0, hb: 179.0 },
    HardnessConversion { hrc:  0.0, hrb: Some(100.0), hv: 189.0, hb: 187.0 },
    // HRC range (HRB not applicable)
    HardnessConversion { hrc: 20.0, hrb: None, hv: 228.0, hb: 226.0 },
    HardnessConversion { hrc: 21.0, hrb: None, hv: 233.0, hb: 231.0 },
    HardnessConversion { hrc: 22.0, hrb: None, hv: 238.0, hb: 237.0 },
    HardnessConversion { hrc: 23.0, hrb: None, hv: 243.0, hb: 242.0 },
    HardnessConversion { hrc: 24.0, hrb: None, hv: 247.0, hb: 247.0 },
    HardnessConversion { hrc: 25.0, hrb: None, hv: 253.0, hb: 253.0 },
    HardnessConversion { hrc: 26.0, hrb: None, hv: 258.0, hb: 258.0 },
    HardnessConversion { hrc: 27.0, hrb: None, hv: 264.0, hb: 264.0 },
    HardnessConversion { hrc: 28.0, hrb: None, hv: 271.0, hb: 271.0 },
    HardnessConversion { hrc: 29.0, hrb: None, hv: 279.0, hb: 279.0 },
    HardnessConversion { hrc: 30.0, hrb: None, hv: 286.0, hb: 286.0 },
    HardnessConversion { hrc: 31.0, hrb: None, hv: 294.0, hb: 294.0 },
    HardnessConversion { hrc: 32.0, hrb: None, hv: 301.0, hb: 301.0 },
    HardnessConversion { hrc: 33.0, hrb: None, hv: 311.0, hb: 311.0 },
    HardnessConversion { hrc: 34.0, hrb: None, hv: 319.0, hb: 319.0 },
    HardnessConversion { hrc: 35.0, hrb: None, hv: 327.0, hb: 327.0 },
    HardnessConversion { hrc: 36.0, hrb: None, hv: 336.0, hb: 336.0 },
    HardnessConversion { hrc: 37.0, hrb: None, hv: 344.0, hb: 344.0 },
    HardnessConversion { hrc: 38.0, hrb: None, hv: 353.0, hb: 353.0 },
    HardnessConversion { hrc: 39.0, hrb: None, hv: 362.0, hb: 362.0 },
    HardnessConversion { hrc: 40.0, hrb: None, hv: 371.0, hb: 371.0 },
    HardnessConversion { hrc: 41.0, hrb: None, hv: 381.0, hb: 381.0 },
    HardnessConversion { hrc: 42.0, hrb: None, hv: 390.0, hb: 390.0 },
    HardnessConversion { hrc: 43.0, hrb: None, hv: 400.0, hb: 400.0 },
    HardnessConversion { hrc: 44.0, hrb: None, hv: 409.0, hb: 409.0 },
    HardnessConversion { hrc: 45.0, hrb: None, hv: 421.0, hb: 421.0 },
    HardnessConversion { hrc: 46.0, hrb: None, hv: 432.0, hb: 432.0 },
    HardnessConversion { hrc: 47.0, hrb: None, hv: 442.0, hb: 442.0 },
    HardnessConversion { hrc: 48.0, hrb: None, hv: 455.0, hb: 455.0 },
    HardnessConversion { hrc: 49.0, hrb: None, hv: 469.0, hb:   0.0 },
    HardnessConversion { hrc: 50.0, hrb: None, hv: 481.0, hb:   0.0 },
    HardnessConversion { hrc: 51.0, hrb: None, hv: 496.0, hb:   0.0 },
    HardnessConversion { hrc: 52.0, hrb: None, hv: 512.0, hb:   0.0 },
    HardnessConversion { hrc: 53.0, hrb: None, hv: 528.0, hb:   0.0 },
    HardnessConversion { hrc: 54.0, hrb: None, hv: 545.0, hb:   0.0 },
    HardnessConversion { hrc: 55.0, hrb: None, hv: 562.0, hb:   0.0 },
    HardnessConversion { hrc: 56.0, hrb: None, hv: 580.0, hb:   0.0 },
    HardnessConversion { hrc: 57.0, hrb: None, hv: 598.0, hb:   0.0 },
    HardnessConversion { hrc: 58.0, hrb: None, hv: 614.0, hb:   0.0 },
    HardnessConversion { hrc: 59.0, hrb: None, hv: 634.0, hb:   0.0 },
    HardnessConversion { hrc: 60.0, hrb: None, hv: 654.0, hb:   0.0 },
    HardnessConversion { hrc: 61.0, hrb: None, hv: 670.0, hb:   0.0 },
    HardnessConversion { hrc: 62.0, hrb: None, hv: 690.0, hb:   0.0 },
    HardnessConversion { hrc: 63.0, hrb: None, hv: 710.0, hb:   0.0 },
    HardnessConversion { hrc: 64.0, hrb: None, hv: 733.0, hb:   0.0 },
    HardnessConversion { hrc: 65.0, hrb: None, hv: 756.0, hb:   0.0 },
    HardnessConversion { hrc: 66.0, hrb: None, hv: 780.0, hb:   0.0 },
    HardnessConversion { hrc: 67.0, hrb: None, hv: 806.0, hb:   0.0 },
    HardnessConversion { hrc: 68.0, hrb: None, hv: 832.0, hb:   0.0 },
];

// ---------------------------------------------------------------------------
// 7. Wire Gauge — AWG & SWG
// ---------------------------------------------------------------------------
// Source: ASTM B258-14 (AWG), BS 3737 (SWG)
// AWG diameter formula: d_inch = 0.005 * 92^((36 - AWG)/39)
// SWG values from BS 3737 / IEC 60228 reference tables.
// gauge field: -3 = 0000 (4/0), -2 = 000 (3/0), -1 = 00 (2/0), 0 = 0 (1/0)

#[derive(Debug, Clone, Copy)]
pub struct WireGauge {
    pub gauge: i32,
    pub awg_diameter_mm: f64,
    pub swg_diameter_mm: f64,
}

pub static WIRE_GAUGES: &[WireGauge] = &[
    WireGauge { gauge: -3, awg_diameter_mm: 11.684, swg_diameter_mm: 10.160 },  // 0000 (4/0)
    WireGauge { gauge: -2, awg_diameter_mm: 10.405, swg_diameter_mm:  9.449 },  // 000 (3/0)
    WireGauge { gauge: -1, awg_diameter_mm:  9.266, swg_diameter_mm:  8.839 },  // 00 (2/0)
    WireGauge { gauge:  0, awg_diameter_mm:  8.251, swg_diameter_mm:  8.230 },  // 0 (1/0)
    WireGauge { gauge:  1, awg_diameter_mm:  7.348, swg_diameter_mm:  7.620 },
    WireGauge { gauge:  2, awg_diameter_mm:  6.544, swg_diameter_mm:  7.010 },
    WireGauge { gauge:  3, awg_diameter_mm:  5.827, swg_diameter_mm:  6.401 },
    WireGauge { gauge:  4, awg_diameter_mm:  5.189, swg_diameter_mm:  5.893 },
    WireGauge { gauge:  5, awg_diameter_mm:  4.621, swg_diameter_mm:  5.385 },
    WireGauge { gauge:  6, awg_diameter_mm:  4.115, swg_diameter_mm:  4.877 },
    WireGauge { gauge:  7, awg_diameter_mm:  3.665, swg_diameter_mm:  4.470 },
    WireGauge { gauge:  8, awg_diameter_mm:  3.264, swg_diameter_mm:  4.064 },
    WireGauge { gauge:  9, awg_diameter_mm:  2.906, swg_diameter_mm:  3.658 },
    WireGauge { gauge: 10, awg_diameter_mm:  2.588, swg_diameter_mm:  3.251 },
    WireGauge { gauge: 11, awg_diameter_mm:  2.305, swg_diameter_mm:  2.946 },
    WireGauge { gauge: 12, awg_diameter_mm:  2.053, swg_diameter_mm:  2.642 },
    WireGauge { gauge: 13, awg_diameter_mm:  1.828, swg_diameter_mm:  2.337 },
    WireGauge { gauge: 14, awg_diameter_mm:  1.628, swg_diameter_mm:  2.032 },
    WireGauge { gauge: 15, awg_diameter_mm:  1.450, swg_diameter_mm:  1.829 },
    WireGauge { gauge: 16, awg_diameter_mm:  1.291, swg_diameter_mm:  1.626 },
    WireGauge { gauge: 17, awg_diameter_mm:  1.150, swg_diameter_mm:  1.422 },
    WireGauge { gauge: 18, awg_diameter_mm:  1.024, swg_diameter_mm:  1.219 },
    WireGauge { gauge: 19, awg_diameter_mm:  0.912, swg_diameter_mm:  1.016 },
    WireGauge { gauge: 20, awg_diameter_mm:  0.812, swg_diameter_mm:  0.914 },
    WireGauge { gauge: 21, awg_diameter_mm:  0.723, swg_diameter_mm:  0.813 },
    WireGauge { gauge: 22, awg_diameter_mm:  0.644, swg_diameter_mm:  0.711 },
    WireGauge { gauge: 23, awg_diameter_mm:  0.573, swg_diameter_mm:  0.610 },
    WireGauge { gauge: 24, awg_diameter_mm:  0.511, swg_diameter_mm:  0.559 },
    WireGauge { gauge: 25, awg_diameter_mm:  0.455, swg_diameter_mm:  0.508 },
    WireGauge { gauge: 26, awg_diameter_mm:  0.405, swg_diameter_mm:  0.457 },
    WireGauge { gauge: 27, awg_diameter_mm:  0.361, swg_diameter_mm:  0.417 },
    WireGauge { gauge: 28, awg_diameter_mm:  0.321, swg_diameter_mm:  0.376 },
    WireGauge { gauge: 29, awg_diameter_mm:  0.286, swg_diameter_mm:  0.345 },
    WireGauge { gauge: 30, awg_diameter_mm:  0.255, swg_diameter_mm:  0.315 },
    WireGauge { gauge: 31, awg_diameter_mm:  0.227, swg_diameter_mm:  0.295 },
    WireGauge { gauge: 32, awg_diameter_mm:  0.202, swg_diameter_mm:  0.274 },
    WireGauge { gauge: 33, awg_diameter_mm:  0.180, swg_diameter_mm:  0.254 },
    WireGauge { gauge: 34, awg_diameter_mm:  0.160, swg_diameter_mm:  0.234 },
    WireGauge { gauge: 35, awg_diameter_mm:  0.143, swg_diameter_mm:  0.213 },
    WireGauge { gauge: 36, awg_diameter_mm:  0.127, swg_diameter_mm:  0.193 },
    WireGauge { gauge: 37, awg_diameter_mm:  0.113, swg_diameter_mm:  0.173 },
    WireGauge { gauge: 38, awg_diameter_mm:  0.101, swg_diameter_mm:  0.152 },
    WireGauge { gauge: 39, awg_diameter_mm:  0.090, swg_diameter_mm:  0.132 },
    WireGauge { gauge: 40, awg_diameter_mm:  0.080, swg_diameter_mm:  0.122 },
];

// ---------------------------------------------------------------------------
// 8. Pipe Sizes — ASME B36.10M / B36.19M
// ---------------------------------------------------------------------------
// Source: ASME B36.10M-2018 Welded and Seamless Wrought Steel Pipe
// OD per ASME B36.10M Table 1; wall thicknesses for Schedule 40 and 80.

#[derive(Debug, Clone, Copy)]
pub struct PipeSize {
    pub nps: &'static str,
    pub dn: u16,
    pub od_mm: f64,
    pub schedule_40_wall_mm: f64,
    pub schedule_80_wall_mm: f64,
}

pub static PIPE_SIZES: &[PipeSize] = &[
    PipeSize { nps: "1/8",    dn:   6,  od_mm:  10.3,  schedule_40_wall_mm: 1.73,  schedule_80_wall_mm: 2.41  },
    PipeSize { nps: "1/4",    dn:   8,  od_mm:  13.7,  schedule_40_wall_mm: 2.24,  schedule_80_wall_mm: 3.02  },
    PipeSize { nps: "3/8",    dn:  10,  od_mm:  17.1,  schedule_40_wall_mm: 2.31,  schedule_80_wall_mm: 3.20  },
    PipeSize { nps: "1/2",    dn:  15,  od_mm:  21.3,  schedule_40_wall_mm: 2.77,  schedule_80_wall_mm: 3.73  },
    PipeSize { nps: "3/4",    dn:  20,  od_mm:  26.7,  schedule_40_wall_mm: 2.87,  schedule_80_wall_mm: 3.91  },
    PipeSize { nps: "1",      dn:  25,  od_mm:  33.4,  schedule_40_wall_mm: 3.38,  schedule_80_wall_mm: 4.55  },
    PipeSize { nps: "1-1/4",  dn:  32,  od_mm:  42.2,  schedule_40_wall_mm: 3.56,  schedule_80_wall_mm: 4.85  },
    PipeSize { nps: "1-1/2",  dn:  40,  od_mm:  48.3,  schedule_40_wall_mm: 3.68,  schedule_80_wall_mm: 5.08  },
    PipeSize { nps: "2",      dn:  50,  od_mm:  60.3,  schedule_40_wall_mm: 3.91,  schedule_80_wall_mm: 5.54  },
    PipeSize { nps: "2-1/2",  dn:  65,  od_mm:  73.0,  schedule_40_wall_mm: 5.16,  schedule_80_wall_mm: 7.01  },
    PipeSize { nps: "3",      dn:  80,  od_mm:  88.9,  schedule_40_wall_mm: 5.49,  schedule_80_wall_mm: 7.62  },
    PipeSize { nps: "4",      dn: 100,  od_mm: 114.3,  schedule_40_wall_mm: 6.02,  schedule_80_wall_mm: 8.56  },
    PipeSize { nps: "5",      dn: 125,  od_mm: 141.3,  schedule_40_wall_mm: 6.55,  schedule_80_wall_mm: 9.53  },
    PipeSize { nps: "6",      dn: 150,  od_mm: 168.3,  schedule_40_wall_mm: 7.11,  schedule_80_wall_mm: 10.97 },
    PipeSize { nps: "8",      dn: 200,  od_mm: 219.1,  schedule_40_wall_mm: 8.18,  schedule_80_wall_mm: 12.70 },
    PipeSize { nps: "10",     dn: 250,  od_mm: 273.1,  schedule_40_wall_mm: 9.27,  schedule_80_wall_mm: 15.09 },
    PipeSize { nps: "12",     dn: 300,  od_mm: 323.8,  schedule_40_wall_mm: 10.31, schedule_80_wall_mm: 17.48 },
    PipeSize { nps: "14",     dn: 350,  od_mm: 355.6,  schedule_40_wall_mm: 11.13, schedule_80_wall_mm: 19.05 },
    PipeSize { nps: "16",     dn: 400,  od_mm: 406.4,  schedule_40_wall_mm: 12.70, schedule_80_wall_mm: 21.44 },
    PipeSize { nps: "18",     dn: 450,  od_mm: 457.2,  schedule_40_wall_mm: 14.27, schedule_80_wall_mm: 23.83 },
    PipeSize { nps: "20",     dn: 500,  od_mm: 508.0,  schedule_40_wall_mm: 15.09, schedule_80_wall_mm: 26.19 },
    PipeSize { nps: "24",     dn: 600,  od_mm: 609.6,  schedule_40_wall_mm: 17.48, schedule_80_wall_mm: 30.96 },
];

// ---------------------------------------------------------------------------
// Lookup Functions
// ---------------------------------------------------------------------------

/// Look up a metric thread by designation (e.g. "M8", "M10").
pub fn lookup_metric_thread(designation: &str) -> Option<&'static MetricThread> {
    METRIC_THREADS
        .iter()
        .find(|t| t.designation.eq_ignore_ascii_case(designation))
}

/// Find all metric threads for a given nominal diameter (in mm).
pub fn lookup_metric_threads_by_diameter(diameter_mm: f64) -> impl Iterator<Item = &'static MetricThread> {
    METRIC_THREADS
        .iter()
        .filter(move |t| (t.nominal_diameter_mm - diameter_mm).abs() < 0.01)
}

/// Look up a UNC/UNF thread by designation (e.g. "1/4-20", "#10-24").
pub fn lookup_unc_thread(designation: &str) -> Option<&'static UncThread> {
    UNC_THREADS
        .iter()
        .find(|t| t.designation == designation)
}

/// Look up a tolerance fit by designation (e.g. "H7/g6").
pub fn lookup_tolerance_fit(designation: &str) -> Option<&'static ToleranceFit> {
    TOLERANCE_FITS
        .iter()
        .find(|f| f.designation == designation)
}

/// Find all fits of a given type.
pub fn lookup_fits_by_type(fit_type: FitType) -> impl Iterator<Item = &'static ToleranceFit> {
    TOLERANCE_FITS.iter().filter(move |f| f.fit_type == fit_type)
}

/// Look up surface finish data by process name (case-insensitive substring match).
pub fn lookup_surface_finish(process: &str) -> Option<&'static SurfaceFinish> {
    // Try exact case-insensitive match first
    SURFACE_FINISHES
        .iter()
        .find(|s| s.process.eq_ignore_ascii_case(process))
        .or_else(|| {
            // Fall back to substring match
            let lower = process.to_ascii_lowercase();
            SURFACE_FINISHES
                .iter()
                .find(|s| s.process.to_ascii_lowercase().contains(&lower))
        })
}

/// Look up an IT tolerance grade by grade number.
pub fn lookup_it_grade(grade: u8) -> Option<&'static ItGrade> {
    IT_GRADES.iter().find(|g| g.grade == grade)
}

/// Get the IT tolerance in micrometres for a given grade and nominal size (mm).
pub fn it_tolerance_um(grade: u8, nominal_mm: f64) -> Option<f64> {
    let g = lookup_it_grade(grade)?;
    let idx = nominal_size_index(nominal_mm)?;
    Some(g.tolerances_um[idx])
}

/// Map a nominal size (mm) to the ISO 286 size range index.
fn nominal_size_index(nominal_mm: f64) -> Option<usize> {
    // ISO 286 size ranges: 0-3, 3-6, 6-10, 10-18, 18-30, 30-50,
    // 50-80, 80-120, 120-180, 180-250, 250-315, 315-400, 400-500
    static UPPER_BOUNDS: [f64; 13] = [
        3.0, 6.0, 10.0, 18.0, 30.0, 50.0, 80.0, 120.0, 180.0, 250.0, 315.0, 400.0, 500.0,
    ];
    if nominal_mm <= 0.0 || nominal_mm > 500.0 {
        return None;
    }
    for (i, &ub) in UPPER_BOUNDS.iter().enumerate() {
        if nominal_mm <= ub {
            return Some(i);
        }
    }
    None
}

/// Look up pipe size by NPS designation (e.g. "1/2", "4").
pub fn lookup_pipe(nps: &str) -> Option<&'static PipeSize> {
    PIPE_SIZES.iter().find(|p| p.nps == nps)
}

/// Convert HRC hardness to approximate Vickers (HV) hardness.
/// Finds the closest HRC entry in the ASTM E140 table and returns the
/// corresponding HV value. Returns `None` if the input is outside the
/// table range (HRC 20..68).
pub fn hardness_hrc_to_hv(hrc: f64) -> Option<f64> {
    if hrc < 20.0 || hrc > 68.0 {
        return None;
    }
    let mut best: Option<&HardnessConversion> = None;
    let mut best_diff = f64::MAX;
    for entry in HARDNESS_CONVERSIONS.iter() {
        if entry.hrc > 0.0 {
            let diff = (entry.hrc - hrc).abs();
            if diff < best_diff {
                best_diff = diff;
                best = Some(entry);
            }
        }
    }
    best.map(|e| e.hv)
}

/// Convert HRC hardness to approximate Brinell (HB) hardness.
/// Returns `None` if the input is outside the reliable range or if
/// the Brinell value is not applicable (HRC > ~48 per ASTM E140).
pub fn hardness_hrc_to_hb(hrc: f64) -> Option<f64> {
    if hrc < 20.0 || hrc > 68.0 {
        return None;
    }
    let mut best: Option<&HardnessConversion> = None;
    let mut best_diff = f64::MAX;
    for entry in HARDNESS_CONVERSIONS.iter() {
        if entry.hrc > 0.0 {
            let diff = (entry.hrc - hrc).abs();
            if diff < best_diff {
                best_diff = diff;
                best = Some(entry);
            }
        }
    }
    best.and_then(|e| if e.hb > 0.0 { Some(e.hb) } else { None })
}

/// Look up a wire gauge entry by AWG number.
/// Gauge -3 = 0000 (4/0), -2 = 000 (3/0), -1 = 00 (2/0), 0 = 0 (1/0).
pub fn lookup_wire_gauge(gauge: i32) -> Option<&'static WireGauge> {
    WIRE_GAUGES.iter().find(|w| w.gauge == gauge)
}

/// Find the closest AWG gauge for a given wire diameter in mm.
pub fn lookup_wire_gauge_by_diameter(diameter_mm: f64) -> Option<&'static WireGauge> {
    WIRE_GAUGES
        .iter()
        .min_by(|a, b| {
            let da = (a.awg_diameter_mm - diameter_mm).abs();
            let db = (b.awg_diameter_mm - diameter_mm).abs();
            da.partial_cmp(&db).unwrap_or(core::cmp::Ordering::Equal)
        })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // --- Metric Threads ---

    #[test]
    fn metric_thread_m6_exists() {
        let t = lookup_metric_thread("M6").unwrap();
        assert_eq!(t.nominal_diameter_mm, 6.0);
        assert_eq!(t.coarse_pitch_mm, 1.0);
        assert!((t.minor_diameter_mm - 4.917).abs() < 0.01);
        assert!((t.pitch_diameter_mm - 5.350).abs() < 0.01);
        assert!((t.tensile_stress_area_mm2 - 20.1).abs() < 0.5);
    }

    #[test]
    fn metric_thread_m8_has_fine_pitches() {
        let t = lookup_metric_thread("M8").unwrap();
        assert!(t.fine_pitches_mm.contains(&1.0));
        assert!(t.fine_pitches_mm.contains(&0.75));
    }

    #[test]
    fn metric_thread_case_insensitive() {
        assert!(lookup_metric_thread("m10").is_some());
        assert!(lookup_metric_thread("M10").is_some());
    }

    #[test]
    fn metric_thread_count() {
        // Must contain at least M1 through M100 (all required sizes)
        assert!(METRIC_THREADS.len() >= 27);
        assert!(lookup_metric_thread("M1").is_some());
        assert!(lookup_metric_thread("M100").is_some());
        assert!(lookup_metric_thread("M64").is_some());
    }

    #[test]
    fn metric_thread_m1_smallest() {
        let t = lookup_metric_thread("M1").unwrap();
        assert_eq!(t.nominal_diameter_mm, 1.0);
        assert_eq!(t.coarse_pitch_mm, 0.25);
    }

    #[test]
    fn metric_thread_m100_largest() {
        let t = lookup_metric_thread("M100").unwrap();
        assert_eq!(t.nominal_diameter_mm, 100.0);
        assert_eq!(t.coarse_pitch_mm, 6.0);
    }

    #[test]
    fn metric_thread_diameter_lookup() {
        let threads: Vec<_> = lookup_metric_threads_by_diameter(12.0).collect();
        assert!(!threads.is_empty());
        assert!(threads.iter().any(|t| t.designation == "M12"));
    }

    // --- UNC Threads ---

    #[test]
    fn unc_quarter_twenty() {
        let t = lookup_unc_thread("1/4-20").unwrap();
        assert_eq!(t.tpi, 20.0);
        assert!((t.nominal_diameter_inch - 0.25).abs() < 0.001);
    }

    #[test]
    fn unc_thread_not_found() {
        assert!(lookup_unc_thread("M6").is_none());
    }

    // --- Tolerance Fits ---

    #[test]
    fn fit_h7g6_is_clearance() {
        let f = lookup_tolerance_fit("H7/g6").unwrap();
        assert_eq!(f.fit_type, FitType::Clearance);
        assert!(f.description.contains("liding"));
    }

    #[test]
    fn fit_h7p6_is_interference() {
        let f = lookup_tolerance_fit("H7/p6").unwrap();
        assert_eq!(f.fit_type, FitType::Interference);
    }

    #[test]
    fn fit_h7k6_is_transition() {
        let f = lookup_tolerance_fit("H7/k6").unwrap();
        assert_eq!(f.fit_type, FitType::Transition);
    }

    #[test]
    fn fit_h7s6_is_heavy_press() {
        let f = lookup_tolerance_fit("H7/s6").unwrap();
        assert_eq!(f.fit_type, FitType::Interference);
        assert!(f.description.to_ascii_lowercase().contains("heavy"));
    }

    #[test]
    fn all_fit_types_present() {
        let clearance: Vec<_> = lookup_fits_by_type(FitType::Clearance).collect();
        let transition: Vec<_> = lookup_fits_by_type(FitType::Transition).collect();
        let interference: Vec<_> = lookup_fits_by_type(FitType::Interference).collect();
        assert!(!clearance.is_empty());
        assert!(!transition.is_empty());
        assert!(!interference.is_empty());
    }

    #[test]
    fn fit_not_found() {
        assert!(lookup_tolerance_fit("H99/z99").is_none());
    }

    // --- Surface Finish ---

    #[test]
    fn surface_finish_grinding() {
        let sf = lookup_surface_finish("Grinding").unwrap();
        assert!(sf.ra_typical_um < 1.0);
        assert!(sf.ra_range_um.0 < sf.ra_range_um.1);
        assert!(sf.rz_typical_um > 0.0);
    }

    #[test]
    fn surface_finish_fdm() {
        let sf = lookup_surface_finish("FDM 3D printing").unwrap();
        assert!(sf.ra_typical_um > 10.0);
    }

    #[test]
    fn surface_finish_lapping_finest() {
        let sf = lookup_surface_finish("Lapping").unwrap();
        assert!(sf.ra_typical_um < 0.1);
    }

    #[test]
    fn surface_finish_case_insensitive() {
        assert!(lookup_surface_finish("grinding").is_some());
        assert!(lookup_surface_finish("GRINDING").is_some());
    }

    #[test]
    fn surface_finish_substring_match() {
        // Should find "EDM (wire)" when searching "edm"
        let sf = lookup_surface_finish("edm");
        assert!(sf.is_some());
    }

    #[test]
    fn surface_finish_rz_is_positive() {
        for sf in SURFACE_FINISHES.iter() {
            assert!(sf.rz_typical_um > 0.0, "Rz must be positive for {}", sf.process);
        }
    }

    // --- IT Tolerance Grades ---

    #[test]
    fn it7_for_25mm() {
        let tol = it_tolerance_um(7, 25.0).unwrap();
        assert_eq!(tol, 21.0); // IT7 for 18..30mm range
    }

    #[test]
    fn it_grade_out_of_range() {
        assert!(it_tolerance_um(7, 0.0).is_none());
        assert!(it_tolerance_um(7, 501.0).is_none());
    }

    #[test]
    fn it_grade_not_found() {
        assert!(it_tolerance_um(1, 25.0).is_none()); // Grade 1 not in table
    }

    // --- Hardness Conversion ---

    #[test]
    fn hardness_hrc_30_to_hv() {
        let hv = hardness_hrc_to_hv(30.0).unwrap();
        assert!((hv - 286.0).abs() < 1.0);
    }

    #[test]
    fn hardness_hrc_45_to_hv() {
        let hv = hardness_hrc_to_hv(45.0).unwrap();
        assert!((hv - 421.0).abs() < 1.0);
    }

    #[test]
    fn hardness_hrc_out_of_range() {
        assert!(hardness_hrc_to_hv(10.0).is_none());
        assert!(hardness_hrc_to_hv(70.0).is_none());
    }

    #[test]
    fn hardness_hrc_to_hb_high_hrc_none() {
        // Above HRC 48, Brinell is not applicable
        assert!(hardness_hrc_to_hb(50.0).is_none());
    }

    #[test]
    fn hardness_hrc_to_hb_valid() {
        let hb = hardness_hrc_to_hb(30.0).unwrap();
        assert!((hb - 286.0).abs() < 1.0);
    }

    #[test]
    fn hardness_conversion_has_option_hrb() {
        // HRB-range entries should have Some(...)
        let hrb_entries: Vec<_> = HARDNESS_CONVERSIONS.iter().filter(|e| e.hrb.is_some()).collect();
        assert!(!hrb_entries.is_empty());
        // HRC-range entries should have None
        let hrc_entries: Vec<_> = HARDNESS_CONVERSIONS.iter().filter(|e| e.hrc > 0.0).collect();
        assert!(hrc_entries.iter().all(|e| e.hrb.is_none()));
    }

    // --- Wire Gauge ---

    #[test]
    fn wire_gauge_awg_12() {
        let w = lookup_wire_gauge(12).unwrap();
        assert!((w.awg_diameter_mm - 2.053).abs() < 0.01);
        assert!(w.swg_diameter_mm > 0.0);
    }

    #[test]
    fn wire_gauge_4_0() {
        let w = lookup_wire_gauge(-3).unwrap();
        assert!((w.awg_diameter_mm - 11.684).abs() < 0.01);
    }

    #[test]
    fn wire_gauge_40_smallest() {
        let w = lookup_wire_gauge(40).unwrap();
        assert!(w.awg_diameter_mm < 0.1);
    }

    #[test]
    fn wire_gauge_full_range() {
        // AWG 0000 (-3) through 40 should all be present
        for gauge in -3..=40_i32 {
            assert!(
                lookup_wire_gauge(gauge).is_some(),
                "Missing wire gauge {}",
                gauge
            );
        }
    }

    #[test]
    fn wire_gauge_swg_populated() {
        for wg in WIRE_GAUGES.iter() {
            assert!(wg.swg_diameter_mm > 0.0, "SWG diameter must be positive for gauge {}", wg.gauge);
        }
    }

    #[test]
    fn wire_gauge_by_diameter() {
        // ~2mm should find AWG 12
        let w = lookup_wire_gauge_by_diameter(2.0).unwrap();
        assert_eq!(w.gauge, 12);
    }

    #[test]
    fn wire_gauge_not_found() {
        assert!(lookup_wire_gauge(50).is_none());
    }

    // --- Pipe ---

    #[test]
    fn pipe_half_inch() {
        let p = lookup_pipe("1/2").unwrap();
        assert!((p.od_mm - 21.3).abs() < 0.1);
        assert_eq!(p.dn, 15);
    }

    #[test]
    fn pipe_not_found() {
        assert!(lookup_pipe("99").is_none());
    }

    // --- Cross-table sanity ---

    #[test]
    fn tables_are_sorted_by_size() {
        // Metric threads sorted by nominal diameter
        for w in METRIC_THREADS.windows(2) {
            assert!(
                w[0].nominal_diameter_mm <= w[1].nominal_diameter_mm,
                "Metric threads not sorted: {} > {}",
                w[0].designation,
                w[1].designation
            );
        }
        // Wire gauges sorted by gauge number
        for w in WIRE_GAUGES.windows(2) {
            assert!(
                w[0].gauge <= w[1].gauge,
                "Wire gauges not sorted: {} > {}",
                w[0].gauge,
                w[1].gauge
            );
        }
    }
}
