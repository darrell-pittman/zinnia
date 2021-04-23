use alsa::{
    pcm::{HwParams, IoFormat, IO, PCM},
    Direction,
};
use mpsc::{Receiver, Sender, SyncSender};
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc, Barrier,
    },
    thread::{self, JoinHandle},
    time::Duration,
};
use zinnia::{
    convert::LossyFrom,
    sound::{
        self, FadeDirection, LeftRightFade, LinearFadeIn, LinearFadeOut, Sound,
        SountTest, Ticks,
    },
    HardwareParams, Result,
};

fn generate<T>(
    running: Arc<AtomicBool>,
    hwp: &HardwareParams,
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
                if sounds.is_empty() {
                    vals.push(T::default());
                } else {
                    vals.push(
                        sounds.iter_mut().fold(T::default(), |acc, s| {
                            acc + s.generate(channel)
                        }),
                    );
                }
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
    params: HardwareParams,
    init: Arc<Barrier>,
    running: Arc<AtomicBool>,
    period_rx: Receiver<Vec<T>>,
    param_tx: Sender<HardwareParams>,
) -> JoinHandle<Result<()>>
where
    T: Send + 'static + Default + std::ops::Add<Output = T> + IoFormat + Copy,
{
    thread::spawn(move || -> Result<()> {
        let pcm = PCM::new(device, Direction::Playback, false).unwrap();
        let hwp = HwParams::any(&pcm)?;
        params.populate_hwp::<T>(&hwp)?;
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

fn run<T>(device: &'static str, params: HardwareParams) -> Result<()>
where
    T: Send
        + 'static
        + Default
        + std::ops::Add<Output = T>
        + IoFormat
        + Copy
        + LossyFrom<f32>,
{
    let init = Arc::new(Barrier::new(2));
    let running = Arc::new(AtomicBool::new(true));

    let (sound_tx, sound_rx): (
        Sender<Box<dyn Sound<Item = T>>>,
        Receiver<Box<dyn Sound<Item = T>>>,
    ) = mpsc::channel();

    let (param_tx, param_rx): (
        Sender<HardwareParams>,
        Receiver<HardwareParams>,
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

    let base = 220.0;
    let duration = Duration::from_millis(2000);
    let duration_ticks = sound::duration_to_ticks(duration, params.rate());
    let fade_ticks = (duration_ticks as f32 * 0.3) as Ticks;

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

        let direction = match i % 2 {
            0 => FadeDirection::LeftRight,
            _ => FadeDirection::RightLeft,
        };

        let mut st = SountTest::<T>::new(freq, 0.7, duration, &params);
        st.add_filter(Box::new(LinearFadeIn::new(fade_ticks)));
        st.add_filter(Box::new(LinearFadeOut::new(fade_ticks, duration_ticks)));
        st.add_filter(Box::new(LeftRightFade::new(
            0.0,
            1.0,
            direction,
            duration_ticks,
        )));

        sound_tx.send(Box::new(st))?;
        thread::sleep(duration.mul_f32(1.01));
    }

    thread::sleep(duration.mul_f32(0.1));
    running.fetch_and(false, Ordering::Relaxed);
    for handle in handles {
        handle.join().unwrap()?;
    }

    Ok(())
}

fn main() {
    let device = "pulse";
    let params = HardwareParams::new(50000, 10000, 2);

    match run::<i16>(device, params) {
        Ok(_) => (),
        Err(err) => println!("{}", err),
    }
}
