#![cfg_attr(not(feature = "std"), no_std)]

//! Compile-time dimensional analysis for engineering quantities.
//!
//! Every quantity carries its dimensions in the type system.
//! Mismatched operations (e.g. adding Length to Force) are compile errors.
//! All internal storage is SI (meters, kilograms, seconds, kelvin).

use core::fmt;
use core::ops::{Add, Div, Mul, Neg, Sub};

/// A physical quantity with compile-time dimensional analysis.
///
/// Type parameters encode SI base dimensions as signed integers:
/// - L: length (meters)
/// - M: mass (kilograms)
/// - T: time (seconds)
/// - K: temperature (kelvin)
///
/// We use a single struct with marker traits to enforce dimensions.
/// For Phase 1, we use a simpler approach: distinct newtypes per quantity.

// ---------------------------------------------------------------------------
// Core newtypes — each wraps f64 in SI units
// ---------------------------------------------------------------------------

macro_rules! quantity {
    ($name:ident, $si_unit:expr) => {
        #[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
        pub struct $name(f64);

        impl $name {
            #[inline(always)]
            pub const fn new(value: f64) -> Self {
                Self(value)
            }

            #[inline(always)]
            pub const fn value(self) -> f64 {
                self.0
            }

            #[inline(always)]
            pub const fn zero() -> Self {
                Self(0.0)
            }
        }

        impl Add for $name {
            type Output = Self;
            #[inline(always)]
            fn add(self, rhs: Self) -> Self {
                Self(self.0 + rhs.0)
            }
        }

        impl Sub for $name {
            type Output = Self;
            #[inline(always)]
            fn sub(self, rhs: Self) -> Self {
                Self(self.0 - rhs.0)
            }
        }

        impl Neg for $name {
            type Output = Self;
            #[inline(always)]
            fn neg(self) -> Self {
                Self(-self.0)
            }
        }

        impl Mul<f64> for $name {
            type Output = Self;
            #[inline(always)]
            fn mul(self, rhs: f64) -> Self {
                Self(self.0 * rhs)
            }
        }

        impl Mul<$name> for f64 {
            type Output = $name;
            #[inline(always)]
            fn mul(self, rhs: $name) -> $name {
                $name(self * rhs.0)
            }
        }

        impl Div<f64> for $name {
            type Output = Self;
            #[inline(always)]
            fn div(self, rhs: f64) -> Self {
                Self(self.0 / rhs)
            }
        }

        impl Div<$name> for $name {
            type Output = f64;
            #[inline(always)]
            fn div(self, rhs: $name) -> f64 {
                self.0 / rhs.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{} {}", self.0, $si_unit)
            }
        }
    };
}

// ---------------------------------------------------------------------------
// Quantities
// ---------------------------------------------------------------------------

quantity!(Length, "m");
quantity!(Area, "m²");
quantity!(Volume, "m³");
quantity!(Mass, "kg");
quantity!(Time, "s");
quantity!(Temperature, "K");
quantity!(Angle, "rad");

quantity!(Force, "N");
quantity!(Pressure, "Pa");
quantity!(Torque, "N·m");
quantity!(Energy, "J");
quantity!(Power, "W");
quantity!(Density, "kg/m³");
quantity!(Velocity, "m/s");
quantity!(Acceleration, "m/s²");

quantity!(ThermalConductivity, "W/(m·K)");
quantity!(SpecificHeat, "J/(kg·K)");
quantity!(CTE, "1/K");

quantity!(MomentOfInertia, "m⁴");
quantity!(SectionModulus, "m³");

quantity!(Dimensionless, "");

// ---------------------------------------------------------------------------
// Constructors with unit conversions
// ---------------------------------------------------------------------------

