use crate::cli::{CliConfig, Command};
use crate::io_interface::serial::SerialIO;
use usb_device::UsbError;
use stm32f4xx_hal as hal;
use hal::serial::{Rx, Tx, Serial};
use hal::serial::config::{Parity, StopBits};
use hal::prelude::*;
use hal::gpio::{Output, PushPull, PE9, PB2};
use cortex_m::interrupt::Mutex;
use core::cell::RefCell;
use nb::block;

type Rs422Rx = Rx<hal::pac::UART5>;
type Rs422Tx = Tx<hal::pac::UART5>;
type RePin = PE9<Output<PushPull>>;
type DePin = PB2<Output<PushPull>>;

static RS422_RX: Mutex<RefCell<Option<Rs422Rx>>> = Mutex::new(RefCell::new(None));
static RS422_TX: Mutex<RefCell<Option<Rs422Tx>>> = Mutex::new(RefCell::new(None));
static RS422_RE: Mutex<RefCell<Option<RePin>>> = Mutex::new(RefCell::new(None));
static RS422_DE: Mutex<RefCell<Option<DePin>>> = Mutex::new(RefCell::new(None));
static RS422_CONFIG: Mutex<RefCell<Rs422Config>> = Mutex::new(RefCell::new(Rs422Config::default()));

#[derive(Clone, Copy)]
struct Rs422Config {
    baud_rate: u32,
    parity: Parity,
    stop_bits: StopBits,
}

impl Rs422Config {
    const fn default() -> Self {
        Self {
            baud_rate: 9600,
            parity: Parity::ParityNone,
            stop_bits: StopBits::STOP1,
        }
    }
}

#[derive(Clone)]
pub struct Rs422Command;

impl Rs422Command {
    pub fn new() -> Self { 
        Self 
    }

