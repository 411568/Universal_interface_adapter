// SPI command handler
// Usage: spi <1|2> <config|write|read|transfer|burst_read|burst_write> [args]

use crate::cli::{CliConfig, Command};
use crate::io_interface::serial::SerialIO;
use usb_device::UsbError;
use stm32f4xx_hal as hal;
use hal::spi::{Spi, Mode, Phase, Polarity};
use cortex_m::interrupt::Mutex;
use core::cell::RefCell;

type Spi1Type = Spi<hal::pac::SPI1>;
type Spi2Type = Spi<hal::pac::SPI2>;

static SPI1: Mutex<RefCell<Option<Spi1Type>>> = Mutex::new(RefCell::new(None));
static SPI2: Mutex<RefCell<Option<Spi2Type>>> = Mutex::new(RefCell::new(None));

// SPI Configuration
#[derive(Clone, Copy)]
struct SpiConfig {
    mode: Mode,
    msb_first: bool,
}

impl SpiConfig {
    const fn default() -> Self {
        Self {
            mode: Mode {
                polarity: Polarity::IdleLow,
                phase: Phase::CaptureOnFirstTransition,
            },
            msb_first: true,
        }
    }
}

static SPI1_CONFIG: Mutex<RefCell<SpiConfig>> = Mutex::new(RefCell::new(SpiConfig::default()));
static SPI2_CONFIG: Mutex<RefCell<SpiConfig>> = Mutex::new(RefCell::new(SpiConfig::default()));

#[derive(Clone)]
pub struct SpiCommand;

impl SpiCommand {
    pub fn new() -> Self {
        Self
    }

    pub fn init_spi1(spi: Spi1Type) {
        cortex_m::interrupt::free(|cs| {
            *SPI1.borrow(cs).borrow_mut() = Some(spi);
        });
    }

    pub fn init_spi2(spi: Spi2Type) {
        cortex_m::interrupt::free(|cs| {
            *SPI2.borrow(cs).borrow_mut() = Some(spi);
        });
    }

