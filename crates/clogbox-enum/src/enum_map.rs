//! A submodule for working with maps indexed by enum variants.
//!
//! This module provides structures and utilities for efficiently
//! creating and manipulating maps where the keys are enum variants. This allows
//! for type-safe indexing and ensures that each key corresponds to a specific
//! enum variant, avoiding potential errors from arbitrary indexing.
//!
//! The core structure provided by this module is `EnumMap<K, V>`, which stores
//! values of type `V` indexed by enum keys of type `K`. This module also
//! includes iterators and utility methods for working with such maps.

use crate::{count, Enum};
use numeric_array::generic_array::{GenericArray, IntoArrayLength};
use numeric_array::ArrayLength;
use std::iter::{Enumerate, Map};
use std::marker::PhantomData;
use std::mem::MaybeUninit;
use std::ops;
use std::ops::{Deref, DerefMut};
use typenum::{Cmp, Equal};

/// A trait that represents a collection of items.
pub trait Collection: Deref<Target = [Self::Item]> {
    /// The type of items in the collection.
    type Item;
}

impl<T, C: Deref<Target = [T]>> Collection for C {
    type Item = T;
}

/// A trait for collections that support mutable operations.
///
/// This trait extends `Collection` and `DerefMut`, allowing mutable access to the collection's items.
pub trait CollectionMut: Collection + DerefMut<Target = [Self::Item]> {}

impl<C: Collection + DerefMut<Target = [C::Item]>> CollectionMut for C {}

/// A type alias for an `EnumMap` where the underlying data is a `GenericArray`.
///
/// This type alias provides a convenient way to create an `EnumMap` where the data
/// is stored in a fixed-size array, using `GenericArray`. The size of the array is
/// determined by the number of enum variants (`E::Count`).
///
/// # Example
/// ```rust
/// use clogbox_enum::enum_map::{EnumMap, EnumMapArray};
/// use clogbox_derive::Enum;
///
/// #[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Enum)]
/// enum Color {
///     Red,
///     Green,
///     Blue,
/// }
///
/// let array_map: EnumMapArray<Color, u32> = EnumMap::new(|color| match color {
///     Color::Red => 10,
///     Color::Green => 20,
///     Color::Blue => 30,
/// });
/// ```
pub type EnumMapArray<E, T> = EnumMap<E, GenericArray<T, <E as Enum>::Count>>;

impl<E: Enum, T> EnumMapArray<E, T>
where
    typenum::Const<0>: IntoArrayLength<ArrayLength = E::Count>,
{
    pub const CONST_DEFAULT: Self = Self {
        data: GenericArray::from_array([]),
        __enum: PhantomData,
    };
}

impl<E: Enum, T> Default for EnumMapArray<E, T>
where
    typenum::Const<0>: IntoArrayLength<ArrayLength = E::Count>,
{
    fn default() -> Self {
        Self::CONST_DEFAULT
    }
}

/// A type alias for an `EnumMap` where the underlying data is stored in a heap-allocated array (`Box<[T]>`).
///
/// This type alias represents an `EnumMap` where the data for each enum variant is stored in a
/// heap-allocated slice (`Box<[T]>`). This is useful when the data size is not known at compile time,
/// or when the map's data needs to be dynamically allocated on the heap.
///
/// # Example
/// ```rust
/// use clogbox_enum::enum_map::{EnumMap, EnumMapBox};
/// use clogbox_derive::Enum;
///
/// #[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Enum)]
/// enum Color {
///     Red,
///     Green,
///     Blue,
/// }
///
/// let box_map: EnumMapBox<Color, u32> = EnumMap::new(|color| match color {
///     Color::Red => 100,
///     Color::Green => 200,
///     Color::Blue => 300,
/// });
/// ```
pub type EnumMapBox<E, T> = EnumMap<E, Box<[T]>>;

