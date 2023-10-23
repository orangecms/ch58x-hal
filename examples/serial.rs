#![no_std]
#![no_main]

use core::arch::{asm, global_asm};
use core::fmt::Write;
use core::writeln;

use embedded_hal_1::delay::DelayUs;
use hal::dma::NoDma;
use hal::gpio::{AnyPin, Input, Level, Output, OutputDrive, Pull};
use hal::interrupt::Interrupt;
use hal::isp::EEPROM_BLOCK_SIZE;
use hal::rtc::{DateTime, Rtc};
use hal::sysctl::Config;
use hal::systick::SysTick;
use hal::uart::UartTx;
use hal::{pac, peripherals, Peripherals};
use {ch58x_hal as hal, panic_halt as _};

const RISCV_BANNER: &str = r"
 ____   ___  ____    ____     __     __
|  _ \ |_ _|/ ___|  / ___|    \ \   / /
| |_) | | | \___ \ | |    _____\ \ / /
|  _ <  | |  ___) || |___|_____|\ V /
|_| \_\|___||____/  \____|       \_/
";

/* see https://five-embeddev.com/riscv-isa-manual/latest/machine.html */
fn machine_info<T: ch58x_hal::uart::BasicInstance>(serial: &mut UartTx<T>) {
    match riscv::register::misa::read() {
        None => {
            writeln!(serial, "ISA unknown").unwrap();
        }
        Some(v) => {
            writeln!(serial, "ISA: {v:x?}").unwrap();
        }
    }

    match riscv::register::mvendorid::read() {
        None => {
            writeln!(serial, "vendor unknown").unwrap();
        }
        Some(v) => {
            writeln!(serial, "vendor: {v:x?}").unwrap();
        }
    }

    match riscv::register::marchid::read() {
        None => {
            writeln!(serial, "arch. ID unknown").unwrap();
        }
        Some(v) => {
            writeln!(serial, "arch. ID: {v:x?}").unwrap();
        }
    }

    match riscv::register::mimpid::read() {
        None => {
            writeln!(serial, "impl. ID unknown").unwrap();
        }
        Some(v) => {
            writeln!(serial, "impl. ID: {v:x?}").unwrap();
        }
    }
}

#[ch32v_rt::entry]
fn main() -> ! {
    // hal::sysctl::Config::pll_60mhz().freeze();
    //hal::sysctl::Config::pll_60mhz().enable_lse().freeze();
    //hal::sysctl::Config::with_lsi_32k().freeze();
    let mut config = hal::Config::default();
    config.clock.use_pll_60mhz().enable_lse();
    let p = hal::init(config);

    let mut delay = SysTick::new(p.SYSTICK);

    // LED PA8
    let mut blue_led = Output::new(p.PA8, Level::Low, OutputDrive::Low);

    let mut serial = UartTx::new(p.UART1, p.PA9, Default::default()).unwrap();
    //let mut serial = UartTx::new(p.UART3, p.PA5, Default::default()).unwrap();
    //let mut serial = UartTx::new(p.UART0, p.PA14, Default::default()).unwrap();
    //let mut serial = UartTx::new(p.UART0, p.PB7, Default::default()).unwrap();

    let download_button = Input::new(p.PB22, Pull::Up);
    let reset_button = Input::new(p.PB23, Pull::Up);

    let mut rtc = Rtc {};
    rtc.set_datatime(DateTime {
        year: 2023,
        month: 10,
        day: 16,
        hour: 15,
        minute: 42,
        second: 10,
    });

    let _ = serial.blocking_flush();
    writeln!(serial, "\n\nHello WCH! 🦀").unwrap();
    writeln!(serial, "Clocks: {}", hal::sysctl::clocks().hclk).unwrap();
    writeln!(serial, "ChipID: {:02x}", hal::signature::get_chip_id()).unwrap();
    let now = rtc.now();
    writeln!(serial, "Boot time: {now} weekday={}", now.isoweekday()).unwrap();

    writeln!(serial, "{RISCV_BANNER}");
    machine_info(&mut serial);

    let now = rtc.now();
    writeln!(serial, "Time: {now} weekday={}", now.isoweekday()).unwrap();

    writeln!(serial, "day {:?}", rtc.counter_day()).unwrap();
    writeln!(serial, "2s {:?}", rtc.counter_2s()).unwrap();

    loop {
        // serial.blocking_flush();
        if download_button.is_low() {
            blue_led.set_low();
        } else {
            blue_led.set_high();
        }

        // FIXME: systick counter is not increasing, delay has no effect
        // writeln!(serial, "tick! {}", SysTick::now()).unwrap();
        delay.delay_ms(1000);
    }
}
