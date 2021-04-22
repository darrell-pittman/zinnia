use alsa::{
    nix::errno::Errno,
    pcm::{Access, Format, HwParams, IoFormat, State, IO, PCM},
    poll::{poll, pollfd, Flags},
    Direction, Error as AlsaError, PollDescriptors, ValueOr,
};
use mpsc::{Receiver, Sender, SyncSender};
use std::{
    sync::{atomic::AtomicBool, mpsc, Arc, Barrier},
    thread::{self, JoinHandle},
    time::Duration,
};
use zinnia::{
    convert::LossyFrom,
    error::{Error, Kind},
    sound::{Sound, SountTest},
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
    let period_size = hwp.period_size as usize;
    thread::spawn(move || -> Result<()> {
        let mut vals = Vec::<T>::with_capacity(period_size);
        let mut sounds = Vec::<Box<dyn Sound<Item = T>>>::new();
        while running.load(std::sync::atomic::Ordering::Relaxed) {
            if let Ok(sound) = sound_rx.try_recv() {
                sounds.push(sound);
            }

            if sounds.is_empty() {
                vals.push(T::default());
            } else {
                vals.push(
                    sounds
                        .iter_mut()
                        .fold(T::default(), |acc, s| acc + s.tick()),
                );
                sounds = sounds.into_iter().filter(|s| !s.complete()).collect();
            }

            if vals.len() == period_size {
                period_tx.send(vals)?;
                vals = Vec::<T>::with_capacity(period_size);
            }
        }
        Ok(())
    })
}

fn write_and_loop<T>(
    device: &'static str,
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
        // Set hardware parameters: 44100 Hz / Mono / 16 bit
        let hwp = HwParams::any(&pcm)?;
        hwp.set_channels(1)?;
        hwp.set_rate(44100, ValueOr::Nearest)?;
        hwp.set_buffer_time_near(50000, ValueOr::Nearest)?;
        hwp.set_period_time_near(10000, ValueOr::Nearest)?;
        hwp.set_format(<T as IoFormat>::FORMAT)?;
        hwp.set_access(Access::RWInterleaved)?;
        pcm.hw_params(&hwp)?;

        param_tx.send(HardwareParams::from(&hwp))?;

        init.wait();

        drop(param_tx);

        let io: IO<T> = pcm.io_checked()?;

        // Make sure we don't start the stream too early
        let hwp = pcm.hw_params_current()?;
        let swp = pcm.sw_params_current()?;
        swp.set_start_threshold(hwp.get_buffer_size()?)?;
        pcm.sw_params(&swp)?;

        while running.load(std::sync::atomic::Ordering::Relaxed) {
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

fn run<T>(device: &'static str)
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
        Arc::clone(&init),
        Arc::clone(&running),
        period_rx,
        param_tx,
    );

    handles.push(handle);

    let params = param_rx.recv().unwrap();
    init.wait();
    drop(param_rx);
    println!("Initialized: {:?}", params);

    let handle = generate(Arc::clone(&running), &params, sound_rx, period_tx);

    handles.push(handle);

    let base = 220.0;
    let duration = Duration::from_millis(1000);

    // let st = SountTest::<i16>::new(base, 0.0, duration, &params);
    // sound_tx.send(Box::new(st)).unwrap();
    // thread::sleep(duration.mul_f32(1.01));

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
        let st = SountTest::<T>::new(freq, 7000.0, duration, &params);
        sound_tx.send(Box::new(st)).unwrap();
        thread::sleep(duration.mul_f32(1.01));
    }

    thread::sleep(duration.mul_f32(0.1));
    running.fetch_and(false, std::sync::atomic::Ordering::Relaxed);
    for handle in handles {
        match handle.join().unwrap() {
            _ => (),
        }
    }
}

fn main() {
    let device = "pulse";
    //zinnia::sound_test(device).unwrap();
    run::<i16>(device);
}

// fn _write_and_poll(
//     device: &'static str,
//     init: Arc<Barrier>,
//     running: Arc<AtomicBool>,
//     sound_rx: Receiver<Box<dyn Sound<Item = i16>>>,
//     param_transmitters: Vec<Sender<HardwareParams>>,
// ) -> JoinHandle<Result<()>> {
//     thread::spawn(move || -> Result<()> {
//         let mut sounds = Vec::<Box<dyn Sound<Item = i16>>>::new();

//         let pcm = PCM::new(device, Direction::Playback, false).unwrap();
//         // Set hardware parameters: 44100 Hz / Mono / 16 bit
//         let hwp = HwParams::any(&pcm)?;
//         hwp.set_channels(1)?;
//         hwp.set_rate(44100, ValueOr::Nearest)?;
//         hwp.set_buffer_time_near(50000, ValueOr::Nearest)?;
//         hwp.set_period_time_near(10000, ValueOr::Nearest)?;
//         hwp.set_format(Format::s16())?;
//         hwp.set_access(Access::RWInterleaved)?;
//         pcm.hw_params(&hwp)?;

//         for param_tx in param_transmitters {
//             param_tx.send(HardwareParams::from(&hwp))?;
//         }

//         init.wait();

//         drop(param_tx);

//         let io = pcm.io_i16()?;

//         // Make sure we don't start the stream too early
//         let hwp = pcm.hw_params_current()?;
//         let swp = pcm.sw_params_current()?;
//         swp.set_start_threshold(hwp.get_buffer_size()?)?;
//         pcm.sw_params(&swp)?;

//         let size = hwp.get_period_size()? as usize;

//         let mut vals = Vec::<i16>::with_capacity(size);

//         let mut ufds = pcm.get()?;

//         fn wait_for_poll(pcm: &PCM, ufds: &mut [pollfd]) -> alsa::Result<()> {
//             loop {
//                 poll(ufds, -1)?;
//                 let flags = pcm.revents(ufds)?;
//                 return match flags {
//                     Flags::OUT => Ok(()),
//                     _ => Err(alsa::Error::new(
//                         "wait_for_poll",
//                         Errno::EIO as i32,
//                     )),
//                 };
//             }
//         }

//         wait_for_poll(&pcm, &mut ufds[..])?;

//         let init = true;

//         while running.load(std::sync::atomic::Ordering::Relaxed) {
//             if !init {
//                 match wait_for_poll(&pcm, &mut ufds) {
//                     Err(_) => match pcm.state() {
//                         s @ State::XRun | s @ State::Suspended => {
//                             let err = match s {
//                                 State::XRun => Errno::EPIPE,
//                                 _ => Errno::ESTRPIPE,
//                             } as i32;
//                             pcm.try_recover(
//                                 AlsaError::new("wait_for_poll", err),
//                                 true,
//                             )?
//                         }
//                         _ => {
//                             return Err(Error::new("wait_for_poll", Kind::Poll))
//                         }
//                     },
//                     _ => (),
//                 }
//             }

//             if let Ok(sound) = sound_rx.try_recv() {
//                 sounds.push(sound);
//             }

//             if sounds.is_empty() {
//                 vals.push(0);
//             } else {
//                 vals.push(
//                     sounds.iter_mut().fold(0i16, |acc, s| acc + s.tick()),
//                 );
//                 sounds = sounds.into_iter().filter(|s| !s.complete()).collect();
//             }

//             if vals.len() == size {
//                 match io.writei(&vals[..]) {
//                     Ok(_) => (),
//                     Err(err) => {
//                         println!("Error: {}", err);
//                         pcm.try_recover(err, true)?
//                     }
//                 }
//                 vals.clear();
//             }
//         }

//         Ok(())
//     })
// }
