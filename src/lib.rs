use alsa::{
    pcm::{Access, Format, HwParams, IoFormat},
    ValueOr,
};

pub mod convert;
pub mod error;
pub mod sound;

pub type Result<T> = std::result::Result<T, error::Error>;

#[derive(Debug)]
pub struct HardwareParams {
    channels: u32,
    rate: u32,
    buffer_size: i64,
    period_size: i64,
    format: Format,
    access: Access,
    buffer_time: u32,
    period_time: u32,
}

impl HardwareParams {
    pub fn periods_per_second(&self) -> u32 {
        self.rate / self.period_size as u32
    }

    pub fn new(
        buffer_time: u32,
        period_time: u32,
        channels: u32,
    ) -> HardwareParams {
        let mut hwp = HardwareParams::default();
        hwp.buffer_time = buffer_time;
        hwp.period_time = period_time;
        hwp.channels = channels;
        hwp
    }

    pub fn populate_hwp<T: IoFormat>(&self, hwp: &HwParams) -> Result<()> {
        hwp.set_channels(self.channels)?;
        hwp.set_rate(self.rate, ValueOr::Nearest)?;
        hwp.set_buffer_time_near(self.buffer_time, ValueOr::Nearest)?;
        hwp.set_period_time_near(self.period_time, ValueOr::Nearest)?;
        hwp.set_format(<T as IoFormat>::FORMAT)?;
        hwp.set_access(self.access)?;
        Ok(())
    }

    pub fn period_size(&self) -> i64 {
        self.period_size
    }

    pub fn rate(&self) -> u32 {
        self.rate
    }

    pub fn channels(&self) -> u32 {
        self.channels
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