/// A type alias for an `EnumMap` that contains immutable references to the underlying data (`&[T]`).
///
/// This type alias represents an `EnumMap` where the values are immutable references to a slice of data.
/// It allows you to work with a read-only view of the underlying array without taking ownership.
///
/// # Example
/// ```rust
/// use clogbox_enum::enum_map::{EnumMap, EnumMapArray, EnumMapRef};
/// use clogbox_derive::Enum;
///
/// #[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Enum)]
/// enum Color {
///     Red,
///     Green,
///     Blue,
/// }
///
/// let values = [10, 20, 30];
/// let map: EnumMapArray<Color, u32> = EnumMap::new(|color| match color {
///     Color::Red => 10,
///     Color::Green => 20,
///     Color::Blue => 30,
/// });
/// let ref_map: EnumMapRef<_, _> = map.to_ref();
/// ```
pub type EnumMapRef<'a, E, T> = EnumMap<E, &'a [T]>;

/// A type alias for an `EnumMap` that contains mutable references to the underlying data (`&mut [T]`).
///
/// This type alias represents an `EnumMap` where the values are mutable references to a slice of data.
/// It allows you to modify the underlying data in place without taking ownership of the original slice.
///
/// # Example
/// ```rust
/// use clogbox_enum::enum_map::{EnumMap, EnumMapArray, EnumMapMut};
/// use clogbox_derive::Enum;
///
/// #[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Enum)]
/// enum Color {
///     Red,
///     Green,
///     Blue,
/// }
///
/// let mut map: EnumMapArray<Color, u32> = EnumMap::new(|color| match color {
///     Color::Red => 10,
///     Color::Green => 20,
///     Color::Blue => 30,
/// });
/// let mut map_mut: EnumMapMut<_, _> = map.to_mut();
/// map_mut[Color::Red] += 5;
/// assert_eq!(15, map[Color::Red]);
/// ```
pub type EnumMapMut<'a, E, T> = EnumMap<E, &'a mut [T]>;

/// A map that uses an enum type `K` as keys and stores associated values of type `V`.
///
/// `EnumMap` provides an efficient way to manage collections where the keys are
/// specific enum variants. It offers constant-time access to elements and ensures
/// type safety, as the keys must be valid enum variants.
///
/// # Example
/// ```rust
/// use clogbox_derive::Enum;
/// use clogbox_enum::enum_map::{EnumMap, EnumMapArray};
///
///  #[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Enum)]
/// enum Color {
///     Red,
///     Green,
///     Blue,
/// }
///
/// let mut map: EnumMapArray<Color, u32> = EnumMap::new(|color| match color {
///     Color::Red => 10,
///     Color::Green => 20,
///     Color::Blue => 30,
/// });
/// assert_eq!(map[Color::Red], 10);
/// ```
#[derive(Debug, Copy, Ord, PartialOrd, Eq, PartialEq)]
pub struct EnumMap<E, D> {
    pub(crate) data: D,
    pub(crate) __enum: PhantomData<E>, // <!> This needs to stay PhantomData for the unsafe blocks below!
}

impl<E, D: Clone> Clone for EnumMap<E, D> {
    fn clone(&self) -> Self {
        Self {
            data: self.data.clone(),
            __enum: PhantomData,
        }
    }
}

impl<E, D> AsRef<EnumMap<usize, D>> for EnumMap<E, D> {
    fn as_ref(&self) -> &EnumMap<usize, D> {
        // Safety: This is safe because we are only changing the type of the enum, which is behind a PhantomData
        unsafe { &*(self as *const EnumMap<E, D> as *const EnumMap<usize, D>) }
    }
}

#[cfg(feature = "serialize")]
impl<'de, E, Data: serde::Deserialize<'de>> serde::Deserialize<'de> for EnumMap<E, Data> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let data = Data::deserialize(deserializer)?;
        Ok(Self {
            data,
            __enum: PhantomData,
        })
    }
}

#[cfg(feature = "serialize")]
impl<E, D: serde::Serialize> serde::Serialize for EnumMap<E, D> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.data.serialize(serializer)
    }
}

impl<E: Enum, D: IntoIterator> IntoIterator for EnumMap<E, D> {
    type Item = (E, D::Item);
    type IntoIter = Map<Enumerate<D::IntoIter>, fn((usize, D::Item)) -> (E, D::Item)>;

