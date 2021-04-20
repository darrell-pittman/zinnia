use alsa::pcm::{Access, Format, HwParams, PCM};
use alsa::{Direction, ValueOr};
use mpsc::{Receiver, Sender};
use std::{
    sync::{mpsc, Arc, Barrier},
    thread,
    time::Duration,
};
use zinnia::{
    sound::{Sound, SountTest},
    HardwareParams, Result,
};

fn main() {
    let device = "pulse";

    //zinnia::sound_test(device).unwrap();
    let init = Arc::new(Barrier::new(2));
    let running = Arc::new(std::sync::atomic::AtomicBool::new(true));

    let (sound_tx, sound_rx): (
        Sender<Box<dyn Sound<Item = i16>>>,
        Receiver<Box<dyn Sound<Item = i16>>>,
    ) = mpsc::channel();

    let (param_tx, param_rx): (
        Sender<HardwareParams>,
        Receiver<HardwareParams>,
    ) = mpsc::channel();

    let handle;

    {
        let init = Arc::clone(&init);
        let running = Arc::clone(&running);

        handle = thread::spawn(move || -> Result<()> {
            let mut sounds = Vec::<Box<dyn Sound<Item = i16>>>::new();

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

            param_tx.send(HardwareParams::from(&hwp))?;

            init.wait();

            drop(param_tx);

            let io = pcm.io_i16()?;

            // Make sure we don't start the stream too early
            let hwp = pcm.hw_params_current()?;
            let swp = pcm.sw_params_current()?;
            swp.set_start_threshold(hwp.get_buffer_size()?)?;
            pcm.sw_params(&swp)?;

            // let periods_per_second =
            //     hwp.get_rate()? / hwp.get_period_size()? as u32;

            let size = hwp.get_period_size()? as usize;

            let mut vals = Vec::<i16>::with_capacity(size);

            while running.load(std::sync::atomic::Ordering::Relaxed) {
                if let Ok(sound) = sound_rx.try_recv() {
                    sounds.push(sound);
                }

                sounds = sounds.into_iter().filter(|s| !s.complete()).collect();

                if !sounds.is_empty() {
                    vals.push(
                        sounds.iter_mut().fold(0i16, |acc, s| acc + s.tick()),
                    );
                } else {
                    vals.push(0);
                }

                if vals.len() == size {
                    match io.writei(&vals[..]) {
                        Ok(_) => (),
                        Err(err) => {
                            println!("Error: {}", err);
                            pcm.try_recover(err, true)?
                        }
                    }
                    vals.clear();
                }
            }

            Ok(())
        });
    }

    let params = param_rx.recv().unwrap();
    init.wait();

    drop(param_rx);

    println!("Initialized: {:?}", params);

    let base = 220.0;
    let duration = Duration::from_millis(2000);
    for i in 0..8 {
        let freq = match i {
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
        let st = SountTest::<i16>::new(freq, 7000.0, duration, &params);
        sound_tx.send(Box::new(st)).unwrap();
        thread::sleep(duration.mul_f32(1.01));
    }

    thread::sleep(Duration::from_secs(5));
    running.fetch_and(false, std::sync::atomic::Ordering::Relaxed);
    handle.join().unwrap().unwrap();
}
