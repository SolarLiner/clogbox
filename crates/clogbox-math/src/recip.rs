use az::Cast;
use num_traits::float::FloatCore;
use num_traits::real::Real;
use num_traits::{Float, FromPrimitive, Num, NumCast, NumOps, One, ToPrimitive, Zero};
use std::num::FpCategory;
use std::ops::{Add, Div, Mul, Neg, Rem, Sub};

/// Holds a value and its reciprocal.
///
/// Invariants: value.recip() == recip && recip.recip() == value && value.signum() == recip.signum()
#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Hash)]
pub struct Recip<T> {
    value: T,
    recip: T,
}

#[cfg(any(feature = "std", feature = "no_std"))]
impl<T: Float> From<T> for Recip<T> {
    fn from(value: T) -> Self {
        Self::new(value)
    }
}

#[cfg(not(any(feature = "std", feature = "no_std")))]
impl<T: FloatCore> From<T> for Recip<T> {
    fn from(value: T) -> Self {
        Self::core_new(value)
    }
}

impl<T: FloatCore> Add<Self> for Recip<T> {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self::core_new(self.value + rhs.value)
    }
}

impl<T: FloatCore> Sub<Self> for Recip<T> {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self::core_new(self.value - rhs.value)
    }
}

impl<T: FloatCore> Mul<Self> for Recip<T> {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self::Output {
        Self {
            value: self.value * rhs.value,
            recip: self.recip * rhs.recip,
        }
    }
}

impl<T: FloatCore> Div<Self> for Recip<T> {
    type Output = Self;

    fn div(self, rhs: Self) -> Self::Output {
        Self {
            value: self.value / rhs.value,
            recip: rhs.value / self.value,
        }
    }
}

impl<T: FloatCore> Rem<Self> for Recip<T> {
    type Output = Self;

    fn rem(self, rhs: Self) -> Self::Output {
        Self::core_new(self.value % rhs.value)
    }
}

impl<T: FloatCore> Num for Recip<T>
where
    Self: NumOps,
{
    type FromStrRadixErr = T::FromStrRadixErr;

    fn from_str_radix(str: &str, radix: u32) -> Result<Self, Self::FromStrRadixErr> {
        T::from_str_radix(str, radix).map(Self::core_new)
    }
}

impl<T: FloatCore> Zero for Recip<T> {
    fn zero() -> Self {
        Self {
            value: T::zero(),
            recip: T::infinity(),
        }
    }

    fn is_zero(&self) -> bool {
        self.value.is_zero()
    }
}

impl<T: FloatCore> One for Recip<T> {
    fn one() -> Self {
        Self {
            value: T::one(),
            recip: T::one(),
        }
    }
}

impl<F: FloatCore + ToPrimitive> NumCast for Recip<F> {
    fn from<T: ToPrimitive>(n: T) -> Option<Self> {
        F::from(n).map(Self::core_new)
    }
}

impl<T: FloatCore> ToPrimitive for Recip<T> {
    fn to_i64(&self) -> Option<i64> {
        self.value.to_i64()
    }

    fn to_u64(&self) -> Option<u64> {
        self.value.to_u64()
    }
}

impl<T: FloatCore> Neg for Recip<T> {
    type Output = Self;

    fn neg(self) -> Self::Output {
        Self {
            value: -self.value,
            recip: -self.recip,
        }
    }
}

impl<T: FloatCore> FloatCore for Recip<T> {
    fn infinity() -> Self {
        Self {
            value: T::infinity(),
            recip: T::zero(),
        }
    }

    fn neg_infinity() -> Self {
        Self {
            value: T::neg_infinity(),
            recip: T::neg_zero(),
        }
    }

    fn nan() -> Self {
        Self {
            value: T::nan(),
            recip: T::nan(),
        }
    }

    fn neg_zero() -> Self {
        Self {
            value: T::neg_zero(),
            recip: T::neg_infinity(),
        }
    }

    fn min_value() -> Self {
        Self::core_new(T::min_value())
    }

    fn min_positive_value() -> Self {
        Self::core_new(T::min_positive_value())
    }

    fn epsilon() -> Self {
        Self::core_new(T::epsilon())
    }

    fn max_value() -> Self {
        Self::core_new(T::max_value())
    }

    fn classify(self) -> FpCategory {
        self.value.classify()
    }

    fn to_degrees(self) -> Self {
        Self::core_new(self.value.to_degrees())
    }

