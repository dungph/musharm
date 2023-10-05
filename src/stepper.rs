use embassy_stm32::gpio::{AnyPin, Output};
use embassy_time::{Duration, Timer};

pub struct Stepper<'a> {
    dir_pin: Output<'a, AnyPin>,
    step_pin: Output<'a, AnyPin>,
    current_pos: i32,
    step_per_mm: u32,
    speed_min: u32,
    speed_max: u32,
    speed_accel: u32,
}

impl<'a> Stepper<'a> {
    pub fn new(dir_pin: Output<'a, AnyPin>, step_pin: Output<'a, AnyPin>) -> Self {
        Stepper {
            dir_pin,
            step_pin,
            current_pos: 0,
            step_per_mm: 20,
            speed_min: 10,
            speed_max: 250,
            speed_accel: 50,
        }
    }
    pub async fn goto(&mut self, pos: i32) {
        let diff = pos - self.current_pos;
        match diff {
            i32::MIN..=-1 => {
                self.dir_pin.set_low();
            }
            1..=i32::MAX => {
                self.dir_pin.set_high();
            }
            _ => (),
        }
        step_move(
            &mut self.step_pin,
            diff.unsigned_abs() * self.step_per_mm,
            self.speed_min * self.step_per_mm,
            self.speed_max * self.step_per_mm,
            self.speed_accel * self.step_per_mm,
        )
        .await;
        self.current_pos = pos;
    }
    pub async fn r#move(&mut self, distance: i32) {
        self.goto(self.current_pos + distance).await;
    }

    pub fn current_pos(&self) -> i32 {
        self.current_pos
    }

    pub fn set_current_pos(&mut self, current_pos: i32) {
        self.current_pos = current_pos;
    }

    pub fn speed_max(&self) -> u32 {
        self.speed_max
    }

    pub fn set_speed_max(&mut self, speed_max: u32) {
        self.speed_max = speed_max;
    }
    pub fn speed_min(&self) -> u32 {
        self.speed_min
    }

    pub fn set_speed_min(&mut self, speed_min: u32) {
        self.speed_min = speed_min;
    }
    pub fn speed_accel(&self) -> u32 {
        self.speed_accel
    }

    pub fn set_speed_accel(&mut self, speed_accel: u32) {
        self.speed_accel = speed_accel;
    }
    pub fn step_per_mm(&self) -> u32 {
        self.step_per_mm
    }

    pub fn set_step_per_mm(&mut self, step_per_mm: u32) {
        self.step_per_mm = step_per_mm;
    }
}

pub async fn step_move(
    step_pin: &mut Output<'_, AnyPin>,
    step: u32,
    min_sps: u32,
    max_sps: u32,
    accel: u32,
) {
    let mut sps = min_sps as f32;
    let mut step_count = 0;
    let mid_step = step / 2;
    while (sps < max_sps as f32) && step_count < mid_step {
        let period = 1f32 / sps;
        step_pin.set_high();
        Timer::after(Duration::from_micros(10)).await;
        step_pin.set_low();
        Timer::after(Duration::from_micros((period * 1_000_000f32) as u64)).await;
        sps += accel as f32 * period;
        step_count += 1;
    }

    if step_count < mid_step {
        let period = 1f32 / sps;
        for _ in 0..step_count * 2 {
            step_pin.set_high();
            Timer::after(Duration::from_micros(10)).await;
            step_pin.set_low();
            Timer::after(Duration::from_micros((period * 1_000_000f32) as u64)).await;
            step_count += 1;
        }
    }
    while step_count < step {
        let period = 1f32 / sps;
        step_pin.set_high();
        Timer::after(Duration::from_micros(10)).await;
        step_pin.set_low();
        Timer::after(Duration::from_micros((period * 1_000_000f32) as u64)).await;
        sps -= accel as f32 * period;
        step_count += 1;
    }
}
