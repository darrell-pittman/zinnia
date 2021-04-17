use std::marker::PhantomData;

use alsa::{self, pcm};

const MAX_PHASE: f32 = 2.0 * std::f32::consts::PI;

pub trait LossyFrom<T: Sized>: Sized {
    fn lossy_from(_: T) -> Self;
}

macro_rules! impl_lossy_from {
    ($from:ty;$($ty:ty)*) => {
        $(
            impl LossyFrom<$from> for $ty {
                #[inline]
                fn lossy_from(v: $from) -> $ty {
                    v as $ty
                }
            }
        )*
    }
}

impl_lossy_from!(f32; i16 u16 i32 u32 i64 u64 f32 f64);

pub trait Sound {
    type Item;

    fn generate(&mut self, hwp: &pcm::HwParams) -> Vec<Self::Item>;
}

pub struct SountTest<T> {
    phase: f32,
    phantom: PhantomData<T>,
}

impl<T: LossyFrom<f32>> SountTest<T> {
    pub fn new() -> SountTest<T> {
        SountTest::<T> {
            phase: 0.0,
            phantom: PhantomData::default(),
        }
    }
}

impl<T: LossyFrom<f32> + Clone> Sound for SountTest<T> {
    type Item = T;

    fn generate(&mut self, hwp: &pcm::HwParams) -> Vec<Self::Item> {
        let size = hwp.get_period_size().unwrap() as usize;
        let rate = hwp.get_rate().unwrap();
        let freq = 440.0;
        let step = MAX_PHASE * freq / rate as f32;
        let max_val = 8192.0;

        let mut buf: Vec<T> = vec![T::lossy_from(0.0); size];

        for a in buf.iter_mut() {
            let res = self.phase.sin() * max_val;
            *a = T::lossy_from(res);
            self.phase += step;
            if self.phase >= MAX_PHASE {
                self.phase -= MAX_PHASE;
            }
            // let f = (i as f32 * MAX_PHASE / 128.0).sin() * 8192.0;
            // *a = T::lossy_from(f);
        }
        buf
    }
}

pub fn sound_test(device: &str) -> alsa::Result<()> {
    // Open default playback device
    let pcm = alsa::PCM::new(device, alsa::Direction::Playback, false)?;

    // Set hardware parameters: 44100 Hz / Mono / 16 bit
    let hwp = pcm::HwParams::any(&pcm)?;
    hwp.set_channels(1)?;
    hwp.set_rate(44100, alsa::ValueOr::Nearest)?;
    hwp.set_format(pcm::Format::s16())?;
    hwp.set_access(pcm::Access::RWInterleaved)?;
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
        *a = ((i as f32 * 2.0 * ::std::f32::consts::PI / 128.0).sin() * 8192.0)
            as i16
    }

    // Play it back for 2 seconds.
    for _ in 0..20 * 44100 / 1024 {
        assert_eq!(io.writei(&buf[..])?, 1024);
    }

    // In case the buffer was larger than 2 seconds, start the stream manually.
    if pcm.state() != pcm::State::Running {
        pcm.start()?;
    };
    // Wait for the stream to finish playback.
    pcm.drain()
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
