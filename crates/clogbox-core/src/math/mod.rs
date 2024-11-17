//! Math module. 
//!
//! This module provides various mathematical functions and algorithms.

use std::ops::{Add, Div, Mul, Neg, Rem, Sub};
use num_traits::float::FloatCore;
use num_traits::{Num, NumCast, NumOps, One, ToPrimitive, Zero};

pub mod interpolation;
pub mod dsp;
pub mod recip;

