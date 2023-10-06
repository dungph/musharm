use embassy_stm32::gpio::{AnyPin, Output};

pub struct Pump {
    pin: Output<'static, AnyPin>,
}

impl Pump {
    pub fn new(pin: Output<'static, AnyPin>) -> Self {
        Self { pin }
    }
    pub fn on(&mut self) {
        self.pin.set_high();
    }
    pub fn off(&mut self) {
        self.pin.set_low();
    }
}
