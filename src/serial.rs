use defmt::info;
use embassy_stm32::{
    bind_interrupts, peripherals,
    usb_otg::{Driver, Instance},
};
use embassy_time::{Duration, Timer};
use embassy_usb::{
    class::cdc_acm::{CdcAcmClass, State},
    driver::EndpointError,
    Builder,
};
use futures::future::join;
use heapless::Vec;

use crate::controller::{self};

bind_interrupts!(struct Irqs {
    OTG_FS => embassy_stm32::usb_otg::InterruptHandler<peripherals::USB_OTG_FS>;
});

#[embassy_executor::task]
pub async fn serial(
    usb: peripherals::USB_OTG_FS,
    pa12: peripherals::PA12,
    pa11: peripherals::PA11,
) {
    // Create the driver, from the HAL.
    let mut ep_out_buffer = [0u8; 256];
    let mut config = embassy_stm32::usb_otg::Config::default();
    config.vbus_detection = false;
    let driver = Driver::new_fs(usb, Irqs, pa12, pa11, &mut ep_out_buffer, config);

    let mut config = embassy_usb::Config::new(0xc0de, 0xcafe);
    config.manufacturer = Some("Embassy");
    config.product = Some("USB-serial example");
    config.serial_number = Some("12345678");

    // Required for windows compatibility.
    // https://developer.nordicsemi.com/nRF_Connect_SDK/doc/1.9.1/kconfig/CONFIG_CDC_ACM_IAD.html#help
    config.device_class = 0xEF;
    config.device_sub_class = 0x02;
    config.device_protocol = 0x01;
    config.composite_with_iads = true;

    // Create embassy-usb DeviceBuilder using the driver and config.
    // It needs some buffers for building the descriptors.
    let mut device_descriptor = [0; 256];
    let mut config_descriptor = [0; 256];
    let mut bos_descriptor = [0; 256];
    let mut control_buf = [0; 64];

    let mut state = State::new();

    let mut builder = Builder::new(
        driver,
        config,
        &mut device_descriptor,
        &mut config_descriptor,
        &mut bos_descriptor,
        &mut control_buf,
    );

    // Create classes on the builder.
    let mut class = CdcAcmClass::new(&mut builder, &mut state, 64);

    // Build the builder.
    let mut usb = builder.build();

    // Run the USB device.
    let usb_fut = usb.run();

    // Do stuff with the class!
    let echo_fut = async {
        loop {
            class.wait_connection().await;
            info!("Connected");
            let _ = handle(&mut class).await;
            info!("Disconnected");
        }
    };

    join(usb_fut, echo_fut).await;
}
struct Disconnected {}

impl From<EndpointError> for Disconnected {
    fn from(val: EndpointError) -> Self {
        match val {
            EndpointError::BufferOverflow => panic!("Buffer overflow"),
            EndpointError::Disabled => Disconnected {},
        }
    }
}

async fn handle<'d, T: Instance + 'd>(
    class: &mut CdcAcmClass<'d, Driver<'d, T>>,
) -> Result<(), Disconnected> {
    let mut buf = [0; 64];
    let mut sbuf = Vec::<u8, 32>::new();
    Timer::after(Duration::from_millis(100)).await;

    loop {
        let n = class.read_packet(&mut buf).await?;
        for b in &buf[..n] {
            match *b {
                b'\x7f' | b'\x08' => {
                    if sbuf.pop().is_some() {
                        class.write_packet(b"\x08\x1b[K").await?;
                    }
                }
                b'\x0d' | b'\x0a' => {
                    if let Ok(st) = core::str::from_utf8(&sbuf) {
                        info!("data: {}", st);
                        class.write_packet(b"\x0A\x0D").await?;
                        if let Ok((_, cmd)) = crate::command::parse_cmd(st) {
                            let ret = controller::send_msg(cmd).await;
                            for c in ret.as_bytes() {
                                if *c == b'\n' {
                                    class.write_packet(&[b'\r', b'\n']).await?;
                                } else {
                                    class.write_packet(&[*c]).await?;
                                }
                            }
                            class.write_packet(b"[OK]\x0A\x0D").await?;
                        } else {
                            class.write_packet(b"[Parse fail]\x0A\x0D").await?;
                        }
                    }
                    sbuf = Vec::new();
                }
                b'0'..=b'9' | b' ' | b'a'..=b'z' | b'A'..=b'Z' | b'-' => {
                    sbuf.push(*b).ok();
                    class.write_packet(&[*b]).await?;
                }
                _ => {}
            }
        }
    }
}
