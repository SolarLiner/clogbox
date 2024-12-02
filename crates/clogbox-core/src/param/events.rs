use crate::math::interpolation::{InterpolateSingle, Linear};
use crate::r#enum::{count, Enum};
use duplicate::duplicate_item;
use numeric_array::generic_array::{ConstArrayLength, IntoArrayLength};
use numeric_array::NumericArray;
use std::ops;
use std::ops::RangeBounds;
use std::rc::Rc;
use std::sync::Arc;
use typenum::{Const, IsLessOrEqual, True, Unsigned};

pub struct EventInterval {
    sample_pos: (Option<usize>, Option<usize>),
    values: (f32, f32),
}

impl EventInterval {
    pub fn contains_pos(&self, pos: usize) -> bool {
        match self.sample_pos {
            (None, None) => true,
            (None, Some(end)) => pos <= end,
            (Some(start), None) => pos >= start,
            (Some(start), Some(end)) => pos >= start && pos <= end,
        }
    }

    pub fn changes(&self, min_diff: f32) -> bool {
        (self.values.1 - self.values.0).abs() > min_diff
    }

    pub fn interpolate_value<I: InterpolateSingle<f32>>(&self, interpolator: I, pos: usize) -> f32
    where
        <I::Count as IntoArrayLength>::ArrayLength:
            IsLessOrEqual<ConstArrayLength<2>, Output = True>,
    {
        match self.sample_pos {
            (None, None) | (None, Some(_)) => self.values.1,
            (Some(_), None) => self.values.0,
            (Some(start), Some(end)) => {
                let range = end - start;
                let offset = pos - start;
                let fract = offset as f32 / range as f32;
                let values = match <I::Count as IntoArrayLength>::ArrayLength::USIZE {
                    0 | 1 => NumericArray::splat(self.values.0),
                    2 => NumericArray::from_slice(&[self.values.0, self.values.1]).clone(),
                    _ => unreachable!(),
                };
                interpolator.interpolate_single(&values, fract)
            }
        }
    }

    pub fn as_discrete(&self, max: usize, at: usize) -> Option<usize> {
        let value = match self.sample_pos {
            (Some(start), Some(end)) => {
                if at < start {
                    return None;
                } else if at < end {
                    self.values.0
                } else {
                    self.values.1
                }
            }
            (None, None) => self.values.1,
            (None, Some(end)) => {
                if at <= end {
                    self.values.0
                } else {
                    self.values.1
                }
            }
            (Some(start), None) => {
                if at < start {
                    return None;
                } else {
                    self.values.0
                }
            }
        };
        let findex = value * max as f32;
        Some(findex.floor() as usize)
    }

    pub fn as_enum<E: Enum>(&self, at: usize) -> Option<E> {
        self.as_discrete(count::<E>(), at).map(E::cast_from)
    }
}

pub trait ParamEvents {
    fn num_events(&self) -> usize;

    fn event(&self, i: usize) -> Option<f32>;

    fn event_pos(&self, i: usize) -> Option<usize>;

    fn events_around(&self, at: usize) -> EventInterval;

    fn interpolate(&self, pos: usize) -> f32 {
        self.events_around(pos).interpolate_value(Linear, pos)
    }

    fn changes_at(&self, pos: usize, min_diff: f32) -> bool {
        self.events_around(pos).changes(min_diff)
    }

    fn get_discrete(&self, max: usize, at: usize) -> usize {
        self.events_around(at).as_discrete(max, at).unwrap()
    }

    fn write_to_buffer(&self, start_pos: usize, buffer: &mut [f32]) {
        for i in 0..buffer.len() {
            buffer[i] = self.interpolate(start_pos + i);
        }
    }
}

pub trait ParamEventsExt: ParamEvents {
    fn get_enum<E: Enum>(&self, at: usize) -> E {
        E::cast_from(self.get_discrete(count::<E>() - 1, at))
    }
}

impl<Ev: ParamEvents> ParamEventsExt for Ev {}

impl<Ev: ?Sized + ParamEvents> ParamEvents for &Ev {
    fn num_events(&self) -> usize {
        Ev::num_events(self)
    }

    fn event(&self, i: usize) -> Option<f32> {
        Ev::event(self, i)
    }

    fn event_pos(&self, i: usize) -> Option<usize> {
        Ev::event_pos(self, i)
    }

    fn events_around(&self, at: usize) -> EventInterval {
        Ev::events_around(self, at)
    }

    fn interpolate(&self, pos: usize) -> f32 {
        Ev::interpolate(self, pos)
    }

    fn changes_at(&self, pos: usize, min_diff: f32) -> bool {
        Ev::changes_at(self, pos, min_diff)
    }

    fn get_discrete(&self, max: usize, at: usize) -> usize {
        Ev::get_discrete(self, max, at)
    }

    fn write_to_buffer(&self, start_pos: usize, buffer: &mut [f32]) {
        Ev::write_to_buffer(self, start_pos, buffer)
    }
}

#[duplicate_item(
container;
[Box];
[Rc];
[Arc];
)]
impl<Ev: ?Sized + ParamEvents> ParamEvents for container<Ev> {
    fn num_events(&self) -> usize {
        Ev::num_events(self)
    }

    fn event(&self, i: usize) -> Option<f32> {
        Ev::event(self, i)
    }

