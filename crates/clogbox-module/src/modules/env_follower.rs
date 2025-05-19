//! Peak envelope follower module
use crate::context::StreamContext;
use crate::sample::{SampleModule, SampleProcessResult};
use crate::{PrepareResult, Samplerate};
use az::CastFrom;
use clogbox_enum::enum_map::{EnumMap, EnumMapArray, EnumMapRef};
use clogbox_enum::generic_array::GenericArray;
use clogbox_enum::{Enum, Mono};
use clogbox_math::recip::Recip;
use num_traits::{Float, Num, Zero};
use numeric_literals::replace_float_literals;

/// Parameters of the envelope follower
#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Enum)]
pub enum Params {
    /// Attack time (s) (rising edge) of the envelope follower
    Attack,
    /// Release time (s) (falling edge) of the envelope follower
    Release,
}

/// Computes the envelope of an input signal.
#[derive(Debug, Clone)]
pub struct EnvFollower<T: 'static + Send, Audio: Enum = Mono> {
    sample_rate: Option<Recip<T>>,
    attack: T,
    attack_tau: T,
    release: T,
    release_tau: T,
    last: EnumMapArray<Audio, T>,
}

impl<T: 'static + Send + Copy + Zero, Audio: Enum> EnvFollower<T, Audio> {
    /// Create a new envelope follower module.
    ///
    /// # Arguments
    ///
    /// * `attack`: Attack time (s)
    /// * `release`: Release time (s)
    ///
    /// returns: EnvFollower<T, Audio>
    ///
    /// # Examples
    ///
    /// ```
    ///
    /// ```
    pub fn new(attack: T, release: T) -> Self {
        Self {
            sample_rate: None,
            attack,
            attack_tau: attack,
            release,
            release_tau: release,
            last: EnumMapArray::new(|_| T::zero()),
        }
    }
}

fn tau<T: Copy + Num + CastFrom<f64>>(samplerate: Recip<T>, rt60: T) -> T {
    let t60 = T::cast_from(1e4.ln());
    samplerate.recip() * (t60 / rt60)
}

impl<T: 'static + Send + CastFrom<f64> + Copy + Num, Audio: Enum> EnvFollower<T, Audio> {
    /// Changes the attack time of this envelope follower.
    ///
    /// # Arguments
    ///
    /// * `release`: Attack time (s)
    #[replace_float_literals(T::cast_from(literal))]
    pub fn set_attack(&mut self, attack: T) {
        self.attack = attack;
        let Some(sample_rate) = self.sample_rate else {
            return;
        };
        self.attack_tau = tau(sample_rate, attack);
    }

    /// Changes the release time of this envelope follower.
    ///
    /// # Arguments
    ///
    /// * `release`: Release time (s)
    #[replace_float_literals(T::cast_from(literal))]
    pub fn set_release(&mut self, release: T) {
        self.release = release;
        let Some(sample_rate) = self.sample_rate else {
            return;
        };
        self.release_tau = tau(sample_rate, release);
    }

    /// Sets the sample rate of this module.
    ///
    /// # Arguments
    ///
    /// * `sample_rate`: New sample rate (Hz)
    #[replace_float_literals(T::cast_from(literal))]
    pub fn set_sample_rate(&mut self, sample_rate: Samplerate)
    where
        T: Float,
    {
        let sample_rate = Recip::new(T::cast_from(sample_rate.value()));
        self.sample_rate = Some(sample_rate);
        self.attack_tau = tau(sample_rate, self.attack);
        self.release_tau = tau(sample_rate, self.release);
    }

    /// Directly process a single frame of audio through this module.
    ///
    /// # Arguments
    ///
    /// * `inputs`: Input frame, where each sample corresponds to the channel of `Audio`.
    pub fn process_follower(
        &mut self,
        inputs: EnumMapArray<Audio, T>,
    ) -> EnumMap<Audio, GenericArray<T, <Audio as Enum>::Count>>
    where
        T: Float,
    {
        let output = EnumMapArray::new(|e| {
            let x = inputs[e].abs();
            let last = self.last[e];
            if x < last {
                last + (x - last) * self.release_tau
            } else {
                last + (x - last) * self.attack_tau
            }
        });
        self.last = output.clone();
        output
    }
}

impl<T: 'static + Send + Default + Float + CastFrom<f32> + CastFrom<f64>, Audio: Enum> SampleModule
    for EnvFollower<T, Audio>
{
    type Sample = T;
    type AudioIn = Audio;
    type AudioOut = Audio;
    type Params = Params;

    fn prepare(&mut self, sample_rate: Samplerate) -> PrepareResult {
        self.set_sample_rate(sample_rate);
        PrepareResult { latency: 0.0 }
    }

    fn process(
        &mut self,
        _stream_context: &StreamContext,
        inputs: EnumMapArray<Self::AudioIn, Self::Sample>,
        params: EnumMapRef<Self::Params, f32>,
    ) -> SampleProcessResult<Self::AudioOut, Self::Sample> {
        self.set_attack(T::cast_from(params[Params::Attack]));
        self.set_release(T::cast_from(params[Params::Release]));

        // Process the input based on the follow mode
        let output = self.process_follower(inputs);

        SampleProcessResult { output, tail: None }
    }
}
