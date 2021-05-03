pub mod filter;

use crate::hwp::HardwareParams;
use alsa::pcm::IoFormat;
use core::f32;
use filter::{Filter, FilterCollection};
use lazy_static::lazy_static;
use std::{f32::consts::PI, mem, slice::Iter, time::Duration, usize};

pub type Ticks = u32;

const MAX_PHASE: f32 = 2.0 * PI;
const MAX_CONCURRENT: u32 = 4;
const INDEX_PRECISION: usize = 1000;
const PERIOD_SAMPLE_SIZE: usize = 8000;

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

pub struct SoundConfig {
    freq: f32,
    phase: f32,
    amplitude_scale: f32,
}

impl SoundConfig {
    pub fn new(freq: f32, phase: f32, amplitude_scale: f32) -> Self {
        SoundConfig {
            freq,
            phase,
            amplitude_scale,
        }
    }
}

pub struct SoundConfigCollection {
    configs: Option<Vec<SoundConfig>>,
}

impl SoundConfigCollection {
    pub fn new() -> Self {
        SoundConfigCollection { configs: None }
    }

    pub fn with_configs(configs: &[(f32, f32, f32)]) -> Self {
        let configs: Vec<SoundConfig> = configs
            .iter()
            .map(|c| SoundConfig::new(c.0, c.1, c.2))
            .collect();

        Self {
            configs: Some(configs),
        }
    }

    pub fn add_config(&mut self, freq: f32, phase: f32, amplitude_scale: f32) {
        let config = SoundConfig::new(freq, phase, amplitude_scale);
        match self.configs {
            Some(ref mut configs) => configs.push(config),
            None => self.configs = Some(vec![config]),
        }
    }
}

impl<'a> IntoIterator for &'a SoundConfigCollection {
    type Item = &'a SoundConfig;

    type IntoIter = SoundConfigIterator<'a>;

    fn into_iter(self) -> Self::IntoIter {
        SoundConfigIterator {
            iterator: match self.configs {
                Some(ref configs) => Some(configs.iter()),
                None => None,
            },
        }
    }
}

pub struct SoundConfigIterator<'a> {
    iterator: Option<Iter<'a, SoundConfig>>,
}

impl<'a> Iterator for SoundConfigIterator<'a> {
    type Item = &'a SoundConfig;

    fn next(&mut self) -> Option<Self::Item> {
        match self.iterator {
            Some(ref mut iter) => iter.next(),
            None => None,
        }
    }
}

pub struct Sinusoid {
    phase: Vec<f32>,
    step: Vec<f32>,
    amplitude: Vec<f32>,
    filters: FilterCollection,
    ticker: Ticker,
}

impl Sinusoid {
    pub fn new<T>(
        config: &SoundConfigCollection,
        duration: Duration,
        hwp: &HardwareParams<T>,
    ) -> Sinusoid
    where
        T: IoFormat,
    {
        let d = duration_to_ticks(duration, hwp.rate());

        Sinusoid {
            phase: config.into_iter().map(|c| verify_scale(c.phase)).collect(),
            step: config
                .into_iter()
                .map(|c| calc_step(c.freq, hwp.rate()))
                .collect(),
            amplitude: config
                .into_iter()
                .map(|c| {
                    verify_scale(c.amplitude_scale)
                        * max_amplitude::<T>() as f32
                })
                .collect(),
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
        let ch = channel as usize;
        let res = self.phase[ch].sin() * self.amplitude[ch];
        self.phase[ch] += self.step[ch];
        self.filters.apply(res, self.ticker.tick_count, channel)
    }

    fn tick(&mut self) {
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
    amplitude: Vec<f32>,
    idx: Vec<usize>,
    idx_step: Vec<usize>,
    idx_limit: usize,
    filters: FilterCollection,
    ticker: Ticker,
}

impl<'a> CachedPeriod<'a> {
    pub fn new<T>(
        data: &'a [f32],
        config: &SoundConfigCollection,
        duration: Duration,
        params: &HardwareParams<T>,
    ) -> Self
    where
        T: IoFormat,
    {
        let d = duration_to_ticks(duration, params.rate());

        let idx_step: Vec<usize> = config
            .into_iter()
            .map(|c| {
                let ticks_per_cycle = params.rate() as f32 / c.freq;

                ((data.len() as f32 / ticks_per_cycle) * INDEX_PRECISION as f32)
                    as usize
            })
            .collect();

        let idx: Vec<usize> = config
            .into_iter()
            .map(|c| (c.phase / MAX_PHASE * data.len() as f32) as usize)
            .collect();

        let amplitude: Vec<f32> = config
            .into_iter()
            .map(|c| {
                verify_scale(c.amplitude_scale) * max_amplitude::<T>() as f32
            })
            .collect();

        CachedPeriod {
            data,
            amplitude,
            idx,
            idx_step,
            idx_limit: data.len() * INDEX_PRECISION,
            filters: FilterCollection::new(),
            ticker: Ticker::new(d),
        }
    }

    pub fn add_filter(&mut self, filter: Box<dyn Filter>) {
        self.filters.add_filter(filter);
    }
}

impl Sound for CachedPeriod<'_> {
    fn generate(&mut self, channel: u32) -> f32 {
        let ch = channel as usize;
        let idx = self.idx[ch] / INDEX_PRECISION;
        self.idx[ch] += self.idx_step[ch];
        if self.idx[ch] >= self.idx_limit {
            self.idx[ch] -= self.idx_limit;
        }
        let val = self.data[idx] * self.amplitude[ch];
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