    fn parse_hex_byte(&self, s: &str) -> Result<u8, &'static str> {
        let s = s.trim_start_matches("0x").trim_start_matches("0X");
        u8::from_str_radix(s, 16).map_err(|_| "Invalid hex byte")
    }

    fn parse_hex_data(&self, arg: &str) -> Result<[u8; 256], &'static str> {
        let mut data = [0u8; 256];
        let s = arg.trim_start_matches("0x").trim_start_matches("0X");
        
        // Parse pairs of hex digits
        if s.len() % 2 != 0 {
            return Err("Hex data must have even number of digits");
        }
        
        let byte_count = s.len() / 2;
        if byte_count > 256 {
            return Err("Data too long (max 256 bytes)");
        }
        
        for i in 0..byte_count {
            let byte_str = &s[i*2..i*2+2];
            data[i] = u8::from_str_radix(byte_str, 16)
                .map_err(|_| "Invalid hex data")?;
        }
        
        Ok(data)
    }

    fn parse_spi_num(&self, arg: &str) -> Result<u8, &'static str> {
        match arg {
            "1" => Ok(1),
            "2" => Ok(2),
            _ => Err("SPI number must be 1 or 2"),
        }
    }

    fn parse_int(&self, arg: &str) -> Result<usize, &'static str> {
        arg.parse::<usize>().map_err(|_| "Invalid integer value")
    }

    fn parse_mode(&self, arg: &str) -> Result<Mode, &'static str> {
        match arg {
            "0" => Ok(Mode {
                polarity: Polarity::IdleLow,
                phase: Phase::CaptureOnFirstTransition,
            }),
            "1" => Ok(Mode {
                polarity: Polarity::IdleLow,
                phase: Phase::CaptureOnSecondTransition,
            }),
            "2" => Ok(Mode {
                polarity: Polarity::IdleHigh,
                phase: Phase::CaptureOnFirstTransition,
            }),
            "3" => Ok(Mode {
                polarity: Polarity::IdleHigh,
                phase: Phase::CaptureOnSecondTransition,
            }),
            _ => Err("Mode must be 0-3"),
        }
    }

    fn parse_speed(&self, arg: &str) -> Result<u32, &'static str> {
        // Parse speed strings like "1m", "5m", "10m" into Hz
        // Check last character
        if arg.len() == 0 {
            return Err("Invalid speed format");
        }
        
        let last_char = arg.as_bytes()[arg.len() - 1];
        
        if last_char == b'm' || last_char == b'M' {
            // Parse MHz
            let num_str = &arg[..arg.len()-1];
            let num = num_str.parse::<u32>()
                .map_err(|_| "Invalid speed format")?;
            Ok(num * 1_000_000)
        } else if last_char == b'k' || last_char == b'K' {
            // Parse kHz
            let num_str = &arg[..arg.len()-1];
            let num = num_str.parse::<u32>()
                .map_err(|_| "Invalid speed format")?;
            Ok(num * 1_000)
        } else {
            // Assume Hz
            arg.parse::<u32>().map_err(|_| "Invalid speed format")
        }
    }

    fn format_hex_u8<'a>(&self, n: u8, buf: &'a mut [u8; 4]) -> Option<&'a str> {
        buf[0] = b'0';
        buf[1] = b'x';
        let high = (n >> 4) & 0x0F;
        let low = n & 0x0F;
        buf[2] = if high < 10 { b'0' + high } else { b'A' + high - 10 };
        buf[3] = if low < 10 { b'0' + low } else { b'A' + low - 10 };
        core::str::from_utf8(&buf[..]).ok()
    }

    fn config_spi(&self, spi_num: u8, speed: &str, mode: &str, bit_order: &str, out: &mut SerialIO, cfg: &CliConfig) -> Result<(), UsbError> {
        let _speed_hz = match self.parse_speed(speed) {
            Ok(s) => s,
            Err(e) => {
                out.write_str("Error: ")?;
                out.write_str(e)?;
                out.write_str("\r\n")?;
                return Ok(());
            }
        };

        let spi_mode = match self.parse_mode(mode) {
            Ok(m) => m,
            Err(e) => {
                out.write_str("Error: ")?;
                out.write_str(e)?;
                out.write_str("\r\n")?;
                return Ok(());
            }
        };

        let msb_first = match bit_order {
            "0" => true,  // MSB first
            "1" => false, // LSB first
            _ => {
                out.write_str("Error: Bit order must be 0 (MSB) or 1 (LSB)\r\n")?;
                return Ok(());
            }
        };

        // Store configuration
        cortex_m::interrupt::free(|cs| {
            if spi_num == 1 {
                let mut config = SPI1_CONFIG.borrow(cs).borrow_mut();
                config.mode = spi_mode;
                config.msb_first = msb_first;
            } else {
                let mut config = SPI2_CONFIG.borrow(cs).borrow_mut();
                config.mode = spi_mode;
                config.msb_first = msb_first;
            }
        });

        // TODO: Reconfigure SPI peripheral with new settings
        // This requires reinitializing the SPI peripheral

        if cfg.is_short_output() {
            out.write_str("OK\r\n")?;
        } else {
            out.write_str("SPI")?;
            out.write_str(if spi_num == 1 { "1" } else { "2" })?;
            out.write_str(" configured\r\n")?;
        }
        Ok(())
    }

    fn write_data(&self, spi_num: u8, data_str: &str, out: &mut SerialIO, cfg: &CliConfig) -> Result<(), UsbError> {
        let data = match self.parse_hex_data(data_str) {
            Ok(d) => d,
            Err(e) => {
                out.write_str("Error: ")?;
                out.write_str(e)?;
                out.write_str("\r\n")?;
                return Ok(());
            }
        };

        // Calculate actual byte count
        let s = data_str.trim_start_matches("0x").trim_start_matches("0X");
        let byte_count = s.len() / 2;

        let result = cortex_m::interrupt::free(|cs| {
            if spi_num == 1 {
                let mut spi_opt = SPI1.borrow(cs).borrow_mut();
                if let Some(spi) = spi_opt.as_mut() {
                    if let Err(_) = spi.write(&data[..byte_count]) {
                        return Err("Write failed");
                    }
                    Ok(())
                } else {
                    Err("SPI not initialized")
                }
            } else {
                let mut spi_opt = SPI2.borrow(cs).borrow_mut();
                if let Some(spi) = spi_opt.as_mut() {
                    if let Err(_) = spi.write(&data[..byte_count]) {
                        return Err("Write failed");
                    }
                    Ok(())
                } else {
                    Err("SPI not initialized")
                }
            }
        });

        match result {
            Ok(_) => {
                if cfg.is_short_output() {
                    out.write_str("OK\r\n")?;
                } else {
                    out.write_str("Wrote ")?;
                    let mut buf = [0u8; 10];
                    if let Some(s) = format_u32(byte_count as u32, &mut buf) {
                        out.write_str(s)?;
                    }
                    out.write_str(" byte(s)\r\n")?;
                }
            }
            Err(e) => {
                if !cfg.is_short_output() {
                    out.write_str("Error: ")?;
                    out.write_str(e)?;
                    out.write_str("\r\n")?;
                }
            }
        }

        Ok(())
    }

    fn read_data(&self, spi_num: u8, byte_count: usize, out: &mut SerialIO, cfg: &CliConfig) -> Result<(), UsbError> {
        if byte_count > 256 {
            out.write_str("Error: Max 256 bytes\r\n")?;
            return Ok(());
        }

        let mut rx_data = [0u8; 256];

        let result = cortex_m::interrupt::free(|cs| {
            if spi_num == 1 {
                let mut spi_opt = SPI1.borrow(cs).borrow_mut();
                if let Some(spi) = spi_opt.as_mut() {
                    if let Err(_) = spi.read(&mut rx_data[..byte_count]) {
                        return Err("Read failed");
                    }
                    Ok(())
                } else {
                    Err("SPI not initialized")
                }
            } else {
                let mut spi_opt = SPI2.borrow(cs).borrow_mut();
                if let Some(spi) = spi_opt.as_mut() {
                    if let Err(_) = spi.read(&mut rx_data[..byte_count]) {
                        return Err("Read failed");
                    }
                    Ok(())
                } else {
                    Err("SPI not initialized")
                }
            }
        });

        match result {
            Ok(_) => {
                // Output received data
                for i in 0..byte_count {
                    let mut buf = [0u8; 4];
                    if let Some(s) = self.format_hex_u8(rx_data[i], &mut buf) {
                        out.write_str(s)?;
                        if i < byte_count - 1 {
                            out.write_str(" ")?;
                        }
                    }
                }
                out.write_str("\r\n")?;
            }
            Err(e) => {
                if !cfg.is_short_output() {
                    out.write_str("Error: ")?;
                    out.write_str(e)?;
                    out.write_str("\r\n")?;
                }
            }
        }

        Ok(())
    }

    fn transfer_data(&self, spi_num: u8, data_str: &str, out: &mut SerialIO, cfg: &CliConfig) -> Result<(), UsbError> {
        let tx_data = match self.parse_hex_data(data_str) {
            Ok(d) => d,
            Err(e) => {
                out.write_str("Error: ")?;
                out.write_str(e)?;
                out.write_str("\r\n")?;
                return Ok(());
            }
        };

        // Calculate actual byte count
        let s = data_str.trim_start_matches("0x").trim_start_matches("0X");
        let byte_count = s.len() / 2;

        let mut rx_data = [0u8; 256];

        let result = cortex_m::interrupt::free(|cs| {
            if spi_num == 1 {
                let mut spi_opt = SPI1.borrow(cs).borrow_mut();
                if let Some(spi) = spi_opt.as_mut() {
                    if let Err(_) = spi.transfer(&mut rx_data[..byte_count], &tx_data[..byte_count]) {
                        return Err("Transfer failed");
                    }
                    Ok(())
                } else {
                    Err("SPI not initialized")
                }
            } else {
                let mut spi_opt = SPI2.borrow(cs).borrow_mut();
                if let Some(spi) = spi_opt.as_mut() {
                    if let Err(_) = spi.transfer(&mut rx_data[..byte_count], &tx_data[..byte_count]) {
                        return Err("Transfer failed");
                    }
                    Ok(())
                } else {
                    Err("SPI not initialized")
                }
            }
        });

        match result {
            Ok(_) => {
                // Output received data
                for i in 0..byte_count {
                    let mut buf = [0u8; 4];
                    if let Some(s) = self.format_hex_u8(rx_data[i], &mut buf) {
                        out.write_str(s)?;
                        if i < byte_count - 1 {
                            out.write_str(" ")?;
                        }
                    }
                }
                out.write_str("\r\n")?;
            }
            Err(e) => {
                if !cfg.is_short_output() {
                    out.write_str("Error: ")?;
                    out.write_str(e)?;
                    out.write_str("\r\n")?;
                }
            }
        }

        Ok(())
    }

    fn burst_read(&self, spi_num: u8, address_str: &str, byte_count: usize, out: &mut SerialIO, cfg: &CliConfig) -> Result<(), UsbError> {
        let address = match self.parse_hex_byte(address_str) {
            Ok(a) => a,
            Err(e) => {
                out.write_str("Error: ")?;
                out.write_str(e)?;
                out.write_str("\r\n")?;
                return Ok(());
            }
        };

        if byte_count > 256 {
            out.write_str("Error: Max 256 bytes\r\n")?;
            return Ok(());
        }

        let mut rx_data = [0u8; 256];
        let addr_byte = [address | 0x80]; // Set read bit

        let result = cortex_m::interrupt::free(|cs| {
            if spi_num == 1 {
                let mut spi_opt = SPI1.borrow(cs).borrow_mut();
                if let Some(spi) = spi_opt.as_mut() {
                    // Send address byte with read bit set
                    if let Err(_) = spi.write(&addr_byte) {
                        return Err("Burst read failed");
                    }
                    
                    // Read bytes
                    if let Err(_) = spi.read(&mut rx_data[..byte_count]) {
                        return Err("Burst read failed");
                    }
                    Ok(())
                } else {
                    Err("SPI not initialized")
                }
            } else {
                let mut spi_opt = SPI2.borrow(cs).borrow_mut();
                if let Some(spi) = spi_opt.as_mut() {
                    // Send address byte with read bit set
                    if let Err(_) = spi.write(&addr_byte) {
                        return Err("Burst read failed");
                    }
                    
                    // Read bytes
                    if let Err(_) = spi.read(&mut rx_data[..byte_count]) {
                        return Err("Burst read failed");
                    }
                    Ok(())
                } else {
                    Err("SPI not initialized")
                }
            }
        });

        match result {
            Ok(_) => {
                // Output received data
                for i in 0..byte_count {
                    let mut buf = [0u8; 4];
                    if let Some(s) = self.format_hex_u8(rx_data[i], &mut buf) {
                        out.write_str(s)?;
                        if i < byte_count - 1 {
                            out.write_str(" ")?;
                        }
                    }
                }
                out.write_str("\r\n")?;
            }
            Err(e) => {
                if !cfg.is_short_output() {
                    out.write_str("Error: ")?;
                    out.write_str(e)?;
                    out.write_str("\r\n")?;
                }
            }
        }

        Ok(())
    }

    fn burst_write(&self, spi_num: u8, address_str: &str, data_str: &str, out: &mut SerialIO, cfg: &CliConfig) -> Result<(), UsbError> {
        let address = match self.parse_hex_byte(address_str) {
            Ok(a) => a,
            Err(e) => {
                out.write_str("Error: ")?;
                out.write_str(e)?;
                out.write_str("\r\n")?;
                return Ok(());
            }
        };

        let tx_data = match self.parse_hex_data(data_str) {
            Ok(d) => d,
            Err(e) => {
                out.write_str("Error: ")?;
                out.write_str(e)?;
                out.write_str("\r\n")?;
                return Ok(());
            }
        };

        // Calculate actual byte count
        let s = data_str.trim_start_matches("0x").trim_start_matches("0X");
        let byte_count = s.len() / 2;

        let addr_byte = [address & 0x7F]; // Clear write bit (for devices where bit 7 = R/W)

        let result = cortex_m::interrupt::free(|cs| {
            if spi_num == 1 {
                let mut spi_opt = SPI1.borrow(cs).borrow_mut();
                if let Some(spi) = spi_opt.as_mut() {
                    // Send address byte (write bit clear)
                    if let Err(_) = spi.write(&addr_byte) {
                        return Err("Burst write failed");
                    }

                    // Write bytes
                    if let Err(_) = spi.write(&tx_data[..byte_count]) {
                        return Err("Burst write failed");
                    }
                    Ok(())
                } else {
                    Err("SPI not initialized")
                }
            } else {
                let mut spi_opt = SPI2.borrow(cs).borrow_mut();
                if let Some(spi) = spi_opt.as_mut() {
                    // Send address byte (write bit clear)
                    if let Err(_) = spi.write(&addr_byte) {
                        return Err("Burst write failed");
                    }

                    // Write bytes
                    if let Err(_) = spi.write(&tx_data[..byte_count]) {
                        return Err("Burst write failed");
                    }
                    Ok(())
                } else {
                    Err("SPI not initialized")
                }
            }
        });

        match result {
            Ok(_) => {
                if cfg.is_short_output() {
                    out.write_str("OK\r\n")?;
                } else {
                    out.write_str("Wrote ")?;
                    let mut buf = [0u8; 10];
                    if let Some(s) = format_u32(byte_count as u32, &mut buf) {
                        out.write_str(s)?;
                    }
                    out.write_str(" byte(s) to address ")?;
                    let mut buf = [0u8; 4];
                    if let Some(s) = self.format_hex_u8(address, &mut buf) {
                        out.write_str(s)?;
                    }
                    out.write_str("\r\n")?;
                }
            }
            Err(e) => {
                if !cfg.is_short_output() {
                    out.write_str("Error: ")?;
                    out.write_str(e)?;
                    out.write_str("\r\n")?;
                }
            }
        }

        Ok(())
    }
}

