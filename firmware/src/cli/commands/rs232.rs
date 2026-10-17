use crate::cli::{CliConfig, Command};
use crate::io_interface::serial::SerialIO;
use usb_device::UsbError;
use stm32f4xx_hal as hal;
use hal::serial::{Rx, Tx, Serial};
use hal::serial::config::{Parity, StopBits};
use hal::prelude::*;
use cortex_m::interrupt::Mutex;
use core::cell::RefCell;
use nb::block;

type Rs232Rx = Rx<hal::pac::USART3>;
type Rs232Tx = Tx<hal::pac::USART3>;

static RS232_RX: Mutex<RefCell<Option<Rs232Rx>>> = Mutex::new(RefCell::new(None));
static RS232_TX: Mutex<RefCell<Option<Rs232Tx>>> = Mutex::new(RefCell::new(None));
static RS232_CONFIG: Mutex<RefCell<Rs232Config>> = Mutex::new(RefCell::new(Rs232Config::default()));
static RS232_PCLK1_HZ: Mutex<RefCell<u32>> = Mutex::new(RefCell::new(0));

#[derive(Clone, Copy)]
struct Rs232Config {
    baud_rate: u32,
    parity: Parity,
    stop_bits: StopBits,
    append_cr: bool,
}

impl Rs232Config {
    const fn default() -> Self {
        Self {
            baud_rate: 9600,
            parity: Parity::ParityNone,
            stop_bits: StopBits::STOP1,
            append_cr: false,
        }
    }
}

#[derive(Clone)]
pub struct Rs232Command;

impl Rs232Command {
    pub fn new() -> Self { 
        Self 
    }

    pub fn init(serial: Serial<hal::pac::USART3>, pclk1_hz: u32) {
        let (tx, rx) = serial.split();
        cortex_m::interrupt::free(|cs| {
            *RS232_RX.borrow(cs).borrow_mut() = Some(rx);
            *RS232_TX.borrow(cs).borrow_mut() = Some(tx);
            *RS232_PCLK1_HZ.borrow(cs).borrow_mut() = pclk1_hz;
        });
    }

