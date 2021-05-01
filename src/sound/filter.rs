use super::{verify_scale, Ticks};

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

pub struct FilterCollection {
    filters: Option<Vec<Box<dyn Filter>>>,
}

impl FilterCollection {
    pub fn new() -> Self {
        FilterCollection { filters: None }
    }

    pub fn add_filter(&mut self, filter: Box<dyn Filter>) {
        match &mut self.filters {
            Some(filters) => filters.push(filter),
            None => {
                let v = vec![filter];
                self.filters = Some(v);
            }
        }
    }

    pub fn apply(&self, value: f32, tick_count: Ticks, channel: u32) -> f32 {
        match &self.filters {
            Some(filters) => filters
                .iter()
                .fold(value, |v, f| f.apply(v, tick_count, channel)),
            None => value,
        }
    }
}
