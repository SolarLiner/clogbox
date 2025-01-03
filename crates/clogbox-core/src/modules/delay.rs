use crate::graph::context::GraphContext;
use crate::graph::module::{Module, ModuleError, ProcessStatus};
use crate::math::interpolation::{InterpolateSingle, Linear};
use ::serde::{Deserializer, Serializer};
use clogbox_enum::Mono;
use generic_array::sequence::GenericSequence;
use num_traits::Float;
use numeric_array::NumericArray;
use std::collections::VecDeque;

#[cfg(feature = "serialize")]
mod serde {
    use crate::modules::delay::FixedAudioDelay;
    use num_traits::Zero;
    use serde::{Deserialize, Serialize};
    use std::collections::VecDeque;

    #[derive(Serialize, Deserialize)]
    pub(super) struct Serialized<T> {
        size: usize,
        fractional: T,
    }

    impl<'a, T: Copy> From<&'a super::FixedAudioDelay<T>> for Serialized<T> {
        fn from(value: &'a FixedAudioDelay<T>) -> Self {
            Self {
                size: value.delay.len(),
                fractional: value.pos_fract,
            }
        }
    }

    impl<T: Zero> From<Serialized<T>> for super::FixedAudioDelay<T> {
        fn from(value: Serialized<T>) -> Self {
            Self {
                delay: VecDeque::from_iter(std::iter::repeat_with(T::zero).take(value.size)),
                pos_fract: value.fractional,
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct FixedAudioDelay<T> {
    delay: VecDeque<T>,
    pos_fract: T,
}

impl<T: Float + az::Cast<usize>> Module for FixedAudioDelay<T> {
    type Sample = T;
    type Inputs = Mono;
    type Outputs = Mono;

    fn process(&mut self, graph_context: GraphContext<Self>) -> Result<ProcessStatus, ModuleError> {
        use clogbox_enum::Mono;
        let input = graph_context.get_audio_input(Mono)?;
        let mut output = graph_context.get_audio_output(Mono)?;

        for (i, o) in input.iter().zip(output.iter_mut()) {
            *o = Linear
                .interpolate_single(&NumericArray::generate(|i| self.delay[i]), self.pos_fract);
            self.delay.pop_front().unwrap();
            self.delay.push_back(*i);
        }
        Ok(ProcessStatus::Running)
    }
}

#[cfg(feature = "serialize")]
impl<T: Copy + ::serde::Serialize> ::serde::Serialize for FixedAudioDelay<T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serde::Serialized::<T>::from(self).serialize(serializer)
    }
}

#[cfg(feature = "serialize")]
impl<'de, T: num_traits::Zero + ::serde::Deserialize<'de>> ::serde::Deserialize<'de>
    for FixedAudioDelay<T>
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(serde::Serialized::<T>::deserialize(deserializer)?.into())
    }
}
