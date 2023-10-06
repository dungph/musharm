#![no_std]
#![no_main]
#![feature(type_alias_impl_trait)]

mod command;
mod controller;
mod pump;
mod serial;
mod stepper;
mod storage;

use defmt_rtt as _;
use embassy_executor::Spawner;
use embassy_stm32::dma::NoDma;
use embassy_stm32::gpio::{Level, Output, Speed};
use embassy_stm32::time::Hertz;
use embassy_stm32::{bind_interrupts, peripherals, Config};
use embassy_time::{Duration, Timer};
use panic_probe as _;
use storage::Storage;

use crate::stepper::Stepper;

bind_interrupts!(struct Irqs {
    I2C1_EV => embassy_stm32::i2c::InterruptHandler<peripherals::I2C1>;
});

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let mut config = Config::default();
    config.rcc.hse = Some(Hertz(25_000_000));
    config.rcc.sys_ck = Some(Hertz(48_000_000));
    config.rcc.pclk1 = Some(Hertz(24_000_000));
    let mut p = embassy_stm32::init(config);

    let mut led = Output::new(p.PC13, Level::High, Speed::Low);

    let mut pump_pin = embassy_stm32::gpio::Output::new(p.PA0, Level::Low, Speed::Medium).degrade();

    let step_pin1 = Output::new(p.PA2, Level::Low, Speed::Medium).degrade();
    let step_pin2 = Output::new(p.PA3, Level::Low, Speed::Medium).degrade();
    let step_pin3 = Output::new(p.PA4, Level::Low, Speed::Medium).degrade();
    let dir_pin1 = Output::new(p.PA5, Level::Low, Speed::Medium).degrade();
    let dir_pin2 = Output::new(p.PA6, Level::Low, Speed::Medium).degrade();
    let dir_pin3 = Output::new(p.PA7, Level::Low, Speed::Medium).degrade();

    {
        // BluePill board has a pull-up resistor on the D+ line.
        // Pull the D+ pin down to send a RESET condition to the USB bus.
        // This forced reset is needed only for development, without it host
        // will not reset your device when you upload new firmware.
        let _dp = Output::new(&mut p.PA12, Level::Low, Speed::Low);
        Timer::after(Duration::from_millis(10)).await;
    }

    _spawner.must_spawn(serial::serial(p.USB_OTG_FS, p.PA12, p.PA11));

    let mut i2c_cfg = embassy_stm32::i2c::Config::default();
    i2c_cfg.sda_pullup = true;
    i2c_cfg.scl_pullup = true;
    let i2c = embassy_stm32::i2c::I2c::new(
        p.I2C1,
        p.PB8,
        p.PB7,
        Irqs,
        NoDma,
        NoDma,
        Hertz(400_000),
        i2c_cfg,
    );

    _spawner.must_spawn(controller::run(
        Stepper::new(dir_pin1, step_pin1),
        Stepper::new(dir_pin2, step_pin2),
        Stepper::new(dir_pin3, step_pin3),
        Storage::new(i2c),
        pump::Pump::new(pump_pin),
    ));

    loop {
        led.toggle();
        Timer::after(Duration::from_millis(1000)).await;
    }
}