    pub fn init(serial: Serial<hal::pac::UART5>, mut re_pin: RePin, mut de_pin: DePin) {
        // Initialize control pins to safe state (receiver enabled, driver disabled)
        re_pin.set_low(); // Active LOW - enable receiver
        de_pin.set_low(); // Active HIGH - disable driver
        
        let (tx, rx) = serial.split();
        cortex_m::interrupt::free(|cs| {
            *RS422_RX.borrow(cs).borrow_mut() = Some(rx);
            *RS422_TX.borrow(cs).borrow_mut() = Some(tx);
            *RS422_RE.borrow(cs).borrow_mut() = Some(re_pin);
            *RS422_DE.borrow(cs).borrow_mut() = Some(de_pin);
        });
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
            RS422_CONFIG.borrow(cs).borrow_mut().stop_bits = stop_bits;
        });

        if cfg.is_short_output() {
            out.write_str("OK\r\n")?;
        } else {
            out.write_str("RS422 stop bits set to ")?;
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
            RS422_CONFIG.borrow(cs).borrow_mut().parity = parity;
        });

        if cfg.is_short_output() {
            out.write_str("OK\r\n")?;
        } else {
            let parity_str = match value {
                0 => "none",
                1 => "even",
                2 => "odd",
                _ => unreachable!(),
            };
            out.write_str("RS422 parity set to ")?;
            out.write_str(parity_str)?;
            out.write_str("\r\n")?;
        }
        Ok(())
    }

    fn set_baud(&self, value: u32, out: &mut SerialIO, cfg: &CliConfig) -> Result<(), UsbError> {
        cortex_m::interrupt::free(|cs| {
            RS422_CONFIG.borrow(cs).borrow_mut().baud_rate = value;
        });

        if cfg.is_short_output() {
            out.write_str("OK\r\n")?;
        } else {
            out.write_str("RS422 baud rate set to ")?;
            let mut buf = [0u8; 10];
            if let Some(s) = format_u32(value, &mut buf) {
                out.write_str(s)?;
            }
            out.write_str("\r\n")?;
        }
        Ok(())
    }

    fn write_string(&self, args: &[&str], out: &mut SerialIO, cfg: &CliConfig) -> Result<(), UsbError> {
        cortex_m::interrupt::free(|cs| {
            // Enable driver, disable receiver for transmitting
            if let Some(de) = RS422_DE.borrow(cs).borrow_mut().as_mut() {
                de.set_high();
            }
            if let Some(re) = RS422_RE.borrow(cs).borrow_mut().as_mut() {
                re.set_high();
            }
            
            if let Some(tx) = RS422_TX.borrow(cs).borrow_mut().as_mut() {
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
            }
            
            // Small delay to ensure transmission completes
            for _ in 0..1000 {
                cortex_m::asm::nop();
            }
            
            // Disable driver, enable receiver after transmission
            if let Some(de) = RS422_DE.borrow(cs).borrow_mut().as_mut() {
                de.set_low();
            }
            if let Some(re) = RS422_RE.borrow(cs).borrow_mut().as_mut() {
                re.set_low();
            }
        });

        if cfg.is_short_output() {
            out.write_str("OK\r\n")?;
        } else {
            out.write_str("Sent to RS422\r\n")?;
        }
        Ok(())
    }

    fn passthrough(&self, out: &mut SerialIO) -> Result<(), UsbError> {
        out.write_str("RS422 passthrough mode. Press ESC to exit\r\n")?;
        
        // Enable both receiver and driver for full-duplex communication
        cortex_m::interrupt::free(|cs| {
            if let Some(re) = RS422_RE.borrow(cs).borrow_mut().as_mut() {
                re.set_low(); // Enable receiver (active LOW)
            }
            if let Some(de) = RS422_DE.borrow(cs).borrow_mut().as_mut() {
                de.set_high(); // Enable driver (active HIGH)
            }
        });
        
        loop {
            out.poll();

            // RS422 RX -> CLI
            cortex_m::interrupt::free(|cs| {
                if let Some(rx) = RS422_RX.borrow(cs).borrow_mut().as_mut() {
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

            // CLI -> RS422 TX
            while let Some(b) = out.read_byte() {
                if b == 0x1B { // ESC key
                    // Restore to safe state before exiting
                    cortex_m::interrupt::free(|cs| {
                        if let Some(de) = RS422_DE.borrow(cs).borrow_mut().as_mut() {
                            de.set_low(); // Disable driver
                        }
                        if let Some(re) = RS422_RE.borrow(cs).borrow_mut().as_mut() {
                            re.set_low(); // Keep receiver enabled
                        }
                    });
                    out.write_str("\r\nExiting passthrough mode\r\n")?;
                    return Ok(());
                }
                
                cortex_m::interrupt::free(|cs| {
                    if let Some(tx) = RS422_TX.borrow(cs).borrow_mut().as_mut() {
                        let _ = block!(tx.write(b));
                    }
                });
            }
        }
    }
}

impl Command for Rs422Command {
    fn name(&self) -> &'static str { 
        "rs422" 
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
            out.write_str("Usage: rs422 <stop_bit|parity|baud|write|passthrough> [args]\r\n")?;
            out.write_str("Type 'help rs422' for more information\r\n")?;
            return Ok(());
        }

        match args[0] {
            "stop_bit" => {
                if args.len() != 2 {
                    out.write_str("Usage: rs422 stop_bit <1|2>\r\n")?;
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
                    out.write_str("Usage: rs422 parity <0|1|2>\r\n")?;
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
                    out.write_str("Usage: rs422 baud <rate>\r\n")?;
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
            "write" => {
                if args.len() < 2 {
                    out.write_str("Usage: rs422 write <text>\r\n")?;
                    return Ok(());
                }
                self.write_string(&args[1..], out, cfg)
            }
            "passthrough" => {
                if args.len() != 1 {
                    out.write_str("Usage: rs422 passthrough\r\n")?;
                    return Ok(());
                }
                self.passthrough(out)
            }
            _ => {
                out.write_str("Unknown subcommand: ")?;
                out.write_str(args[0])?;
                out.write_str("\r\n")?;
                out.write_str("Available: stop_bit, parity, baud, write, passthrough\r\n")?;
                Ok(())
            }
        }
    }

    fn print_help(&self, out: &mut SerialIO) -> Result<(), UsbError> {
        out.write_str("rs422 <subcommand> [args] - RS422 serial communication\r\n")?;
        out.write_str("RS422 uses UART5 with DE (PB2) and RE (PE9) control pins\r\n")?;
        out.write_str("Subcommands:\r\n")?;
        out.write_str("  stop_bit <1|2>    - Set stop bits (1 or 2)\r\n")?;
        out.write_str("  parity <0|1|2>    - Set parity (0=none, 1=even, 2=odd)\r\n")?;
        out.write_str("  baud <rate>       - Set baud rate (e.g., 9600, 115200)\r\n")?;
        out.write_str("  write <text>      - Write string to RS422 TX\r\n")?;
        out.write_str("  passthrough       - Enter passthrough mode (ESC to exit)\r\n")?;
        out.write_str("\r\nExamples:\r\n")?;
        out.write_str("  rs422 baud 115200\r\n")?;
        out.write_str("  rs422 write Hello World\r\n")?;
        out.write_str("  rs422 passthrough\r\n")?;
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
