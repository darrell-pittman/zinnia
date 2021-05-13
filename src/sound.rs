pub mod config;
pub mod filter;

use crate::hwp::HardwareParams;
use alsa::pcm::IoFormat;
use config::SoundConfigCollection;
use core::f32;
use filter::{Filter, FilterCollection};
use lazy_static::lazy_static;
use std::{
    f32::consts::PI, fs::File, io::Read, mem, path::Path, time::Duration, usize,
};

pub type Ticks = u32;

const MAX_PHASE: f32 = 2.0 * PI;
const MAX_CONCURRENT: u32 = 4;
const PERIOD_SAMPLE_SIZE: usize = 1000;

lazy_static! {
    pub static ref SINE_PERIOD: Vec<f32> =
        sine_period_n_channels(PERIOD_SAMPLE_SIZE, 1);
    pub static ref SINE_PERIOD_2_CH: Vec<f32> =
        sine_period_n_channels(PERIOD_SAMPLE_SIZE, 2);
    pub static ref C4_PIANO_2_CH_PERIOD: Vec<f32> = c4_2_channel_sound()
        .into_iter()
        .skip(10000)
        .take(64)
        .collect();
    pub static ref C4_PIANO_2_CH_SOUND: Vec<f32> = c4_2_channel_sound();
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

pub fn max_amplitude<T>() -> usize {
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
            phase: config.iter().map_phase(|phase| phase).collect(),
            step: config
                .iter()
                .map_freq(|freq| calc_step(freq, hwp.rate()))
                .collect(),
            amplitude: config.iter().map_amplitude(|amp| amp).collect(),
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

pub struct InputConfig<'a> {
    data: &'a [f32],
    channels: u32,
}

impl<'a> InputConfig<'a> {
    pub fn new(data: &'a [f32], channels: u32) -> Self {
        Self { data, channels }
    }
}

pub struct CachedPeriod<'a> {
    period_config: InputConfig<'a>,
    amplitude: Vec<f32>,
    idx: Vec<f32>,
    idx_step: Vec<f32>,
    idx_limit: f32,
    filters: FilterCollection,
    ticker: Ticker,
}

impl<'a> CachedPeriod<'a> {
    pub fn new<T>(
        period_config: InputConfig<'a>,
        sound_config: &SoundConfigCollection,
        duration: Duration,
        params: &HardwareParams<T>,
    ) -> Self
    where
        T: IoFormat,
    {
        let d = duration_to_ticks(duration, params.rate());
        let data_size =
            (period_config.data.len() / period_config.channels as usize) as f32;

        let idx_step: Vec<f32> = sound_config
            .iter()
            .map_freq(|freq| {
                let ticks_per_cycle = params.rate() as f32 / freq;
                data_size / ticks_per_cycle
            })
            .collect();

        let idx: Vec<f32> = sound_config
            .iter()
            .map_phase(|phase| phase / MAX_PHASE * data_size)
            .collect();

        let amplitude: Vec<f32> =
            sound_config.iter().map_amplitude(|amp| amp).collect();

        CachedPeriod {
            period_config,
            amplitude,
            idx,
            idx_step,
            idx_limit: data_size - std::f32::EPSILON,
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
        let in_ch = ch % self.period_config.channels as usize;
        let in_chs = self.period_config.channels as usize;

        let idx_f = self.idx[ch].floor();
        let idx = idx_f as usize * in_chs + in_ch;

        let lower = self.period_config.data[idx];

        let upper = self.period_config.data
            [(idx + in_chs) % self.period_config.data.len()];

        let val = (lower + ((upper - lower) * (self.idx[ch] - idx_f).abs()))
            * self.amplitude[ch];

        self.idx[ch] += self.idx_step[ch];

        if self.idx[ch] > self.idx_limit {
            self.idx[ch] -= self.idx_limit;
        }

        self.filters.apply(val, self.ticker.tick_count, channel)
    }

    fn tick(&mut self) {
        self.ticker.tick();
    }

    fn is_complete(&self) -> bool {
        self.ticker.is_complete()
    }
}

pub struct CachedSound<'a> {
    period_config: InputConfig<'a>,
    idx: usize,
}

impl<'a> CachedSound<'a> {
    pub fn new(period_config: InputConfig<'a>) -> Self {
        Self {
            period_config,
            idx: 0,
        }
    }
}

impl Sound for CachedSound<'_> {
    fn generate(&mut self, channel: u32) -> f32 {
        let in_channel = (channel % self.period_config.channels) as usize;
        if self.is_complete() {
            0.0
        } else {
            self.period_config.data
                [self.idx * self.period_config.channels as usize + in_channel]
        }
    }

    fn tick(&mut self) {
        self.idx += 1;
    }

    fn is_complete(&self) -> bool {
        self.idx * self.period_config.channels as usize
            >= self.period_config.data.len()
    }
}

fn sine_period_n_channels(num_samples: usize, channels: usize) -> Vec<f32> {
    let step = 2.0 * PI / num_samples as f32;
    let mut data_phase = 0.0f32;
    let mut data = Vec::with_capacity(num_samples * 2);
    for _ in 0..num_samples {
        let val = data_phase.sin();
        for _ in 0..channels {
            data.push(val);
        }
        data_phase += step;
    }
    data
}

fn c4_2_channel_sound() -> Vec<f32> {
    let filename = Path::new("data/C4.raw");

    let mut input = File::open(&filename).unwrap();

    let mut buf = Vec::new();
    let bytes_read = input.read_to_end(&mut buf).unwrap();
    let mut result = Vec::new();
    for i in { 0..bytes_read }.step_by(2) {
        let val = ((buf[i] as i16) << 8) + (buf[i + 1] as i16);
        result.push(val as f32);
    }
    result
}
