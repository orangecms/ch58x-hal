/*!
    GPIO_ModeIN_Floating, //浮空输入
    GPIO_ModeIN_PU,       //上拉输入
    GPIO_ModeIN_PD,       //下拉输入
    GPIO_ModeOut_PP_5mA,  //推挽输出最大5mA
    GPIO_ModeOut_PP_20mA, //推挽输出最大20mA
*/

#![macro_use]

use crate::{impl_peripheral, into_ref, pac, peripherals, Peripheral, PeripheralRef};

/// GPIO flexible pin.
///
/// This pin can either be a disconnected, input, or output pin, or both. The level register bit will remain
/// set while not in output mode, so the pin's level will be 'remembered' when it is not in output
/// mode.
pub struct Flex<'d, T: Pin> {
    pub(crate) pin: PeripheralRef<'d, T>,
}

impl<'d, T: Pin> Flex<'d, T> {
    /// Wrap the pin in a `Flex`.
    ///
    /// The pin remains disconnected. The initial output level is unspecified, but can be changed
    /// before the pin is put into output mode.
    ///
    #[inline]
    pub fn new(pin: impl Peripheral<P = T> + 'd) -> Self {
        into_ref!(pin);
        // Pin will be in disconnected state.
        Self { pin }
    }

    #[inline]
    pub fn degrade(self) -> Flex<'d, AnyPin> {
        // Safety: We are about to drop the other copy of this pin, so
        // this clone is safe.
        let pin = unsafe { self.pin.clone_unchecked() };

        // We don't want to run the destructor here, because that would
        // deconfigure the pin.
        core::mem::forget(self);

