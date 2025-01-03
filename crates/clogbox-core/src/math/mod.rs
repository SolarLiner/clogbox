//! Math module.
//!
//! This module provides various mathematical functions and algorithms.

use num_traits::float::FloatCore;
use num_traits::{Num, NumCast, NumOps, One, ToPrimitive, Zero};
use std::ops::{Add, Div, Mul, Neg, Rem, Sub};

pub mod dsp;
pub mod interpolation;
pub mod recip;
