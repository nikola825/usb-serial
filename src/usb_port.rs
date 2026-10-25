use core::{cmp::min, fmt::Write};

use embassy_usb::{
    Builder, UsbDevice,
    class::cdc_acm::{CdcAcmClass, State},
};
use embedded_io::ErrorType;

use crate::{UsbSerialConfig, UsbSerialError};

pub struct UsbPort<Driver: embassy_usb::driver::Driver<'static>> {
    cdc_acm: Option<CdcAcmClass<'static, Driver>>,
    usb_device: UsbDevice<'static, Driver>,
}

impl<Driver: embassy_usb::driver::Driver<'static>> UsbPort<Driver> {
    pub(crate) fn new_internal(
        usb_serial_config: UsbSerialConfig,
        driver: Driver,
        config_descriptor: &'static mut [u8],
        bos_descriptor: &'static mut [u8],
        control_buf: &'static mut [u8],
        state: &'static mut State<'static>,
    ) -> Self {
        let mut config =
            embassy_usb::Config::new(usb_serial_config.vendor_id, usb_serial_config.product_id);
        config.manufacturer = Some(usb_serial_config.manufacturer);
        config.product = Some(usb_serial_config.product_name);
        config.serial_number = Some(usb_serial_config.usb_serial);

        config.device_class = usb_serial_config.device_class;
        config.device_sub_class = usb_serial_config.device_subclass;
        config.device_protocol = usb_serial_config.device_protocol;
        config.composite_with_iads = true;
        config.self_powered = true;
        config.max_power = 0;

        // Create embassy-usb DeviceBuilder using the driver and config.
        // It needs some buffers for building the descriptors.
        let mut builder = Builder::new(
            driver,
            config,
            config_descriptor,
            bos_descriptor,
            &mut [], // no msos descriptors
            control_buf,
        );

        // Create classes on the builder.
        let cdc_acm = CdcAcmClass::new(&mut builder, state, 64);

        // Build the builder.
        let usb_device = builder.build();

        UsbPort {
            cdc_acm: Some(cdc_acm),
            usb_device,
        }
    }

    pub fn begin_serial(
        &mut self,
    ) -> Result<(UsbSerialWrapperGeneric<Driver>, impl Future), UsbSerialError> {
        let cdc_acm = self
            .cdc_acm
            .take()
            .ok_or(UsbSerialError::PortAlreadyInitialized)?;
        let usb_device_future = self.usb_device.run();
        let serial_wrapper = UsbSerialWrapperGeneric::new(cdc_acm);

        Ok((serial_wrapper, usb_device_future))
    }
}

impl From<embassy_usb::driver::EndpointError> for UsbSerialError {
    fn from(val: embassy_usb::driver::EndpointError) -> Self {
        match val {
            embassy_usb::driver::EndpointError::BufferOverflow => Self::BufferOverflow,
            embassy_usb::driver::EndpointError::Disabled => Self::Disconnected,
        }
    }
}

pub struct UsbSerialWrapperGeneric<Driver: embassy_usb::driver::Driver<'static>> {
    cdc_acm: CdcAcmClass<'static, Driver>,
    current_packet: [u8; 64],
    cursor: Option<usize>,
    packet_len: usize,
}

impl<Driver: embassy_usb::driver::Driver<'static>> UsbSerialWrapperGeneric<Driver> {
    pub(crate) fn new(cdc_acm: CdcAcmClass<'static, Driver>) -> Self {
        Self {
            cdc_acm,
            current_packet: [0u8; 64],
            cursor: None,
            packet_len: 0,
        }
    }
    pub async fn write(&mut self, data: &[u8]) -> Result<(), UsbSerialError> {
        let mut index = 0;
        while index < data.len() {
            let end = core::cmp::min(index + 32, data.len());
            self.cdc_acm.write_packet(&data[index..end]).await?;
            index += 32;
        }

        Ok(())
    }

    async fn fill_packet(&mut self) -> Result<(), UsbSerialError> {
        loop {
            self.packet_len = self.cdc_acm.read_packet(&mut self.current_packet).await?;
            if self.packet_len > 0 {
                self.cursor = Some(0);
                return Ok(());
            }
        }
    }

    async fn get_current_cursor(&mut self) -> Result<usize, UsbSerialError> {
        if let Some(cursor) = self.cursor {
            Ok(cursor)
        } else {
            self.fill_packet().await?;
            Ok(0)
        }
    }

    fn advance_cursor(&mut self, by: usize) -> Result<(), UsbSerialError> {
        let cursor = if let Some(cursor) = self.cursor {
            cursor
        } else {
            return Err(UsbSerialError::InternalErrorBug(
                "Cursor advanced attempted with None existing cursor",
            ));
        };

        let next_cursor = cursor + by;

        match next_cursor.cmp(&self.packet_len) {
            core::cmp::Ordering::Less => {
                self.cursor = Some(next_cursor);
            }
            core::cmp::Ordering::Equal => {
                self.cursor = None;
            }
            core::cmp::Ordering::Greater => {
                return Err(UsbSerialError::InternalErrorBug(
                    "Cursor advanced to position beyond packet len",
                ));
            }
        }

        Ok(())
    }

    pub async fn read_exact(&mut self, buffer: &mut [u8]) -> Result<(), UsbSerialError> {
        let mut bytes_remaining_to_read = buffer.len();
        let mut out_index = 0usize;

        while bytes_remaining_to_read > 0 {
            let cursor = self.get_current_cursor().await?;
            let bytes_remaining_in_packet = self.packet_len - cursor;

            let bytes_to_copy = min(bytes_remaining_in_packet, bytes_remaining_to_read);

            buffer[out_index..out_index + bytes_to_copy]
                .copy_from_slice(&self.current_packet[cursor..cursor + bytes_to_copy]);

            bytes_remaining_to_read -= bytes_to_copy;
            out_index += bytes_to_copy;
            self.advance_cursor(bytes_to_copy)?;
        }

        Ok(())
    }

    pub async fn wait_for_connection(&mut self) {
        self.cdc_acm.wait_connection().await
    }

    async fn send(&mut self, buffer: &ChunkingBuffer) -> Result<(), UsbSerialError> {
        self.write(&buffer.get_contents_to_write()).await
    }

    pub async fn log(&mut self, args: core::fmt::Arguments<'_>) -> Result<(), UsbSerialError> {
        let mut buffer = ChunkingBuffer::new();
        buffer.write_chunked(args, async |x| self.send(x).await).await
    }
}

