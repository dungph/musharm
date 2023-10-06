use crate::command::Cmd;
use crate::{pump::Pump, stepper::Stepper, storage::Storage};
use defmt::info;
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex as Raw, channel::Channel};
use embassy_time::{Duration, Timer};
use futures::future::join;
use heapless::Vec;
use serde::{Deserialize, Serialize};

static CH: Channel<Raw, Cmd, 10> = Channel::new();

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct WateringPosition {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub dur_ms: u32,
}

pub async fn send_msg(cmd: Cmd) {
    CH.send(cmd).await;
}

#[embassy_executor::task]
pub async fn run(
    mut x: Stepper<'static>,
    mut y: Stepper<'static>,
    mut z: Stepper<'static>,
    mut storage: Storage,
    mut pump: Pump,
) {
    let mut positions = Vec::<WateringPosition, 100>::new();

    let mut schedule_enabled = true;

    if let Ok(list) = restore(&mut storage).await {
        positions = list;
    } else {
        info!("Restore Error");
    }

    loop {
        while schedule_enabled {
            for pos in positions.iter() {
                if let Ok(Cmd::Stop) = CH.try_receive() {
                    schedule_enabled = false;
                }
                join(x.goto(pos.x), y.goto(pos.y)).await;
                z.goto(pos.z).await;
                pump.on();
                Timer::after(Duration::from_millis(pos.dur_ms.into())).await;
                pump.off();
                z.goto(0).await;
            }
        }

        Timer::after(Duration::from_millis(1000)).await;

        while !schedule_enabled {
            let cmd = CH.receive().await;
            match cmd {
                Cmd::Goto(val) => {
                    futures::future::join3(
                        x.goto(val.x.unwrap_or(x.current_pos())),
                        y.goto(val.y.unwrap_or(y.current_pos())),
                        z.goto(val.z.unwrap_or(z.current_pos())),
                    )
                    .await;
                }
                Cmd::Move(val) => {
                    futures::future::join3(
                        x.r#move(val.x.unwrap_or(0)),
                        y.r#move(val.y.unwrap_or(0)),
                        z.r#move(val.z.unwrap_or(0)),
                    )
                    .await;
                }
                Cmd::SpeedMin(val) => {
                    x.set_speed_min(val.x.unwrap_or(x.speed_min()));
                    y.set_speed_min(val.y.unwrap_or(y.speed_min()));
                    z.set_speed_min(val.z.unwrap_or(z.speed_min()));
                }
                Cmd::SpeedMax(val) => {
                    x.set_speed_max(val.x.unwrap_or(x.speed_max()));
                    y.set_speed_max(val.y.unwrap_or(y.speed_max()));
                    z.set_speed_max(val.z.unwrap_or(z.speed_max()));
                }
                Cmd::SpeedAccel(val) => {
                    x.set_speed_accel(val.x.unwrap_or(x.speed_accel()));
                    y.set_speed_accel(val.y.unwrap_or(y.speed_accel()));
                    z.set_speed_accel(val.z.unwrap_or(z.speed_accel()));
                }
                Cmd::StepPerMM(val) => {
                    x.set_step_per_mm(val.x.unwrap_or(x.step_per_mm()));
                    y.set_step_per_mm(val.y.unwrap_or(y.step_per_mm()));
                    z.set_step_per_mm(val.z.unwrap_or(z.step_per_mm()));
                }
                Cmd::AddPos(val) => {
                    if let Some(x) = val.x {
                        if let Some(y) = val.y {
                            if let Some(z) = val.z {
                                positions.push(WateringPosition {
                                    x,
                                    y,
                                    z,
                                    dur_ms: 1000,
                                });
                                backup(&mut storage, &positions).await;
                            }
                        }
                    }
                }
                Cmd::DelPos(id) => {
                    if let Some(_pos) = positions.get(id as usize).cloned() {
                        positions.remove(id as usize);
                        positions[..].sort_unstable();
                        backup(&mut storage, &positions).await;
                    }
                }
                Cmd::PumpOn => {
                    pump.on();
                }
                Cmd::PumpOff => {
                    pump.off();
                }
                Cmd::ListPos => {
                    todo!()
                }
                Cmd::Start => {
                    schedule_enabled = true;
                }
                Cmd::Stop => (),
                Cmd::Home => {
                    x.set_current_pos(0);
                    y.set_current_pos(0);
                    z.set_current_pos(0);
                }
                Cmd::Help => todo!(),
            }
        }
    }
}

async fn restore(storage: &mut Storage) -> Result<Vec<WateringPosition, 100>, ()> {
    let mut size = 0;
    if let Ok(page) = storage.read_page(0) {
        size = page[0];
    }

    let list = (1..=size)
        .filter_map(|idx| storage.read_page(idx).ok())
        .filter_map(|page| postcard::from_bytes::<WateringPosition>(&page).ok())
        .collect::<Vec<WateringPosition, 100>>();
    Ok(list)
}

