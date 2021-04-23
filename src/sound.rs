use crate::{convert::LossyFrom, impl_lossy_from, HardwareParams};

use std::{f32::consts::PI, marker::PhantomData, mem, time::Duration};

pub type Ticks = u32;

const MAX_PHASE: f32 = 2.0 * PI;

impl_lossy_from!(f32; i16 u16 i32 u32 i64 u64 f32 f64);

fn verify_scale(scale: f32) -> f32 {
    scale.abs().min(1.0)
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
}

impl LinearFadeIn {
    pub fn new(duration: Ticks) -> LinearFadeIn {
        LinearFadeIn { duration }
    }
}

impl Filter for LinearFadeIn {
    fn apply(&self, val: f32, tick: Ticks, _: u32) -> f32 {
        if tick > self.duration {
            val
        } else {
            tick as f32 / self.duration as f32 * val
        }
    }
}

pub struct LinearFadeOut {
    duration: Ticks,
    start: Ticks,
    end: Ticks,
}

impl LinearFadeOut {
    pub fn new(duration: Ticks, end: Ticks) -> LinearFadeOut {
        LinearFadeOut {
            duration,
            end,
            start: end - duration,
        }
    }
}

impl Filter for LinearFadeOut {
    fn apply(&self, val: f32, tick: Ticks, _: u32) -> f32 {
        if tick < self.start || tick > self.end {
            val
        } else {
            (self.end - tick) as f32 / self.duration as f32 * val
        }
    }
}

pub enum FadeDirection {
    LeftRight,
    RightLeft,
}

pub struct LeftRightFade {
    min_scale: f32,
    direction: FadeDirection,
    duration: Ticks,
    range: f32,
}

impl LeftRightFade {
    pub fn new(
        min_scale: f32,
        max_scale: f32,
        direction: FadeDirection,
        duration: Ticks,
    ) -> LeftRightFade {
        LeftRightFade {
            min_scale: verify_scale(min_scale),
            direction,
            duration,
            range: verify_scale(max_scale - min_scale),
        }
    }
}

impl Filter for LeftRightFade {
    fn apply(&self, val: f32, tick: Ticks, channel: u32) -> f32 {
        let percent_complete = tick as f32 / self.duration as f32;
        let (complete, remaining) = match self.direction {
            FadeDirection::RightLeft => {
                (percent_complete, 1.0 - percent_complete)
            }
            FadeDirection::LeftRight => {
                (1.0 - percent_complete, percent_complete)
            }
        };

        match channel {
            0 => val * (self.min_scale as f32 + self.range * complete),
            1 => val * (self.min_scale as f32 + self.range * remaining),
            _ => val,
        }
    }
}

pub trait Sound: Send {
    type Item;

    fn generate(&mut self, channel: u32) -> Self::Item;
    fn tick(&mut self);
    fn tick_count(&self) -> Ticks;
    fn duration(&self) -> Ticks;

    fn complete(&self) -> bool {
        self.tick_count() > self.duration()
    }
}

pub struct SountTest<T> {
    tick_count: Ticks,
    phase: f32,
    step: f32,
    amplitude: f32,
    duration: Ticks,
    filters: Option<Vec<Box<dyn Filter>>>,
    phantom: PhantomData<T>,
}

impl<T: Default> SountTest<T> {
    pub fn new(
        freq: f32,
        amplitude_scale: f32,
        duration: Duration,
        hwp: &HardwareParams,
    ) -> SountTest<T> {
        let d = duration_to_ticks(duration, hwp.rate);

        let amplitude =
            verify_scale(amplitude_scale) * max_amplitude::<T>() as f32;

        SountTest::<T> {
            duration: d,
            tick_count: 0,
            phase: 1.0,
            step: calc_step(freq, hwp.rate),
            amplitude,
            filters: None,
            phantom: PhantomData::default(),
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

impl<T> Sound for SountTest<T>
where
    T: LossyFrom<f32> + Send + Copy,
{
    type Item = T;

    fn generate(&mut self, channel: u32) -> Self::Item {
        let mut res = self.phase.sin() * self.amplitude;
        if let Some(filters) = &self.filters {
            res = filters
                .iter()
                .fold(res, |res, f| f.apply(res, self.tick_count, channel));
        }
        LossyFrom::lossy_from(res)
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