    fn into_iter(self) -> Self::IntoIter {
        self.data.into_iter().enumerate().map(|(i, v)| (E::from_usize(i), v))
    }
}

impl<E, D> EnumMap<E, D> {
    /// Consumes the `EnumMap` and returns the underlying data.
    pub fn into_inner(self) -> D {
        self.data
    }
}

impl<E, D: Collection> EnumMap<E, D> {
    pub fn runtime_len(&self) -> usize {
        self.data.len()
    }

    pub fn dyn_get(&self, index: usize) -> Option<&D::Item> {
        self.data.get(index)
    }
}

impl<E: Enum, D> EnumMap<E, D> {
    /// Returns the number of elements in the `EnumMap`.
    ///
    /// # Returns
    ///
    /// The number of elements in the `EnumMap`, which is equal to the number of variants in the enum `E`.
    pub const fn len(&self) -> usize {
        count::<E>()
    }

    /// Checks if the `EnumMap` is empty.
    ///
    /// # Returns
    ///
    /// `true` if the `EnumMap` is empty, `false` otherwise.
    pub const fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl<E: Enum, D: CollectionMut> EnumMap<E, D> {
    /// Sets all values in the `EnumMap` from a given array of values.
    ///
    /// This method takes a `GenericArray` containing a value for each variant of the
    /// enum `E`, and updates the `EnumMap` such that each enum variant is associated
    /// with the corresponding value from the array. The length of the array must match
    /// the number of variants in the enum.
    ///
    /// # Arguments
    /// - `values`: A `GenericArray` containing one value for each enum variant.
    ///
    /// The length of the array (`E::Count`) must match the number of variants in `E`,
    /// ensuring that all enum variants have corresponding values.
    ///
    /// # Example
    /// ```rust
    /// use typenum::U3;
    /// use clogbox_enum::enum_map::{EnumMap, EnumMapArray};
    /// use clogbox_enum::Sequential;
    /// use numeric_array::generic_array::arr;
    ///
    /// let mut color_map = EnumMapArray::<Sequential<U3>, u32>::new(|_| 0);
    /// color_map.set_all(arr![1, 2, 3]);  // Assuming Color enum has 3 variants
    /// ```
    pub fn set_all<E2: ArrayLength>(&mut self, values: GenericArray<D::Item, E2>)
    where
        E::Count: Cmp<E2, Output = Equal>,
    {
        for (storage, value) in self.data.iter_mut().zip(values) {
            *storage = value;
        }
    }
}

impl<E: Enum, D: Collection> ops::Index<E> for EnumMap<E, D> {
    type Output = D::Item;

    fn index(&self, index: E) -> &Self::Output {
        &self.data[index.to_usize()]
    }
}

impl<E: Enum, D: CollectionMut> ops::IndexMut<E> for EnumMap<E, D> {
    fn index_mut(&mut self, index: E) -> &mut Self::Output {
        &mut self.data[index.to_usize()]
    }
}

impl<E, D: Collection> EnumMap<E, D> {
    /// Returns an iterator over the values in the `EnumMap`.
    ///
    /// This method provides an immutable iterator over all values stored in the
    /// `EnumMap`. Each value corresponds to one of the enum variants.
    ///
    /// # Returns
    /// An iterator over references to the values of type `D::Item`.
    ///
    /// # Example
    /// ```rust
    /// use clogbox_enum::Enum;
    /// use typenum::U3;
    /// use clogbox_enum::enum_map::EnumMapArray;
    /// use clogbox_enum::Sequential;
    /// let map = EnumMapArray::<Sequential<U3>, usize>::new(|k| k.to_usize());
    /// let output = Vec::from_iter(map.values());
    /// assert_eq!(vec![&0, &1, &2], output);
    /// ```
    pub fn values(&self) -> impl Iterator<Item = &D::Item> {
        self.data.iter()
    }

