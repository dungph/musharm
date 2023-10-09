use crate::command::Cmd;
use crate::{pump::Pump, stepper::Stepper, storage::Storage};
use core::fmt::Write;
use defmt::info;
use embassy_sync::signal::Signal;
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex as Raw, channel::Channel};
use embassy_time::{Duration, Timer};
use futures::future::join;
use heapless::{String, Vec};
use serde::{Deserialize, Serialize};

static CH: Channel<Raw, Cmd, 10> = Channel::new();
static CH_R: Signal<Raw, String<5000>> = Signal::new();

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct WateringPosition {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub dur_ms: u32,
}

pub async fn send_msg(cmd: Cmd) -> String<5000> {
    CH.send(cmd).await;
    CH_R.wait().await
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
    let mut repeat_duration = Duration::from_secs(1);
    let default_dur_ms = 1000;

    if let Ok(list) = restore(&mut storage).await {
        positions = list;
        info!("Restored");
    } else {
        info!("Restore Error");
    }

    loop {
        while schedule_enabled {
            pump.off();
            info!("repeat");
            Timer::after(repeat_duration).await;
            match CH.try_receive() {
                Ok(Cmd::Stop) => {
                    CH_R.signal(String::new());
                    schedule_enabled = false;
                    break;
                }
                Ok(_) => {
                    CH_R.signal(String::new());
                }
                Err(_) => {}
            }
            for pos in positions.iter() {
                match CH.try_receive() {
                    Ok(Cmd::Stop) => {
                        CH_R.signal(String::new());
                        schedule_enabled = false;
                        break;
                    }
                    Ok(_) => {
                        CH_R.signal(String::new());
                    }
                    Err(_) => {}
                }
                join(x.goto(pos.x), y.goto(pos.y)).await;
                z.goto(pos.z).await;
                pump.on();
                Timer::after(Duration::from_millis(pos.dur_ms.into())).await;
                pump.off();
                z.goto(0).await;
            }
        }

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
                Cmd::AddPos(val, dur) => {
                    if let Some(x) = val.x {
                        if let Some(y) = val.y {
                            if let Some(z) = val.z {
                                positions
                                    .push(WateringPosition {
                                        x,
                                        y,
                                        z,
                                        dur_ms: dur.unwrap_or(default_dur_ms),
                                    })
                                    .ok();
                                positions[..].sort_unstable();
                                backup(&mut storage, &positions).await;
                            }
                        }
                    }
                }
                Cmd::WaterDuration(id, dur) => {
                    if let Some(id) = id {
                        if let Some(ref mut pos) = positions.get_mut(id as usize) {
                            pos.dur_ms = dur;
                        }
                    } else {
                        for pos in positions.iter_mut() {
                            pos.dur_ms = dur;
                        }
                    }
                    backup(&mut storage, &positions).await;
                }
                Cmd::DelPos(id) => {
                    if let Some(_pos) = positions.get(id as usize).cloned() {
                        positions.remove(id as usize);
                        backup(&mut storage, &positions).await;
                    }
                }
                Cmd::RepeatDur(dur) => {
                    repeat_duration = Duration::from_millis(dur.into());
                }
                Cmd::PumpOn => {
                    pump.on();
                }
                Cmd::PumpOff => {
                    pump.off();
                }
                Cmd::ListPos => {
                    let mut buf = String::<5000>::new();
                    for (id, pos) in positions.iter().enumerate() {
                        writeln!(
                            &mut buf,
                            "{:2}: ({:4}, {:4}, {:4}) {:5}ms",
                            id, pos.x, pos.y, pos.z, pos.dur_ms
                        )
                        .ok();
                    }
                    CH_R.signal(buf);
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
                Cmd::Help => {
                    let help = include_str!("./help.txt");
                    CH_R.signal(String::try_from(help).unwrap());
                }
            }
            if CH_R.signaled() {
            } else {
                CH_R.signal(String::try_from("").unwrap());
            }
        }
    }
}

async fn restore(sto: &mut Storage) -> Result<Vec<WateringPosition, 100>, ()> {
    let mut size = 0;
    Timer::after(Duration::from_millis(100)).await;
    if let Ok(page) = sto.read_page(0) {
        size = page[0];
    }

    let list = (1..=size)
        .filter_map(|idx| sto.read_page(idx).ok())
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
