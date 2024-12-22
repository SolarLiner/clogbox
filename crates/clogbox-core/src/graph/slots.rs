use std::borrow::Cow;
use duplicate::duplicate_item;
use crate::graph::SlotType;
use clogbox_enum::{Enum, Mono, Stereo};

/// Trait for types which describe input/output slots in a module.
pub trait Slots: Enum {
    fn slot_type(&self) -> SlotType;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EnumSlots<E: Enum, const SLOT_TYPE_IDX: usize>(pub E);

impl<E: Enum, const SLOT_TYPE_IDX: usize> Enum for EnumSlots<E, SLOT_TYPE_IDX> {
    type Count = E::Count;
    
    fn from_usize(value: usize) -> Self {
        Self(E::from_usize(value))
    }
    
    fn to_usize(self) -> usize {
        self.0.to_usize()
    }

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

impl Slots for Mono {
    fn slot_type(&self) -> SlotType {
        SlotType::Audio
    }
}

impl Slots for Stereo {
    fn slot_type(&self) -> SlotType {
        SlotType::Audio
    }
}
