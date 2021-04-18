use alsa::pcm::{Access, Format, HwParams, PCM};
use alsa::{Direction, ValueOr};
use std::{sync::mpsc, thread};
use zinnia::{
    sound::{Sound as _, SountTest},
    Result,
};

fn main() {
    let device = "pulse";

    //zinnia::sound_test(device).unwrap();

    let (tx, rx): (mpsc::Sender<u32>, mpsc::Receiver<u32>) = mpsc::channel();

    let handle = thread::spawn(move || -> Result<()> {
        let pcm = PCM::new(device, Direction::Playback, false).unwrap();
        // Set hardware parameters: 44100 Hz / Mono / 16 bit
        let hwp = HwParams::any(&pcm)?;
        hwp.set_channels(1)?;
        hwp.set_rate(44100, ValueOr::Nearest)?;
        hwp.set_buffer_time_near(500000, ValueOr::Nearest)?;
        hwp.set_period_time_near(100000, ValueOr::Nearest)?;
        hwp.set_format(Format::s16())?;
        hwp.set_access(Access::RWInterleaved)?;
        pcm.hw_params(&hwp)?;
        let io = pcm.io_i16()?;

        // Make sure we don't start the stream too early
        let hwp = pcm.hw_params_current()?;
        let swp = pcm.sw_params_current()?;
        swp.set_start_threshold(hwp.get_buffer_size()?)?;
        pcm.sw_params(&swp)?;

        let periods_per_second =
            hwp.get_rate()? / hwp.get_period_size()? as u32;

        let mut st = SountTest::<i16>::new(110, &hwp);

        for received in rx {
            if received == 0 {
                break;
            }

            st.freq(received);

            for _ in 0..periods_per_second / 2 {
                match io.writei(&st.generate()[..]) {
                    Ok(_) => (),
                    Err(err) => println!("Error: {}", err),
                }
            }
        }

        Ok(())
    });

    let base = 220.0;
    for _ in 0..1000 {
        for t in 0..8 {
            let freq = match t {
                0 => base,
                1 => base * 1.125,
                2 => base * 1.25,
                3 => base * 1.333,
                4 => base * 1.5,
                5 => base * 1.666,
                6 => base * 1.875,
                7 => base * 2.0,
                _ => base,
            };
            tx.send(freq as u32).unwrap();
        }
    }

    tx.send(0).unwrap();

    handle.join().unwrap().unwrap();
}
