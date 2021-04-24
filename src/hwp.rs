use std::marker::PhantomData;

use alsa::{
    pcm::{Access, Format, HwParams, IoFormat},
    ValueOr,
};

use super::Result;

#[derive(Debug)]
pub struct HardwareParams<T: IoFormat> {
    channels: u32,
    rate: u32,
    buffer_size: i64,
    period_size: i64,
    format: Format,
    access: Access,
    buffer_time: u32,
    period_time: u32,
    phantom: PhantomData<T>,
}

impl<T: IoFormat> HardwareParams<T> {
    pub fn periods_per_second(&self) -> u32 {
        self.rate / self.period_size as u32
    }

    pub fn populate_hwp(&self, hwp: &HwParams) -> Result<()> {
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

impl<T: IoFormat> From<&HwParams<'_>> for HardwareParams<T> {
    fn from(hwp: &HwParams) -> Self {
        HardwareParams::<T> {
            channels: hwp.get_channels().unwrap(),
            rate: hwp.get_rate().unwrap(),
            buffer_size: hwp.get_buffer_size().unwrap(),
            period_size: hwp.get_period_size().unwrap(),
            format: hwp.get_format().unwrap(),
            access: hwp.get_access().unwrap(),
            buffer_time: Default::default(),
            period_time: Default::default(),
            phantom: PhantomData::<T>::default(),
        }
    }
}

pub struct HwpBuilder<T>
where
    T: IoFormat + Copy,
{
    channels: u32,
    rate: u32,
    format: Format,
    access: Access,
    buffer_time: u32,
    period_time: u32,
    phantom: PhantomData<T>,
}

impl<T> HwpBuilder<T>
where
    T: IoFormat + Copy,
{
    pub fn new(buffer_time: u32, period_time: u32, channels: u32) -> Self {
        HwpBuilder::<T> {
            channels,
            rate: 44100,
            format: <T as IoFormat>::FORMAT,
            access: Access::RWInterleaved,
            buffer_time,
            period_time,
            phantom: PhantomData::<T>::default(),
        }
    }
    pub fn rate(mut self, rate: u32) -> Self {
        self.rate = rate;
        self
    }

    pub fn access(mut self, access: Access) -> Self {
        self.access = access;
        self
    }

    pub fn build(self) -> HardwareParams<T> {
        HardwareParams::<T> {
            channels: self.channels,
            rate: self.rate,
            buffer_time: self.buffer_time,
            period_time: self.period_time,
            format: self.format,
            access: self.access,
            buffer_size: Default::default(),
            period_size: Default::default(),
            phantom: PhantomData::<T>::default(),
        }
    }
}
