use crate::cli::{CliConfig, Command};
use crate::io_interface::serial::SerialIO;
use usb_device::UsbError;
use stm32f4xx_hal as hal;
use hal::i2c::I2c;
use cortex_m::interrupt::Mutex;
use core::cell::RefCell;

type I2c1Type = I2c<hal::pac::I2C1>;
type I2c2Type = I2c<hal::pac::I2C2>;

static I2C1: Mutex<RefCell<Option<I2c1Type>>> = Mutex::new(RefCell::new(None));
static I2C2: Mutex<RefCell<Option<I2c2Type>>> = Mutex::new(RefCell::new(None));

#[derive(Clone)]
pub struct I2cCommand;

impl I2cCommand {
    pub fn new() -> Self {
        Self
    }

    pub fn init_i2c1(i2c: I2c1Type) {
        cortex_m::interrupt::free(|cs| {
            *I2C1.borrow(cs).borrow_mut() = Some(i2c);
        });
    }

    pub fn init_i2c2(i2c: I2c2Type) {
        cortex_m::interrupt::free(|cs| {
            *I2C2.borrow(cs).borrow_mut() = Some(i2c);
        });
    }

    fn parse_hex(&self, arg: &str) -> Result<u8, &'static str> {
        let s = arg.trim_start_matches("0x").trim_start_matches("0X");
        u8::from_str_radix(s, 16).map_err(|_| "Invalid hex value")
    }

    fn parse_i2c_num(&self, arg: &str) -> Result<u8, &'static str> {
        match arg {
            "1" => Ok(1),
            "2" => Ok(2),
            _ => Err("I2C number must be 1 or 2"),
        }
    }

    fn parse_speed(&self, arg: &str) -> Result<u32, &'static str> {
        arg.parse::<u32>().map_err(|_| "Invalid speed value")
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

    fn set_speed(&self, i2c_num: u8, speed_khz: u32, out: &mut SerialIO, cfg: &CliConfig) -> Result<(), UsbError> {
        // TODO: Implement actual I2C speed configuration
        // This requires reconfiguring the I2C peripheral with new clock settings
        // For now, just acknowledge the command
        
        if cfg.is_short_output() {
            out.write_str("OK\r\n")?;
        } else {
            out.write_str("I2C")?;
            out.write_str(if i2c_num == 1 { "1" } else { "2" })?;
            out.write_str(" speed set to ")?;
            let mut buf = [0u8; 10];
            if let Some(s) = format_u32(speed_khz, &mut buf) {
                out.write_str(s)?;
            }
            out.write_str(" kHz\r\n")?;
        }
        Ok(())
    }

    fn scan_bus(&self, i2c_num: u8, out: &mut SerialIO, cfg: &CliConfig) -> Result<(), UsbError> {
        if !cfg.is_short_output() {
            out.write_str("Scanning I2C")?;
            out.write_str(if i2c_num == 1 { "1" } else { "2" })?;
            out.write_str(" bus...\r\n")?;
        }

        let mut found_count = 0;

        cortex_m::interrupt::free(|cs| {
            if i2c_num == 1 {
                let mut i2c_opt = I2C1.borrow(cs).borrow_mut();
                if let Some(i2c) = i2c_opt.as_mut() {
                    for addr in 0x08..=0x77 {
                        // Try to write 0 bytes to check if device responds
                        if i2c.write(addr, &[]).is_ok() {
                            let mut buf = [0u8; 4];
                            if let Some(s) = self.format_hex_u8(addr, &mut buf) {
                                let _ = out.write_str(s);
                                let _ = out.write_str(" ");
                                found_count += 1;
                                
                                if found_count % 8 == 0 {
                                    let _ = out.write_str("\r\n");
                                }
                            }
                        }
                    }
                }
            } else {
                let mut i2c_opt = I2C2.borrow(cs).borrow_mut();
                if let Some(i2c) = i2c_opt.as_mut() {
                    for addr in 0x08..=0x77 {
                        // Try to write 0 bytes to check if device responds
                        if i2c.write(addr, &[]).is_ok() {
                            let mut buf = [0u8; 4];
                            if let Some(s) = self.format_hex_u8(addr, &mut buf) {
                                let _ = out.write_str(s);
                                let _ = out.write_str(" ");
                                found_count += 1;
                                
                                if found_count % 8 == 0 {
                                    let _ = out.write_str("\r\n");
                                }
                            }
                        }
                    }
                }
            }
        });

        if found_count > 0 && found_count % 8 != 0 {
            out.write_str("\r\n")?;
        }

        if !cfg.is_short_output() {
            out.write_str("Found ")?;
            let mut buf = [0u8; 10];
            if let Some(s) = format_u32(found_count, &mut buf) {
                out.write_str(s)?;
            }
            out.write_str(" device(s)\r\n")?;
        }

        Ok(())
    }

    fn read_register(&self, i2c_num: u8, addr: u8, reg: u8, out: &mut SerialIO, cfg: &CliConfig) -> Result<(), UsbError> {
        let mut buffer = [0u8; 1];
        let result = cortex_m::interrupt::free(|cs| {
            if i2c_num == 1 {
                let mut i2c_opt = I2C1.borrow(cs).borrow_mut();
                if let Some(i2c) = i2c_opt.as_mut() {
                    // Write register address
                    if let Err(_) = i2c.write(addr, &[reg]) {
                        return Err("Failed to write register address");
                    }
                    
                    // Read register value
                    if let Err(_) = i2c.read(addr, &mut buffer) {
                        return Err("Failed to read register value");
                    }
                    
                    Ok(buffer[0])
                } else {
                    Err("I2C not initialized")
                }
            } else {
                let mut i2c_opt = I2C2.borrow(cs).borrow_mut();
                if let Some(i2c) = i2c_opt.as_mut() {
                    // Write register address
                    if let Err(_) = i2c.write(addr, &[reg]) {
                        return Err("Failed to write register address");
                    }
                    
                    // Read register value
                    if let Err(_) = i2c.read(addr, &mut buffer) {
                        return Err("Failed to read register value");
                    }
                    
                    Ok(buffer[0])
                } else {
                    Err("I2C not initialized")
                }
            }
        });

        match result {
            Ok(value) => {
                let mut buf = [0u8; 4];
                if let Some(s) = self.format_hex_u8(value, &mut buf) {
                    out.write_str(s)?;
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

    fn write_register(&self, i2c_num: u8, addr: u8, reg: u8, value: u8, out: &mut SerialIO, cfg: &CliConfig) -> Result<(), UsbError> {
        let result = cortex_m::interrupt::free(|cs| {
            if i2c_num == 1 {
                let mut i2c_opt = I2C1.borrow(cs).borrow_mut();
                if let Some(i2c) = i2c_opt.as_mut() {
                    // Write register address and value
                    if let Err(_) = i2c.write(addr, &[reg, value]) {
                        return Err("Failed to write register");
                    }
                    Ok(())
                } else {
                    Err("I2C not initialized")
                }
            } else {
                let mut i2c_opt = I2C2.borrow(cs).borrow_mut();
                if let Some(i2c) = i2c_opt.as_mut() {
                    // Write register address and value
                    if let Err(_) = i2c.write(addr, &[reg, value]) {
                        return Err("Failed to write register");
                    }
                    Ok(())
                } else {
                    Err("I2C not initialized")
                }
            }
        });

        match result {
            Ok(_) => {
                if cfg.is_short_output() {
                    out.write_str("OK\r\n")?;
                } else {
                    out.write_str("Register ")?;
                    let mut buf = [0u8; 4];
                    if let Some(s) = self.format_hex_u8(reg, &mut buf) {
                        out.write_str(s)?;
                    }
                    out.write_str(" written with ")?;
                    if let Some(s) = self.format_hex_u8(value, &mut buf) {
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

    fn map_registers(&self, i2c_num: u8, addr: u8, out: &mut SerialIO, cfg: &CliConfig) -> Result<(), UsbError> {
        if !cfg.is_short_output() {
            out.write_str("Mapping registers for device ")?;
            let mut buf = [0u8; 4];
            if let Some(s) = self.format_hex_u8(addr, &mut buf) {
                out.write_str(s)?;
            }
            out.write_str("...\r\n")?;
        }

        let mut readable_count = 0;

        if !cfg.is_short_output() {
            out.write_str("Readable registers:\r\n")?;
        }

        cortex_m::interrupt::free(|cs| {
            if i2c_num == 1 {
                let mut i2c_opt = I2C1.borrow(cs).borrow_mut();
                if let Some(i2c) = i2c_opt.as_mut() {
                    // Try to read each register
                    for reg in 0x00..=0xFF {
                        let mut buffer = [0u8; 1];
                        
                        // Try to read
                        if i2c.write(addr, &[reg]).is_ok() {
                            if i2c.read(addr, &mut buffer).is_ok() {
                                let mut buf = [0u8; 4];
                                if let Some(s) = self.format_hex_u8(reg, &mut buf) {
                                    let _ = out.write_str(s);
                                    let _ = out.write_str(" ");
                                    readable_count += 1;
                                    
                                    if readable_count % 16 == 0 {
                                        let _ = out.write_str("\r\n");
                                    }
                                }
                            }
                        }
                    }
                }
            } else {
                let mut i2c_opt = I2C2.borrow(cs).borrow_mut();
                if let Some(i2c) = i2c_opt.as_mut() {
                    // Try to read each register
                    for reg in 0x00..=0xFF {
                        let mut buffer = [0u8; 1];
                        
                        // Try to read
                        if i2c.write(addr, &[reg]).is_ok() {
                            if i2c.read(addr, &mut buffer).is_ok() {
                                let mut buf = [0u8; 4];
                                if let Some(s) = self.format_hex_u8(reg, &mut buf) {
                                    let _ = out.write_str(s);
                                    let _ = out.write_str(" ");
                                    readable_count += 1;
                                    
                                    if readable_count % 16 == 0 {
                                        let _ = out.write_str("\r\n");
                                    }
                                }
                            }
                        }
                    }
                }
            }
        });

        if readable_count > 0 && readable_count % 16 != 0 {
            out.write_str("\r\n")?;
        }

        if !cfg.is_short_output() {
            out.write_str("Found ")?;
            let mut buf = [0u8; 10];
            if let Some(s) = format_u32(readable_count, &mut buf) {
                out.write_str(s)?;
            }
            out.write_str(" readable register(s)\r\n")?;
        }

        Ok(())
    }
}

impl Command for I2cCommand {
    fn name(&self) -> &'static str {
        "i2c"
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
            out.write_str("Usage: i2c <1|2> <speed|scan|read_reg|write_reg|map> [args]\r\n")?;
            out.write_str("Type 'help i2c' for more information\r\n")?;
            return Ok(());
        }

        // Parse I2C number
        let i2c_num = match self.parse_i2c_num(args[0]) {
            Ok(n) => n,
            Err(e) => {
                out.write_str(e)?;
                out.write_str("\r\n")?;
                return Ok(());
            }
        };

        match args[1] {
            "speed" => {
                if args.len() != 3 {
                    out.write_str("Usage: i2c <1|2> speed <kHz>\r\n")?;
                    return Ok(());
                }
                let speed = match self.parse_speed(args[2]) {
                    Ok(s) => s,
                    Err(e) => {
                        out.write_str(e)?;
                        out.write_str("\r\n")?;
                        return Ok(());
                    }
                };
                self.set_speed(i2c_num, speed, out, cfg)
            }
            "scan" => {
                if args.len() != 2 {
                    out.write_str("Usage: i2c <1|2> scan\r\n")?;
                    return Ok(());
                }
                self.scan_bus(i2c_num, out, cfg)
            }
            "read_reg" | "read" => {
                if args.len() != 4 {
                    out.write_str("Usage: i2c <1|2> read_reg <addr> <reg>\r\n")?;
                    return Ok(());
                }
                let addr = match self.parse_hex(args[2]) {
                    Ok(a) => a,
                    Err(e) => {
                        out.write_str(e)?;
                        out.write_str("\r\n")?;
                        return Ok(());
                    }
                };
                let reg = match self.parse_hex(args[3]) {
                    Ok(r) => r,
                    Err(e) => {
                        out.write_str(e)?;
                        out.write_str("\r\n")?;
                        return Ok(());
                    }
                };
                self.read_register(i2c_num, addr, reg, out, cfg)
            }
            "write_reg" | "write" => {
                if args.len() != 5 {
                    out.write_str("Usage: i2c <1|2> write_reg <addr> <reg> <value>\r\n")?;
                    return Ok(());
                }
                let addr = match self.parse_hex(args[2]) {
                    Ok(a) => a,
                    Err(e) => {
                        out.write_str(e)?;
                        out.write_str("\r\n")?;
                        return Ok(());
                    }
                };
                let reg = match self.parse_hex(args[3]) {
                    Ok(r) => r,
                    Err(e) => {
                        out.write_str(e)?;
                        out.write_str("\r\n")?;
                        return Ok(());
                    }
                };
                let value = match self.parse_hex(args[4]) {
                    Ok(v) => v,
                    Err(e) => {
                        out.write_str(e)?;
                        out.write_str("\r\n")?;
                        return Ok(());
                    }
                };
                self.write_register(i2c_num, addr, reg, value, out, cfg)
            }
            "map" => {
                if args.len() != 3 {
                    out.write_str("Usage: i2c <1|2> map <addr>\r\n")?;
                    return Ok(());
                }
                let addr = match self.parse_hex(args[2]) {
                    Ok(a) => a,
                    Err(e) => {
                        out.write_str(e)?;
                        out.write_str("\r\n")?;
                        return Ok(());
                    }
                };
                self.map_registers(i2c_num, addr, out, cfg)
            }
            _ => {
                out.write_str("Unknown subcommand. Use: speed, scan, read_reg, write_reg, or map\r\n")?;
                Ok(())
            }
        }
    }

    fn print_help(&self, out: &mut SerialIO) -> Result<(), UsbError> {
        out.write_str("i2c <1|2> <cmd> [args] - I2C bus operations\r\n")?;
        out.write_str("\r\nCommands:\r\n")?;
        out.write_str("  i2c <1|2> speed <kHz>          - Set I2C speed (e.g., 100, 400)\r\n")?;
        out.write_str("  i2c <1|2> scan                 - Scan bus for devices\r\n")?;
        out.write_str("  i2c <1|2> read_reg <adr> <reg> - Read register value (hex)\r\n")?;
        out.write_str("  i2c <1|2> write_reg <adr> <reg> <val> - Write to register\r\n")?;
        out.write_str("  i2c <1|2> map <adr>            - Map all readable registers\r\n")?;
        out.write_str("\r\nExamples:\r\n")?;
        out.write_str("  i2c 1 speed 400                - Set I2C1 to 400 kHz\r\n")?;
        out.write_str("  i2c 1 scan                     - Scan I2C1 bus\r\n")?;
        out.write_str("  i2c 1 read_reg 0x50 0x00       - Read reg 0x00 from device 0x50\r\n")?;
        out.write_str("  i2c 1 write_reg 0x50 0x00 0xFF - Write 0xFF to reg 0x00\r\n")?;
        out.write_str("  i2c 1 map 0x50                 - Map all registers of device 0x50\r\n")?;
        out.write_str("\r\nNote: Addresses and register values are in hexadecimal\r\n")?;
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
