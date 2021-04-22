use alsa::{
    pcm::{Access, Format, HwParams, IoFormat},
    ValueOr,
};

pub mod convert;
pub mod error;
pub mod sound;

pub type Result<T> = std::result::Result<T, error::Error>;

#[derive(Debug, Copy, Clone)]
pub struct HardwareParams {
    pub channels: u32,
    pub rate: u32,
    pub buffer_size: i64,
    pub period_size: i64,
    pub format: Format,
    pub access: Access,
    buffer_time: i64,
    period_time: i64,
}

impl HardwareParams {
    pub fn periods_per_second(&self) -> u32 {
        self.rate / self.period_size as u32
    }

    pub fn new(buffer_time: i64, period_time: i64) -> HardwareParams {
        let mut hwp = HardwareParams::default();
        hwp.buffer_time = buffer_time;
        hwp.period_time = period_time;
        hwp
    }

    pub fn get_buffer_time(&self) -> i64 {
        self.buffer_time
    }

    pub fn get_period_time(&self) -> i64 {
        self.period_time
    }

    pub fn populate_hwp<T: IoFormat>(&self, hwp: &HwParams) -> Result<()> {
        hwp.set_channels(1)?;
        hwp.set_rate(44100, ValueOr::Nearest)?;
        hwp.set_buffer_time_near(50000, ValueOr::Nearest)?;
        hwp.set_period_time_near(10000, ValueOr::Nearest)?;
        hwp.set_format(<T as IoFormat>::FORMAT)?;
        hwp.set_access(Access::RWInterleaved)?;
        Ok(())
    }
}

impl From<&HwParams<'_>> for HardwareParams {
    fn from(hwp: &HwParams) -> Self {
        HardwareParams {
            channels: hwp.get_channels().unwrap(),
            rate: hwp.get_rate().unwrap(),
            buffer_size: hwp.get_buffer_size().unwrap(),
            period_size: hwp.get_period_size().unwrap(),
            format: hwp.get_format().unwrap(),
            access: hwp.get_access().unwrap(),
            buffer_time: Default::default(),
            period_time: Default::default(),
        }
    }
}

impl Default for HardwareParams {
    fn default() -> Self {
        HardwareParams {
            channels: 1,
            rate: 44100,
            buffer_size: 0,
            period_size: 0,
            format: Format::s16(),
            access: Access::RWInterleaved,
            buffer_time: Default::default(),
            period_time: Default::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
