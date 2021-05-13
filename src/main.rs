use alsa::{
    pcm::{HwParams, IoFormat, IO, PCM},
    Direction,
};
use mpsc::{Receiver, Sender, SyncSender};
use std::{
    fmt::Debug,
    io,
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
    music::Note,
    sound::{
        self,
        config::SoundConfigCollection,
        filter::{LinearFadeIn, LinearFadeOut},
        CachedPeriod, CachedSound, PeriodConfig, Sinusoid, Sound, Ticks,
        C4_PIANO_2_CH_SOUND, SINE_PERIOD_2_CH,
    },
    Result,
};

fn generate<T>(
    running: Arc<AtomicBool>,
    hwp: &HardwareParams<T>,
    sound_rx: Receiver<Box<dyn Sound>>,
    period_tx: SyncSender<Vec<T>>,
) -> JoinHandle<Result<()>>
where
    T: Send + 'static + IoFormat + LossyFrom<f32>,
{
    let period_size = hwp.period_size() as usize;
    let channels = hwp.channels();

    thread::spawn(move || -> Result<()> {
        let size = period_size * channels as usize;
        let mut vals = Vec::<T>::with_capacity(size);
        let mut sounds = Vec::<Box<dyn Sound>>::new();
        while running.load(Ordering::Relaxed) {
            if let Ok(sound) = sound_rx.try_recv() {
                sounds.push(sound);
            }

            for channel in 0..channels {
                vals.push(LossyFrom::lossy_from(sound::mix_fixed(
                    &mut sounds,
                    channel,
                )));
            }

            sounds.iter_mut().for_each(|s| s.tick());
            sounds = sounds.into_iter().filter(|s| !s.is_complete()).collect();

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
    T: Send + 'static + IoFormat + Copy,
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
                Err(err) => pcm.try_recover(err, true)?,
            }
        }
        Ok(())
    })
}

fn input<T>(
    running: Arc<AtomicBool>,
    sound_tx: Sender<Box<dyn Sound>>,
    params: HardwareParams<T>,
) -> JoinHandle<Result<()>>
where
    T: Send + 'static + IoFormat + Copy + LossyFrom<f32> + Debug,
{
    thread::spawn(move || {
        let duration = Duration::from_millis(1000);
        let amplitude_scale = sound::max_amplitude::<T>() as f32;
        //let phase = std::f32::consts::PI / 2.0;
        let phase = 0.0;
        let duration_ticks = sound::duration_to_ticks(duration, params.rate());
        let fade_ticks = (duration_ticks as f32 * 0.3) as Ticks;
        while running.load(Ordering::Relaxed) {
            let mut note = String::new();
            io::stdin().read_line(&mut note)?;
            if note.trim().is_empty() {
                println!("Done!");
                running.fetch_and(false, Ordering::Relaxed);
                break;
            }

            match Note::parse(note.as_str()) {
                Ok(note) => {
                    if let Ok(freq) = note.freq() {
                        let config = SoundConfigCollection::with_configs(
                            [
                                (freq, phase, amplitude_scale),
                                (freq, phase, amplitude_scale),
                            ]
                            .as_ref(),
                        );
                        let mut sound =
                            Box::new(Sinusoid::new(&config, duration, &params));

                        sound.add_filter(Box::new(LinearFadeIn::new(
                            fade_ticks,
                        )));

                        sound.add_filter(Box::new(LinearFadeOut::new(
                            fade_ticks,
                            duration_ticks,
                        )));

                        sound_tx.send(sound)?;
                        thread::sleep(duration.mul_f32(1.1));

                        let config = SoundConfigCollection::with_configs(
                            [
                                (freq, phase, amplitude_scale),
                                (freq, phase, amplitude_scale),
                            ]
                            .as_ref(),
                        );

                        let mut sound = Box::new(CachedPeriod::new(
                            PeriodConfig::new(&SINE_PERIOD_2_CH[..], 2),
                            &config,
                            duration,
                            &params,
                        ));

                        sound.add_filter(Box::new(LinearFadeIn::new(
                            fade_ticks,
                        )));

                        sound.add_filter(Box::new(LinearFadeOut::new(
                            fade_ticks,
                            duration_ticks,
                        )));

                        sound_tx.send(sound)?;
                        thread::sleep(duration.mul_f32(1.1));

                        let sound = Box::new(CachedSound::new(
                            PeriodConfig::new(&C4_PIANO_2_CH_SOUND[..], 2),
                        ));

                        sound_tx.send(sound)?;
                        thread::sleep(Duration::from_secs(5));
                    } else {
                        println!("Invalid Note!");
                    }
                }
                Err(_) => {
                    println!("Failed to parse note!");
                }
            }
        }
        Ok(())
    })
}

fn run<T>(device: &'static str, params: HardwareParams<T>) -> Result<()>
where
    T: Send + 'static + IoFormat + Copy + LossyFrom<f32> + Debug,
{
    let init = Arc::new(Barrier::new(2));
    let running = Arc::new(AtomicBool::new(true));

    let (sound_tx, sound_rx): (
        Sender<Box<dyn Sound>>,
        Receiver<Box<dyn Sound>>,
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

    let handle = input(Arc::clone(&running), sound_tx, params);
    handles.push(handle);

    for handle in handles {
        handle.join().unwrap()?;
    }

    Ok(())
}

fn main() {
    let device = "pulse";
    let params = HwpBuilder::<i16>::new(25000, 5000, 2).rate(8000).build();

    match run(device, params) {
        Ok(_) => (),
        Err(err) => println!("{}", err),
    }
}
