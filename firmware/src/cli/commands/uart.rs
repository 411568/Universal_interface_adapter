use crate::cli::{CliConfig, Command};
use crate::io_interface::serial::SerialIO;
use usb_device::UsbError;
use stm32f4xx_hal as hal;
use hal::serial::{Rx, Tx, Serial};
use hal::serial::config::{Parity, StopBits};
use hal::prelude::*; // This imports the embedded-hal traits
use cortex_m::interrupt::Mutex;
use core::cell::RefCell;
use nb::block;

type Uart1Rx = Rx<hal::pac::USART2>;
type Uart1Tx = Tx<hal::pac::USART2>;
type Uart2Rx = Rx<hal::pac::USART3>;
type Uart2Tx = Tx<hal::pac::USART3>;

// UART 1 (USART2)
static UART1_RX: Mutex<RefCell<Option<Uart1Rx>>> = Mutex::new(RefCell::new(None));
static UART1_TX: Mutex<RefCell<Option<Uart1Tx>>> = Mutex::new(RefCell::new(None));
static UART1_CONFIG: Mutex<RefCell<UartConfig>> = Mutex::new(RefCell::new(UartConfig::default()));

// UART 2 (USART3)
static UART2_RX: Mutex<RefCell<Option<Uart2Rx>>> = Mutex::new(RefCell::new(None));
static UART2_TX: Mutex<RefCell<Option<Uart2Tx>>> = Mutex::new(RefCell::new(None));
static UART2_CONFIG: Mutex<RefCell<UartConfig>> = Mutex::new(RefCell::new(UartConfig::default()));

#[derive(Clone, Copy)]
struct UartConfig {
    baud_rate: u32,
    parity: Parity,
    stop_bits: StopBits,
}

impl UartConfig {
    const fn default() -> Self {
        Self {
            baud_rate: 9600,
            parity: Parity::ParityNone,
            stop_bits: StopBits::STOP1,
        }
    }
}

#[derive(Clone)]
pub struct UartCommand;

impl UartCommand {
    pub fn new() -> Self { 
        Self 
    }

    pub fn init_uart1(serial: Serial<hal::pac::USART2>) {
        let (tx, rx) = serial.split();
        cortex_m::interrupt::free(|cs| {
            *UART1_RX.borrow(cs).borrow_mut() = Some(rx);
            *UART1_TX.borrow(cs).borrow_mut() = Some(tx);
        });
    }

    pub fn init_uart2(serial: Serial<hal::pac::USART3>) {
        let (tx, rx) = serial.split();
        cortex_m::interrupt::free(|cs| {
            *UART2_RX.borrow(cs).borrow_mut() = Some(rx);
            *UART2_TX.borrow(cs).borrow_mut() = Some(tx);
        });
    }

