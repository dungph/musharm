use core::sync::atomic::AtomicBool;
use core::sync::atomic::Ordering::Relaxed;

use crate::command::{self, Cmd, Set, UnsignSet};
use crate::stepper::Stepper;
use defmt::info;
use embassy_stm32::i2c::I2c;
use embassy_stm32::peripherals::I2C1;
use embassy_sync::mutex::Mutex;
use embassy_sync::signal::Signal;
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex as Raw, channel::Channel};
use embassy_time::{Duration, Timer};
use futures::future::join;
use heapless::Vec;
use serde::{Deserialize, Serialize};

type Steppers = (Stepper<'static>, Stepper<'static>, Stepper<'static>);
static STEPPERS: Mutex<Raw, Option<Steppers>> = Mutex::new(None);
static STORAGE: Mutex<Raw, Option<I2c<'static, I2C1>>> = Mutex::new(None);
static POSITIONS: Mutex<Raw, Vec<WateringPosition, 100>> = Mutex::new(Vec::new());

static RUNNING: AtomicBool = AtomicBool::new(true);

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct WateringPosition {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub dur_ms: u32,
}

pub enum Error {
    Running,
    Uninit,
}

#[embassy_executor::task]
pub async fn run(
    x: Stepper<'static>,
    y: Stepper<'static>,
    z: Stepper<'static>,
    mut i2c: I2c<'static, I2C1>,
) {
    STEPPERS.lock().await.replace((x, y, z));
    STORAGE.lock().await.replace(i2c);
    if let Ok(list) = restore().await {
        *POSITIONS.lock().await = list;
    } else {
        info!("Restore Error");
    }
    loop {
        let mut pos_id = 0;
        while let Some(pos) = POSITIONS.lock().await.get(pos_id) {
            if RUNNING.load(Relaxed) {
                if let Some((x, y, z)) = STEPPERS.lock().await.as_mut() {
                    join(x.goto(pos.x), y.goto(pos.y)).await;
                }
                pos_id += 1;
            } else {
                break;
            }
            Timer::after(Duration::from_millis(1000)).await;
        }
        while !RUNNING.load(Relaxed) {
            Timer::after(Duration::from_millis(1000)).await;
        }
        Timer::after(Duration::from_millis(1000)).await;
    }
}

const ADD: [u8; 2] = 64u16.to_be_bytes();

async fn restore() -> Result<Vec<WateringPosition, 100>, ()> {
    let mut buf = [0; 2048];
    if let Some(i2c) = STORAGE.lock().await.as_mut() {
        i2c.blocking_write_read(0x50, &ADD, &mut buf)
            .map_err(|_| ())?;
        info!("i2c read ok: {:?}", &buf[..32]);
        if let Ok(vec) = postcard::from_bytes::<Vec<WateringPosition, 100>>(&buf) {
            info!("postcard ok");

            for p in vec.iter() {
                info!("restore {}, {}, {}", p.x, p.y, p.z);
            }
            Ok(vec)
        } else {
            info!("postcard err");
            Err(())
        }
    } else {
        Err(())
    }
}
async fn backup(list: &Vec<WateringPosition, 100>) -> Result<(), ()> {
    for p in list.iter() {
        info!("backup {}, {}, {}", p.x, p.y, p.z);
    }
    let mut buf = [0; 2050];
    buf[..2].copy_from_slice(&ADD);

    if let Ok(_bufout) = postcard::to_slice(list, &mut buf[2..]) {
        let len = _bufout.len();
        if let Some(i2c) = STORAGE.lock().await.as_mut() {
            info!("backup writing {:?}", &buf[..34]);
            info!(
                "{:?}",
                i2c.blocking_write(0x50, &buf[..len + 2]).map_err(|_| ())
            );
            info!("backup write ok");
            Timer::after(Duration::from_millis(100)).await;
            info!(
                "{:?}",
                i2c.blocking_write_read(0x50, &ADD, &mut buf[..])
                    .map_err(|_| ())
            );
            info!("checking {:?}", &buf[..34]);
            Ok(())
        } else {
            Err(())
        }
    } else {
        Err(())
    }
}
fn check_running() -> Result<(), Error> {
    if RUNNING.load(Relaxed) {
        info!("is running");
        Err(Error::Running)
    } else {
        info!("is not running");
        Ok(())
    }
}

pub async fn r#move(x: Option<i32>, y: Option<i32>, z: Option<i32>) -> Result<(), Error> {
    check_running()?;
    if let Some((ref mut s1, ref mut s2, ref mut s3)) = STEPPERS.lock().await.as_mut() {
        info!("move {:?}, {:?}, {:?}", x, y, z);
        futures::future::join3(
            s1.r#move(x.unwrap_or(0)),
            s2.r#move(y.unwrap_or(0)),
            s3.r#move(z.unwrap_or(0)),
        )
        .await;
        Ok(())
    } else {
        Err(Error::Uninit)
    }
}

