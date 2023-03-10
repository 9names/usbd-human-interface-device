#![no_std]
#![no_main]

use bsp::entry;
use bsp::hal;
use defmt::*;
use defmt_rtt as _;
use embedded_hal::digital::v2::*;
use embedded_hal::prelude::*;
use fugit::ExtU32;
use hal::pac;
use panic_probe as _;
use usb_device::class_prelude::*;
use usb_device::prelude::*;
use usbd_human_interface_device::device::joystick::JoystickReport;
use usbd_human_interface_device::prelude::*;

use rp_pico as bsp;

#[entry]
fn main() -> ! {
    let mut pac = pac::Peripherals::take().unwrap();

    let mut watchdog = hal::Watchdog::new(pac.WATCHDOG);
    let clocks = hal::clocks::init_clocks_and_plls(
        bsp::XOSC_CRYSTAL_FREQ,
        pac.XOSC,
        pac.CLOCKS,
        pac.PLL_SYS,
        pac.PLL_USB,
        &mut pac.RESETS,
        &mut watchdog,
    )
    .ok()
    .unwrap();

    let timer = hal::Timer::new(pac.TIMER, &mut pac.RESETS);

    let sio = hal::Sio::new(pac.SIO);
    let pins = hal::gpio::Pins::new(
        pac.IO_BANK0,
        pac.PADS_BANK0,
        sio.gpio_bank0,
        &mut pac.RESETS,
    );

    info!("Starting");

    //USB
    let usb_bus = UsbBusAllocator::new(hal::usb::UsbBus::new(
        pac.USBCTRL_REGS,
        pac.USBCTRL_DPRAM,
        clocks.usb_clock,
        true,
        &mut pac.RESETS,
    ));

    let mut joy = UsbHidClassBuilder::new()
        .add_interface(
            usbd_human_interface_device::device::joystick::JoystickInterface::default_config(),
        )
        .build(&usb_bus);

    //https://pid.codes
    let mut usb_dev = UsbDeviceBuilder::new(&usb_bus, UsbVidPid(0x1209, 0x0001))
        .manufacturer("usbd-human-interface-device")
        .product("Rusty joystick")
        .serial_number("TEST")
        .build();

    //GPIO pins
    let mut led_pin = pins.gpio13.into_push_pull_output();

    let keys: &[&dyn InputPin<Error = core::convert::Infallible>] = &[
        &pins.gpio0.into_pull_up_input(),
        &pins.gpio1.into_pull_up_input(),
        &pins.gpio2.into_pull_up_input(),
        &pins.gpio3.into_pull_up_input(),
        &pins.gpio4.into_pull_up_input(),
        &pins.gpio5.into_pull_up_input(),
        &pins.gpio6.into_pull_up_input(),
        &pins.gpio7.into_pull_up_input(),
        &pins.gpio8.into_pull_up_input(),
        &pins.gpio9.into_pull_up_input(),
        &pins.gpio10.into_pull_up_input(),
        &pins.gpio11.into_pull_up_input(),
    ];

    led_pin.set_low().ok();

    let mut report = JoystickReport::default();

    let mut input_count_down = timer.count_down();
    input_count_down.start(10.millis());

    loop {
        // Poll every 10ms
        if input_count_down.wait().is_ok() {
            report = update_report(report, keys);

            match joy.interface().write_report(&report) {
                Err(UsbHidError::WouldBlock) => {}
                Ok(_) => {
                    report = JoystickReport::default();
                }
                Err(e) => {
                    core::panic!("Failed to write joystick report: {:?}", e)
                }
            }
        }

        if usb_dev.poll(&mut [&mut joy]) {}
    }
}

fn update_report(
    mut report: JoystickReport,
    keys: &[&dyn InputPin<Error = core::convert::Infallible>],
) -> JoystickReport {
    // Read out 8 buttons first
    // Clippy wants us to use an iterator here but it hurts readability
    #[allow(clippy::needless_range_loop)]
    for button in 0..=7 {
        if keys[button].is_low().unwrap() {
            report.buttons |= 1 << button;
        } else {
            report.buttons &= 0xFF - (1 << button);
        }
    }

    // We're using digital switches in a D-PAD style configuration
    //    10
    //  8    9
    //    11
    // These are mapped to the limits of an axis
    if keys[8].is_low().unwrap() {
        report.x = -127; // left
    } else if keys[9].is_low().unwrap() {
        report.x = 127; // right
    } else {
        report.x = 0; // center
    }
    if keys[10].is_low().unwrap() {
        report.y = -127; // up
    } else if keys[11].is_low().unwrap() {
        report.y = 127; // down
    } else {
        report.y = 0; // center
    }

    report
}