    /// Converts the `EnumMap` into a reference-based `EnumMapRef`.
    ///
    /// This method returns an `EnumMapRef`, which is a reference-based version
    /// of the original `EnumMap`. It allows for working with references to the
    /// data instead of owning the data, without transferring ownership.
    ///
    /// # Returns
    /// An `EnumMapRef<E, D::Item>`, where `E` is the enum type and `D::Item` is
    /// the value type.
    ///
    /// # Example
    /// ```rust
    /// use clogbox_enum::Enum;
    /// use typenum::U3;
    /// use clogbox_enum::enum_map::{EnumMapArray, EnumMapRef};
    /// use clogbox_enum::Sequential;
    /// let map = EnumMapArray::<Sequential<U3>, usize>::new(|k| k.to_usize());
    /// let map_ref: EnumMapRef<_, _> = map.to_ref();
    /// let values: Vec<(Sequential<U3>, &usize)> = Vec::from_iter(map_ref.into_iter());
    /// ```
    pub fn to_ref(&self) -> EnumMapRef<E, D::Item> {
        EnumMapRef {
            data: &*self.data,
            __enum: PhantomData,
        }
    }

    /// Returns a slice of the underlying data in the `EnumMap`.
    ///
    /// This method provides access to the underlying data in the `EnumMap` as
    /// a slice. The slice contains all the values, in the order of the enum's
    /// variants.
    ///
    /// # Returns
    /// A slice containing all values in the `EnumMap`, of type `&[D::Item]`.
    ///
    /// # Example
    /// ```rust
    /// use clogbox_enum::Enum;
    /// use typenum::U3;
    /// use clogbox_enum::enum_map::EnumMapArray;
    /// use clogbox_enum::Sequential;
    /// let map = EnumMapArray::<Sequential<U3>, usize>::new(|k| k.to_usize());
    /// let values_slice = map.as_slice();
    /// assert_eq!(&[0, 1, 2], values_slice);
    /// ```
    pub fn as_slice(&self) -> &[D::Item] {
        &self.data
    }
}

impl<E, D: CollectionMut> EnumMap<E, D> {
    /// Returns an iterator over mutable references to the values in the `EnumMap`.
    ///
    /// This method provides an immutable iterator over all values stored in the
    /// `EnumMap`. Each value corresponds to one of the enum variants.
    ///
    /// # Returns
    /// An iterator over mutable references to the values of type `D::Item`.
    ///
    /// # Example
    /// ```rust
    /// use clogbox_enum::Enum;
    /// use typenum::U3;
    /// use clogbox_enum::enum_map::EnumMapArray;
    /// use clogbox_enum::Sequential;
    /// let mut map = EnumMapArray::<Sequential<U3>, usize>::new(|k| k.to_usize());
    /// let output = Vec::from_iter(map.values_mut());
    /// assert_eq!(vec![&0, &1, &2], output);
    /// ```
    pub fn values_mut(&mut self) -> impl Iterator<Item = &mut D::Item> {
        self.data.iter_mut()
    }

    /// Converts the `EnumMap` into a mutable reference-based `EnumMapMut`.
    ///
    /// This method returns an `EnumMapMut`, which is a mutable reference-based version
    /// of the original `EnumMap`. It allows for working with references to the
    /// data instead of owning the data, without transferring ownership.
    ///
    /// # Returns
    /// An `EnumMapMut<E, D::Item>`, where `E` is the enum type and `D::Item` is
    /// the value type.
    ///
    /// # Example
    /// ```rust
    /// use clogbox_enum::Enum;
    /// use typenum::U3;
    /// use clogbox_enum::enum_map::{EnumMapArray, EnumMapMut};
    /// use clogbox_enum::Sequential;
    /// let mut map = EnumMapArray::<Sequential<U3>, usize>::new(|k| k.to_usize());
    /// let map_mut: EnumMapMut<_, _> = map.to_mut();
    /// let values: Vec<(Sequential<U3>, &mut usize)> = Vec::from_iter(map_mut.into_iter());
    /// ```
    pub fn to_mut(&mut self) -> EnumMapMut<E, D::Item> {
        EnumMapMut {
            data: &mut *self.data,
            __enum: PhantomData,
        }
    }