    fn to_radians(self) -> Self {
        Self::core_new(self.value.to_radians())
    }

    fn integer_decode(self) -> (u64, i16, i8) {
        self.value.integer_decode()
    }
}

impl<T: FloatCore + FromPrimitive> FromPrimitive for Recip<T> {
    fn from_isize(n: isize) -> Option<Self> {
        T::from_isize(n).map(Self::core_new)
    }

    fn from_i8(n: i8) -> Option<Self> {
        T::from_i8(n).map(Self::core_new)
    }

    fn from_i16(n: i16) -> Option<Self> {
        T::from_i16(n).map(Self::core_new)
    }

    fn from_i32(n: i32) -> Option<Self> {
        T::from_i32(n).map(Self::core_new)
    }

    fn from_i64(n: i64) -> Option<Self> {
        T::from_i64(n).map(Self::core_new)
    }

    fn from_i128(n: i128) -> Option<Self> {
        T::from_i128(n).map(Self::core_new)
    }

    fn from_usize(n: usize) -> Option<Self> {
        T::from_usize(n).map(Self::core_new)
    }

    fn from_u8(n: u8) -> Option<Self> {
        T::from_u8(n).map(Self::core_new)
    }

    fn from_u16(n: u16) -> Option<Self> {
        T::from_u16(n).map(Self::core_new)
    }

    fn from_u32(n: u32) -> Option<Self> {
        T::from_u32(n).map(Self::core_new)
    }

    fn from_u64(n: u64) -> Option<Self> {
        T::from_u64(n).map(Self::core_new)
    }

    fn from_u128(n: u128) -> Option<Self> {
        T::from_u128(n).map(Self::core_new)
    }

    fn from_f32(n: f32) -> Option<Self> {
        T::from_f32(n).map(Self::core_new)
    }

    fn from_f64(n: f64) -> Option<Self> {
        T::from_f64(n).map(Self::core_new)
    }
}

impl<T> Cast<T> for Recip<T> {
    fn cast(self) -> T {
        self.value
    }
}

impl<T: Copy> Recip<T> {
    /// Returns the stored value.
    ///
    /// # Returns
    ///
    /// The stored value.
    pub fn value(&self) -> T {
        self.value
    }

    /// Returns the reciprocal of the stored value. This operation is no-op because the reciprocal
    /// has already been computed.
    ///
    /// # Returns
    ///
    /// The reciprocal of the stored value.
    pub fn recip(&self) -> T {
        self.recip
    }
}

impl<T: Float> Recip<T> {
    /// Creates a new `Recip` instance.
    ///
    /// # Parameters
    ///
    /// - `value`: The value to store.
    ///
    /// # Returns
    ///
    /// A new `Recip` instance with the given value and its reciprocal.
    pub fn new(value: T) -> Self {
        Self {
            value,
            recip: value.recip(),
        }
    }
}

impl<T: FloatCore> Recip<T> {
    /// Creates a new `Recip` instance. This method is like `new`, but it does not require a full
    /// `Float` implementation for the type `T`.
    ///
    /// # Parameters
    ///
    /// - `value`: The value to store.
    ///
    /// # Returns
    ///
    /// A new `Recip` instance with the given value and its reciprocal.
    pub fn core_new(value: T) -> Self {
        Self {
            value,
            recip: value.recip(),
        }
    }
}