pub async fn goto(x: Option<i32>, y: Option<i32>, z: Option<i32>) -> Result<(), Error> {
    check_running()?;
    if let Some((ref mut s1, ref mut s2, ref mut s3)) = STEPPERS.lock().await.as_mut() {
        futures::future::join3(
            s1.goto(x.unwrap_or(s1.current_pos())),
            s2.goto(y.unwrap_or(s2.current_pos())),
            s3.goto(z.unwrap_or(s3.current_pos())),
        )
        .await;
        Ok(())
    } else {
        Err(Error::Uninit)
    }
}

pub async fn set_speed_min(x: Option<u32>, y: Option<u32>, z: Option<u32>) -> Result<(), Error> {
    check_running()?;
    STEPPERS.lock().await.as_mut().map(|(s1, s2, s3)| {
        x.map(|v| s1.set_speed_min(v));
        y.map(|v| s2.set_speed_min(v));
        z.map(|v| s3.set_speed_min(v));
    });
    Ok(())
}

pub async fn set_speed_max(x: Option<u32>, y: Option<u32>, z: Option<u32>) -> Result<(), Error> {
    check_running()?;
    STEPPERS.lock().await.as_mut().map(|(s1, s2, s3)| {
        x.map(|v| s1.set_speed_max(v));
        y.map(|v| s2.set_speed_max(v));
        z.map(|v| s3.set_speed_max(v));
    });
    Ok(())
}
pub async fn set_accel(x: Option<u32>, y: Option<u32>, z: Option<u32>) -> Result<(), Error> {
    check_running()?;
    STEPPERS.lock().await.as_mut().map(|(s1, s2, s3)| {
        x.map(|v| s1.set_speed_accel(v));
        y.map(|v| s2.set_speed_accel(v));
        z.map(|v| s3.set_speed_accel(v));
    });
    Ok(())
}
pub async fn set_step_per_mm(x: Option<u32>, y: Option<u32>, z: Option<u32>) -> Result<(), Error> {
    check_running()?;
    STEPPERS.lock().await.as_mut().map(|(s1, s2, s3)| {
        x.map(|v| s1.set_step_per_mm(v));
        y.map(|v| s2.set_step_per_mm(v));
        z.map(|v| s3.set_step_per_mm(v));
    });
    Ok(())
}
pub async fn set_home() -> Result<(), Error> {
    check_running()?;
    STEPPERS.lock().await.as_mut().map(|(s1, s2, s3)| {
        s1.set_current_pos(0);
        s2.set_current_pos(0);
        s3.set_current_pos(0);
    });
    Ok(())
}

pub fn enable() -> Result<(), Error> {
    RUNNING
        .compare_exchange_weak(false, true, Relaxed, Relaxed)
        .map(|_| ())
        .map_err(|_| Error::Running)
}

pub fn disable() -> Result<(), Error> {
    RUNNING
        .compare_exchange_weak(true, false, Relaxed, Relaxed)
        .map(|_| ())
        .map_err(|_| Error::Running)
}
pub async fn add_pos(x: i32, y: i32, z: i32) -> Result<(), ()> {
    let mut list = POSITIONS.lock().await;
    list.push(WateringPosition { x, y, z, dur_ms: 0 })
        .map_err(|_| ())?;
    backup(&list).await?;
    Ok(())
}
pub async fn del_pos(id: usize) -> Option<WateringPosition> {
    let mut list = POSITIONS.lock().await;

    if let Some(pos) = list.get(id).cloned() {
        list.remove(id);
        list.sort_unstable();
        backup(&list).await;
        Some(pos)
    } else {
        None
    }
}
pub async fn list_pos() -> Vec<WateringPosition, 100> {
    POSITIONS.lock().await.clone()
}