    /// Returns a mutable slice of the underlying data in the `EnumMap`.
    ///
    /// This method provides access to the underlying data in the `EnumMap` as
    /// a slice. The slice contains all the values, in the order of the enum's
    /// variants.
    ///
    /// # Returns
    /// A mutable slice containing all values in the `EnumMap`, of type `&[D::Item]`.
    ///
    /// # Example
    /// ```rust
    /// use typenum::U3;
    /// use clogbox_enum::Enum;
    /// use clogbox_enum::enum_map::EnumMapArray;
    /// use clogbox_enum::Sequential;
    /// let mut map = EnumMapArray::<Sequential<U3>, usize>::new(|k| k.to_usize());
    /// let values_slice = map.as_slice_mut();
    /// assert_eq!(&mut [0, 1, 2], values_slice);
    /// ```
    pub fn as_slice_mut(&mut self) -> &mut [D::Item] {
        &mut self.data
    }
}

impl<E: Enum, D: Collection + FromIterator<D::Item>> FromIterator<D::Item> for EnumMap<E, D> {
    fn from_iter<T: IntoIterator<Item = D::Item>>(iter: T) -> Self {
        let data = D::from_iter(iter);
        assert_eq!(data.len(), count::<E>(), "Invalid number of elements for EnumMap");
        Self {
            data,
            __enum: PhantomData,
        }
    }
}

impl<E: Enum, D: Collection + FromIterator<D::Item>> EnumMap<E, D> {
    /// Creates a new `EnumMap` by filling it with values generated by a given function.
    ///
    /// This constructor initializes an `EnumMap` where the values for each enum variant
    /// are generated by the provided function. The function takes an enum variant as input
    /// and returns a value of type `D::Item`, which is then stored in the map for that variant.
    ///
    /// # Arguments
    /// - `fill`: A function that takes an enum variant `E` and returns a value of type `D::Item`.
    ///
    /// # Returns
    /// A new `EnumMap<E, D>` where each enum variant is associated with a value produced by the `fill` function.
    ///
    /// # Example
    /// ```rust
    /// use clogbox_enum::enum_map::EnumMapArray;
    /// use clogbox_derive::Enum;
    ///
    /// #[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Enum)]
    /// enum Color {
    ///     Red,
    ///     Green,
    ///     Blue,
    /// }
    ///
    /// let color_map = EnumMapArray::new(|color| {
    ///     match color {
    ///         Color::Red => 10,
    ///         Color::Green => 20,
    ///         Color::Blue => 30,
    ///     }
    /// });
    /// assert_eq!(color_map[Color::Red], 10);
    /// assert_eq!(color_map[Color::Green], 20);
    /// assert_eq!(color_map[Color::Blue], 30);
    /// ```
    pub fn new(fill: impl FnMut(E) -> D::Item) -> Self {
        Self {
            data: crate::enum_iter().map(fill).collect(),
            __enum: PhantomData,
        }
    }
}

impl<E: Enum, D: Collection + IntoIterator<Item = <D as Collection>::Item>> EnumMap<E, D> {
    /// Transforms the values in the `EnumMap` by applying a function to each value.
    ///
    /// This method takes ownership of the `EnumMap` and applies the provided function
    /// to each value, creating a new `EnumMap` where each value is transformed into a
    /// new type `U`. The function receives both the enum variant and the corresponding
    /// value from the original map, allowing the transformation to depend on both the
    /// key (enum variant) and the value.
    ///
    /// # Arguments
    /// - `func`: A function that takes an enum variant `E` and a value of type `D::Item`, and
    ///   returns a new value of type `U`.
    ///
    /// # Returns
    /// A new `EnumMap<E, C>` where each value is the result of applying the `func`
    /// to the corresponding value in the original map.
    ///
    /// # Example
    /// ```rust
    /// use numeric_array::generic_array::GenericArray;
    /// use clogbox_enum::Enum;
    /// use clogbox_enum::enum_map::{EnumMap, EnumMapArray};
    ///
    ///  #[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Enum)]
    /// enum Color {
    ///     Red,
    ///     Green,
    ///     Blue,
    /// }
    ///
    /// let color_map = EnumMapArray::new(|color| match color {
    ///     Color::Red => 1,
    ///     Color::Green => 2,
    ///     Color::Blue => 3,
    /// });
    ///
    /// let transformed_map = color_map.map::<GenericArray<_,<Color as Enum>::Count>>(|color, value| format!("{:?}: {}", color, value));
    ///
    /// assert_eq!(transformed_map[Color::Red], "Red: 1");
    /// assert_eq!(transformed_map[Color::Green], "Green: 2");
    /// assert_eq!(transformed_map[Color::Blue], "Blue: 3");
    /// ```
    pub fn map<C: Collection + FromIterator<C::Item>>(
        self,
        func: impl Fn(E, <D as Collection>::Item) -> C::Item,
    ) -> EnumMap<E, C> {
        EnumMap {
            data: self
                .data
                .into_iter()
                .enumerate()
                .map(|(i, v)| func(E::from_usize(i), v))
                .collect(),
            __enum: PhantomData,
        }
    }
}

impl<E: Enum, D: Collection> EnumMap<E, D> {
    /// Returns an iterator over the `EnumMap` that yields pairs of enum variants and references to their values.
    ///
    /// This method provides an iterator that goes through all the entries in the `EnumMap`, returning a tuple
    /// for each entry where the first item is an enum variant `E` and the second item is a reference to the value
    /// of type `&D::Item` associated with that variant. It allows you to iterate over both the keys (enum variants)
    /// and their corresponding values.
    ///
    /// You can also iterate over references of this map by creating a borrow (e.g. `for (variant, value_ref) in &map {}`).
    ///
    /// # Returns
    /// An iterator over the `EnumMap`, yielding tuples of the form `(E, &D::Item)`.
    ///
    /// # Example
    /// ```rust
    /// use clogbox_enum::enum_map::{EnumMap, EnumMapArray};
    /// use clogbox_derive::Enum;
    ///
    ///  #[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Enum)]
    /// enum Color {
    ///     Red,
    ///     Green,
    ///     Blue,
    /// }
    ///
    /// let color_map = EnumMapArray::new(|color| match color {
    ///     Color::Red => 1,
    ///     Color::Green => 2,
    ///     Color::Blue => 3,
    /// });
    ///
    /// for (color, value) in color_map.iter() {
    ///     println!("{:?}: {}", color, value);
    /// }
    /// ```
    ///
    /// This example will output:
    /// ```text
    /// Red: 1
    /// Green: 2
    /// Blue: 3
    /// ```
    pub fn iter(&self) -> impl Iterator<Item = (E, &D::Item)> {
        self.data.iter().enumerate().map(|(i, v)| (E::from_usize(i), v))
    }