        Flex {
            pin: pin.map_into::<AnyPin>(),
        }
    }

    /// Put the pin into input mode.
    #[inline]
    pub fn set_as_input(&mut self, pull: Pull) {
        let n = self.pin.pin();
        let rb = self.pin.block();
        critical_section::with(|_| {
            match pull {
                Pull::None => unsafe {
                    // In_floating
                    rb.pd_drv.modify(|r, w| w.bits(r.bits() & !(1 << n)));
                    rb.pu.modify(|r, w| w.bits(r.bits() & !(1 << n)));
                    rb.dir.modify(|r, w| w.bits(r.bits() & !(1 << n)));
                },
                Pull::Up => unsafe {
                    // In_PU
                    rb.pd_drv.modify(|r, w| w.bits(r.bits() & !(1 << n)));
                    rb.pu.modify(|r, w| w.bits(r.bits() | (1 << n)));
                    rb.dir.modify(|r, w| w.bits(r.bits() & !(1 << n)));
                },
                Pull::Down => unsafe {
                    // In_PD
                    rb.pd_drv.modify(|r, w| w.bits(r.bits() | (1 << n)));
                    rb.pu.modify(|r, w| w.bits(r.bits() & !(1 << n)));
                    rb.dir.modify(|r, w| w.bits(r.bits() & !(1 << n)));
                },
            }
        });
    }

    /// Put the pin into output mode.
    ///
    /// The pin level will be whatever was set before (or low by default). If you want it to begin
    /// at a specific level, call `set_high`/`set_low` on the pin first.
    #[inline]
    pub fn set_as_output(&mut self, drive: OutputDrive) {
        critical_section::with(|_| {
            let rb = self.pin.block();
            let n = self.pin.pin();
            match drive {
                OutputDrive::Low => unsafe {
                    rb.pd_drv.modify(|r, w| w.bits(r.bits() & !(1 << n)));
                    rb.dir.modify(|r, w| w.bits(r.bits() | (1 << n)));
                },
                OutputDrive::High => unsafe {
                    rb.pd_drv.modify(|r, w| w.bits(r.bits() | (1 << n)));
                    rb.dir.modify(|r, w| w.bits(r.bits() | (1 << n)));
                },
            }
        });
    }

    #[inline]
    pub fn disable_interrupt(&mut self) {
        critical_section::with(|_| {
            let gpioctl = unsafe { &*pac::GPIOCTL::PTR };
            let n = self.pin.pin();
            match self.pin.port() {
                0 => unsafe {
                    gpioctl.pa_int_en.modify(|r, w| w.bits(r.bits() & !(1 << n)));
                },
                1 if n >= 22 => unsafe {
                    // map PB[23:22] to PB[9:8]
                    gpioctl.pb_int_en.modify(|r, w| w.bits(r.bits() & !(1 << (n - 14))));
                },
                1 => unsafe {
                    gpioctl.pb_int_en.modify(|r, w| w.bits(r.bits() & !(1 << n)));
                },
                _ => unreachable!(),
            }
        })
    }

    #[inline]
    pub fn set_trigger(&mut self, trigger: InterruptTrigger) {
        critical_section::with(|_| {
            let gpioctl = unsafe { &*pac::GPIOCTL::PTR };
            let rb = self.pin.block();
            let mut n = self.pin.pin();
            use InterruptTrigger::*;
            // map PB[23:22] to PB[9:8]

            match self.pin.port() {
                0 => unsafe {
                    if matches!(trigger, LowLevel | HighLevel) {
                        gpioctl.pa_int_mode.modify(|r, w| w.bits(r.bits() & !(1 << n)));
                    } else {
                        gpioctl.pa_int_mode.modify(|r, w| w.bits(r.bits() | (1 << n)));
                    }
                },
                1 => unsafe {
                    if n >= 22 {
                        n -= 14;
                        gpioctl.pin_alternate.modify(|_, w| w.intx().set_bit());
                    }

                    if matches!(trigger, LowLevel | HighLevel) {
                        gpioctl.pb_int_mode.modify(|r, w| w.bits(r.bits() & !(1 << n)));
                    } else {
                        gpioctl.pb_int_mode.modify(|r, w| w.bits(r.bits() | (1 << n)));
                    }
                },
                _ => unreachable!(),
            }
            if matches!(trigger, LowLevel | FallingEdge) {
                rb.clr.modify(|r, w| unsafe { w.bits(r.bits() | (1 << n)) });
            } else {
                rb.out.modify(|r, w| unsafe { w.bits(r.bits() | (1 << n)) });
            }
        });
    }

    // TODO: R16_PB_INT_MODE[9:8]由 RB_PIN_INTX 选择对应 PB[23:22]或 PB[9:8
    #[inline]
    pub fn enable_interrupt(&mut self) {
        critical_section::with(|_| {
            let gpioctl = unsafe { &*pac::GPIOCTL::PTR };
            let mut n = self.pin.pin();
            // map PB[23:22] to PB[9:8]

            match self.pin.port() {
                0 => unsafe {
                    gpioctl.pa_int_if.write(|w| w.bits(1 << n));
                    gpioctl.pa_int_en.modify(|r, w| w.bits(r.bits() | (1 << n)));
                },
                1 => unsafe {
                    if n >= 22 {
                        n -= 14;
                        gpioctl.pin_alternate.modify(|_, w| w.intx().set_bit());
                    }
                    gpioctl.pb_int_if.write(|w| w.bits(1 << n));
                    gpioctl.pb_int_en.modify(|r, w| w.bits(r.bits() | (1 << n)));
                },
                _ => unreachable!(),
            }
        });
    }

    #[inline]
    pub fn clear_interrupt(&mut self) {
        let gpioctl = unsafe { &*pac::GPIOCTL::PTR };
        let n = self.pin.pin();
        // clear int_if, write 1 to clear
        match self.pin.port() {
            0 => unsafe {
                gpioctl.pa_int_if.write(|w| w.bits(1 << n));
            },
            1 if n >= 22 => unsafe {
                // remap to PB[9:8]
                gpioctl.pb_int_if.modify(|r, w| w.bits(r.bits() | (1 << (n - 14))));
            },
            1 => unsafe {
                gpioctl.pb_int_if.write(|w| w.bits(1 << n));
            },
            _ => unreachable!(),
        }
    }

    #[inline]
    pub fn is_high(&self) -> bool {
        !self.is_low()
    }

    #[inline]
    pub fn is_low(&self) -> bool {
        let rb = self.pin.block();
        rb.pin.read().bits() & (1 << self.pin.pin()) == 0
    }

    #[inline]
    pub fn get_level(&self) -> Level {
        self.is_high().into()
    }

    #[inline]
    pub fn is_set_high(&self) -> bool {
        !self.is_set_low()
    }

    /// Is the output pin set as low?
    #[inline]
    pub fn is_set_low(&self) -> bool {
        let rb = self.pin.block();
        let mask = 1 << self.pin.pin();
        rb.out.read().bits() & mask == 0
    }

    /// What level output is set to
    #[inline]
    pub fn get_output_level(&self) -> Level {
        self.is_set_high().into()
    }

    #[inline]
    pub fn set_high(&mut self) {
        self.pin.set_high();
    }

    /// Set the output as low.
    #[inline]
    pub fn set_low(&mut self) {
        self.pin.set_low();
    }

    #[inline]
    pub fn set_level(&mut self, level: Level) {
        match level {
            Level::Low => self.pin.set_low(),
            Level::High => self.pin.set_high(),
        }
    }

    /// Toggle pin output
    #[inline]
    pub fn toggle(&mut self) {
        if self.is_set_low() {
            self.set_high()
        } else {
            self.set_low()
        }
    }
}