    fn parse_int(&self, arg: &str) -> Result<u32, &'static str> {
        arg.parse::<u32>().map_err(|_| "Invalid integer value")
    }

    fn parse_uart_num(&self, arg: &str) -> Result<u8, &'static str> {
        match arg {
            "1" => Ok(1),
            "2" => Ok(2),
            _ => Err("UART number must be 1 or 2"),
        }
    }

    fn set_stop_bits(&self, uart_num: u8, value: u32, out: &mut SerialIO, cfg: &CliConfig) -> Result<(), UsbError> {
        let stop_bits = match value {
            1 => StopBits::STOP1,
            2 => StopBits::STOP2,
            _ => {
                out.write_str("Error: Stop bits must be 1 or 2\r\n")?;
                return Ok(());
            }
        };

        cortex_m::interrupt::free(|cs| {
            match uart_num {
                1 => UART1_CONFIG.borrow(cs).borrow_mut().stop_bits = stop_bits,
                2 => UART2_CONFIG.borrow(cs).borrow_mut().stop_bits = stop_bits,
                _ => {}
            }
        });

        if cfg.is_short_output() {
            out.write_str("OK\r\n")?;
        } else {
            out.write_str("UART")?;
            out.write_str(if uart_num == 1 { "1" } else { "2" })?;
            out.write_str(" stop bits set to ")?;
            out.write_str(if value == 1 { "1" } else { "2" })?;
            out.write_str("\r\n")?;
        }
        Ok(())
    }

    fn set_parity(&self, uart_num: u8, value: u32, out: &mut SerialIO, cfg: &CliConfig) -> Result<(), UsbError> {
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
            match uart_num {
                1 => UART1_CONFIG.borrow(cs).borrow_mut().parity = parity,
                2 => UART2_CONFIG.borrow(cs).borrow_mut().parity = parity,
                _ => {}
            }
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
            out.write_str("UART")?;
            out.write_str(if uart_num == 1 { "1" } else { "2" })?;
            out.write_str(" parity set to ")?;
            out.write_str(parity_str)?;
            out.write_str("\r\n")?;
        }
        Ok(())
    }

    fn set_baud(&self, uart_num: u8, value: u32, out: &mut SerialIO, cfg: &CliConfig) -> Result<(), UsbError> {
        cortex_m::interrupt::free(|cs| {
            match uart_num {
                1 => UART1_CONFIG.borrow(cs).borrow_mut().baud_rate = value,
                2 => UART2_CONFIG.borrow(cs).borrow_mut().baud_rate = value,
                _ => {}
            }
        });

        if cfg.is_short_output() {
            out.write_str("OK\r\n")?;
        } else {
            out.write_str("UART")?;
            out.write_str(if uart_num == 1 { "1" } else { "2" })?;
            out.write_str(" baud rate set to ")?;
            // Convert u32 to string
            let mut buf = [0u8; 10];
            if let Some(s) = format_u32(value, &mut buf) {
                out.write_str(s)?;
            }
            out.write_str("\r\n")?;
        }
        Ok(())
    }

    fn write_string(&self, uart_num: u8, args: &[&str], out: &mut SerialIO, cfg: &CliConfig) -> Result<(), UsbError> {
        match uart_num {
            1 => {
                cortex_m::interrupt::free(|cs| {
                    if let Some(tx) = UART1_TX.borrow(cs).borrow_mut().as_mut() {
                        // Manually concatenate arguments with spaces
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
                });
            }
            2 => {
                cortex_m::interrupt::free(|cs| {
                    if let Some(tx) = UART2_TX.borrow(cs).borrow_mut().as_mut() {
                        // Manually concatenate arguments with spaces
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
                });
            }
            _ => {}
        }

        if cfg.is_short_output() {
            out.write_str("OK\r\n")?;
        } else {
            out.write_str("Sent to UART")?;
            out.write_str(if uart_num == 1 { "1" } else { "2" })?;
            out.write_str("\r\n")?;
        }
        Ok(())
    }

    fn passthrough(&self, uart_num: u8, out: &mut SerialIO) -> Result<(), UsbError> {
        out.write_str("UART")?;
        out.write_str(if uart_num == 1 { "1" } else { "2" })?;
        out.write_str(" passthrough mode. Press ESC to exit\r\n")?;
        
        match uart_num {
            1 => self.passthrough_uart1(out),
            2 => self.passthrough_uart2(out),
            _ => {
                out.write_str("Invalid UART number\r\n")?;
                Ok(())
            }
        }
    }

    fn passthrough_uart1(&self, out: &mut SerialIO) -> Result<(), UsbError> {
        loop {
            out.poll();

            // UART RX -> CLI
            cortex_m::interrupt::free(|cs| {
                if let Some(rx) = UART1_RX.borrow(cs).borrow_mut().as_mut() {
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

            // CLI -> UART TX
            while let Some(b) = out.read_byte() {
                if b == 0x1B { // ESC key
                    out.write_str("\r\nExiting passthrough mode\r\n")?;
                    return Ok(());
                }
                
                cortex_m::interrupt::free(|cs| {
                    if let Some(tx) = UART1_TX.borrow(cs).borrow_mut().as_mut() {
                        let _ = block!(tx.write(b));
                    }
                });
            }
        }
    }

    fn passthrough_uart2(&self, out: &mut SerialIO) -> Result<(), UsbError> {
        loop {
            out.poll();

            // UART RX -> CLI
            cortex_m::interrupt::free(|cs| {
                if let Some(rx) = UART2_RX.borrow(cs).borrow_mut().as_mut() {
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

            // CLI -> UART TX
            while let Some(b) = out.read_byte() {
                if b == 0x1B { // ESC key
                    out.write_str("\r\nExiting passthrough mode\r\n")?;
                    return Ok(());
                }
                
                cortex_m::interrupt::free(|cs| {
                    if let Some(tx) = UART2_TX.borrow(cs).borrow_mut().as_mut() {
                        let _ = block!(tx.write(b));
                    }
                });
            }
        }
    }
}

impl Command for UartCommand {
    fn name(&self) -> &'static str { 
        "uart" 
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
            out.write_str("Usage: uart <1|2> <stop_bit|parity|baud|write|passthrough> [args]\r\n")?;
            out.write_str("Type 'help uart' for more information\r\n")?;
            return Ok(());
        }

        // Parse UART number (first argument)
        let uart_num = match self.parse_uart_num(args[0]) {
            Ok(n) => n,
            Err(e) => {
                out.write_str(e)?;
                out.write_str("\r\n")?;
                return Ok(());
            }
        };

        // Check if subcommand is provided
        if args.len() < 2 {
            out.write_str("Usage: uart ")?;
            out.write_str(args[0])?;
            out.write_str(" <stop_bit|parity|baud|write|passthrough> [args]\r\n")?;
            return Ok(());
        }

        // Parse subcommand (second argument)
        match args[1] {
            "stop_bit" => {
                if args.len() != 3 {
                    out.write_str("Usage: uart <1|2> stop_bit <1|2>\r\n")?;
                    return Ok(());
                }
                let value = match self.parse_int(args[2]) {
                    Ok(v) => v,
                    Err(e) => {
                        out.write_str(e)?;
                        out.write_str("\r\n")?;
                        return Ok(());
                    }
                };
                self.set_stop_bits(uart_num, value, out, cfg)
            }
            "parity" => {
                if args.len() != 3 {
                    out.write_str("Usage: uart <1|2> parity <0|1|2>\r\n")?;
                    out.write_str("  0 = None, 1 = Even, 2 = Odd\r\n")?;
                    return Ok(());
                }
                let value = match self.parse_int(args[2]) {
                    Ok(v) => v,
                    Err(e) => {
                        out.write_str(e)?;
                        out.write_str("\r\n")?;
                        return Ok(());
                    }
                };
                self.set_parity(uart_num, value, out, cfg)
            }
            "baud" => {
                if args.len() != 3 {
                    out.write_str("Usage: uart <1|2> baud <rate>\r\n")?;
                    out.write_str("Common rates: 9600, 19200, 38400, 57600, 115200\r\n")?;
                    return Ok(());
                }
                let value = match self.parse_int(args[2]) {
                    Ok(v) => v,
                    Err(e) => {
                        out.write_str(e)?;
                        out.write_str("\r\n")?;
                        return Ok(());
                    }
                };
                self.set_baud(uart_num, value, out, cfg)
            }
            "write" => {
                if args.len() < 3 {
                    out.write_str("Usage: uart <1|2> write <text>\r\n")?;
                    return Ok(());
                }
                // Pass all arguments after "write" to write_string
                self.write_string(uart_num, &args[2..], out, cfg)
            }
            "passthrough" => {
                if args.len() != 2 {
                    out.write_str("Usage: uart <1|2> passthrough\r\n")?;
                    return Ok(());
                }
                self.passthrough(uart_num, out)
            }
            _ => {
                out.write_str("Unknown subcommand: ")?;
                out.write_str(args[1])?;
                out.write_str("\r\n")?;
                out.write_str("Available: stop_bit, parity, baud, write, passthrough\r\n")?;
                Ok(())
            }
        }
    }

    fn print_help(&self, out: &mut SerialIO) -> Result<(), UsbError> {
        out.write_str("uart <1|2> <subcommand> [args] - UART/Serial communication\r\n")?;
        out.write_str("UART 1 = USART2, UART 2 = USART3\r\n")?;
        out.write_str("Subcommands:\r\n")?;
        out.write_str("  stop_bit <1|2>    - Set stop bits (1 or 2)\r\n")?;
        out.write_str("  parity <0|1|2>    - Set parity (0=none, 1=even, 2=odd)\r\n")?;
        out.write_str("  baud <rate>       - Set baud rate (e.g., 9600, 115200)\r\n")?;
        out.write_str("  write <text>      - Write string to UART TX\r\n")?;
        out.write_str("  passthrough       - Enter passthrough mode (ESC to exit)\r\n")?;
        out.write_str("\r\nExamples:\r\n")?;
        out.write_str("  uart 1 baud 115200\r\n")?;
        out.write_str("  uart 2 write Hello World\r\n")?;
        out.write_str("  uart 1 passthrough\r\n")?;
        out.write_str("\r\nNote: RS232, RS422, RS485 commands reserved for future use\r\n")?;
        Ok(())
    }
}

// Helper function to convert u32 to string
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