    fn apply_hardware_config(&self) -> Result<(), &'static str> {
        cortex_m::interrupt::free(|cs| {
            let cfg = *RS232_CONFIG.borrow(cs).borrow();
            let pclk1_hz = *RS232_PCLK1_HZ.borrow(cs).borrow();

            if pclk1_hz == 0 {
                return Err("RS232 clock is not initialized");
            }
            if cfg.baud_rate == 0 {
                return Err("Baud rate must be greater than 0");
            }

            let brr = (pclk1_hz + (cfg.baud_rate / 2)) / cfg.baud_rate;
            if brr == 0 {
                return Err("Baud rate is too high for current peripheral clock");
            }
            if brr > u16::MAX as u32 {
                return Err("Baud rate is too low for current peripheral clock");
            }
            let brr = brr as u16;

            let usart3 = unsafe { &*hal::pac::USART3::ptr() };

            // Disable UART before touching format/baud bits, then restore UE.
            usart3.cr1().modify(|_, w| w.ue().clear_bit());

            usart3.brr().write(|w| unsafe { w.bits(brr) });

            usart3.cr2().modify(|r, w| {
                let mut bits = r.bits() & !(0b11 << 12);
                bits |= match cfg.stop_bits {
                    StopBits::STOP1 => 0b00 << 12,
                    StopBits::STOP2 => 0b10 << 12,
                    _ => 0b00 << 12,
                };
                unsafe { w.bits(bits) }
            });

            usart3.cr1().modify(|r, w| {
                let mut bits = r.bits();
                bits &= !(1 << 9); // PS
                bits &= !(1 << 10); // PCE
                match cfg.parity {
                    Parity::ParityNone => {}
                    Parity::ParityEven => {
                        bits |= 1 << 10;
                    }
                    Parity::ParityOdd => {
                        bits |= 1 << 10;
                        bits |= 1 << 9;
                    }
                }
                bits |= 1 << 13; // UE
                unsafe { w.bits(bits) }
            });

            Ok(())
        })
    }

    fn parse_int(&self, arg: &str) -> Result<u32, &'static str> {
        arg.parse::<u32>().map_err(|_| "Invalid integer value")
    }

    fn set_stop_bits(&self, value: u32, out: &mut SerialIO, cfg: &CliConfig) -> Result<(), UsbError> {
        let stop_bits = match value {
            1 => StopBits::STOP1,
            2 => StopBits::STOP2,
            _ => {
                out.write_str("Error: Stop bits must be 1 or 2\r\n")?;
                return Ok(());
            }
        };

        cortex_m::interrupt::free(|cs| {
            RS232_CONFIG.borrow(cs).borrow_mut().stop_bits = stop_bits;
        });
        if let Err(e) = self.apply_hardware_config() {
            out.write_str("Error: ")?;
            out.write_str(e)?;
            out.write_str("\r\n")?;
            return Ok(());
        }

        if cfg.is_short_output() {
            out.write_str("OK\r\n")?;
        } else {
            out.write_str("RS232 stop bits set to ")?;
            out.write_str(if value == 1 { "1" } else { "2" })?;
            out.write_str("\r\n")?;
        }
        Ok(())
    }

    fn set_parity(&self, value: u32, out: &mut SerialIO, cfg: &CliConfig) -> Result<(), UsbError> {
        let parity = match value {
            0 => Parity::ParityNone,
            1 => Parity::ParityEven,
            2 => Parity::ParityOdd,
            _ => {
                out.write_str("Error: Parity must be 0 (none), 1 (even), or 2 (odd)\r\n")?;
                return Ok(());
            }
        };

        cortex_m::interrupt::free(|cs| {
            RS232_CONFIG.borrow(cs).borrow_mut().parity = parity;
        });
        if let Err(e) = self.apply_hardware_config() {
            out.write_str("Error: ")?;
            out.write_str(e)?;
            out.write_str("\r\n")?;
            return Ok(());
        }

        if cfg.is_short_output() {
            out.write_str("OK\r\n")?;
        } else {
            let parity_str = match value {
                0 => "none",
                1 => "even",
                2 => "odd",
                _ => unreachable!(),
            };
            out.write_str("RS232 parity set to ")?;
            out.write_str(parity_str)?;
            out.write_str("\r\n")?;
        }
        Ok(())
    }

    fn set_baud(&self, value: u32, out: &mut SerialIO, cfg: &CliConfig) -> Result<(), UsbError> {
        if value == 0 {
            out.write_str("Error: Baud rate must be greater than 0\r\n")?;
            return Ok(());
        }

        cortex_m::interrupt::free(|cs| {
            RS232_CONFIG.borrow(cs).borrow_mut().baud_rate = value;
        });
        if let Err(e) = self.apply_hardware_config() {
            out.write_str("Error: ")?;
            out.write_str(e)?;
            out.write_str("\r\n")?;
            return Ok(());
        }

        if cfg.is_short_output() {
            out.write_str("OK\r\n")?;
        } else {
            out.write_str("RS232 baud rate set to ")?;
            let mut buf = [0u8; 10];
            if let Some(s) = format_u32(value, &mut buf) {
                out.write_str(s)?;
            }
            out.write_str("\r\n")?;
        }
        Ok(())
    }

    fn set_cr(&self, value: u32, out: &mut SerialIO, cfg: &CliConfig) -> Result<(), UsbError> {
        let append_cr = match value {
            0 => false,
            1 => true,
            _ => {
                out.write_str("Error: CR mode must be 0 (disabled) or 1 (enabled)\r\n")?;
                return Ok(());
            }
        };

        cortex_m::interrupt::free(|cs| {
            RS232_CONFIG.borrow(cs).borrow_mut().append_cr = append_cr;
        });

        if cfg.is_short_output() {
            out.write_str("OK\r\n")?;
        } else {
            out.write_str("RS232 CR append ")?;
            out.write_str(if append_cr { "enabled" } else { "disabled" })?;
            out.write_str("\r\n")?;
        }
        Ok(())
    }

    fn write_string(&self, args: &[&str], out: &mut SerialIO, cfg: &CliConfig) -> Result<(), UsbError> {
        cortex_m::interrupt::free(|cs| {
            let append_cr = RS232_CONFIG.borrow(cs).borrow().append_cr;
            if let Some(tx) = RS232_TX.borrow(cs).borrow_mut().as_mut() {
                for (i, arg) in args.iter().enumerate() {
                    if i > 0 {
                        for &b in b" " {
                            let _ = block!(tx.write(b));
                        }
                    }
                    for &b in arg.as_bytes() {
                        let _ = block!(tx.write(b));
                    }
                }
                if append_cr {
                    let _ = block!(tx.write(b'\r'));
                }
            }
        });

        if cfg.is_short_output() {
            out.write_str("OK\r\n")?;
        } else {
            out.write_str("Sent to RS232\r\n")?;
        }
        Ok(())
    }

    fn passthrough(&self, out: &mut SerialIO) -> Result<(), UsbError> {
        out.write_str("RS232 passthrough mode. Press ESC to exit\r\n")?;
        
        loop {
            out.poll();

            // RS232 RX -> CLI
            cortex_m::interrupt::free(|cs| {
                if let Some(rx) = RS232_RX.borrow(cs).borrow_mut().as_mut() {
                    loop {
                        match rx.read() {
                            Ok(b) => { 
                                let _ = out.write_bytes(&[b]); 
                            }
                            Err(nb::Error::WouldBlock) => break,
                            Err(_) => break,
                        }
                    }
                }
            });

            // CLI -> RS232 TX
            while let Some(b) = out.read_byte() {
                if b == 0x1B { // ESC key
                    out.write_str("\r\nExiting passthrough mode\r\n")?;
                    return Ok(());
                }
                
                cortex_m::interrupt::free(|cs| {
                    if let Some(tx) = RS232_TX.borrow(cs).borrow_mut().as_mut() {
                        let _ = block!(tx.write(b));
                    }
                });
            }
        }
    }
}

