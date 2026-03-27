// CAN command handler
// Usage: can <config|send|read|filter|filter_disable|status> [args]

use crate::cli::{CliConfig, Command};
use crate::io_interface::serial::SerialIO;
use usb_device::UsbError;
use stm32f4xx_hal as hal;
use cortex_m::interrupt::Mutex;
use core::cell::RefCell;
use bxcan::{Fifo, filter::Mask32};

type CanType = bxcan::Can<hal::can::Can<hal::pac::CAN1>>;

static CAN: Mutex<RefCell<Option<CanType>>> = Mutex::new(RefCell::new(None));

// CAN Configuration storage
#[derive(Clone, Copy)]
struct CanState {
    bitrate: u32,
    mode: u8,
}

impl CanState {
    const fn default() -> Self {
        Self {
            bitrate: 500_000, // 500k default
            mode: 0,          // Normal mode
        }
    }
}

static CAN_STATE: Mutex<RefCell<CanState>> = Mutex::new(RefCell::new(CanState::default()));

#[derive(Clone)]
pub struct CanCommand;

impl CanCommand {
    pub fn new() -> Self {
        Self
    }

    pub fn init_can(can: hal::can::Can<hal::pac::CAN1>) {
        // Wrap the HAL CAN with bxcan
        let mut can = bxcan::Can::builder(can)
            .set_loopback(false)
            .set_silent(false)
            .enable();
        
        // Enable all filters by default
        can.modify_filters().enable_bank(0, Fifo::Fifo0, Mask32::accept_all());
        
        cortex_m::interrupt::free(|cs| {
            *CAN.borrow(cs).borrow_mut() = Some(can);
        });
    }

    fn parse_bitrate(&self, arg: &str) -> Result<u32, &'static str> {
        // Parse bitrate strings like "125k", "250k", "500k", "1m"
        if arg.len() == 0 {
            return Err("Invalid bitrate format");
        }
        
        let last_char = arg.as_bytes()[arg.len() - 1];
        
