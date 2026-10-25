#![no_std]
use embassy_usb::class::cdc_acm::State;

#[cfg(feature = "stm32")]
pub mod stm32;


#[cfg(feature = "stm32")]
pub use stm32::UsbSerialWrapperStm32 as UsbSerial;

pub mod usb_port;

#[derive(Debug)]
pub enum UsbSerialError {
    Disconnected,
    BufferOverflow,
    PortAlreadyInitialized,
    InternalErrorBug(&'static str),
}

#[macro_export]
macro_rules! usb_wrapper_make_static_buffer {
    ($size: literal) => {{
        use static_cell::StaticCell;

        #[allow(non_upper_case_globals)]
        static buffer_cell: StaticCell<&'static mut [u8]> = StaticCell::new();
        buffer_cell.init_with(|| {
            static mut BUFFER: [u8; $size] = [0u8; $size];

            #[allow(static_mut_refs)]
            unsafe {
                &mut BUFFER
            }
        })
    }};
    ($size: literal, $section: literal) => {{
        use static_cell::StaticCell;

        #[allow(non_upper_case_globals)]
        static buffer_cell: StaticCell<&'static mut [u8]> = StaticCell::new();
        buffer_cell.init_with(|| {
            #[unsafe(link_section = $section)]
            static mut BUFFER: [u8; $size] = [0u8; $size];

            #[allow(static_mut_refs)]
            unsafe {
                &mut BUFFER
            }
        })
    }};
}

#[macro_export]
macro_rules! make_usb_state_pointers {
    () => {{
        use embassy_usb::class::cdc_acm::State;
        $crate::UsbStatePointers {
            usb_buffer: $crate::usb_wrapper_make_static_buffer!(256),
            config_descriptor: $crate::usb_wrapper_make_static_buffer!(256),
            bos_descriptor: $crate::usb_wrapper_make_static_buffer!(256),
            control_buf: $crate::usb_wrapper_make_static_buffer!(64),
            state: $crate::usb_wrapper_make_static_object!(State, State::new()),
        }
    }};
    ($section: literal) => {{
        use embassy_usb::class::cdc_acm::State;
        $crate::UsbStatePointers {
            usb_buffer: $crate::usb_wrapper_make_static_buffer!(256, $section),
            config_descriptor: $crate::usb_wrapper_make_static_buffer!(256, $section),
            bos_descriptor: $crate::usb_wrapper_make_static_buffer!(256, $section),
            control_buf: $crate::usb_wrapper_make_static_buffer!(64, $section),
            state: $crate::usb_wrapper_make_static_object!(State, State::new()),
        }
    }};
}

#[macro_export]
macro_rules! usb_wrapper_make_static_object {
    ($type: ident, $value: expr) => {{
        use static_cell::StaticCell;

        #[allow(non_upper_case_globals)]
        static buffer_cell: StaticCell<$type> = StaticCell::new();
        buffer_cell.init_with(|| $value)
    }};
}

pub struct UsbStatePointers {
    pub usb_buffer: &'static mut [u8],
    pub config_descriptor: &'static mut [u8],
    pub bos_descriptor: &'static mut [u8],
    pub control_buf: &'static mut [u8],
    pub state: &'static mut State<'static>,
}

// Boilerplate settings needed for USB protocol
pub struct UsbSerialConfig {
    pub manufacturer: &'static str,
    pub usb_serial: &'static str,
    pub product_name: &'static str,
    pub vendor_id: u16,
    pub product_id: u16,
    pub device_class: u8,
    pub device_subclass: u8,
    pub device_protocol: u8,
}

impl Default for UsbSerialConfig {
    fn default() -> Self {
        Self {
            manufacturer: "nikola825",
            usb_serial: "12345678",
            product_name: "nikola825 USB logging",
            vendor_id: 0xdead,
            product_id: 0xbeef,
            device_class: 0xEF, // 0xEF stands for "Miscelaneous" - https://www.usb.org/defined-class-codes
            device_subclass: 0x02,
            device_protocol: 0x01, // subclass=2, protocol=1 - interface association descriptor
        }
    }
}

#[macro_export]
macro_rules! usb_log {
    ($usb_serial_wrapper: expr, $($arg:tt)*) => {{
        let _ = $usb_serial_wrapper.log(format_args!($($arg)*)).await;
    }};
}
