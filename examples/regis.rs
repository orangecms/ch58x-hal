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

const INIT: &str = "\x1bP1p"; // turn on graphics
const FINI: &str = "\x1b\\"; // exit graphics
const CLEAR: &str = "S(E)";

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
            writeln!(serial, "ISA: {v:08x?}").unwrap();
        }
    }

    match riscv::register::mvendorid::read() {
        None => {
            writeln!(serial, "vendor unknown").unwrap();
        }
        Some(v) => {
            writeln!(serial, "vendor: {v:08x?}").unwrap();
        }
    }

    match riscv::register::marchid::read() {
        None => {
            writeln!(serial, "arch. ID unknown").unwrap();
        }
        Some(v) => {
            writeln!(serial, "arch. ID: {v:08x?}").unwrap();
        }
    }

    match riscv::register::mimpid::read() {
        None => {
            writeln!(serial, "impl. ID unknown").unwrap();
        }
        Some(v) => {
            writeln!(serial, "impl. ID: {v:08x?}").unwrap();
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

    delay.delay_ms(2000);
    writeln!(serial, "Let's go ReGIS!");

    delay.delay_ms(2000);
    write!(serial, "{INIT}");
    write!(serial, "{CLEAR}");
    write!(serial, "W(I0)");
    write!(serial, "P[0,0]V(W(S1))(B)[+300,][,+36][-300,](E)");
    write!(serial, "P[20,6]W(I(W))T(S02)\"ReGIS demo :)\"");
    write!(serial, "{FINI}");

    write!(serial, "{INIT}");
    write!(serial, "{CLEAR}");
    write!(serial, "W(I(Y))");
    // top rectangle, but not full size for circular cut to easily work
    write!(serial, "P[150,39]V(W(S1))(B)[+60,][,+30][-60,](E)");
    // triangle below rectangle
    write!(serial, "P[114,70]V(W(S1))(B)[+58,+58][+37,-58][-95,](E)");
    // hack: clear circle ;)
    write!(serial, "W(I0)");
    write!(serial, "P[144,70]C(W(S1))[+31]C(W(S1,E))[+0]");
    // blue parts
    write!(serial, "W(I(B))");
    write!(serial, "P[140,70]C(W(S1))[+20]C(W(S1,E))[+0]");
    write!(serial, "P[120,50]V(W(S1))(B)[+25,][,+40][-25,](E)");
    write!(serial, "P[114,50]V(W(S1))(B)[+6,][,+90][-6,](E)");
    write!(serial, "P[120,140]V(W(S1))(B)[,-40][+40,+40][-40,](E)");
    write!(serial, "P[210,140]V(W(S1))(B)[,-40][-25,+40][+25,](E)");
    write!(serial, "P[80,160]W(I(B))T(S03)\"RISC\"W(I(Y))T(S03)\"-V\"");
    write!(serial, "{FINI}");

    // serial.blocking_flush();
    delay.delay_ms(5000);

    loop {
        if download_button.is_low() {
            blue_led.set_low();
        } else {
            blue_led.set_high();
        }

        let tick = SysTick::now();
        writeln!(serial, "tick! {}", tick).unwrap();
        delay.delay_ms(1000);
    }
}