impl Command for Rs232Command {
    fn name(&self) -> &'static str { 
        "rs232" 
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
        if args.is_empty() {
            out.write_str("Usage: rs232 <stop_bit|parity|baud|cr|write|passthrough> [args]\r\n")?;
            out.write_str("Type 'help rs232' for more information\r\n")?;
            return Ok(());
        }

        match args[0] {
            "stop_bit" => {
                if args.len() != 2 {
                    out.write_str("Usage: rs232 stop_bit <1|2>\r\n")?;
                    return Ok(());
                }
                let value = match self.parse_int(args[1]) {
                    Ok(v) => v,
                    Err(e) => {
                        out.write_str(e)?;
                        out.write_str("\r\n")?;
                        return Ok(());
                    }
                };
                self.set_stop_bits(value, out, cfg)
            }
            "parity" => {
                if args.len() != 2 {
                    out.write_str("Usage: rs232 parity <0|1|2>\r\n")?;
                    out.write_str("  0 = None, 1 = Even, 2 = Odd\r\n")?;
                    return Ok(());
                }
                let value = match self.parse_int(args[1]) {
                    Ok(v) => v,
                    Err(e) => {
                        out.write_str(e)?;
                        out.write_str("\r\n")?;
                        return Ok(());
                    }
                };
                self.set_parity(value, out, cfg)
            }
            "baud" => {
                if args.len() != 2 {
                    out.write_str("Usage: rs232 baud <rate>\r\n")?;
                    out.write_str("Common rates: 9600, 19200, 38400, 57600, 115200\r\n")?;
                    return Ok(());
                }
                let value = match self.parse_int(args[1]) {
                    Ok(v) => v,
                    Err(e) => {
                        out.write_str(e)?;
                        out.write_str("\r\n")?;
                        return Ok(());
                    }
                };
                self.set_baud(value, out, cfg)
            }
            "cr" => {
                if args.len() != 2 {
                    out.write_str("Usage: rs232 cr <0|1>\r\n")?;
                    out.write_str("  0 = Disabled, 1 = Enabled\r\n")?;
                    return Ok(());
                }
                let value = match self.parse_int(args[1]) {
                    Ok(v) => v,
                    Err(e) => {
                        out.write_str(e)?;
                        out.write_str("\r\n")?;
                        return Ok(());
                    }
                };
                self.set_cr(value, out, cfg)
            }
            "write" => {
                if args.len() < 2 {
                    out.write_str("Usage: rs232 write <text>\r\n")?;
                    return Ok(());
                }
                self.write_string(&args[1..], out, cfg)
            }
            "passthrough" => {
                if args.len() != 1 {
                    out.write_str("Usage: rs232 passthrough\r\n")?;
                    return Ok(());
                }
                self.passthrough(out)
            }
            _ => {
                out.write_str("Unknown subcommand: ")?;
                out.write_str(args[0])?;
                out.write_str("\r\n")?;
                out.write_str("Available: stop_bit, parity, baud, cr, write, passthrough\r\n")?;
                Ok(())
            }
        }
    }

    fn print_help(&self, out: &mut SerialIO) -> Result<(), UsbError> {
        out.write_str("rs232 <subcommand> [args] - RS232 serial communication\r\n")?;
        out.write_str("RS232 uses USART3 and applies baud/parity/stop settings via BRR/CR1/CR2 registers\r\n")?;
        out.write_str("Subcommands:\r\n")?;
        out.write_str("  stop_bit <1|2>    - Set stop bits (1 or 2)\r\n")?;
        out.write_str("  parity <0|1|2>    - Set parity (0=none, 1=even, 2=odd)\r\n")?;
        out.write_str("  baud <rate>       - Set baud rate (e.g., 9600, 115200)\r\n")?;
        out.write_str("  cr <0|1>          - Append CR (\\r) after each write (0=off, 1=on)\r\n")?;
        out.write_str("  write <text>      - Write string to RS232 TX\r\n")?;
        out.write_str("  passthrough       - Enter passthrough mode (ESC to exit)\r\n")?;
        out.write_str("\r\nExamples:\r\n")?;
        out.write_str("  rs232 baud 115200\r\n")?;
        out.write_str("  rs232 cr 1\r\n")?;
        out.write_str("  rs232 write Hello World\r\n")?;
        out.write_str("  rs232 passthrough\r\n")?;
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