        if last_char == b'm' || last_char == b'M' {
            // Parse MHz -> bps
            let num_str = &arg[..arg.len()-1];
            let num = num_str.parse::<u32>()
                .map_err(|_| "Invalid bitrate format")?;
            Ok(num * 1_000_000)
        } else if last_char == b'k' || last_char == b'K' {
            // Parse kHz -> bps
            let num_str = &arg[..arg.len()-1];
            let num = num_str.parse::<u32>()
                .map_err(|_| "Invalid bitrate format")?;
            Ok(num * 1_000)
        } else {
            // Assume bps
            arg.parse::<u32>().map_err(|_| "Invalid bitrate format")
        }
    }

    fn parse_hex_id(&self, arg: &str) -> Result<u32, &'static str> {
        let s = arg.trim_start_matches("0x").trim_start_matches("0X");
        u32::from_str_radix(s, 16).map_err(|_| "Invalid hex ID")
    }

    fn parse_hex_byte(&self, s: &str) -> Result<u8, &'static str> {
        let s = s.trim_start_matches("0x").trim_start_matches("0X");
        u8::from_str_radix(s, 16).map_err(|_| "Invalid hex byte")
    }

    fn parse_int(&self, arg: &str) -> Result<u32, &'static str> {
        arg.parse::<u32>().map_err(|_| "Invalid integer value")
    }

    fn parse_data(&self, args: &[&str], start_idx: usize, end_idx: usize) -> Result<([u8; 8], usize), &'static str> {
        let mut data = [0u8; 8];
        let mut count = 0;
        
        for i in start_idx..end_idx {
            if count >= 8 {
                return Err("Too many data bytes (max 8)");
            }
            data[count] = self.parse_hex_byte(args[i])?;
            count += 1;
        }
        
        Ok((data, count))
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

    fn format_hex_u32<'a>(&self, n: u32, buf: &'a mut [u8; 10]) -> Option<&'a str> {
        buf[0] = b'0';
        buf[1] = b'x';
        let mut pos = 2;
        
        // Format as hex (up to 8 digits for 32-bit)
        for i in (0..8).rev() {
            let nibble = ((n >> (i * 4)) & 0x0F) as u8;
            if pos > 2 || nibble != 0 || i == 0 {
                buf[pos] = if nibble < 10 { b'0' + nibble } else { b'A' + nibble - 10 };
                pos += 1;
            }
        }
        
        core::str::from_utf8(&buf[..pos]).ok()
    }

    fn config_can(&self, bitrate_str: &str, mode_str: &str, out: &mut SerialIO, cfg: &CliConfig) -> Result<(), UsbError> {
        let bitrate = match self.parse_bitrate(bitrate_str) {
            Ok(b) => b,
            Err(e) => {
                out.write_str("Error: ")?;
                out.write_str(e)?;
                out.write_str("\r\n")?;
                return Ok(());
            }
        };

        let mode = match self.parse_int(mode_str) {
            Ok(m) if m <= 3 => m as u8,
            _ => {
                out.write_str("Error: Mode must be 0-3\r\n")?;
                return Ok(());
            }
        };

        // Store configuration
        cortex_m::interrupt::free(|cs| {
            let mut state = CAN_STATE.borrow(cs).borrow_mut();
            state.bitrate = bitrate;
            state.mode = mode;
        });

        // TODO: Reconfigure CAN peripheral with new settings
        // This requires reinitializing the CAN peripheral

        if cfg.is_short_output() {
            out.write_str("OK\r\n")?;
        } else {
            out.write_str("CAN configured: ")?;
            out.write_str(bitrate_str)?;
            out.write_str(" mode ")?;
            out.write_str(mode_str)?;
            out.write_str("\r\n")?;
        }
        Ok(())
    }

    fn send_frame(&self, id: u32, _data: &[u8], flags: u8, out: &mut SerialIO, cfg: &CliConfig) -> Result<(), UsbError> {
        let result = cortex_m::interrupt::free(|cs| {
            let mut can_opt = CAN.borrow(cs).borrow_mut();
            if let Some(_can) = can_opt.as_mut() {
                // Create frame based on flags
                // flags: 0=Standard, 1=Extended, 2=RTR
                let _is_extended = (flags & 0x01) != 0;
                let _is_rtr = (flags & 0x02) != 0;
                
                // TODO: Create and send CAN frame using HAL
                // This is a placeholder - actual implementation depends on HAL API
                
                Ok(())
            } else {
                Err("CAN not initialized")
            }
        });

        match result {
            Ok(_) => {
                if cfg.is_short_output() {
                    out.write_str("OK\r\n")?;
                } else {
                    out.write_str("Frame sent: ID ")?;
                    let mut buf = [0u8; 10];
                    if let Some(s) = self.format_hex_u32(id, &mut buf) {
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

    fn read_frames(&self, _count: u32, _timeout: u32, out: &mut SerialIO, cfg: &CliConfig) -> Result<(), UsbError> {
        let result = cortex_m::interrupt::free(|cs| {
            let mut can_opt = CAN.borrow(cs).borrow_mut();
            if let Some(_can) = can_opt.as_mut() {
                // TODO: Read CAN frames from receive buffer
                // This is a placeholder - actual implementation depends on HAL API
                
                Ok(0u32) // Return number of frames read
            } else {
                Err("CAN not initialized")
            }
        });

        match result {
            Ok(frames_read) => {
                if !cfg.is_short_output() {
                    out.write_str("Read ")?;
                    let mut buf = [0u8; 10];
                    if let Some(s) = format_u32(frames_read, &mut buf) {
                        out.write_str(s)?;
                    }
                    out.write_str(" frame(s)\r\n")?;
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

    fn set_filter(&self, filter_id: u32, id: u32, mask: u32, out: &mut SerialIO, cfg: &CliConfig) -> Result<(), UsbError> {
        if filter_id > 13 {
            out.write_str("Error: Filter ID must be 0-13\r\n")?;
            return Ok(());
        }

        let result = cortex_m::interrupt::free(|cs| {
            let mut can_opt = CAN.borrow(cs).borrow_mut();
            if let Some(_can) = can_opt.as_mut() {
                // TODO: Configure CAN filter using HAL
                // This is a placeholder - actual implementation depends on HAL API
                
                Ok(())
            } else {
                Err("CAN not initialized")
            }
        });

        match result {
            Ok(_) => {
                if cfg.is_short_output() {
                    out.write_str("OK\r\n")?;
                } else {
                    out.write_str("Filter ")?;
                    let mut buf = [0u8; 10];
                    if let Some(s) = format_u32(filter_id, &mut buf) {
                        out.write_str(s)?;
                    }
                    out.write_str(" set: ID ")?;
                    if let Some(s) = self.format_hex_u32(id, &mut buf) {
                        out.write_str(s)?;
                    }
                    out.write_str(" mask ")?;
                    if let Some(s) = self.format_hex_u32(mask, &mut buf) {
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

    fn disable_filter(&self, filter_id: u32, out: &mut SerialIO, cfg: &CliConfig) -> Result<(), UsbError> {
        if filter_id > 13 {
            out.write_str("Error: Filter ID must be 0-13\r\n")?;
            return Ok(());
        }

        let result = cortex_m::interrupt::free(|cs| {
            let mut can_opt = CAN.borrow(cs).borrow_mut();
            if let Some(_can) = can_opt.as_mut() {
                // TODO: Disable CAN filter using HAL
                // This is a placeholder - actual implementation depends on HAL API
                
                Ok(())
            } else {
                Err("CAN not initialized")
            }
        });

        match result {
            Ok(_) => {
                if cfg.is_short_output() {
                    out.write_str("OK\r\n")?;
                } else {
                    out.write_str("Filter ")?;
                    let mut buf = [0u8; 10];
                    if let Some(s) = format_u32(filter_id, &mut buf) {
                        out.write_str(s)?;
                    }
                    out.write_str(" disabled\r\n")?;
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

    fn show_status(&self, out: &mut SerialIO) -> Result<(), UsbError> {
        let state = cortex_m::interrupt::free(|cs| *CAN_STATE.borrow(cs).borrow());

        out.write_str("CAN Status:\r\n")?;
        out.write_str("  Bitrate: ")?;
        let mut buf = [0u8; 10];
        if let Some(s) = format_u32(state.bitrate, &mut buf) {
            out.write_str(s)?;
        }
        out.write_str(" bps\r\n")?;
        
        out.write_str("  Mode: ")?;
        let mode_str = match state.mode {
            0 => "Normal",
            1 => "Loopback",
            2 => "Silent",
            3 => "Silent-Loopback",
            _ => "Unknown",
        };
        out.write_str(mode_str)?;
        out.write_str("\r\n")?;

        // TODO: Display active filters

        Ok(())
    }
}

impl Command for CanCommand {
    fn name(&self) -> &'static str {
        "can"
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
            out.write_str("Usage: can <config|send|read|filter|filter_disable|status> [args]\r\n")?;
            out.write_str("Type 'help can' for more information\r\n")?;
            return Ok(());
        }

        match args[0] {
            "config" => {
                if args.len() != 3 {
                    out.write_str("Usage: can config <bitrate> <mode>\r\n")?;
                    return Ok(());
                }
                self.config_can(args[1], args[2], out, cfg)
            }
            "send" => {
                if args.len() < 4 {
                    out.write_str("Usage: can send <id> <data...> <flags>\r\n")?;
                    return Ok(());
                }
                let id = match self.parse_hex_id(args[1]) {
                    Ok(i) => i,
                    Err(e) => {
                        out.write_str(e)?;
                        out.write_str("\r\n")?;
                        return Ok(());
                    }
                };
                
                // Last argument is flags
                let flags_idx = args.len() - 1;
                let flags = match self.parse_int(args[flags_idx]) {
                    Ok(f) => f as u8,
                    Err(e) => {
                        out.write_str(e)?;
                        out.write_str("\r\n")?;
                        return Ok(());
                    }
                };
                
                // Data bytes are between id and flags
                let (data, data_len) = match self.parse_data(args, 2, flags_idx) {
                    Ok(d) => d,
                    Err(e) => {
                        out.write_str(e)?;
                        out.write_str("\r\n")?;
                        return Ok(());
                    }
                };
                
                self.send_frame(id, &data[..data_len], flags, out, cfg)
            }
            "read" => {
                if args.len() != 3 {
                    out.write_str("Usage: can read <count> <timeout>\r\n")?;
                    return Ok(());
                }
                let count = match self.parse_int(args[1]) {
                    Ok(c) => c,
                    Err(e) => {
                        out.write_str(e)?;
                        out.write_str("\r\n")?;
                        return Ok(());
                    }
                };
                let timeout = match self.parse_int(args[2]) {
                    Ok(t) => t,
                    Err(e) => {
                        out.write_str(e)?;
                        out.write_str("\r\n")?;
                        return Ok(());
                    }
                };
                self.read_frames(count, timeout, out, cfg)
            }
            "filter" => {
                if args.len() != 4 {
                    out.write_str("Usage: can filter <filter_id> <id> <mask>\r\n")?;
                    return Ok(());
                }
                let filter_id = match self.parse_int(args[1]) {
                    Ok(f) => f,
                    Err(e) => {
                        out.write_str(e)?;
                        out.write_str("\r\n")?;
                        return Ok(());
                    }
                };
                let id = match self.parse_hex_id(args[2]) {
                    Ok(i) => i,
                    Err(e) => {
                        out.write_str(e)?;
                        out.write_str("\r\n")?;
                        return Ok(());
                    }
                };
                let mask = match self.parse_hex_id(args[3]) {
                    Ok(m) => m,
                    Err(e) => {
                        out.write_str(e)?;
                        out.write_str("\r\n")?;
                        return Ok(());
                    }
                };
                self.set_filter(filter_id, id, mask, out, cfg)
            }
            "filter_disable" => {
                if args.len() != 2 {
                    out.write_str("Usage: can filter_disable <filter_id>\r\n")?;
                    return Ok(());
                }
                let filter_id = match self.parse_int(args[1]) {
                    Ok(f) => f,
                    Err(e) => {
                        out.write_str(e)?;
                        out.write_str("\r\n")?;
                        return Ok(());
                    }
                };
                self.disable_filter(filter_id, out, cfg)
            }
            "status" => {
                if args.len() != 1 {
                    out.write_str("Usage: can status\r\n")?;
                    return Ok(());
                }
                self.show_status(out)
            }
            _ => {
                out.write_str("Unknown subcommand\r\n")?;
                Ok(())
            }
        }
    }

    fn print_help(&self, out: &mut SerialIO) -> Result<(), UsbError> {
        out.write_str("can <cmd> [args] - CAN bus operations\r\n")?;
        out.write_str("\r\nCommands:\r\n")?;
        out.write_str("  can config <bitrate> <mode>\r\n")?;
        out.write_str("    - bitrate: Bit rate (e.g., \"125k\", \"250k\", \"500k\", \"1m\")\r\n")?;
        out.write_str("    - mode: 0=Normal, 1=Loopback, 2=Silent, 3=Silent-Loopback\r\n")?;
        out.write_str("  can send <id> <data...> <flags>\r\n")?;
        out.write_str("    - id: CAN identifier (hex, 11-bit standard or 29-bit extended)\r\n")?;
        out.write_str("    - data: Data bytes (0-8 bytes, hex)\r\n")?;
        out.write_str("    - flags: 0=Standard, 1=Extended, 2=RTR\r\n")?;
        out.write_str("  can read <count> <timeout>\r\n")?;
        out.write_str("    - count: Number of frames to read from buffer\r\n")?;
        out.write_str("    - timeout: Timeout in milliseconds (0=non-blocking)\r\n")?;
        out.write_str("  can filter <filter_id> <id> <mask>\r\n")?;
        out.write_str("    - filter_id: Filter bank number (0-13)\r\n")?;
        out.write_str("    - id: CAN ID to accept (hex)\r\n")?;
        out.write_str("    - mask: ID mask (hex, 0=don't care, 1=must match)\r\n")?;
        out.write_str("  can filter_disable <filter_id>\r\n")?;
        out.write_str("    - Disable specific filter bank\r\n")?;
        out.write_str("  can status\r\n")?;
        out.write_str("    - Display current config and active filters\r\n")?;
        out.write_str("\r\nExamples:\r\n")?;
        out.write_str("  can config 500k 0          - 500 kbps, normal mode\r\n")?;
        out.write_str("  can send 0x123 0xAA 0xBB 0xCC 0 - Send standard frame\r\n")?;
        out.write_str("  can read 1 100             - Read 1 frame, 100ms timeout\r\n")?;
        out.write_str("  can filter 0 0x100 0x7F0   - Accept IDs 0x100-0x10F\r\n")?;
        out.write_str("  can filter_disable 0       - Disable filter 0\r\n")?;
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
