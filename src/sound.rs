pub mod filter;

use crate::hwp::HardwareParams;
use alsa::pcm::IoFormat;
use core::f32;
use filter::{Filter, FilterCollection};
use lazy_static::lazy_static;
use std::{f32::consts::PI, mem, time::Duration, usize};

pub type Ticks = u32;

const MAX_PHASE: f32 = 2.0 * PI;
const MAX_CONCURRENT: u32 = 4;
const FREQ_PRECISION: u32 = 100;
const PERIOD_SAMPLE_SIZE: usize = 2000;

lazy_static! {
    pub static ref SINE_PERIOD: Vec<f32> = sine_period(PERIOD_SAMPLE_SIZE);
}

pub fn mix_fixed(sounds: &mut Vec<Box<dyn Sound>>, channel: u32) -> f32 {
    mix_internal(sounds, channel, MAX_CONCURRENT)
}

pub fn mix(sounds: &mut Vec<Box<dyn Sound>>, channel: u32) -> f32 {
    let size = sounds.len() as u32;
    mix_internal(sounds, channel, size)
}

fn mix_internal(
    sounds: &mut Vec<Box<dyn Sound>>,
    channel: u32,
    num_sounds: u32,
) -> f32 {
    sounds.iter_mut().fold(0.0f32, |acc, s| {
        acc + s.generate(channel) / num_sounds as f32
    })
}

fn verify_scale(scale: f32) -> f32 {
    scale.abs().clamp(0.0, 1.0)
}

fn calc_step(freq: f32, rate: Ticks) -> f32 {
    MAX_PHASE * freq / rate as f32
}

pub fn duration_to_ticks(duration: Duration, rate: Ticks) -> Ticks {
    (duration.as_secs_f32() * rate as f32) as Ticks
}

fn max_amplitude<T>() -> usize {
    (1 << (mem::size_of::<T>() * 8 - 1)) - 1
}

struct Ticker {
    tick_count: Ticks,
    duration: Ticks,
}

impl Ticker {
    fn new(duration: Ticks) -> Ticker {
        Ticker {
            tick_count: 0,
            duration,
        }
    }

    fn is_complete(&self) -> bool {
        self.tick_count >= self.duration
    }

    fn tick(&mut self) {
        self.tick_count += 1;
    }
}

pub trait Sound: Send {
    fn generate(&mut self, channel: u32) -> f32;
    fn tick(&mut self);
    fn is_complete(&self) -> bool;
}

pub struct Sinusoid {
    phase: f32,
    step: f32,
    amplitude: f32,
    filters: FilterCollection,
    ticker: Ticker,
}

impl Sinusoid {
    pub fn new<T>(
        freq: f32,
        phase: f32,
        amplitude_scale: f32,
        duration: Duration,
        hwp: &HardwareParams<T>,
    ) -> Sinusoid
    where
        T: IoFormat,
    {
        let d = duration_to_ticks(duration, hwp.rate());

        let amplitude =
            verify_scale(amplitude_scale) * max_amplitude::<T>() as f32;

        Sinusoid {
            phase: verify_scale(phase),
            step: calc_step(freq, hwp.rate()),
            amplitude,
            filters: FilterCollection::new(),
            ticker: Ticker::new(d),
        }
    }

    pub fn add_filter(&mut self, filter: Box<dyn Filter>) {
        self.filters.add_filter(filter);
    }
}

impl Sound for Sinusoid {
    fn generate(&mut self, channel: u32) -> f32 {
        let res = self.phase.sin() * self.amplitude;
        self.filters.apply(res, self.ticker.tick_count, channel)
    }

    fn tick(&mut self) {
        self.phase += self.step;
        if self.phase >= MAX_PHASE {
            self.phase -= MAX_PHASE;
        }
        self.ticker.tick();
    }

    fn is_complete(&self) -> bool {
        self.ticker.is_complete()
    }
}

pub struct MultiSound {
    sounds: Vec<Box<dyn Sound>>,
}

impl MultiSound {
    pub fn new(sound: Box<dyn Sound>) -> MultiSound {
        MultiSound {
            sounds: vec![sound],
        }
    }

    pub fn with_sounds(sounds: &mut Vec<Box<dyn Sound>>) -> MultiSound {
        let mut result = MultiSound {
            sounds: Vec::with_capacity(sounds.len()),
        };
        result.add_sounds(sounds);
        result
    }

    pub fn add_sound(&mut self, sound: Box<dyn Sound>) {
        self.sounds.push(sound);
    }

    pub fn add_sounds(&mut self, sounds: &mut Vec<Box<dyn Sound>>) {
        self.sounds.append(sounds);
    }
}

impl Sound for MultiSound {
    fn generate(&mut self, channel: u32) -> f32 {
        mix(&mut self.sounds, channel)
    }

    fn tick(&mut self) {
        for sound in &mut self.sounds {
            sound.tick();
        }
    }

    fn is_complete(&self) -> bool {
        self.sounds.iter().all(|s| s.is_complete())
    }
}

pub struct CachedPeriod<'a> {
    data: &'a [f32],
    amplitude: f32,
    idx_scale: u32,
    phase_ticks: Ticks,
    filters: FilterCollection,
    ticker: Ticker,
}

impl<'a> CachedPeriod<'a> {
    pub fn new<'b, T>(
        data: &'a [f32],
        freq: f32,
        phase: f32,
        amplitude_scale: f32,
        duration: Duration,
        params: &'b HardwareParams<T>,
    ) -> Self
    where
        T: IoFormat,
    {
        let d = duration_to_ticks(duration, params.rate());
        let idx_scale = (freq * FREQ_PRECISION as f32) as u32
            * data.len() as u32
            / params.rate();

        let phase_ticks =
            (phase / MAX_PHASE * params.rate() as f32 / freq) as Ticks;
        let amplitude =
            verify_scale(amplitude_scale) * max_amplitude::<T>() as f32;

        CachedPeriod {
            data,
            amplitude,
            idx_scale,
            phase_ticks,
            filters: FilterCollection::new(),
            ticker: Ticker::new(d),
        }
    }

    pub fn add_filter(&mut self, filter: Box<dyn Filter>) {
        self.filters.add_filter(filter);
    }
}

impl<'a> Sound for CachedPeriod<'a> {
    fn generate(&mut self, channel: u32) -> f32 {
        let idx = (self.idx_scale
            * (self.ticker.tick_count + self.phase_ticks)
            / FREQ_PRECISION)
            % self.data.len() as u32;

        let val = self.data[idx as usize] * self.amplitude;
        self.filters.apply(val, self.ticker.tick_count, channel)
    }

    fn tick(&mut self) {
        self.ticker.tick();
    }

    fn is_complete(&self) -> bool {
        self.ticker.is_complete()
    }
}

fn sine_period(num_samples: usize) -> Vec<f32> {
    let step = 2.0 * PI / num_samples as f32;
    let mut data_phase = 0.0f32;
    let mut data = Vec::with_capacity(num_samples);
    for _ in 0..num_samples {
        data.push(data_phase.sin());
        data_phase += step;
    }
    data
}
