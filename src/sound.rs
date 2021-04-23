use crate::{convert::LossyFrom, impl_lossy_from, HardwareParams};

use std::{f32::consts::PI, marker::PhantomData, mem, time::Duration};

use alsa::{
    self,
    pcm::{Access, Format, HwParams, State},
    Direction, ValueOr, PCM,
};

pub type Ticks = u32;

const MAX_PHASE: f32 = 2.0 * PI;

impl_lossy_from!(f32; i16 u16 i32 u32 i64 u64 f32 f64);

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
    fn apply(&self, val: f32, tick: Ticks) -> f32;
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
    fn apply(&self, val: f32, tick: Ticks) -> f32 {
        if tick > self.duration {
            val
        } else {
            tick as f32 / self.duration as f32 * val
        }
    }
}

pub struct LinearFadeOut {
    start: Ticks,
    end: Ticks,
}

impl LinearFadeOut {
    pub fn new(duration: Ticks, end: Ticks) -> LinearFadeOut {
        LinearFadeOut {
            start: end - duration,
            end,
        }
    }
}

impl Filter for LinearFadeOut {
    fn apply(&self, val: f32, tick: Ticks) -> f32 {
        if tick < self.start || tick > self.end {
            val
        } else {
            (self.end - tick) as f32 / (self.end - self.start) as f32 * val
        }
    }
}

pub trait Sound: Send {
    type Item;

    fn tick(&mut self) -> Self::Item;
    fn complete(&self) -> bool;
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

impl<T> SountTest<T> {
    pub fn new(
        freq: f32,
        amplitude_scale: f32,
        duration: Duration,
        hwp: &HardwareParams,
    ) -> SountTest<T> {
        let d = duration_to_ticks(duration, hwp.rate);

        let amplitude =
            amplitude_scale.abs().min(1.0) * max_amplitude::<T>() as f32;

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
    T: LossyFrom<f32> + Send,
{
    type Item = T;

    fn tick(&mut self) -> Self::Item {
        let mut res = self.phase.sin() * self.amplitude;
        self.phase += self.step;
        if self.phase >= MAX_PHASE {
            self.phase -= MAX_PHASE;
        }
        if let Some(filters) = &self.filters {
            for filter in filters {
                res = filter.apply(res, self.tick_count);
            }
        }
        self.tick_count += 1;
        LossyFrom::lossy_from(res)
    }

    fn complete(&self) -> bool {
        self.tick_count > self.duration
    }
}

pub fn sound_test(device: &str) -> alsa::Result<()> {
    // Open default playback device
    let pcm = PCM::new(device, Direction::Playback, false)?;

    // Set hardware parameters: 44100 Hz / Mono / 16 bit
    let hwp = HwParams::any(&pcm)?;
    hwp.set_channels(1)?;
    hwp.set_rate(44100, ValueOr::Nearest)?;
    hwp.set_format(Format::s16())?;
    hwp.set_access(Access::RWInterleaved)?;
    pcm.hw_params(&hwp)?;
    let io = pcm.io_i16()?;

    // Make sure we don't start the stream too early
    let hwp = pcm.hw_params_current()?;
    let swp = pcm.sw_params_current()?;
    swp.set_start_threshold(hwp.get_buffer_size()?)?;
    pcm.sw_params(&swp)?;

    // Make a sine wave
    let mut buf = vec![0i16; 1024];
    for (i, a) in buf.iter_mut().enumerate() {
        *a = ((i as f32 * 2.0 * PI / 128.0).sin() * 8192.0) as i16
    }

    // Play it back for 2 seconds.
    for _ in 0..20 * 44100 / 1024 {
        assert_eq!(io.writei(&buf[..])?, 1024);
    }

    // In case the buffer was larger than 2 seconds, start the stream manually.
    if pcm.state() != State::Running {
        pcm.start()?;
    };
    // Wait for the stream to finish playback.
    pcm.drain()
}
