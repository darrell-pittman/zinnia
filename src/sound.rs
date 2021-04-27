use alsa::pcm::IoFormat;

use crate::hwp::HardwareParams;

use std::{f32::consts::PI, mem, time::Duration};

pub type Ticks = u32;

const MAX_PHASE: f32 = 2.0 * PI;

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

pub trait Filter: Send {
    fn apply(&self, val: f32, tick: Ticks, channel: u32) -> f32;
}

pub struct LinearFadeIn {
    duration: Ticks,
    slope: f32,
}

impl LinearFadeIn {
    pub fn new(duration: Ticks) -> LinearFadeIn {
        LinearFadeIn {
            duration,
            slope: 1.0 / duration as f32,
        }
    }
}

impl Filter for LinearFadeIn {
    fn apply(&self, val: f32, tick: Ticks, _: u32) -> f32 {
        if tick > self.duration {
            val
        } else {
            tick as f32 * self.slope * val
        }
    }
}

pub struct LinearFadeOut {
    start: Ticks,
    end: Ticks,
    slope: f32,
}

impl LinearFadeOut {
    pub fn new(duration: Ticks, end: Ticks) -> LinearFadeOut {
        LinearFadeOut {
            end,
            start: end - duration,
            slope: -1.0 / duration as f32,
        }
    }
}

impl Filter for LinearFadeOut {
    fn apply(&self, val: f32, tick: Ticks, _: u32) -> f32 {
        if tick < self.start || tick > self.end {
            val
        } else {
            (1.0 + self.slope * (tick - self.start) as f32) * val
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum FadeDirection {
    LeftRight,
    RightLeft,
}

pub struct LeftRightFade {
    min_scale: f32,
    max_scale: f32,
    direction: FadeDirection,
    duration: Ticks,
    slope: f32,
}

impl LeftRightFade {
    pub fn new(
        min_scale: f32,
        max_scale: f32,
        direction: FadeDirection,
        duration: Ticks,
    ) -> LeftRightFade {
        let min_scale = verify_scale(min_scale);
        let max_scale = verify_scale(max_scale);

        LeftRightFade {
            min_scale,
            max_scale,
            direction,
            duration,
            slope: (max_scale - min_scale).abs() / duration as f32,
        }
    }
}

impl Filter for LeftRightFade {
    fn apply(&self, val: f32, tick: Ticks, channel: u32) -> f32 {
        let (y_intercept, slope) = match self.direction {
            FadeDirection::RightLeft => (self.min_scale, self.slope),
            FadeDirection::LeftRight => (self.max_scale, -self.slope),
        };

        let progress = match channel {
            0 => tick,
            1 => self.duration - tick,
            _ => tick,
        } as f32;

        (progress * slope + y_intercept) * val
    }
}

pub trait Sound: Send {
    fn generate(&mut self, channel: u32) -> f32;
    fn tick(&mut self);
    fn tick_count(&self) -> Ticks;
    fn duration(&self) -> Ticks;

    fn complete(&self) -> bool {
        self.tick_count() > self.duration()
    }
}

pub struct SountTest {
    tick_count: Ticks,
    phase: f32,
    step: f32,
    amplitude: f32,
    duration: Ticks,
    filters: Option<Vec<Box<dyn Filter>>>,
}

impl SountTest {
    pub fn new<T>(
        freq: f32,
        phase: f32,
        amplitude_scale: f32,
        duration: Duration,
        hwp: &HardwareParams<T>,
    ) -> SountTest
    where
        T: IoFormat,
    {
        let d = duration_to_ticks(duration, hwp.rate());

        let amplitude =
            verify_scale(amplitude_scale) * max_amplitude::<T>() as f32;

        SountTest {
            duration: d,
            tick_count: 0,
            phase: verify_scale(phase),
            step: calc_step(freq, hwp.rate()),
            amplitude,
            filters: None,
        }
    }

    pub fn add_filter(&mut self, filter: Box<dyn Filter>) {
        if let Some(filters) = &mut self.filters {
            filters.push(filter);
        } else {
            self.filters = Some(vec![filter]);
        }
    }
}

impl Sound for SountTest {
    fn generate(&mut self, channel: u32) -> f32 {
        let mut res = self.phase.sin() * self.amplitude;
        if let Some(filters) = &self.filters {
            res = filters
                .iter()
                .fold(res, |res, f| f.apply(res, self.tick_count, channel));
        }
        res
    }

    fn tick(&mut self) {
        self.phase += self.step;
        if self.phase >= MAX_PHASE {
            self.phase -= MAX_PHASE;
        }
        self.tick_count += 1;
    }

    fn tick_count(&self) -> Ticks {
        self.tick_count
    }

    fn duration(&self) -> Ticks {
        self.duration
    }
}
