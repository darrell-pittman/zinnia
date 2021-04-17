use alsa::pcm::{Access, Format, HwParams, PCM};
use alsa::{Direction, ValueOr};
use std::{sync::mpsc, thread};
use zinnia::Sound;

fn main() {
    let device = "pulse";

    //zinnia::sound_test(device).unwrap();

    let (tx, rx): (mpsc::Sender<Vec<i16>>, mpsc::Receiver<Vec<i16>>) =
        mpsc::channel();

    let handle = thread::spawn(move || -> alsa::Result<()> {
        let pcm = PCM::new(device, Direction::Playback, false).unwrap();
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
        let mut st = zinnia::SountTest::<i16>::new();

        let g = st.generate(&hwp);
        println!("{:?}", g);

        let swp = pcm.sw_params_current()?;
        swp.set_start_threshold(hwp.get_buffer_size()?)?;
        pcm.sw_params(&swp)?;

        for received in rx {
            if received.is_empty() {
                break;
            }
            // Play it back for 2 seconds.
            for _ in 0..1 * 44100 / 1024 {
                match io.writei(&received[..]) {
                    Ok(_) => (),
                    Err(err) => println!("Error: {}", err),
                }
            }
        }

        pcm.drain()?;

        Ok(())
    });

    for t in 1..10 {
        let mut buf = vec![0i16; 1024];
        for (i, a) in buf.iter_mut().enumerate() {
            *a = ((i as f32 * 2.0 * ::std::f32::consts::PI
                / (256.0 / t as f32))
                .sin()
                * 2048.0) as i16
        }
        tx.send(buf).unwrap();
    }

    tx.send(Vec::new()).unwrap();

    handle.join().unwrap().unwrap();
}
