use az::{Cast, CastFrom};
use std::borrow::Cow;
use std::cmp::Ordering;
use std::marker::PhantomData;
use std::ops;
use numeric_array::generic_array::GenericArray;
use typenum::{Unsigned, U0};

pub trait Enum: Copy + Send + Eq + Ord + Cast<usize> + CastFrom<usize> {
    type Count: Unsigned;

    fn name(&self) -> Cow<str>;
}

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

#[derive(Debug, Copy, Clone)]
pub struct Dynamic<N: Unsigned>(pub N, pub usize);

impl<N: Unsigned> PartialEq<Self> for Dynamic<N> {
    fn eq(&self, other: &Self) -> bool {
        self.1 == other.1
    }
}

impl<N: Unsigned> Eq for Dynamic<N> {}

impl<N: Unsigned> Ord for Dynamic<N> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.1.cmp(&other.1)
    }
}

impl<N: Unsigned> PartialOrd<Self> for Dynamic<N> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<N: Unsigned> Cast<usize> for Dynamic<N> {
    fn cast(self) -> usize {
        self.1
    }
}

impl<N: Unsigned> CastFrom<usize> for Dynamic<N> {
    fn cast_from(src: usize) -> Self {
        assert!(src < N::USIZE);
        Self(N::default(), src)
    }
}

impl<N: Send + Unsigned> Enum for Dynamic<N> {
    type Count = N;

    fn name(&self) -> Cow<str> {
        Cow::Owned(format!("{}", 1 + self.1))
    }
}

pub trait Collection: ops::Deref<Target = [Self::Item]> {
    type Item;
}

impl<T, C: ops::Deref<Target = [T]>> Collection for C {
    type Item = T;
}

pub trait CollectionMut: Collection + ops::DerefMut<Target = [Self::Item]> {}

impl<C: Collection + ops::DerefMut<Target = [C::Item]>> CollectionMut for C {}

pub type EnumMapArray<E, T> = EnumMap<E, GenericArray<T, <E as Enum>::Count>>;
pub type EnumMapBox<E, T> = EnumMap<E, Box<[T]>>;

pub type EnumMapRef<'a, E, T> = EnumMap<E, &'a [T]>;

pub type EnumMapMut<'a, E, T> = EnumMap<E, &'a mut [T]>;

#[derive(Debug, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub struct EnumMap<E, D> {
    data: D,
    __enum: PhantomData<E>,
}

impl<E, D> EnumMap<E, D> {
    pub fn into_inner(self) -> D {
        self.data
    }
}

impl<E: Enum, D: Collection> ops::Index<E> for EnumMap<E, D> {
    type Output = D::Item;

    fn index(&self, index: E) -> &Self::Output {
        &self.data[index.cast()]
    }
}

impl<E: Enum, D: CollectionMut> ops::IndexMut<E> for EnumMap<E, D> {
    fn index_mut(&mut self, index: E) -> &mut Self::Output {
        &mut self.data[index.cast()]
    }
}

impl<E, D: Collection> EnumMap<E, D> {
    pub fn values(&self) -> impl Iterator<Item = &D::Item> {
        self.data.iter()
    }
    
    pub fn as_ref(&self) -> EnumMapRef<E, D::Item> {
        EnumMapRef {
            data: &*self.data,
            __enum: PhantomData,
        }
    }
}

impl<E, D: CollectionMut> EnumMap<E, D> {
    pub fn values_mut(&mut self) -> impl Iterator<Item = &mut D::Item> {
        self.data.iter_mut()
    }

    pub fn as_mut(&mut self) -> EnumMapMut<E, D::Item> {
        EnumMapMut {
            data: &mut *self.data,
            __enum: PhantomData,
        }
    }
}

impl<E: Enum, D: Collection + FromIterator<D::Item>> EnumMap<E, D> {
    pub fn new(fill: impl Fn(E) -> D::Item) -> Self {
        Self {
            data: (0..E::Count::USIZE)
                .map(|i| E::cast_from(i))
                .map(fill)
                .collect(),
            __enum: PhantomData,
        }
    }
}

impl<E: Enum, D: Collection + IntoIterator<Item = <D as Collection>::Item>> EnumMap<E, D> {
    pub fn map<U, C: Collection<Item = U> + FromIterator<U>>(
        self,
        func: impl Fn(E, <D as Collection>::Item) -> U,
    ) -> EnumMap<E, C> {
        EnumMap {
            data: self
                .data
                .into_iter()
                .enumerate()
                .map(|(i, v)| func(E::cast_from(i), v))
                .collect(),
            __enum: PhantomData,
        }
    }
}

impl<E: Enum, D: Collection> EnumMap<E, D> {
    pub fn iter(&self) -> impl Iterator<Item = (E, &D::Item)> {
        self.data
            .iter()
            .enumerate()
            .map(|(i, v)| (E::cast_from(i), v))
    }
}

impl<E: Enum, D: CollectionMut> EnumMap<E, D> {
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (E, &mut D::Item)> {
        self.data
            .iter_mut()
            .enumerate()
            .map(|(i, v)| (E::cast_from(i), v))
    }
}