// TOOD: Drop

/// Pull setting for an input.
#[derive(Debug, Eq, PartialEq, Copy, Clone)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Pull {
    None,
    Up,
    Down,
}

/// Drive current settings for PushPull outputs.
#[derive(Debug, Copy, Clone)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum OutputDrive {
    // The drive current is 5mA
    Low,
    // The drive current is 20mA
    High,
}

/// GPIO input driver.
pub struct Input<'d, T: Pin> {
    pub(crate) pin: Flex<'d, T>,
}

impl<'d, T: Pin> Input<'d, T> {
    #[inline]
    pub fn new(pin: impl Peripheral<P = T> + 'd, pull: Pull) -> Self {
        let mut pin = Flex::new(pin);
        pin.set_as_input(pull);
        Self { pin }
    }

    #[inline]
    pub fn degrade(self) -> Input<'d, AnyPin> {
        Input {
            pin: self.pin.degrade(),
        }
    }

    #[inline]
    pub fn is_high(&self) -> bool {
        self.pin.is_high()
    }

    #[inline]
    pub fn is_low(&self) -> bool {
        self.pin.is_low()
    }

    #[inline]
    pub fn get_level(&self) -> Level {
        self.pin.get_level()
    }

    #[inline]
    pub fn disable_interrupt(&mut self) {
        self.pin.disable_interrupt();
    }

    #[inline]
    pub fn set_trigger(&mut self, trigger: InterruptTrigger) {
        self.pin.set_trigger(trigger);
    }

    #[inline]
    pub fn enable_interrupt(&mut self) {
        self.pin.enable_interrupt();
    }

    #[inline]
    pub fn clear_interrupt(&mut self) {
        self.pin.clear_interrupt();
    }
}

/// Digital input or output level.
#[derive(Debug, Eq, PartialEq, Copy, Clone)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Level {
    Low,
    High,
}

impl From<bool> for Level {
    fn from(val: bool) -> Self {
        match val {
            true => Self::High,
            false => Self::Low,
        }
    }
}

impl From<Level> for bool {
    fn from(level: Level) -> bool {
        match level {
            Level::Low => false,
            Level::High => true,
        }
    }
}

/// GPIO output driver.
///
/// Note that pins will **return to their floating state** when `Output` is dropped.
/// If pins should retain their state indefinitely, either keep ownership of the
/// `Output`, or pass it to [`core::mem::forget`].
pub struct Output<'d, T: Pin> {
    pub(crate) pin: Flex<'d, T>,
}