impl Command for SpiCommand {
    fn name(&self) -> &'static str {
        "spi"
    }

    fn initialize(&mut self) -> Result<(), &'static str> {
        Ok(())
    }

    fn execute(
        &mut self,
        args: &[&str],
        out: &mut SerialIO,
        cfg: &mut CliConfig,
    ) -> Result<(), UsbError> {
        if args.len() < 2 {
            out.write_str("Usage: spi <1|2> <config|write|read|transfer|burst_read|burst_write> [args]\r\n")?;
            out.write_str("Type 'help spi' for more information\r\n")?;
            return Ok(());
        }

        // Parse SPI number
        let spi_num = match self.parse_spi_num(args[0]) {
            Ok(n) => n,
            Err(e) => {
                out.write_str(e)?;
                out.write_str("\r\n")?;
                return Ok(());
            }
        };

        match args[1] {
            "config" => {
                if args.len() != 5 {
                    out.write_str("Usage: spi <1|2> config <speed> <mode> <bit_order>\r\n")?;
                    return Ok(());
                }
                self.config_spi(spi_num, args[2], args[3], args[4], out, cfg)
            }
            "write" => {
                if args.len() != 3 {
                    out.write_str("Usage: spi <1|2> write <data_hex>\r\n")?;
                    return Ok(());
                }
                self.write_data(spi_num, args[2], out, cfg)
            }
            "read" => {
                if args.len() != 3 {
                    out.write_str("Usage: spi <1|2> read <byte_count>\r\n")?;
                    return Ok(());
                }
                let byte_count = match self.parse_int(args[2]) {
                    Ok(n) => n,
                    Err(e) => {
                        out.write_str(e)?;
                        out.write_str("\r\n")?;
                        return Ok(());
                    }
                };
                self.read_data(spi_num, byte_count, out, cfg)
            }
            "transfer" => {
                if args.len() != 3 {
                    out.write_str("Usage: spi <1|2> transfer <data_hex>\r\n")?;
                    return Ok(());
                }
                self.transfer_data(spi_num, args[2], out, cfg)
            }
            "burst_read" => {
                if args.len() != 4 {
                    out.write_str("Usage: spi <1|2> burst_read <address> <byte_count>\r\n")?;
                    return Ok(());
                }
                let byte_count = match self.parse_int(args[3]) {
                    Ok(n) => n,
                    Err(e) => {
                        out.write_str(e)?;
                        out.write_str("\r\n")?;
                        return Ok(());
                    }
                };
                self.burst_read(spi_num, args[2], byte_count, out, cfg)
            }
            "burst_write" => {
                if args.len() != 4 {
                    out.write_str("Usage: spi <1|2> burst_write <address> <data_hex>\r\n")?;
                    return Ok(());
                }
                self.burst_write(spi_num, args[2], args[3], out, cfg)
            }
            _ => {
                out.write_str("Unknown subcommand\r\n")?;
                Ok(())
            }
        }
    }

    fn print_help(&self, out: &mut SerialIO) -> Result<(), UsbError> {
        out.write_str("spi <1|2> <cmd> [args] - SPI bus operations\r\n")?;
        out.write_str("\r\nCommands:\r\n")?;
        out.write_str("  spi <1|2> config <speed> <mode> <bit_order>\r\n")?;
        out.write_str("    - speed: Baud rate (e.g., \"1m\", \"5m\", \"10m\")\r\n")?;
        out.write_str("    - mode: 0-3 (clock polarity/phase)\r\n")?;
        out.write_str("    - bit_order: 0=MSB first, 1=LSB first\r\n")?;
        out.write_str("  spi <1|2> write <data_hex>\r\n")?;
        out.write_str("    - Write data only (hex format)\r\n")?;
        out.write_str("  spi <1|2> read <byte_count>\r\n")?;
        out.write_str("    - Read data only (sends dummy bytes)\r\n")?;
        out.write_str("  spi <1|2> transfer <data_hex>\r\n")?;
        out.write_str("    - Full-duplex transfer (simultaneous write/read)\r\n")?;
        out.write_str("  spi <1|2> burst_read <address> <byte_count>\r\n")?;
        out.write_str("    - Read sequential data from address\r\n")?;
        out.write_str("  spi <1|2> burst_write <address> <data_hex>\r\n")?;
        out.write_str("    - Write sequential data to address\r\n")?;
        out.write_str("\r\nExamples:\r\n")?;
        out.write_str("  spi 1 config 1m 0 0        - 1MHz, mode 0, MSB first\r\n")?;
        out.write_str("  spi 1 write 0xAB 0xCD      - Write 2 bytes\r\n")?;
        out.write_str("  spi 1 read 4               - Read 4 bytes\r\n")?;
        out.write_str("  spi 1 transfer 0x0F 0x0D   - Write/read simultaneously\r\n")?;
        out.write_str("  spi 1 burst_read 0x1000 256 - Read 256 bytes from 0x1000\r\n")?;
        out.write_str("  spi 1 burst_write 0x1000 0xE4 - Write 0xE4 to 0x1000\r\n")?;
        Ok(())
    }
}

fn format_u32(mut n: u32, buf: &mut [u8; 10]) -> Option<&str> {
    if n == 0 {
        buf[0] = b'0';
        return core::str::from_utf8(&buf[..1]).ok();
    }
    
    let mut pos = 10;
    while n > 0 {
        pos -= 1;
        buf[pos] = b'0' + (n % 10) as u8;
        n /= 10;
    }
    
    core::str::from_utf8(&buf[pos..]).ok()
}
