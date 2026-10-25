use embassy_stm32::{
    Peri,
    usb::{DmPin, DpPin, Driver, InterruptHandler},
};

use crate::{
    UsbSerialConfig, UsbStatePointers,
    usb_port::{UsbPort, UsbSerialWrapperGeneric},
};

/// Can be used to write and read bytes via USB serial
/// To be used on embassy-stm32 compatible targets
/// # Example
/// ```
/// mod example {
///     use embassy_stm32::bind_interrupts;
///
///     use crate::{UsbSerial, UsbSerialConfig, make_usb_state_pointers, usb_log, usb_port::UsbPort};
///
///     bind_interrupts!(pub struct Irqs {
///         OTG_FS => embassy_stm32::usb::InterruptHandler<embassy_stm32::peripherals::USB_OTG_FS>;
///     });
///
///     async fn example(peripherals: embassy_stm32::Peripherals) {
///         let config = UsbSerialConfig::default();
///         let state_pointers = make_usb_state_pointers!();
///         let mut usb_port = UsbPort::new(
///             peripherals.USB_OTG_FS,
///             peripherals.PA12,
///             peripherals.PA11,
///             config,
///             state_pointers,
///             Irqs,
///         );
///
///         let (serial, usb_future) = usb_port.begin_serial().unwrap();
///         embassy_futures::join::join(usb_future, example_usb_loop(serial)).await;
///     }
///
///     async fn example_usb_loop(mut usb: UsbSerial) -> ! {
///         let mut counter = 0;
///         loop {
///             counter += 1;
///             usb_log!(usb, "Hello world, iteration: {}", counter);
///         }
///     }
/// }
/// ```
pub type UsbSerialWrapperStm32 = UsbSerialWrapperGeneric<
    embassy_stm32::usb::Driver<'static, embassy_stm32::peripherals::USB_OTG_FS>,
>;

impl<Instance: embassy_stm32::usb::Instance>
    UsbPort<embassy_stm32::usb::Driver<'static, Instance>>
{
    /// Create a new UsbPort from the given hardware and configuration
    /// To be used on embassy-stm32 compatible targets
    /// # Example
    /// ```
    /// mod example {
    ///     use embassy_stm32::bind_interrupts;
    ///
    ///     use crate::{UsbSerial, UsbSerialConfig, make_usb_state_pointers, usb_log, usb_port::UsbPort};
    ///
    ///     bind_interrupts!(pub struct Irqs {
    ///         OTG_FS => embassy_stm32::usb::InterruptHandler<embassy_stm32::peripherals::USB_OTG_FS>;
    ///     });
    ///
    ///     async fn example(peripherals: embassy_stm32::Peripherals) {
    ///         let config = UsbSerialConfig::default();
    ///         let state_pointers = make_usb_state_pointers!();
    ///         let mut usb_port = UsbPort::new(
    ///             peripherals.USB_OTG_FS,
    ///             peripherals.PA12,
    ///             peripherals.PA11,
    ///             config,
    ///             state_pointers,
    ///             Irqs,
    ///         );
    ///
    ///         let (serial, usb_future) = usb_port.begin_serial().unwrap();
    ///         embassy_futures::join::join(usb_future, example_usb_loop(serial)).await;
    ///     }
    ///
    ///     async fn example_usb_loop(mut usb: UsbSerial) -> ! {
    ///         let mut counter = 0;
    ///         loop {
    ///             counter += 1;
    ///             usb_log!(usb, "Hello world, iteration: {}", counter);
    ///         }
    ///     }
    /// }
    /// ```
    pub fn new<Dp: DpPin<Instance>, Dm: DmPin<Instance>>(
        peripheral: Peri<'static, Instance>,
        dp_pin: Peri<'static, Dp>,
        dm_pin: Peri<'static, Dm>,
        usb_serial_config: UsbSerialConfig,
        state_pointers: UsbStatePointers,
        irqs: impl embassy_stm32::interrupt::typelevel::Binding<
            Instance::Interrupt,
            InterruptHandler<Instance>,
        > + 'static,
    ) -> UsbPort<Driver<'static, Instance>> {
        let mut config = embassy_stm32::usb::Config::default();

        config.vbus_detection = false;
        let driver = Driver::new_fs(
            peripheral,
            irqs,
            dp_pin,
            dm_pin,
            state_pointers.usb_buffer,
            config,
        );

        UsbPort::new_internal(
            usb_serial_config,
            driver,
            state_pointers.config_descriptor,
            state_pointers.bos_descriptor,
            state_pointers.control_buf,
            state_pointers.state,
        )
    }
}
