use defmt::info;
use embassy_stm32::{i2c::I2c, peripherals::I2C1};

pub struct Storage {
    i2c: I2c<'static, I2C1>,
}

impl Storage {
    pub fn new(i2c: I2c<'static, I2C1>) -> Self {
        Self { i2c }
    }
    pub fn write_page(&mut self, idx: u8, page: [u8; 32]) -> Result<(), ()> {
        info!("write page {}", idx);
        let mut buf = [0; 34];
        buf[..2].copy_from_slice(&((idx as u16) << 5).to_be_bytes());
        buf[2..].copy_from_slice(&page);
        self.i2c.blocking_write(0x50, &buf[..]).map_err(|_| ())?;
        info!("write page {} success {:?}", idx, buf[2..]);
        Ok(())
    }
    pub fn read_page(&mut self, idx: u8) -> Result<[u8; 32], ()> {
        info!("read page {}", idx);
        let mut buf = [0; 32];
        self.i2c
            .blocking_write_read(0x50, &((idx as u16) << 5).to_be_bytes(), &mut buf[..])
            .map_err(|_| ())?;
        info!("read page {} success {:?}", idx, buf);
        Ok(buf)
    }
}