impl<'d, T: Pin> Output<'d, T> {
    #[inline]
    pub fn new(pin: impl Peripheral<P = T> + 'd, initial_output: Level, drive: OutputDrive) -> Self {
        let mut pin = Flex::new(pin);
        match initial_output {
            Level::High => pin.set_high(),
            Level::Low => pin.set_low(),
        }
        pin.set_as_output(drive);
        Self { pin }
    }

    #[inline]
    pub fn degrade(self) -> Output<'d, AnyPin> {
        Output {
            pin: self.pin.degrade(),
        }
    }

    /// Set the output as high.
    #[inline]
    pub fn set_high(&mut self) {
        self.pin.set_high();
    }

    /// Set the output as low.
    #[inline]
    pub fn set_low(&mut self) {
        self.pin.set_low();
    }

    /// Set the output level.
    #[inline]
    pub fn set_level(&mut self, level: Level) {
        self.pin.set_level(level)
    }

    /// Is the output pin set as high?
    #[inline]
    pub fn is_set_high(&self) -> bool {
        self.pin.is_set_high()
    }

    /// Is the output pin set as low?
    #[inline]
    pub fn is_set_low(&self) -> bool {
        self.pin.is_set_low()
    }

    /// What level output is set to
    #[inline]
    pub fn get_output_level(&self) -> Level {
        self.pin.get_output_level()
    }

    /// Toggle pin output
    #[inline]
    pub fn toggle(&mut self) {
        self.pin.toggle();
    }
}

// No OpenDrain for CH59x

pub(crate) mod sealed {
    use super::*;

    pub trait Pin {
        fn pin_port(&self) -> u8;

        #[inline]
        fn _pin(&self) -> u8 {
            // self.pin_port() % 32
            self.pin_port() & 0x1f
        }
        #[inline]
        fn _port(&self) -> u8 {
            // self.pin_port() / 32
            self.pin_port() >> 5
        }

        #[inline]
        fn block(&self) -> &'static pac::gpioa::RegisterBlock {
            match self._port() {
                0 => unsafe { &*pac::GPIOA::PTR },
                1 => unsafe { &*pac::GPIOB::PTR },
                _ => unreachable!(),
            }
        }

        /// Set the output as high.
        #[inline]
        fn set_high(&self) {
            let rb = self.block();
            let n = self._pin();
            rb.out.modify(|r, w| unsafe { w.bits(r.bits() | (1 << n)) });
        }

        /// Set the output as low.
        #[inline]
        fn set_low(&self) {
            let rb = self.block();
            let n = self._pin();
            rb.clr.modify(|r, w| unsafe { w.bits(r.bits() | (1 << n)) });
        }

        #[inline]
        fn set_as_analog(&self) {
            // GPIO_ModeIN_Floating
            let rb = self.block();
            let pin = self._pin() as usize;
            unsafe {
                rb.pd_drv.modify(|r, w| w.bits(r.bits() & !(1 << pin)));
                rb.pu.modify(|r, w| w.bits(r.bits() & !(1 << pin)));
                rb.dir.modify(|r, w| w.bits(r.bits() & !(1 << pin)));
            }
        }

        /// Set the pin as an input, for peripherals functions
        #[inline]
        fn set_as_output_with_drive_low(&self) {
            let rb = self.block();
            let pin = self._pin() as usize;
            unsafe {
                rb.dir.modify(|r, w| w.bits(r.bits() | (1 << pin)));
                rb.pd_drv.modify(|r, w| w.bits(r.bits() & !(1 << pin)));
            }
        }

        // input floatingZF
        #[inline]
        fn set_as_input(&self) {
            let rb = self.block();
            let pin = self._pin() as usize;
            unsafe {
                rb.dir.modify(|r, w| w.bits(r.bits() & !(1 << pin)));
                rb.pu.modify(|r, w| w.bits(r.bits() & !(1 << pin)));
                rb.pd_drv.modify(|r, w| w.bits(r.bits() & !(1 << pin)));
            }
        }

