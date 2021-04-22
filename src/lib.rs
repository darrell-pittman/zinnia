use alsa::pcm::{Access, Format, HwParams};

pub mod convert;
pub mod error;
pub mod sound;

pub type Result<T> = std::result::Result<T, error::Error>;

#[derive(Debug)]
pub struct HardwareParams {
    pub channels: u32,
    pub rate: u32,
    pub buffer_size: i64,
    pub period_size: i64,
    pub format: Format,
    pub access: Access,
}

impl HardwareParams {
    pub fn periods_per_second(&self) -> u32 {
        self.rate / self.period_size as u32
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
