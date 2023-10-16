#![no_std]
#![no_main]

use core::arch::{asm, global_asm};
use core::fmt::Write;
use core::writeln;

use embedded_hal_alpha::delay::DelayUs;
use hal::dma::NoDma;
use hal::gpio::{AnyPin, Input, Level, Output, OutputDrive, Pull};
use hal::interrupt::Interrupt;
use hal::isp::EEPROM_BLOCK_SIZE;
use hal::rtc::{DateTime, Rtc};
use hal::sysctl::Config;
use hal::systick::SysTick;
use hal::uart::{Uart, UartTx};
use hal::{pac, peripherals, Peripherals};
use {ch58x_hal as hal, panic_halt as _};

#[ch32v_rt::entry]
fn main() -> ! {
    // LED PA8
    // hal::sysctl::Config::pll_60mhz().freeze();
    hal::sysctl::Config::pll_60mhz().use_lse().freeze();

    let p = Peripherals::take();

    let mut delay = SysTick::new(p.SYSTICK);

    let mut serial = UartTx::new(p.UART1, p.PA9, NoDma, Default::default()).unwrap();
    //let mut serial = UartTx::new(p.UART3, p.PA5, NoDma, Default::default()).unwrap();

    //let mut serial = UartTx::new(p.UART1, p.PB13, NoDma, Default::default()).unwrap();

    let mut blue_led = Output::new(p.PA8, Level::Low, OutputDrive::Low);

    let mut download_button = Input::new(p.PB22, Pull::Up);
    let mut reset_button = Input::new(p.PB23, Pull::Up);
    let mut rtc = Rtc {};

    //      rtc.set_datatime(DateTime {
    //        year: 2023,
    //        month: 10,
    //        day: 16,
    //        hour: 15,
    //        minute: 42,
    //        second: 10,
    //    });

    writeln!(serial, "\n\n\nHello World!").unwrap();
    writeln!(serial, "Clocks: {}", hal::sysctl::clocks().hclk).unwrap();
    writeln!(serial, "ChipID: {:02x}", hal::signature::get_chip_id());
    let now = rtc.now();
    writeln!(serial, "Boot time: {} weekday={}", now, now.isoweekday()).unwrap();

    loop {
        blue_led.toggle();

        // writeln!(uart, "day {:?}", rtc.counter_day()).unwrap();
        // writeln!(uart, "2s {:?}", rtc.counter_2s()).unwrap();

        //  writeln!(uart, "tick! {}", SysTick::now()).unwrap();
        delay.delay_ms(300);

        let now = rtc.now();
        writeln!(
            serial,
            "{}: weekday={}, button: download={} reset={}",
            now,
            now.isoweekday(),
            download_button.is_low(),
            reset_button.is_low()
        )
        .unwrap();
        //writeln!(serial, "Current time: {} weekday={}", now, now.isoweekday()).unwrap();
        //writeln!(serial, "button: {} {}", ).unwrap();
    }
}