        #[inline]
        fn set_pullup(&self) {
            let rb = self.block();
            let pin = self._pin() as usize;
            unsafe {
                rb.pu.modify(|r, w| w.bits(r.bits() | (1 << pin)));
                rb.pd_drv.modify(|r, w| w.bits(r.bits() & !(1 << pin)));
            }
        }

        #[inline]
        fn set_drive(&self, drive: OutputDrive) {
            let rb = self.block();
            let pin = self._pin() as usize;
            match drive {
                OutputDrive::Low => unsafe {
                    rb.pd_drv.modify(|r, w| w.bits(r.bits() & !(1 << pin)));
                },
                OutputDrive::High => unsafe {
                    rb.pd_drv.modify(|r, w| w.bits(r.bits() | (1 << pin)));
                },
            }
        }
    }
}

pub trait Pin: Peripheral<P = Self> + Into<AnyPin> + sealed::Pin + Sized + 'static {
    /// Number of the pin within the port (0..31)
    #[inline]
    fn pin(&self) -> u8 {
        self._pin()
    }

    /// Port of the pin
    #[inline]
    fn port(&self) -> u8 {
        self._port()
    }

    /// Convert from concrete pin type PX_XX to type erased `AnyPin`.
    #[inline]
    fn degrade(self) -> AnyPin {
        AnyPin {
            pin_port: self.pin_port(),
        }
    }
}

// Type-erased GPIO pin
pub struct AnyPin {
    pin_port: u8,
}

impl AnyPin {
    #[inline]
    pub unsafe fn steal(pin_port: u8) -> Self {
        Self { pin_port }
    }

    #[inline]
    fn _port(&self) -> u8 {
        self.pin_port / 32
    }
}

impl_peripheral!(AnyPin);
impl Pin for AnyPin {}
impl sealed::Pin for AnyPin {
    #[inline]
    fn pin_port(&self) -> u8 {
        self.pin_port
    }
}

// interrupt handling

// also control by CLR/OUT
#[derive(Debug, Eq, PartialEq, Copy, Clone)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum InterruptTrigger {
    LowLevel,
    HighLevel,
    RaisingEdge,
    FallingEdge,
}

mod eh02 {
    use core::convert::Infallible;

    use embedded_hal_02::digital::v2::{InputPin, OutputPin, StatefulOutputPin, ToggleableOutputPin};

    use super::*;

    impl<'d, T: Pin> InputPin for Input<'d, T> {
        type Error = Infallible;

        #[inline]
        fn is_high(&self) -> Result<bool, Self::Error> {
            Ok(self.is_high())
        }

        #[inline]
        fn is_low(&self) -> Result<bool, Self::Error> {
            Ok(self.is_low())
        }
    }

    impl<'d, T: Pin> OutputPin for Output<'d, T> {
        type Error = Infallible;

        #[inline]
        fn set_high(&mut self) -> Result<(), Self::Error> {
            self.set_high();
            Ok(())
        }

        #[inline]
        fn set_low(&mut self) -> Result<(), Self::Error> {
            self.set_low();
            Ok(())
        }
    }

    impl<'d, T: Pin> StatefulOutputPin for Output<'d, T> {
        #[inline]
        fn is_set_high(&self) -> Result<bool, Self::Error> {
            Ok(self.is_set_high())
        }

        /// Is the output pin set as low?
        #[inline]
        fn is_set_low(&self) -> Result<bool, Self::Error> {
            Ok(self.is_set_low())
        }
    }

    impl<'d, T: Pin> ToggleableOutputPin for Output<'d, T> {
        type Error = Infallible;
        #[inline]
        fn toggle(&mut self) -> Result<(), Self::Error> {
            self.toggle();
            Ok(())
        }
    }

    impl<'d, T: Pin> InputPin for Flex<'d, T> {
        type Error = Infallible;

        #[inline]
        fn is_high(&self) -> Result<bool, Self::Error> {
            Ok(self.is_high())
        }