    fn event_pos(&self, i: usize) -> Option<usize> {
        Ev::event_pos(self, i)
    }

    fn events_around(&self, at: usize) -> EventInterval {
        Ev::events_around(self, at)
    }

    fn interpolate(&self, pos: usize) -> f32 {
        Ev::interpolate(self, pos)
    }

    fn changes_at(&self, pos: usize, min_diff: f32) -> bool {
        Ev::changes_at(self, pos, min_diff)
    }

    fn get_discrete(&self, max: usize, at: usize) -> usize {
        Ev::get_discrete(self, max, at)
    }

    fn write_to_buffer(&self, start_pos: usize, buffer: &mut [f32]) {
        Ev::write_to_buffer(self, start_pos, buffer)
    }
}

impl ParamEvents for f32 {
    fn num_events(&self) -> usize {
        1
    }

    fn event(&self, i: usize) -> Option<f32> {
        (i == 0).then_some(*self)
    }

    fn event_pos(&self, i: usize) -> Option<usize> {
        (i == 0).then_some(0)
    }

    fn events_around(&self, at: usize) -> EventInterval {
        EventInterval {
            values: (*self, *self),
            sample_pos: (None, None),
        }
    }

    fn interpolate(&self, pos: usize) -> f32 {
        *self
    }

    fn changes_at(&self, pos: usize, min_diff: f32) -> bool {
        false
    }

    fn get_discrete(&self, max: usize, at: usize) -> usize {
        let findex = *self * max as f32;
        findex.floor() as usize
    }

    fn write_to_buffer(&self, start_pos: usize, buffer: &mut [f32]) {
        buffer.fill(*self);
    }
}

#[duplicate_item(
ty;
[[f32]];
[Vec<f32>];
)]
impl ParamEvents for ty {
    fn num_events(&self) -> usize {
        self.len()
    }

    fn event(&self, i: usize) -> Option<f32> {
        self.get(i).copied()
    }

    fn event_pos(&self, i: usize) -> Option<usize> {
        (i < self.len()).then_some(i)
    }

    fn events_around(&self, at: usize) -> EventInterval {
        let start = at.checked_sub(1);
        let end = Some(at).filter(|&a| a < self.len());
        EventInterval {
            values: (
                self[start.unwrap_or(0)],
                self[end.unwrap_or(self.len() - 1)],
            ),
            sample_pos: (start, end),
        }
    }

    fn interpolate(&self, pos: usize) -> f32 {
        self[pos.min(self.len() - 1)]
    }

    fn get_discrete(&self, max: usize, at: usize) -> usize {
        let findex = self[at.min(self.len() - 1)];
        let findex = findex * max as f32;
        findex.floor() as usize
    }

    fn write_to_buffer(&self, start_pos: usize, buffer: &mut [f32]) {
        buffer.copy_from_slice(&self[start_pos..start_pos + buffer.len()]);
    }
}

#[duplicate_item(
    ty;
    [[(usize, f32)]];
    [Vec<(usize, f32)>];
)]
impl ParamEvents for ty {
    fn num_events(&self) -> usize {
        self.len()
    }

    fn event(&self, i: usize) -> Option<f32> {
        self.get(i).map(|&(_, v)| v)
    }

    fn event_pos(&self, i: usize) -> Option<usize> {
        self.get(i).map(|&(pos, _)| pos)
    }

    fn events_around(&self, at: usize) -> EventInterval {
        let i = self
            .binary_search_by_key(&at, |&(pos, _)| pos)
            .unwrap_or_else(|i| i);
        if i == 0 {
            let (pos, value) = self[i];
            EventInterval {
                values: (value, value),
                sample_pos: (None, Some(pos)),
            }
        } else {
            let (start_pos, start_value) = self[i - 1];
            let (end_pos, end_value) = self[i];
            EventInterval {
                values: (start_value, end_value),
                sample_pos: (Some(start_pos), Some(end_pos)),
            }
        }
    }
}

pub trait ParamEventsMut: ParamEvents {
    fn set_event(&mut self, i: usize, value: f32);
}

#[duplicate_item(
ty;
[[(usize, f32)]];
[Box<[(usize, f32)]>];
)]
impl ParamEventsMut for ty {
    fn set_event(&mut self, i: usize, value: f32) {
        match self.binary_search_by_key(&i, |&(pos, _)| pos) {
            Ok(index) => self[index] = (i, value), // Overwrite if event already exists.
            Err(index) => {
                if index < self.len() {
                    // Rotate right by one to shift all values rightward once, and overwrite the
                    // last event.
                    self[index..].rotate_right(1);
                    self[index] = (i, value);
                }
            }
        }
    }
}

impl ParamEventsMut for Vec<(usize, f32)> {
    fn set_event(&mut self, i: usize, value: f32) {
        match self.binary_search_by_key(&i, |&(pos, _)| pos) {
            Ok(i) => self[i] = (i, value),
            Err(i) => self.insert(i, (i, value)),
        }
    }
}
pub type ParamSlice = [(usize, f32)];

#[cfg(test)]
mod tests {
    use super::*;

    fn requires_param_events<Ev: ?Sized + ParamEvents>() {}

    #[test]
    fn check_types_impl() {
        requires_param_events::<Box<ParamSlice>>();
        requires_param_events::<&ParamSlice>();
        requires_param_events::<Vec<f32>>();
    }
}