impl Length {
    #[inline(always)]
    pub const fn m(v: f64) -> Self {
        Self(v)
    }
    #[inline(always)]
    pub const fn mm(v: f64) -> Self {
        Self(v * 1e-3)
    }
    #[inline(always)]
    pub const fn um(v: f64) -> Self {
        Self(v * 1e-6)
    }
    #[inline(always)]
    pub const fn inch(v: f64) -> Self {
        Self(v * 0.0254)
    }
    #[inline(always)]
    pub const fn ft(v: f64) -> Self {
        Self(v * 0.3048)
    }

    pub fn to_mm(self) -> f64 {
        self.0 * 1e3
    }
    pub fn to_inch(self) -> f64 {
        self.0 / 0.0254
    }
}

impl Area {
    #[inline(always)]
    pub const fn m2(v: f64) -> Self {
        Self(v)
    }
    #[inline(always)]
    pub const fn mm2(v: f64) -> Self {
        Self(v * 1e-6)
    }
}

impl Volume {
    #[inline(always)]
    pub const fn m3(v: f64) -> Self {
        Self(v)
    }
    #[inline(always)]
    pub const fn mm3(v: f64) -> Self {
        Self(v * 1e-9)
    }
}

impl Mass {
    #[inline(always)]
    pub const fn kg(v: f64) -> Self {
        Self(v)
    }
    #[inline(always)]
    pub const fn g(v: f64) -> Self {
        Self(v * 1e-3)
    }
    #[inline(always)]
    pub const fn lb(v: f64) -> Self {
        Self(v * 0.453_592_37)
    }
}

impl Force {
    #[inline(always)]
    pub const fn n(v: f64) -> Self {
        Self(v)
    }
    #[inline(always)]
    pub const fn kn(v: f64) -> Self {
        Self(v * 1e3)
    }
    #[inline(always)]
    pub const fn lbf(v: f64) -> Self {
        Self(v * 4.448_222)
    }
}

impl Pressure {
    #[inline(always)]
    pub const fn pa(v: f64) -> Self {
        Self(v)
    }
    #[inline(always)]
    pub const fn kpa(v: f64) -> Self {
        Self(v * 1e3)
    }
    #[inline(always)]
    pub const fn mpa(v: f64) -> Self {
        Self(v * 1e6)
    }
    #[inline(always)]
    pub const fn gpa(v: f64) -> Self {
        Self(v * 1e9)
    }
    #[inline(always)]
    pub const fn psi(v: f64) -> Self {
        Self(v * 6_894.757)
    }

    pub fn to_mpa(self) -> f64 {
        self.0 * 1e-6
    }
}

impl Temperature {
    #[inline(always)]
    pub const fn k(v: f64) -> Self {
        Self(v)
    }
    #[inline(always)]
    pub const fn celsius(v: f64) -> Self {
        Self(v + 273.15)
    }
    #[inline(always)]
    pub const fn fahrenheit(v: f64) -> Self {
        Self((v - 32.0) * 5.0 / 9.0 + 273.15)
    }

    pub fn to_celsius(self) -> f64 {
        self.0 - 273.15
    }
}

impl Angle {
    #[inline(always)]
    pub const fn rad(v: f64) -> Self {
        Self(v)
    }
    #[inline(always)]
    pub const fn deg(v: f64) -> Self {
        Self(v * core::f64::consts::PI / 180.0)
    }

    pub fn to_deg(self) -> f64 {
        self.0 * 180.0 / core::f64::consts::PI
    }
}

impl Density {
    #[inline(always)]
    pub const fn kg_m3(v: f64) -> Self {
        Self(v)
    }
}

impl ThermalConductivity {
    #[inline(always)]
    pub const fn w_mk(v: f64) -> Self {
        Self(v)
    }
}

impl SpecificHeat {
    #[inline(always)]
    pub const fn j_kgk(v: f64) -> Self {
        Self(v)
    }
}

impl CTE {
    #[inline(always)]
    pub const fn per_k(v: f64) -> Self {
        Self(v)
    }
    /// µm/m·°C — same as 1e-6/K
    #[inline(always)]
    pub const fn um_mk(v: f64) -> Self {
        Self(v * 1e-6)
    }
}