    pub fn items_as_ref<T: ?Sized>(&self) -> EnumMapArray<E, &T>
    where
        D::Item: AsRef<T>,
    {
        EnumMapArray::from_iter(self.data.iter().map(|v| v.as_ref()))
    }

    pub fn items_as_deref(&self) -> EnumMapArray<E, &<D::Item as Deref>::Target>
    where
        D::Item: Deref,
    {
        EnumMapArray::from_iter(self.data.iter().map(|v| v.deref()))
    }
}

impl<E: Enum, D: CollectionMut> EnumMap<E, D> {
    /// Returns a mutable iterator over the `EnumMap`, yielding pairs of enum variants and mutable references to their values.
    ///
    /// This method provides a mutable iterator that goes through all the entries in the `EnumMap`, returning a tuple
    /// for each entry where the first item is an enum variant `E` and the second item is a mutable reference to the value
    /// of type `&mut D::Item` associated with that variant. It allows you to iterate over both the keys (enum variants)
    /// and modify their corresponding values.
    ///
    /// # Returns
    /// A mutable iterator over the `EnumMap`, yielding tuples of the form `(E, &mut D::Item)`.
    ///
    /// # Example
    /// ```rust
    /// use clogbox_enum::enum_map::{EnumMap, EnumMapArray};
    /// use clogbox_derive::Enum;
    ///
    ///  #[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Enum)]
    /// enum Color {
    ///     Red,
    ///     Green,
    ///     Blue,
    /// }
    ///
    /// let mut color_map = EnumMapArray::new(|color| match color {
    ///     Color::Red => 1,
    ///     Color::Green => 2,
    ///     Color::Blue => 3,
    /// });
    ///
    /// for (color, value) in color_map.iter_mut() {
    ///     *value += 10;
    ///     println!("{:?}: {}", color, value);
    /// }
    /// ```
    ///
    /// This example will output:
    /// ```text
    /// Red: 11
    /// Green: 12
    /// Blue: 13
    /// ```
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (E, &mut D::Item)> {
        self.data.iter_mut().enumerate().map(|(i, v)| (E::from_usize(i), v))
    }

