use crate::context::StreamContext;
use crate::modules::extract::CircularBuffer;
use crate::sample::{SampleModule, SampleProcessResult};
use crate::{PrepareResult, Samplerate};
use az::CastFrom;
use clogbox_enum::enum_map::{EnumMapArray, EnumMapRef};
use clogbox_enum::{Enum, Mono};
use num_traits::{Float, Num, Zero};
use numeric_literals::replace_float_literals;

#[derive(Debug, Copy, Clone, PartialOrd, PartialEq)]
pub enum FollowMode {
    Peak,
    Rms(f64),
}

#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Enum)]
pub enum Params {
    Attack,
    Release,
    FollowMode,
    RmsTime,
}

pub struct EnvFollower<T: 'static + Send, Audio: Enum = Mono> {
    mode: FollowMode,
    sample_rate: Option<Samplerate>,
    attack: T,
    attack_tau: T,
    release: T,
    release_tau: T,
    last: EnumMapArray<Audio, T>,
    buf: Option<CircularBuffer<T, Audio>>,
}

impl<T: 'static + Send + Copy + Zero, Audio: Enum> EnvFollower<T, Audio> {
    pub fn new(attack: T, release: T, mode: FollowMode) -> Self {
        Self {
            mode,
            sample_rate: None,
            attack,
            attack_tau: attack,
            release,
            release_tau: release,
            last: EnumMapArray::new(|_| T::zero()),
            buf: None,
        }
    }
}

impl<T: 'static + Send + CastFrom<f64> + Copy + Num, Audio: Enum> EnvFollower<T, Audio> {
    #[replace_float_literals(T::cast_from(literal))]
    pub fn set_attack(&mut self, attack: T) {
        self.attack = attack;
        let Some(sample_rate) = self.sample_rate else { return; };
        self.attack_tau = attack * T::cast_from(sample_rate.recip().value());
    }

    #[replace_float_literals(T::cast_from(literal))]
    pub fn set_release(&mut self, release: T) {
        self.release = release;
        let Some(sample_rate) = self.sample_rate else { return; };
        self.release_tau = release * T::cast_from(sample_rate.recip().value());
    }

    #[replace_float_literals(T::cast_from(literal))]
    pub fn set_sample_rate(&mut self, sample_rate: Samplerate) {
        self.sample_rate = Some(sample_rate);
        self.attack_tau = self.attack * T::cast_from(sample_rate.recip().value());
        self.release_tau = self.release * T::cast_from(sample_rate.recip().value());
    }
    
    pub fn set_follow_mode(&mut self, mode: FollowMode) {
        if mode == self.mode {
            return;
        }
        self.mode = mode;
        self.last = EnumMapArray::new(|_| T::zero());
    }
}

impl<T: 'static + Send + Default + Float + CastFrom<f32> + CastFrom<f64>, Audio: Enum> SampleModule for EnvFollower<T, 
    Audio> {
    type Sample = T;
    type AudioIn = Audio;
    type AudioOut = Audio;
    type Params = Params;

    fn prepare(&mut self, sample_rate: Samplerate) -> PrepareResult {
        self.buf = match self.mode {
            FollowMode::Peak => None,
            FollowMode::Rms(rms) => Some(CircularBuffer::<T, Audio>::new(
                (rms * sample_rate.value()).round() as usize
            )),
        };
        self.set_sample_rate(sample_rate);
        PrepareResult { latency: 0.0 }
    }

    fn process(
        &mut self,
        _: &StreamContext,
        inputs: EnumMapArray<Self::AudioIn, Self::Sample>,
        params: EnumMapRef<Self::Params, f32>,
    ) -> SampleProcessResult<Self::AudioOut, Self::Sample> {
        self.set_attack(T::cast_from(params[Params::Attack]));
        self.set_release(T::cast_from(params[Params::Release]));
        self.set_follow_mode({
            let value = params[Params::FollowMode];
            if value > 0.5 {
                FollowMode::Peak
            } else {
                FollowMode::Rms(params[Params::RmsTime] as _)
            }
        });
        
        let value = if let Some(cb) = &self.buf {
            cb.send_frame(inputs.clone());
            cb.iter_frames().fold(EnumMapArray::new(|_| T::zero()), |a, b| {
                EnumMapArray::new(|e| a[e] + b[e].powi(2))
            })
        } else {
            inputs.clone().map(|_, x| x.abs())
        };
        
        let output = EnumMapArray::new(|e| {
            let x = value[e];
            let last = self.last[e];
            if x < last {
                last + (x - last) * self.release_tau
            } else {
                last + (x - last) * self.attack_tau
            }
        });
        self.last = output.clone();
        
        SampleProcessResult {
            output,
            tail: None,
        }
    }
}