        #[inline]
        fn is_low(&self) -> Result<bool, Self::Error> {
            Ok(self.is_low())
        }
    }

    impl<'d, T: Pin> OutputPin for Flex<'d, T> {
        type Error = Infallible;

        #[inline]
        fn set_high(&mut self) -> Result<(), Self::Error> {
            self.set_high();
            Ok(())
        }

        #[inline]
        fn set_low(&mut self) -> Result<(), Self::Error> {
            self.set_low();
            Ok(())
        }
    }

    impl<'d, T: Pin> StatefulOutputPin for Flex<'d, T> {
        #[inline]
        fn is_set_high(&self) -> Result<bool, Self::Error> {
            Ok(self.is_set_high())
        }

        /// Is the output pin set as low?
        #[inline]
        fn is_set_low(&self) -> Result<bool, Self::Error> {
            Ok(self.is_set_low())
        }
    }

    impl<'d, T: Pin> ToggleableOutputPin for Flex<'d, T> {
        type Error = Infallible;
        #[inline]
        fn toggle(&mut self) -> Result<(), Self::Error> {
            self.toggle();
            Ok(())
        }
    }
}

macro_rules! foreach_pin {
    ($($pat:tt => $code:tt;)*) => {
        macro_rules! __foreach_pin_inner {
            $(($pat) => $code;)*
            ($_:tt) => {}
        }
        __foreach_pin_inner!((PA0,GPIOA,0,0));
        __foreach_pin_inner!((PA1,GPIOA,0,1));
        __foreach_pin_inner!((PA2,GPIOA,0,2));
        __foreach_pin_inner!((PA3,GPIOA,0,3));
        __foreach_pin_inner!((PA4,GPIOA,0,4));
        __foreach_pin_inner!((PA5,GPIOA,0,5));
        __foreach_pin_inner!((PA6,GPIOA,0,6));
        __foreach_pin_inner!((PA7,GPIOA,0,7));
        __foreach_pin_inner!((PA8,GPIOA,0,8));
        __foreach_pin_inner!((PA9,GPIOA,0,9));
        __foreach_pin_inner!((PA10,GPIOA,0,10));
        __foreach_pin_inner!((PA11,GPIOA,0,11));
        __foreach_pin_inner!((PA12,GPIOA,0,12));
        __foreach_pin_inner!((PA13,GPIOA,0,13));
        __foreach_pin_inner!((PA14,GPIOA,0,14));
        __foreach_pin_inner!((PA15,GPIOA,0,15));
        __foreach_pin_inner!((PB0,GPIOB,1,0));
        __foreach_pin_inner!((PB4,GPIOB,1,4));
        __foreach_pin_inner!((PB6,GPIOB,1,6));
        __foreach_pin_inner!((PB7,GPIOB,1,7));
        __foreach_pin_inner!((PB10,GPIOB,1,10));
        __foreach_pin_inner!((PB11,GPIOB,1,11));
        __foreach_pin_inner!((PB12,GPIOB,1,12));
        __foreach_pin_inner!((PB13,GPIOB,1,13));
        __foreach_pin_inner!((PB14,GPIOB,1,14));
        __foreach_pin_inner!((PB15,GPIOB,1,15));
        __foreach_pin_inner!((PB20,GPIOB,1,20));
        __foreach_pin_inner!((PB21,GPIOB,1,21));
        __foreach_pin_inner!((PB22,GPIOB,1,22));
        __foreach_pin_inner!((PB23,GPIOB,1,23));
    };
}
foreach_pin!(
    ($pin_name:ident, $port_name:ident, $port_num:expr, $pin_num:expr) => {
        impl Pin for peripherals::$pin_name {}

        impl sealed::Pin for peripherals::$pin_name {
            #[inline]
            fn pin_port(&self) -> u8 {
                $port_num * 32 + $pin_num
            }
        }

        impl From<peripherals::$pin_name> for AnyPin {
            fn from(x: peripherals::$pin_name) -> Self {
                x.degrade()
           }
        }
    };
);
