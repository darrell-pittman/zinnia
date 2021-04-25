use alsa::{
    pcm::{HwParams, IoFormat, IO, PCM},
    Direction,
};
use mpsc::{Receiver, Sender, SyncSender};
use std::{
    fmt::Debug,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc, Barrier,
    },
    thread::{self, JoinHandle},
    time::Duration,
};
use zinnia::{
    convert::LossyFrom,
    hwp::{HardwareParams, HwpBuilder},
    sound::{
        self, FadeDirection, LeftRightFade, LinearFadeIn, LinearFadeOut, Sound,
        SountTest, Ticks,
    },
    Result,
};

fn generate<T: IoFormat>(
    running: Arc<AtomicBool>,
    hwp: &HardwareParams<T>,
    sound_rx: Receiver<Box<dyn Sound<Item = T>>>,
    period_tx: SyncSender<Vec<T>>,
) -> JoinHandle<Result<()>>
where
    T: Send + 'static + Default + std::ops::Add<Output = T>,
{
    let period_size = hwp.period_size() as usize;
    let channels = hwp.channels();

    thread::spawn(move || -> Result<()> {
        let size = period_size * channels as usize;
        let mut vals = Vec::<T>::with_capacity(size);
        let mut sounds = Vec::<Box<dyn Sound<Item = T>>>::new();
        while running.load(Ordering::Relaxed) {
            if let Ok(sound) = sound_rx.try_recv() {
                sounds.push(sound);
            }

            for channel in 0..channels {
                vals.push(
                    sounds
                        .iter_mut()
                        .fold(T::default(), |acc, s| acc + s.generate(channel)),
                );
            }

            sounds.iter_mut().for_each(|s| s.tick());
            sounds = sounds.into_iter().filter(|s| !s.complete()).collect();

            if vals.len() == size {
                period_tx.send(vals)?;
                vals = Vec::<T>::with_capacity(period_size);
            }
        }
        Ok(())
    })
}

fn write_and_loop<T>(
    device: &'static str,
    params: HardwareParams<T>,
    init: Arc<Barrier>,
    running: Arc<AtomicBool>,
    period_rx: Receiver<Vec<T>>,
    param_tx: Sender<HardwareParams<T>>,
) -> JoinHandle<Result<()>>
where
    T: Send + 'static + Default + std::ops::Add<Output = T> + IoFormat + Copy,
{
    thread::spawn(move || -> Result<()> {
        let pcm = PCM::new(device, Direction::Playback, false).unwrap();
        let hwp = HwParams::any(&pcm)?;
        params.populate_hwp(&hwp)?;
        pcm.hw_params(&hwp)?;
        let hwp = pcm.hw_params_current()?;
        param_tx.send(HardwareParams::from(&hwp))?;

        init.wait();
        drop(param_tx);

        let io: IO<T> = pcm.io_checked()?;

        // Make sure we don't start the stream too early
        let hwp = pcm.hw_params_current()?;
        let swp = pcm.sw_params_current()?;
        swp.set_start_threshold(hwp.get_buffer_size()?)?;
        pcm.sw_params(&swp)?;

        while running.load(Ordering::Relaxed) {
            let vals = period_rx.recv()?;
            match io.writei(&vals[..]) {
                Ok(_) => (),
                Err(err) => {
                    println!("Error: {}", err);
                    pcm.try_recover(err, true)?
                }
            }
        }
        Ok(())
    })
}

fn run<T>(device: &'static str, params: HardwareParams<T>) -> Result<()>
where
    T: Send
        + 'static
        + Default
        + std::ops::Add<Output = T>
        + IoFormat
        + Copy
        + LossyFrom<f32>
        + Debug,
{
    let init = Arc::new(Barrier::new(2));
    let running = Arc::new(AtomicBool::new(true));

    let (sound_tx, sound_rx): (
        Sender<Box<dyn Sound<Item = T>>>,
        Receiver<Box<dyn Sound<Item = T>>>,
    ) = mpsc::channel();

    let (param_tx, param_rx): (
        Sender<HardwareParams<T>>,
        Receiver<HardwareParams<T>>,
    ) = mpsc::channel();

    let (period_tx, period_rx): (SyncSender<Vec<T>>, Receiver<Vec<T>>) =
        mpsc::sync_channel(1);

    let mut handles = Vec::new();

    let handle = write_and_loop(
        device,
        params,
        Arc::clone(&init),
        Arc::clone(&running),
        period_rx,
        param_tx,
    );

    handles.push(handle);

    let params = param_rx.recv()?;
    init.wait();
    drop(param_rx);
    println!("Initialized: {:?}", params);

    let handle = generate(Arc::clone(&running), &params, sound_rx, period_tx);

    handles.push(handle);

    let mut base = 110.0;
    let duration = Duration::from_millis(1000);
    let duration_ticks = sound::duration_to_ticks(duration, params.rate());
    let fade_ticks = (duration_ticks as f32 * 0.3) as Ticks;

    let (octaves, notes) = (2, 7);

    let write_note = |freq: f32, fade_direction: FadeDirection| -> Result<()> {
        println!("Frequency: {}", freq);
        let mut st = SountTest::<T>::new(freq, 0.7, duration, &params);
        st.add_filter(Box::new(LinearFadeIn::new(fade_ticks)));
        st.add_filter(Box::new(LinearFadeOut::new(fade_ticks, duration_ticks)));
        st.add_filter(Box::new(LeftRightFade::new(
            0.0,
            1.0,
            fade_direction,
            duration_ticks,
        )));

        sound_tx.send(Box::new(st))?;
        Ok(())
    };

    for _ in 0..octaves {
        for i in 0..notes {
            let freq = match i {
                0 => base,
                1 => base * 1.125,
                2 => base * 1.25,
                3 => base * 1.333,
                4 => base * 1.5,
                5 => base * 1.666,
                6 => base * 1.875,
                _ => base,
            };

            let direction = match i % 2 {
                0 => FadeDirection::LeftRight,
                _ => FadeDirection::RightLeft,
            };

            write_note(freq, direction)?;
            thread::sleep(duration.mul_f32(1.01));
        }
        base *= 2.0;
    }
    write_note(base, FadeDirection::LeftRight)?;
    thread::sleep(duration.mul_f32(1.51));

    running.fetch_and(false, Ordering::Relaxed);
    for handle in handles {
        handle.join().unwrap()?;
    }

    Ok(())
}

fn main() {
    let device = "pulse";
    let params = HwpBuilder::<i32>::new(50000, 10000, 2).rate(44100).build();

    match run(device, params) {
        Ok(_) => (),
        Err(err) => println!("{}", err),
    }
}