impl embedded_io_async::Error for UsbSerialError {
    fn kind(&self) -> embedded_io::ErrorKind {
        embedded_io::ErrorKind::ConnectionAborted
    }
}

impl<Driver: embassy_usb::driver::Driver<'static>> ErrorType for UsbSerialWrapperGeneric<Driver> {
    type Error = UsbSerialError;
}

impl<Driver: embassy_usb::driver::Driver<'static>> embedded_io_async::Write
    for UsbSerialWrapperGeneric<Driver>
{
    async fn write(&mut self, buf: &[u8]) -> Result<usize, UsbSerialError> {
        self.write(buf).await?;

        Ok(buf.len())
    }

    async fn flush(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }

    async fn write_all(&mut self, buf: &[u8]) -> Result<(), Self::Error> {
        self.write(buf).await
    }
}

enum ChunkingState {
    Initial,
    Writing,
    End,
}
struct ChunkingBuffer {
    data: [u8; 64],
    len: usize,
    cycle_cursor: usize,
    input_cursor: usize,
    total_len: usize,
    state: ChunkingState,
}

const BLK_BUF_LEN: usize = 64;

impl ChunkingBuffer {
    fn new() -> Self {
        Self {
            data: [0u8; BLK_BUF_LEN],
            len: 0,
            total_len: 0,
            input_cursor: 0,
            cycle_cursor: 0,
            state: ChunkingState::Initial,
        }
    }

    async fn write_chunked(
        &mut self,
        args: core::fmt::Arguments<'_>,
        mut sink: impl AsyncFnMut(&ChunkingBuffer) -> Result<(), UsbSerialError>,
    ) -> Result<(), UsbSerialError> {
        while !matches!(self.state, ChunkingState::End) {
            let _ = self.write_fmt(args);
            sink(self).await?;
            self.cycle();
        }

        Ok(())
    }

    fn get_contents_to_write(&self) -> &[u8] {
        &self.data[..self.len]
    }

    fn cycle(&mut self) {
        self.input_cursor += self.len;
        self.len = 0;
        self.state = match self.state {
            ChunkingState::Initial => {
                self.total_len = self.cycle_cursor;
                if self.input_cursor < self.total_len {
                    ChunkingState::Writing
                } else {
                    ChunkingState::End
                }
            }
            ChunkingState::Writing => {
                if self.input_cursor < self.total_len {
                    ChunkingState::Writing
                } else {
                    ChunkingState::End
                }
            }
            ChunkingState::End => ChunkingState::End,
        };
        self.cycle_cursor = 0;
    }

    fn append_to_buf(&mut self, s: &str) {
        let bytes = s.as_bytes();
        let start = if self.cycle_cursor > self.input_cursor {
            0
        } else {
            self.input_cursor - self.cycle_cursor
        };

        if start < bytes.len() {
            let input_bytes_to_copy = bytes.len() - start;
            let room_available = BLK_BUF_LEN - self.len;

            let bytes_to_copy = min(input_bytes_to_copy, room_available);
            self.data[self.len..self.len + bytes_to_copy]
                .copy_from_slice(&bytes[start..start + bytes_to_copy]);
            self.len += bytes_to_copy;
        }

        self.cycle_cursor += bytes.len();
    }
}

impl core::fmt::Write for ChunkingBuffer {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        match self.state {
            ChunkingState::Initial => {
                self.append_to_buf(s);
            }
            ChunkingState::Writing => {
                self.append_to_buf(s);
            }
            ChunkingState::End => {}
        }

        Ok(())
    }
}