    pub fn items_as_deref_mut(&mut self) -> EnumMapArray<E, &mut <D::Item as Deref>::Target>
    where
        D::Item: DerefMut,
    {
        EnumMapArray::from_iter(self.data.iter_mut().map(|v| v.deref_mut()))
    }

    pub fn items_as_mut<T: ?Sized>(&mut self) -> EnumMapArray<E, &mut T>
    where
        D::Item: AsMut<T>,
    {
        EnumMapArray::from_iter(self.data.iter_mut().map(|v| v.as_mut()))
    }
}

impl<E: Enum, T, Err, C: IntoIterator<Item = <C as Collection>::Item> + Collection<Item = Result<T, Err>>>
    EnumMap<E, C>
{
    pub fn transpose<D: Collection<Item = T> + FromIterator<T>>(self) -> Result<EnumMap<E, D>, Err> {
        let mut data = EnumMapArray::new(|_| MaybeUninit::uninit());
        for (k, v) in self.into_iter() {
            match v {
                Ok(v) => {
                    data[k].write(v);
                }
                Err(e) => return Err(e),
            }
        }
        // # Safety
        //
        // All values have been written to, or the function has already returned early by this
        // point.
        // Also, after `read`, each value will never be observed again.
        unsafe { Ok(EnumMap::new(|e| data[e].assume_init_read())) }
    }
}

impl<E: Enum, T> EnumMapArray<E, T> {
    /// Creates a new `EnumMapArray` from a given `GenericArray`.
    ///
    /// This associated constant function allows for the creation of an `EnumMapArray`
    /// by taking an existing `GenericArray` of type `T` with a size determined by the
    /// number of enum variants (`E::Count`). It initializes the `EnumMapArray` with
    /// the provided data and maintains the necessary type information using `PhantomData`.
    ///
    /// # Arguments
    /// - `array`: A `GenericArray<T, E::Count>` containing the initial values for each enum variant.
    ///
    /// # Returns
    /// An instance of `EnumMapArray<E, T>` initialized with the provided array.
    ///
    /// # Example
    /// ```rust
    /// use numeric_array::generic_array::GenericArray;
    /// use typenum::U3;
    /// use clogbox_enum::enum_map::EnumMapArray;
    /// use clogbox_derive::Enum;
    ///
    ///  #[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Enum)]
    /// enum Color {
    ///     Red,
    ///     Green,
    ///     Blue,
    /// }
    ///
    /// let array = GenericArray::<u32, U3>::from([1, 2, 3]);
    /// let map = EnumMapArray::from_array([1, 2, 3].into());
    ///
    /// assert_eq!(map[Color::Red], 1);
    /// assert_eq!(map[Color::Green], 2);
    /// assert_eq!(map[Color::Blue], 3);
    /// ```
    pub const fn from_array(array: GenericArray<T, E::Count>) -> Self {
        Self {
            data: array,
            __enum: PhantomData,
        }
    }

    pub const fn from_std_array<const N: usize>(array: [T; N]) -> Self
    where
        typenum::Const<N>: IntoArrayLength<ArrayLength = E::Count>,
    {
        Self::from_array(GenericArray::from_array(array))
    }
}

impl<'a, E, T> EnumMapRef<'a, E, T> {
    pub const fn from_slice(slice: &'a [T]) -> Self {
        Self {
            data: slice,
            __enum: PhantomData,
        }
    }
}

impl<'a, E, T> EnumMapMut<'a, E, T> {
    pub fn from_slice_mut(slice: &'a mut [T]) -> Self {
        Self {
            data: slice,
            __enum: PhantomData,
        }
    }
}
