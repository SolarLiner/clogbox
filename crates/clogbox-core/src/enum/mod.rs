//! This module provides various utilities and traits for working with enums in a strongly-typed manner.
//!
//! The primary focus of this module is the `Enum` trait, which allows enums to be treated like integers
//! for indexing purposes while retaining the type safety and benefits of an enum. A procedural macro can be used
//! to derive this trait for enums automatically.
//!
//! # Example
//!
//! ```rust
//! use clogbox_derive::Enum;
//! use clogbox_core::r#enum::{enum_iter, Enum};
//!
//! #[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Enum)]
//! enum Color {
//!     Red,
//!     Green,
//!     Blue,
//! }
//!
//! let color = Color::Red;
//! println!("Color name: {}", color.name());
//!
//! for variant in enum_iter::<Color>() {
//!     println!("{:?}", variant);
//! }
//! ```
use az::{Cast, CastFrom};
use numeric_array::ArrayLength;
use std::borrow::Cow;
use std::cmp::Ordering;
use std::ops;
use typenum::{Prod, Unsigned, U0};

pub mod enum_map;

/// A trait for enums that are used in strongly-typed indexing.
///
/// This trait allows enums to be treated like integers (`usize`) for indexing
/// purposes, while still retaining the type safety and benefits of an enum.
/// The enum variants must be able to be converted to and from a `usize` and
/// have an associated count representing the total number of variants.
///
/// For enums with unit variants (where each variant has no data), a `#[derive(Enum)]`
/// macro is available to automatically implement this trait, simplifying the process.
///
/// # Example
/// ```rust
/// use clogbox_derive::Enum;
///  #[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Enum)]
/// enum Color {
///     Red,
///     Green,
///     Blue,
/// }
/// ```
pub trait Enum: Copy + Send + Eq + Ord + Cast<usize> + CastFrom<usize> {
    /// An associated constant representing the total number of enum variants.
    ///
    /// This is used to define the length of arrays or other collections
    /// that index using this enum. The type must be unsigned and compatible
    /// with compile-time array lengths.
    type Count: Unsigned + ArrayLength;

    /// Returns the name of the enum variant as a `Cow<str>`.
    ///
    /// This can be used for debugging, logging, or display purposes, allowing
    /// the enum's variant to be converted to a human-readable string.
    ///
    /// # Example
    /// ```rust
    /// use clogbox_core::r#enum::Enum;
    /// use clogbox_derive::Enum;
    ///  #[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Enum)]
    /// enum Color {
    ///     Red,
    ///     Green,
    ///     Blue,
    /// }
    /// let color = Color::Red;
    /// assert_eq!(color.name(), "Red");
    /// ```
    fn name(&self) -> Cow<str>;
}

/// An empty, never instantiable enum.
#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub enum Empty {}

impl Cast<usize> for Empty {
    fn cast(self) -> usize {
        unreachable!()
    }
}

impl CastFrom<usize> for Empty {
    fn cast_from(_: usize) -> Self {
        unreachable!()
    }
}

impl Enum for Empty {
    type Count = U0;

    fn name(&self) -> Cow<str> {
        unreachable!()
    }
}

/// Iterate all variants of the given enum
pub fn enum_iter<E: Enum>() -> impl Iterator<Item = E> {
    (0..E::Count::USIZE).map(|i| E::cast_from(i))
}

/// A wrapper type representing a sequential index with a compile-time known size.
///
/// `Sequential<N>` is a type-safe struct used to track an index at runtime (`usize`)
/// while enforcing bounds at compile-time using `typenum::Unsigned` for the size `N`.
/// This is especially useful for working with collections or enums where the size is
/// known and can be represented as a compile-time constant, preventing invalid indexing.
///
/// The type-level integer `N` from the `typenum` crate represents a non-negative integer
/// at compile time (e.g., the total number of enum variants or elements in an array).
/// The index is stored as a `usize`, ensuring it's always valid within the bounds set
/// by `N`.
///
/// This type can be used as the [`Module::Inputs`](crate::module::Module::Inputs),
/// [`Module::Outputs`](crate::module::Module::Outputs), or
/// [`SetParameter::Param`](crate::param::SetParameter::Param) type without having to create your
/// own type. However, for readability, it is still recommended, where it makes sense, to create and
/// use your own enum type.
///
/// # Example
///
/// ```rust
/// use typenum::U3;
/// use clogbox_core::r#enum::{seq, Sequential};
///
/// let index: Sequential<U3> = seq::<U3>(2); // Creates a sequential index for a size 3 collection
/// ```
///
/// This type is typically used for iterating over enum variants or indexing into
/// fixed-size collections in a type-safe manner.
#[derive(Debug, Copy, Clone)]
pub struct Sequential<N: Unsigned>(N, usize);

