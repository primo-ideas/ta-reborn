use core::ops::{Add, AddAssign, Div, Mul, Sub};

use serde::{Deserialize, Serialize};

const FRAC_BITS: u32 = 32;
const ONE_RAW: i64 = 1 << FRAC_BITS;

/// Q32.32 fixed-point number: integer arithmetic gives the same bits on every platform.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
pub struct Fx(i64);

impl Fx {
    pub const ZERO: Fx = Fx(0);

    pub const fn from_int(n: i32) -> Fx {
        Fx((n as i64) << FRAC_BITS)
    }

    /// `num / den`, e.g. `Fx::ratio(1, 5)` is 0.2.
    pub const fn ratio(num: i32, den: i32) -> Fx {
        Fx(((num as i64) << FRAC_BITS) / den as i64)
    }

    pub const fn from_raw(raw: i64) -> Fx {
        Fx(raw)
    }

    pub const fn raw(self) -> i64 {
        self.0
    }

    /// Largest integer `<= self`.
    pub const fn floor(self) -> i32 {
        (self.0 >> FRAC_BITS) as i32
    }

    /// Lossy conversion for rendering. Never use inside the simulation.
    pub fn to_f32(self) -> f32 {
        (self.0 as f64 / ONE_RAW as f64) as f32
    }

    /// Conversion of client input (e.g. a clicked point). The result travels inside a
    /// command, so every client receives the same value: the rounding happens once.
    pub fn from_f32(v: f32) -> Fx {
        Fx((v as f64 * ONE_RAW as f64) as i64)
    }
}

impl Add for Fx {
    type Output = Fx;
    fn add(self, rhs: Fx) -> Fx {
        Fx(self.0 + rhs.0)
    }
}

impl AddAssign for Fx {
    fn add_assign(&mut self, rhs: Fx) {
        self.0 += rhs.0;
    }
}

impl Sub for Fx {
    type Output = Fx;
    fn sub(self, rhs: Fx) -> Fx {
        Fx(self.0 - rhs.0)
    }
}

impl Mul for Fx {
    type Output = Fx;
    fn mul(self, rhs: Fx) -> Fx {
        Fx(((self.0 as i128 * rhs.0 as i128) >> FRAC_BITS) as i64)
    }
}

impl Div for Fx {
    type Output = Fx;
    fn div(self, rhs: Fx) -> Fx {
        Fx((((self.0 as i128) << FRAC_BITS) / rhs.0 as i128) as i64)
    }
}

/// A position or offset on the ground plane. `x`/`z` map to Bevy's X/Z axes (Y is up).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FxVec2 {
    pub x: Fx,
    pub z: Fx,
}

impl FxVec2 {
    pub const fn new(x: Fx, z: Fx) -> FxVec2 {
        FxVec2 { x, z }
    }

    pub fn length(self) -> Fx {
        let x = self.x.0.unsigned_abs() as u128;
        let z = self.z.0.unsigned_abs() as u128;
        // x² + z² is in Q64.64, so its integer square root is directly in Q32.32.
        Fx((x * x + z * z).isqrt() as i64)
    }
}

impl Add for FxVec2 {
    type Output = FxVec2;
    fn add(self, rhs: FxVec2) -> FxVec2 {
        FxVec2::new(self.x + rhs.x, self.z + rhs.z)
    }
}

impl Sub for FxVec2 {
    type Output = FxVec2;
    fn sub(self, rhs: FxVec2) -> FxVec2 {
        FxVec2::new(self.x - rhs.x, self.z - rhs.z)
    }
}

impl Mul<Fx> for FxVec2 {
    type Output = FxVec2;
    fn mul(self, rhs: Fx) -> FxVec2 {
        FxVec2::new(self.x * rhs, self.z * rhs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arithmetic() {
        let a = Fx::ratio(3, 2);
        assert_eq!(a + a, Fx::from_int(3));
        assert_eq!(a * Fx::from_int(-2), Fx::from_int(-3));
        assert_eq!(Fx::from_int(3) / Fx::from_int(2), a);
        assert_eq!(Fx::ratio(-3, 2).floor(), -2);
    }

    #[test]
    fn length_is_exact_on_integers() {
        let v = FxVec2::new(Fx::from_int(3), Fx::from_int(-4));
        assert_eq!(v.length(), Fx::from_int(5));
    }

    #[test]
    fn f32_round_trip() {
        assert_eq!(Fx::from_f32(-12.25).to_f32(), -12.25);
    }
}
