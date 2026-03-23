// io_interface/serial.rs
use usb_device::prelude::*;
use usbd_serial::SerialPort;
use core::fmt::{Write as FmtWrite, Result as FmtResult};
use stm32f4xx_hal::otg_fs::{UsbBus, USB};


/// Serial IO abstraction for USB CDC on STM32
pub struct SerialIO {
    /// USB serial port
    pub serial: SerialPort<'static, UsbBus<USB>>,
    /// USB device
    pub usb_dev: UsbDevice<'static, UsbBus<USB>>,
}

impl SerialIO {
    /// Create new serial IO from existing USB components
    pub fn new(
        serial: SerialPort<'static, UsbBus<USB>>,
        usb_dev: UsbDevice<'static, UsbBus<USB>>,
    ) -> Self {
        Self {
            serial,
            usb_dev,
        }
    }

    /// Must be called periodically to handle USB events
    pub fn poll(&mut self) {
        if !self.usb_dev.poll(&mut [&mut self.serial]) {
            return;
        }
    }

    /// Write all bytes to USB serial, retrying on partial writes and WouldBlock.
    fn write_all(&mut self, mut bytes: &[u8]) -> Result<(), usb_device::UsbError> {
        while !bytes.is_empty() {
            self.poll();
            match self.serial.write(bytes) {
                Ok(0) => {}
                Ok(written) => {
                    bytes = &bytes[written..];
                }
                Err(usb_device::UsbError::WouldBlock) => {}
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }

    /// Write string to USB serial
    pub fn write_str(&mut self, s: &str) -> Result<(), usb_device::UsbError> {
        self.write_all(s.as_bytes())
    }

    /// Write a string in a specified color
    pub fn write_str_color(&mut self, s: &str, color: &str) -> Result<(), usb_device::UsbError> {
        self.write_str(color)?;
        self.write_str(s)?;
        Ok(())
    }

    /// Write byte slice to USB serial
    pub fn write_bytes(&mut self, bytes: &[u8]) -> Result<usize, usb_device::UsbError> {
        self.write_all(bytes)?;
        Ok(bytes.len())
    }

    /// Read bytes from USB serial (non-blocking)
    pub fn read_bytes(&mut self, buf: &mut [u8]) -> Result<usize, usb_device::UsbError> {
        self.serial.read(buf)
    }

    /// Check if data is available
    pub fn data_available(&mut self) -> bool {
        let mut buf = [0u8; 1];
        match self.serial.read(&mut buf) {
            Ok(1) => true,
            _ => false,
        }
    }

    /// Read a byte (non-blocking)
    pub fn read_byte(&mut self) -> Option<u8> {
        let mut buf = [0u8; 1];
        match self.serial.read(&mut buf) {
            Ok(1) => Some(buf[0]),
            _ => None,
        }
    }
}

/// Implement core::fmt::Write for easy formatting
impl FmtWrite for SerialIO {
    fn write_str(&mut self, s: &str) -> FmtResult {
        self.write_str(s).map_err(|_| core::fmt::Error)
    }
}