async fn backup(sto: &mut Storage, list: &Vec<WateringPosition, 100>) -> Result<(), ()> {
    let mut buf = [0; 32];
    buf[0] = list.len() as u8;
    sto.write_page(0, buf)?;

    for (idx, p) in list.iter().enumerate() {
        Timer::after(Duration::from_millis(10)).await;
        let mut buf = [0; 32];
        postcard::to_slice(p, &mut buf).ok();
        sto.write_page(1 + idx as u8, buf).ok();
    }
    Ok(())
}

//use core::sync::atomic::AtomicBool;
//use core::sync::atomic::Ordering::Relaxed;
//
//use crate::command::Cmd;
//use crate::{pump::Pump, stepper::Stepper, storage::Storage};
//use defmt::info;
//use embassy_sync::mutex::Mutex;
//use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex as Raw, channel::Channel};
//use embassy_time::{Duration, Timer};
//use futures::future::join;
//use heapless::Vec;
//use serde::{Deserialize, Serialize};
//
//static CH: Channel<Raw, Cmd, 10> = Channel::new();
//
//type Steppers = (Stepper<'static>, Stepper<'static>, Stepper<'static>);
//static STEPPERS: Mutex<Raw, Option<Steppers>> = Mutex::new(None);
//static STORAGE: Mutex<Raw, Option<Storage>> = Mutex::new(None);
//static POSITIONS: Mutex<Raw, Vec<WateringPosition, 100>> = Mutex::new(Vec::new());
//
//static RUNNING: AtomicBool = AtomicBool::new(true);
//static PUMP_ON: AtomicBool = AtomicBool::new(false);
//
//#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
//pub struct WateringPosition {
//    pub x: i32,
//    pub y: i32,
//    pub z: i32,
//    pub dur_ms: u32,
//}
//
//pub enum Error {
//    Running,
//    Uninit,
//}
//
//#[embassy_executor::task]
//pub async fn run(
//    x: Stepper<'static>,
//    y: Stepper<'static>,
//    z: Stepper<'static>,
//    storage: Storage,
//    mut pump: Pump,
//) {
//    STEPPERS.lock().await.replace((x, y, z));
//    STORAGE.lock().await.replace(storage);
//
//    if let Ok(list) = restore().await {
//        *POSITIONS.lock().await = list;
//    } else {
//        info!("Restore Error");
//    }
//
//    loop {
//        let mut pos_id = 0;
//        while let Some(pos) = POSITIONS.lock().await.get(pos_id).cloned() {
//            if RUNNING.load(Relaxed) {
//                if let Some((x, y, z)) = STEPPERS.lock().await.as_mut() {
//                    join(x.goto(pos.x), y.goto(pos.y)).await;
//                    z.goto(pos.z).await;
//                    pump.on();
//                    Timer::after(Duration::from_millis(pos.dur_ms.into())).await;
//                    pump.off();
//                    z.goto(0).await;
//                }
//                pos_id += 1;
//            } else {
//                break;
//            }
//            Timer::after(Duration::from_millis(1000)).await;
//        }
//        while !RUNNING.load(Relaxed) {
//            if PUMP_ON.load(Relaxed) {
//                pump.on();
//            } else {
//                pump.off();
//            }
//            Timer::after(Duration::from_millis(1000)).await;
//        }
//        pump.off();
//
//        Timer::after(Duration::from_millis(1000)).await;
//    }
//}
//
//async fn restore() -> Result<Vec<WateringPosition, 100>, ()> {
//    if let Some(ref mut storage) = STORAGE.lock().await.as_mut() {
//        let mut size = 0;
//        if let Ok(page) = storage.read_page(0) {
//            size = page[0];
//        }
//
//        let list = (1..=size)
//            .filter_map(|idx| storage.read_page(idx).ok())
//            .filter_map(|page| postcard::from_bytes::<WateringPosition>(&page).ok())
//            .collect::<Vec<WateringPosition, 100>>();
//        Ok(list)
//    } else {
//        Ok(Vec::new())
//    }
//}
//
//async fn backup(list: &Vec<WateringPosition, 100>) -> Result<(), ()> {
//    let mut buf = [0; 32];
//    buf[0] = list.len() as u8;
//    if let Some(ref mut sto) = STORAGE.lock().await.as_mut() {
//        sto.write_page(0, buf)?;
//
//        for (idx, p) in list.iter().enumerate() {
//            Timer::after(Duration::from_millis(10)).await;
//            let mut buf = [0; 32];
//            postcard::to_slice(p, &mut buf).ok();
//            sto.write_page(1 + idx as u8, buf).ok();
//        }
//    }
//    Ok(())
//}
//
//fn check_running() -> Result<(), Error> {
//    if RUNNING.load(Relaxed) {
//        info!("is running");
//        Err(Error::Running)
//    } else {
//        info!("is not running");
//        Ok(())
//    }
//}
//
//pub async fn r#move(x: Option<i32>, y: Option<i32>, z: Option<i32>) -> Result<(), Error> {
//    check_running()?;
//    if let Some((ref mut s1, ref mut s2, ref mut s3)) = STEPPERS.lock().await.as_mut() {
//        info!("move {:?}, {:?}, {:?}", x, y, z);
//        futures::future::join3(
//            s1.r#move(x.unwrap_or(0)),
//            s2.r#move(y.unwrap_or(0)),
//            s3.r#move(z.unwrap_or(0)),
//        )
//        .await;
//        Ok(())
//    } else {
//        Err(Error::Uninit)
//    }
//}
//
//pub async fn goto(x: Option<i32>, y: Option<i32>, z: Option<i32>) -> Result<(), Error> {
//    check_running()?;
//    if let Some((ref mut s1, ref mut s2, ref mut s3)) = STEPPERS.lock().await.as_mut() {
//        futures::future::join3(
//            s1.goto(x.unwrap_or(s1.current_pos())),
//            s2.goto(y.unwrap_or(s2.current_pos())),
//            s3.goto(z.unwrap_or(s3.current_pos())),
//        )
//        .await;
//        Ok(())
//    } else {
//        Err(Error::Uninit)
//    }
//}
//
//pub async fn set_speed_min(x: Option<u32>, y: Option<u32>, z: Option<u32>) -> Result<(), Error> {
//    if let Some((s1, s2, s3)) = STEPPERS.lock().await.as_mut() {
//        if let Some(v) = x {
//            s1.set_speed_min(v)
//        }
//        if let Some(v) = y {
//            s2.set_speed_min(v)
//        }
//        if let Some(v) = z {
//            s3.set_speed_min(v)
//        }
//    };
//    Ok(())
//}
//
//pub async fn set_speed_max(x: Option<u32>, y: Option<u32>, z: Option<u32>) -> Result<(), Error> {
//    if let Some((s1, s2, s3)) = STEPPERS.lock().await.as_mut() {
//        if let Some(v) = x {
//            s1.set_speed_max(v)
//        }
//        if let Some(v) = y {
//            s2.set_speed_max(v)
//        }
//        if let Some(v) = z {
//            s3.set_speed_max(v)
//        }
//    };
//    Ok(())
//}
//pub async fn set_accel(x: Option<u32>, y: Option<u32>, z: Option<u32>) -> Result<(), Error> {
//    if let Some((s1, s2, s3)) = STEPPERS.lock().await.as_mut() {
//        if let Some(v) = x {
//            s1.set_speed_accel(v)
//        }
//        if let Some(v) = y {
//            s2.set_speed_accel(v)
//        }
//        if let Some(v) = z {
//            s3.set_speed_accel(v)
//        }
//    };
//    Ok(())
//}
//pub async fn set_step_per_mm(x: Option<u32>, y: Option<u32>, z: Option<u32>) -> Result<(), Error> {
//    check_running()?;
//    if let Some((s1, s2, s3)) = STEPPERS.lock().await.as_mut() {
//        if let Some(v) = x {
//            s1.set_step_per_mm(v)
//        }
//        if let Some(v) = y {
//            s2.set_step_per_mm(v)
//        }
//        if let Some(v) = z {
//            s3.set_step_per_mm(v)
//        }
//    };
//    Ok(())
//}
//pub async fn set_home() -> Result<(), Error> {
//    check_running()?;
//    if let Some((s1, s2, s3)) = STEPPERS.lock().await.as_mut() {
//        s1.set_current_pos(0);
//        s2.set_current_pos(0);
//        s3.set_current_pos(0);
//    };
//    Ok(())
//}
//
//pub fn enable() -> Result<(), Error> {
//    RUNNING
//        .compare_exchange_weak(false, true, Relaxed, Relaxed)
//        .map(|_| ())
//        .map_err(|_| Error::Running)
//}
//
//pub fn disable() -> Result<(), Error> {
//    RUNNING
//        .compare_exchange_weak(true, false, Relaxed, Relaxed)
//        .map(|_| ())
//        .map_err(|_| Error::Running)
//}
//pub async fn add_pos(x: i32, y: i32, z: i32) -> Result<(), ()> {
//    let mut list = POSITIONS.lock().await;
//    list.push(WateringPosition {
//        x,
//        y,
//        z,
//        dur_ms: 1000,
//    })
//    .map_err(|_| ())?;
//    backup(&list).await?;
//    Ok(())
//}
//pub async fn del_pos(id: usize) -> Result<Option<WateringPosition>, ()> {
//    let mut list = POSITIONS.lock().await;
//
//    if let Some(pos) = list.get(id).cloned() {
//        list.remove(id);
//        list[..].sort_unstable();
//        backup(&list).await?;
//        Ok(Some(pos))
//    } else {
//        Ok(None)
//    }
//}
//pub async fn list_pos() -> Vec<WateringPosition, 100> {
//    POSITIONS.lock().await.clone()
//}
//pub fn pump_on() {
//    PUMP_ON.store(true, Relaxed);
//}
//pub fn pump_off() {
//    PUMP_ON.store(false, Relaxed);
//}