impl Dimensionless {
    #[inline(always)]
    pub const fn ratio(v: f64) -> Self {
        Self(v)
    }
}

// ---------------------------------------------------------------------------
// Cross-quantity operations (dimensional correctness)
// ---------------------------------------------------------------------------

/// Length × Length = Area
impl Mul for Length {
    type Output = Area;
    #[inline(always)]
    fn mul(self, rhs: Length) -> Area {
        Area::new(self.0 * rhs.0)
    }
}

/// Area × Length = Volume
impl Mul<Length> for Area {
    type Output = Volume;
    #[inline(always)]
    fn mul(self, rhs: Length) -> Volume {
        Volume::new(self.0 * rhs.0)
    }
}

/// Force / Area = Pressure
impl Div<Area> for Force {
    type Output = Pressure;
    #[inline(always)]
    fn div(self, rhs: Area) -> Pressure {
        Pressure::new(self.0 / rhs.0)
    }
}

/// Pressure × Area = Force
impl Mul<Area> for Pressure {
    type Output = Force;
    #[inline(always)]
    fn mul(self, rhs: Area) -> Force {
        Force::new(self.0 * rhs.0)
    }
}

/// Force × Length = Torque/Energy
impl Mul<Length> for Force {
    type Output = Torque;
    #[inline(always)]
    fn mul(self, rhs: Length) -> Torque {
        Torque::new(self.0 * rhs.0)
    }
}

/// Density × Volume = Mass
impl Mul<Volume> for Density {
    type Output = Mass;
    #[inline(always)]
    fn mul(self, rhs: Volume) -> Mass {
        Mass::new(self.0 * rhs.0)
    }
}

/// Mass / Volume = Density
impl Div<Volume> for Mass {
    type Output = Density;
    #[inline(always)]
    fn div(self, rhs: Volume) -> Density {
        Density::new(self.0 / rhs.0)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn length_conversions() {
        let l = Length::mm(25.0);
        assert!((l.value() - 0.025).abs() < 1e-10);
        assert!((l.to_mm() - 25.0).abs() < 1e-10);
    }

    #[test]
    fn area_from_lengths() {
        let a = Length::mm(25.0) * Length::mm(10.0);
        assert!((a.value() - 250e-6).abs() < 1e-12);
    }

    #[test]
    fn stress_from_force_and_area() {
        let force = Force::n(1000.0);
        let area = Area::mm2(100.0);
        let stress = force / area;
        // 1000 N / 100 mm² = 1000 / 100e-6 = 10 MPa
        assert!((stress.to_mpa() - 10.0).abs() < 1e-10);
    }

    #[test]
    fn yield_check_6061_t6() {
        let stress = Force::n(1000.0) / Area::mm2(100.0);
        let yield_strength = Pressure::mpa(276.0);
        assert!(stress < yield_strength);
    }

    #[test]
    fn temperature_conversions() {
        let t = Temperature::celsius(100.0);
        assert!((t.to_celsius() - 100.0).abs() < 1e-10);
        assert!((t.value() - 373.15).abs() < 1e-10);
    }

    #[test]
    fn mass_from_density_and_volume() {
        let steel = Density::kg_m3(7850.0);
        let vol = Volume::mm3(1000.0); // 1 cm³ = 1000 mm³ = 1e-6 m³
        let mass = steel * vol;
        assert!((mass.value() - 7.85e-3).abs() < 1e-6);
    }

    #[test]
    fn imperial_conversions() {
        let l = Length::inch(1.0);
        assert!((l.value() - 0.0254).abs() < 1e-10);

        let f = Force::lbf(1.0);
        assert!((f.value() - 4.448_222).abs() < 1e-3);

        let p = Pressure::psi(1.0);
        assert!((p.value() - 6894.757).abs() < 1.0);
    }
}