impl<T: Float> Float for Recip<T>
where
    Self: Num + NumCast + Neg<Output = Self>,
{
    fn nan() -> Self {
        Self {
            value: T::nan(),
            recip: T::nan(),
        }
    }

    fn infinity() -> Self {
        Self {
            value: T::infinity(),
            recip: T::zero(),
        }
    }

    fn neg_infinity() -> Self {
        Self {
            value: T::neg_infinity(),
            recip: T::neg_zero(),
        }
    }

    fn neg_zero() -> Self {
        Self {
            value: T::neg_zero(),
            recip: T::neg_infinity(),
        }
    }

    fn min_value() -> Self {
        Self::new(T::min_value())
    }

    fn min_positive_value() -> Self {
        Self::new(T::min_positive_value())
    }

    fn epsilon() -> Self {
        Self::new(T::epsilon())
    }

    fn max_value() -> Self {
        Self::new(T::max_value())
    }

    fn is_nan(self) -> bool {
        self.value.is_nan()
    }

    fn is_infinite(self) -> bool {
        self.value.is_infinite()
    }

    fn is_finite(self) -> bool {
        self.value.is_finite()
    }

    fn is_normal(self) -> bool {
        self.value.is_normal()
    }

    fn is_subnormal(self) -> bool {
        self.value.is_subnormal()
    }

    fn classify(self) -> FpCategory {
        self.value.classify()
    }

    fn floor(self) -> Self {
        Self::new(self.value.floor())
    }

    fn ceil(self) -> Self {
        Self::new(self.value.ceil())
    }

    fn round(self) -> Self {
        Self::new(self.value.round())
    }

    fn trunc(self) -> Self {
        Self::new(self.value.trunc())
    }

    fn fract(self) -> Self {
        Self::new(self.value.fract())
    }

    fn abs(self) -> Self {
        Self::new(self.value.abs())
    }

    fn signum(self) -> Self {
        Self::new(self.value.signum())
    }

    fn is_sign_positive(self) -> bool {
        self.value.is_sign_positive()
    }

    fn is_sign_negative(self) -> bool {
        self.value.is_sign_negative()
    }

    fn mul_add(self, a: Self, b: Self) -> Self {
        Self {
            value: self.value.mul_add(a.value, b.value),
            recip: self.recip * a.recip + b.recip,
        }
    }

    fn recip(self) -> Self {
        Self {
            value: self.recip,
            recip: self.value,
        }
    }

    fn powi(self, n: i32) -> Self {
        Self {
            value: self.value.powi(n),
            recip: self.recip.powi(-n),
        }
    }

    fn powf(self, n: Self) -> Self {
        Self {
            value: self.value.powf(n.value),
            recip: self.recip.powf(-n.value),
        }
    }

    fn sqrt(self) -> Self {
        Self::new(self.value.sqrt())
    }

    fn exp(self) -> Self {
        Self {
            value: self.value.exp(),
            recip: T::exp(-self.recip),
        }
    }

    fn exp2(self) -> Self {
        Self {
            value: self.value.exp2(),
            recip: T::exp2(-self.recip),
        }
    }

    fn ln(self) -> Self {
        Self::new(self.value.ln())
    }

    fn log(self, base: Self) -> Self {
        Self::new(self.value.log(base.value))
    }

    fn log2(self) -> Self {
        Self::new(self.value.log2())
    }

    fn log10(self) -> Self {
        Self::new(self.value.log10())
    }

    fn to_degrees(self) -> Self {
        Self {
            value: self.value.to_degrees(),
            recip: self.recip.to_radians(), // Multiplication is division of reciprocals, and deg = rad * 180 / pi => 1/deg = 1/rad * pi / 180
        }
    }

    fn to_radians(self) -> Self {
        Self {
            value: self.value.to_radians(),
            recip: self.recip.to_degrees(),
        }
    }

    fn max(self, other: Self) -> Self {
        Self {
            value: self.value.max(other.value),
            recip: self.recip.min(other.recip),
        }
    }

    fn min(self, other: Self) -> Self {
        Self {
            value: self.value.min(other.value),
            recip: self.recip.max(other.recip),
        }
    }

    fn clamp(self, min: Self, max: Self) -> Self {
        Self {
            value: self.value.clamp(min.value, max.value),
            recip: self.recip.clamp(max.recip, min.recip),
        }
    }

    fn abs_sub(self, other: Self) -> Self {
        Self::new(self.value.abs_sub(other.value))
    }

    fn cbrt(self) -> Self {
        Self::new(self.value.cbrt())
    }

    fn hypot(self, other: Self) -> Self {
        Self::new(self.value.hypot(other.value))
    }

    fn sin(self) -> Self {
        Self::new(self.value.sin())
    }

    fn cos(self) -> Self {
        Self::new(self.value.cos())
    }

    fn tan(self) -> Self {
        Self::new(self.value.tan())
    }

    fn asin(self) -> Self {
        Self::new(self.value.asin())
    }

    fn acos(self) -> Self {
        Self::new(self.value.acos())
    }

    fn atan(self) -> Self {
        Self::new(self.value.atan())
    }

    fn atan2(self, other: Self) -> Self {
        Self::new(self.value.atan2(other.value))
    }

    fn sin_cos(self) -> (Self, Self) {
        let (a, b) = self.value.sin_cos();
        (Self::new(a), Self::new(b))
    }

    fn exp_m1(self) -> Self {
        Self::new(self.value.exp_m1())
    }

    fn ln_1p(self) -> Self {
        Self::new(self.value.ln_1p())
    }

    fn sinh(self) -> Self {
        Self::new(self.value.sinh())
    }

    fn cosh(self) -> Self {
        Self::new(self.value.cosh())
    }

    fn tanh(self) -> Self {
        Self::new(self.value.tanh())
    }

    fn asinh(self) -> Self {
        Self::new(self.value.asinh())
    }

    fn acosh(self) -> Self {
        Self::new(self.value.acosh())
    }

    fn atanh(self) -> Self {
        Self::new(self.value.atanh())
    }

    fn integer_decode(self) -> (u64, i16, i8) {
        self.value.integer_decode()
    }

    fn copysign(self, sign: Self) -> Self {
        Self {
            value: self.value.copysign(sign.value),
            recip: self.recip.copysign(sign.recip),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn non_zero() -> impl Strategy<Value = f64> {
        prop_oneof![(-1e300..-f64::EPSILON), (f64::EPSILON..1e300)]
    }

    #[cfg(not(miri))]
    proptest! {
        #[test]
        fn test_recip_invariants(value in non_zero()) {
            // Skip testing for zero to avoid division by zero
            if value == 0.0 {
                return Ok(());
            }

            let recip = Recip::core_new(value);

            // Check the invariants
            prop_assert!(approx::relative_eq!(recip.value.recip(), recip.recip));
            prop_assert!(approx::relative_eq!(recip.recip.recip(), recip.value));
            prop_assert!(approx::relative_eq!(recip.value.signum(), recip.recip.signum()));
        }

        #[test]
        fn test_recip_add(a in non_zero(), b in non_zero()) {
            let recip_a = Recip::core_new(a);
            let recip_b = Recip::core_new(b);
            let result = recip_a + recip_b;

            prop_assert!(approx::relative_eq!(result.value.recip(), result.recip));
            prop_assert!(approx::relative_eq!(result.recip.recip(), result.value));
            prop_assert!(approx::relative_eq!(result.value.signum(), result.recip.signum()));
        }

        #[test]
        fn test_recip_sub(a in non_zero(), b in non_zero()) {
            let recip_a = Recip::core_new(a);
            let recip_b = Recip::core_new(b);
            let result = recip_a - recip_b;

            prop_assert!(approx::relative_eq!(result.value.recip(), result.recip));
            prop_assert!(approx::relative_eq!(result.recip.recip(), result.value));
            prop_assert!(approx::relative_eq!(result.value.signum(), result.recip.signum()));
        }

        #[test]
        fn test_recip_mul(a in non_zero(), b in non_zero()) {
            let recip_a = Recip::core_new(a);
            let recip_b = Recip::core_new(b);
            let result = recip_a * recip_b;

            prop_assert!(approx::relative_eq!(result.value.recip(), result.recip));
            prop_assert!(approx::relative_eq!(result.recip.recip(), result.value));
            prop_assert!(approx::relative_eq!(result.value.signum(), result.recip.signum()));
        }

        #[test]
        fn test_recip_div(a in non_zero(), b in non_zero()) {
            let recip_a = Recip::core_new(a);
            let recip_b = Recip::core_new(b);
            let result = recip_a / recip_b;

            prop_assert!(approx::relative_eq!(result.value.recip(), result.recip));
            prop_assert!(approx::relative_eq!(result.recip.recip(), result.value));
            prop_assert!(approx::relative_eq!(result.value.signum(), result.recip.signum()));
        }

        #[test]
        fn test_recip_rem(a in non_zero(), b in non_zero()) {
            let recip_a = Recip::core_new(a);
            let recip_b = Recip::core_new(b);
            let result = recip_a % recip_b;

            prop_assert!(approx::relative_eq!(result.value.recip(), result.recip));
            prop_assert!(approx::relative_eq!(result.recip.recip(), result.value));
            prop_assert!(approx::relative_eq!(result.value.signum(), result.recip.signum()));
        }

        #[test]
        fn test_recip_neg(a in non_zero()) {
            let recip_a = Recip::core_new(a);
            let result = -recip_a;

            prop_assert!(approx::relative_eq!(result.value.recip(), result.recip));
            prop_assert!(approx::relative_eq!(result.recip.recip(), result.value));
            prop_assert!(approx::relative_eq!(result.value.signum(), result.recip.signum()));
        }
    }
}