impl<N: Unsigned> Cast<usize> for Sequential<N> {
    fn cast(self) -> usize {
        self.1
    }
}

impl<N: Unsigned> CastFrom<usize> for Sequential<N> {
    fn cast_from(src: usize) -> Self {
        seq(src)
    }
}

impl<N: Unsigned> From<usize> for Sequential<N> {
    fn from(value: usize) -> Self {
        seq(value)
    }
}

/// Constructs a `Sequential<N>` instance, ensuring that the index `n` is valid
/// within the bounds set by `N`.
///
/// The function asserts that the provided index `n` is less than `N::USIZE`,
/// which represents the compile-time constant size associated with the `N` type.
///
/// # Panics
///
/// Panics if the index `n` is greater than or equal to the size `N::USIZE`,
/// preventing out-of-bounds indexing.
///
/// # Example
///
/// ```rust
/// use typenum::U3;
/// use clogbox_core::r#enum::seq;
///
/// let valid_index = seq::<U3>(2); // Valid index within bounds for a size 3 array
/// // let invalid_index = seq::<U3>(3); // Panics because 3 is out of bounds
/// ```
pub fn seq<N: Unsigned>(n: usize) -> Sequential<N> {
    assert!(n < N::USIZE);
    Sequential(N::default(), n)
}

impl<N: Unsigned> PartialEq<Self> for Sequential<N> {
    fn eq(&self, other: &Self) -> bool {
        self.1 == other.1
    }
}

impl<N: Unsigned> Eq for Sequential<N> {}

impl<N: Unsigned> Ord for Sequential<N> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.1.cmp(&other.1)
    }
}

impl<N: Unsigned> PartialOrd<Self> for Sequential<N> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<N: Send + Unsigned + ArrayLength> Enum for Sequential<N> {
    type Count = N;

    fn name(&self) -> Cow<str> {
        Cow::Owned(format!("{}", 1 + self.1))
    }
}

/// A struct representing the Cartesian product of two enum types, `A` and `B`.
///
/// The `CartesianProduct<A, B>` combines two enums into a single type that
/// represents all possible pairs of their variants. It implements the `Enum`
/// trait, allowing it to be used for strongly-typed indexing while maintaining
/// type safety.
///
/// The total number of variants in `CartesianProduct` is the product of the 
/// variants from both enums, making it useful for handling combinations of 
/// states.
///
/// ## Example
/// ```rust
/// use clogbox_core::r#enum::{Enum,CartesianProduct};
/// use clogbox_derive::Enum;
///  #[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Enum)]
/// enum Color {
///     Red,
///     Green,
///     Blue,
/// }
///
///  #[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Enum)]
/// enum Shape {
///     Circle,
///     Square,
///     Triangle,
/// }
///
/// let color_shape = CartesianProduct(Color::Red, Shape::Circle);
/// assert_eq!(color_shape.name(), "Red:Circle");
/// ```
#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub struct CartesianProduct<A, B>(pub A, pub B);

impl<A: Enum, B: Enum> CastFrom<usize> for CartesianProduct<A, B> {
    fn cast_from(src: usize) -> Self {
        let src_a = src / A::Count::USIZE;
        let src_b = src % A::Count::USIZE;
        Self(A::cast_from(src_a), B::cast_from(src_b))
    }
}

impl<A: Enum, B: Enum> Cast<usize> for CartesianProduct<A, B> {
    fn cast(self) -> usize {
        self.0.cast() * A::Count::USIZE + self.1.cast()
    }
}

impl<A: Enum, B: Enum> Enum for CartesianProduct<A, B>
where
    A::Count: ops::Mul<B::Count, Output: Unsigned>,
    <A::Count as ops::Mul<B::Count>>::Output: Unsigned + ArrayLength,
{
    type Count = Prod<A::Count, B::Count>;

    fn name(&self) -> Cow<str> {
        Cow::Owned(format!("{}:{}", self.0.name(), self.1.name()))
    }
}

