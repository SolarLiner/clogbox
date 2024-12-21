use std::borrow::Cow;
use duplicate::duplicate_item;
use crate::graph::SlotType;
use crate::r#enum::Enum;

/// Trait for types which describe input/output slots in a module.
pub trait Slots: Enum {
    fn slot_type(&self) -> SlotType;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EnumSlots<E: Enum, const SLOT_TYPE_IDX: usize>(pub E);

impl<E: Enum, const SLOT_TYPE_IDX: usize> From<E> for EnumSlots<E, SLOT_TYPE_IDX> {
    fn from(value: E) -> Self {
        Self(value)
    }
}

impl<E: Enum, const SLOT_TYPE_IDX: usize> az::CastFrom<usize> for EnumSlots<E, SLOT_TYPE_IDX> {
    fn cast_from(i: usize) -> Self {
        Self(E::cast_from(i))
    }
}

impl<E: Enum, const SLOT_TYPE_IDX: usize> az::Cast<usize> for EnumSlots<E, SLOT_TYPE_IDX> {
    fn cast(self) -> usize {
        self.0.cast()
    }
}

impl<E: Enum, const SLOT_TYPE_IDX: usize> Enum for EnumSlots<E, SLOT_TYPE_IDX> {
    type Count = E::Count;

    fn name(&self) -> Cow<str> {
        self.0.name()
    }
}

#[duplicate_item(
idx         ty;
[AUDIO]     [Audio];
[CONTROL]   [Control];
[NOTE]      [Note];
)]
impl<E: Enum> Slots for EnumSlots<E, idx> {
    fn slot_type(&self) -> SlotType {
        SlotType::ty
    }
}

pub const AUDIO: usize = 0;
pub const CONTROL: usize = 1;
pub const NOTE: usize = 2;

pub type Mono = EnumSlots<crate::r#enum::Mono, AUDIO>;
pub type Stereo = EnumSlots<crate::r#enum::Stereo, AUDIO>;
