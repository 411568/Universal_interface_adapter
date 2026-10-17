// CAN command handler
// Usage: can <config|send|read|filter|filter_disable|status> [args]

use crate::cli::{CliConfig, Command};
use crate::io_interface::serial::SerialIO;
use usb_device::UsbError;
use stm32f4xx_hal as hal;
use cortex_m::interrupt::Mutex;
use core::cell::RefCell;
use bxcan::{Fifo, Frame, Id, StandardId, ExtendedId, filter::Mask32};

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

// The APB1 (PCLK1) frequency feeding CAN1, in Hz. Set once from `init_can`
// (read from `rcc.clocks` at boot, not hardcoded) and reused by `config_can`
// whenever bit timing needs to be recomputed for a new bitrate.
static CAN_PCLK1_HZ: Mutex<RefCell<u32>> = Mutex::new(RefCell::new(0));

// Tracks the configured state of each of the 14 filter banks so `can status`
// can report them. `None` means the bank is disabled.
#[derive(Clone, Copy)]
struct FilterEntry {
    id: u32,
    mask: u32,
}

const NUM_FILTER_BANKS: usize = 14;
static FILTER_STATE: Mutex<RefCell<[Option<FilterEntry>; NUM_FILTER_BANKS]>> =
    Mutex::new(RefCell::new([None; NUM_FILTER_BANKS]));

#[derive(Clone)]
pub struct CanCommand;

impl CanCommand {
    pub fn new() -> Self {
        Self
    }

    /// `pclk1_hz` is the actual APB1 peripheral clock feeding CAN1, read from
    /// `rcc.clocks.pclk1()` at boot — NOT hardcoded, since it depends on
    /// whatever prescaler `rcc.freeze()` picked for your sysclk/HSE config.
    pub fn init_can(can: hal::can::Can<hal::pac::CAN1>, pclk1_hz: u32) {
        cortex_m::interrupt::free(|cs| {
            *CAN_PCLK1_HZ.borrow(cs).borrow_mut() = pclk1_hz;
        });

        // Compute real bit timing for the default 500 kbps instead of relying
        // on the CAN peripheral's power-on-reset BTR value (which is not a
        // valid bit rate for any sane bus).
        let default_bitrate = cortex_m::interrupt::free(|cs| CAN_STATE.borrow(cs).borrow().bitrate);
        let btr = compute_bit_timing(pclk1_hz, default_bitrate)
            .expect("default CAN bitrate must be derivable from PCLK1");

        // Wrap the HAL CAN with bxcan
        let mut can = bxcan::Can::builder(can)
            .set_bit_timing(btr)
            .set_loopback(false)
            .set_silent(false)
            .enable();

        // Enable all filters by default
        can.modify_filters().enable_bank(0, Fifo::Fifo0, Mask32::accept_all());

        // Reflect bank 0's "accept all" state in our tracking table so
        // `can status` matches what's actually configured in hardware.
        cortex_m::interrupt::free(|cs| {
            let mut filters = FILTER_STATE.borrow(cs).borrow_mut();
            filters[0] = Some(FilterEntry { id: 0, mask: 0 });
        });

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

        // Recompute bit timing for the requested bitrate from the actual
        // APB1 clock (captured once in init_can), then push both timing and
        // mode bits into the peripheral via bxcan's `modify_config()` guard,
        // which handles entering/leaving CAN initialization mode for us.
        let pclk1_hz = cortex_m::interrupt::free(|cs| *CAN_PCLK1_HZ.borrow(cs).borrow());
        let btr = match compute_bit_timing(pclk1_hz, bitrate) {
            Some(b) => b,
            None => {
                out.write_str("Error: Bitrate not achievable from current PCLK1\r\n")?;
                return Ok(());
            }
        };

        let reconfig_result = cortex_m::interrupt::free(|cs| {
            let mut can_opt = CAN.borrow(cs).borrow_mut();
            if let Some(can) = can_opt.as_mut() {
                let (loopback, silent) = match mode {
                    0 => (false, false), // Normal
                    1 => (true, false),  // Loopback
                    2 => (false, true),  // Silent
                    3 => (true, true),   // Silent-Loopback
                    _ => (false, false),
                };

                can.modify_config()
                    .set_bit_timing(btr)
                    .set_loopback(loopback)
                    .set_silent(silent)
                    .enable();

                Ok(())
            } else {
                Err("CAN not initialized")
            }
        });

        if let Err(e) = reconfig_result {
            out.write_str("Error: ")?;
            out.write_str(e)?;
            out.write_str("\r\n")?;
            return Ok(());
        }

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

    fn build_frame(&self, id: u32, data: &[u8], flags: u8) -> Result<Frame, &'static str> {
        let is_extended = (flags & 0x01) != 0;
        let is_rtr = (flags & 0x02) != 0;

        let bx_id: Id = if is_extended {
            ExtendedId::new(id)
                .map(Id::Extended)
                .ok_or("Invalid extended ID (must fit in 29 bits)")?
        } else {
            let std_id = u16::try_from(id).map_err(|_| "Invalid standard ID (must fit in 11 bits)")?;
            StandardId::new(std_id)
                .map(Id::Standard)
                .ok_or("Invalid standard ID (must fit in 11 bits)")?
        };

        if is_rtr {
            Ok(Frame::new_remote(bx_id, data.len() as u8))
        } else {
            let can_data = bxcan::Data::new(data).ok_or("Too many data bytes (max 8)")?;
            Ok(Frame::new_data(bx_id, can_data))
        }
    }

    fn send_frame(&self, id: u32, data: &[u8], flags: u8, out: &mut SerialIO, cfg: &CliConfig) -> Result<(), UsbError> {
        let frame = match self.build_frame(id, data, flags) {
            Ok(f) => f,
            Err(e) => {
                out.write_str("Error: ")?;
                out.write_str(e)?;
                out.write_str("\r\n")?;
                return Ok(());
            }
        };

        let result = cortex_m::interrupt::free(|cs| {
            let mut can_opt = CAN.borrow(cs).borrow_mut();
            if let Some(can) = can_opt.as_mut() {
                // `transmit` enqueues the frame into the next free mailbox. If
                // all mailboxes are full it bumps the lowest-priority pending
                // frame and returns it; we just report success either way and
                // surface "would block" as a transient busy condition.
                match can.transmit(&frame) {
                    Ok(_dequeued) => Ok(()),
                    Err(nb::Error::WouldBlock) => Err("CAN TX mailboxes full, try again"),
                    Err(nb::Error::Other(_)) => Err("CAN transmit error"),
                }
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

    fn print_frame(&self, frame: &Frame, out: &mut SerialIO) -> Result<(), UsbError> {
        let id_val: u32 = match frame.id() {
            Id::Standard(sid) => sid.as_raw() as u32,
            Id::Extended(eid) => eid.as_raw(),
        };

        let mut buf = [0u8; 10];
        out.write_str("  ID ")?;
        if let Some(s) = self.format_hex_u32(id_val, &mut buf) {
            out.write_str(s)?;
        }

        if frame.is_extended() {
            out.write_str(" (ext)")?;
        } else {
            out.write_str(" (std)")?;
        }

        if frame.is_remote_frame() {
            out.write_str(" RTR\r\n")?;
        } else {
            out.write_str(" data:")?;
            if let Some(data) = frame.data() {
                let mut byte_buf = [0u8; 4];
                for byte in data.as_ref() {
                    out.write_str(" ")?;
                    if let Some(s) = self.format_hex_u8(*byte, &mut byte_buf) {
                        out.write_str(s)?;
                    }
                }
            }
            out.write_str("\r\n")?;
        }

        Ok(())
    }

    fn read_frames(&self, count: u32, timeout: u32, out: &mut SerialIO, cfg: &CliConfig) -> Result<(), UsbError> {
        let mut frames_read: u32 = 0;
        let verbose = !cfg.is_short_output();

        for _ in 0..count {
            let recv_result = cortex_m::interrupt::free(|cs| {
                let mut can_opt = CAN.borrow(cs).borrow_mut();
                if let Some(can) = can_opt.as_mut() {
                    if timeout == 0 {
                        // Non-blocking: single poll attempt.
                        match can.receive() {
                            Ok(frame) => Ok(Some(frame)),
                            Err(nb::Error::WouldBlock) => Ok(None),
                            Err(nb::Error::Other(_)) => Err("CAN receive error"),
                        }
                    } else {
                        // Blocking up to caller-provided timeout is not
                        // meaningful without a hardware timer reference here,
                        // so we busy-poll until a frame arrives or we give up
                        // after a bounded number of attempts proportional to
                        // the requested timeout (best-effort).
                        let mut attempts = timeout.max(1);
                        loop {
                            match can.receive() {
                                Ok(frame) => break Ok(Some(frame)),
                                Err(nb::Error::WouldBlock) => {
                                    attempts -= 1;
                                    if attempts == 0 {
                                        break Ok(None);
                                    }
                                    continue;
                                }
                                Err(nb::Error::Other(_)) => break Err("CAN receive error"),
                            }
                        }
                    }
                } else {
                    Err("CAN not initialized")
                }
            });

            match recv_result {
                Ok(Some(frame)) => {
                    frames_read += 1;
                    // Print immediately rather than buffering: `bxcan::Frame`
                    // isn't `Copy`, so an array of captured frames isn't an
                    // option without per-frame heap storage.
                    if verbose {
                        self.print_frame(&frame, out)?;
                    }
                }
                Ok(None) => break, // no more frames available before timeout
                Err(e) => {
                    if verbose {
                        out.write_str("Error: ")?;
                        out.write_str(e)?;
                        out.write_str("\r\n")?;
                    }
                    return Ok(());
                }
            }
        }

        if verbose {
            out.write_str("Read ")?;
            let mut buf = [0u8; 10];
            if let Some(s) = format_u32(frames_read, &mut buf) {
                out.write_str(s)?;
            }
            out.write_str(" frame(s)\r\n")?;
        } else {
            out.write_str("OK\r\n")?;
        }

        Ok(())
    }

    fn set_filter(&self, filter_id: u32, id: u32, mask: u32, out: &mut SerialIO, cfg: &CliConfig) -> Result<(), UsbError> {
        if filter_id > 13 {
            out.write_str("Error: Filter ID must be 0-13\r\n")?;
            return Ok(());
        }

        let std_id = match u16::try_from(id) {
            Ok(v) => v,
            Err(_) => {
                out.write_str("Error: Filter ID must fit in 11 bits\r\n")?;
                return Ok(());
            }
        };
        let std_mask = match u16::try_from(mask) {
            Ok(v) => v,
            Err(_) => {
                out.write_str("Error: Filter mask must fit in 11 bits\r\n")?;
                return Ok(());
            }
        };

        let bx_id = match StandardId::new(std_id) {
            Some(s) => s,
            None => {
                out.write_str("Error: Invalid standard ID\r\n")?;
                return Ok(());
            }
        };
        let bx_mask = match StandardId::new(std_mask) {
            Some(s) => s,
            None => {
                out.write_str("Error: Invalid filter mask\r\n")?;
                return Ok(());
            }
        };

        let result = cortex_m::interrupt::free(|cs| {
            let mut can_opt = CAN.borrow(cs).borrow_mut();
            if let Some(can) = can_opt.as_mut() {
                let filter = Mask32::frames_with_std_id(bx_id, bx_mask);
                can.modify_filters()
                    .enable_bank(filter_id as u8, Fifo::Fifo0, filter);
                Ok(())
            } else {
                Err("CAN not initialized")
            }
        });

        match result {
            Ok(_) => {
                cortex_m::interrupt::free(|cs| {
                    let mut filters = FILTER_STATE.borrow(cs).borrow_mut();
                    filters[filter_id as usize] = Some(FilterEntry { id, mask });
                });

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
            if let Some(can) = can_opt.as_mut() {
                can.modify_filters().disable_bank(filter_id as u8);
                Ok(())
            } else {
                Err("CAN not initialized")
            }
        });

        match result {
            Ok(_) => {
                cortex_m::interrupt::free(|cs| {
                    let mut filters = FILTER_STATE.borrow(cs).borrow_mut();
                    filters[filter_id as usize] = None;
                });

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

        out.write_str("  Filters:\r\n")?;
        let filters = cortex_m::interrupt::free(|cs| *FILTER_STATE.borrow(cs).borrow());
        let mut any_active = false;
        for (idx, entry) in filters.iter().enumerate() {
            if let Some(f) = entry {
                any_active = true;
                out.write_str("    Bank ")?;
                if let Some(s) = format_u32(idx as u32, &mut buf) {
                    out.write_str(s)?;
                }
                out.write_str(": ID ")?;
                let mut hexbuf = [0u8; 10];
                if let Some(s) = self.format_hex_u32(f.id, &mut hexbuf) {
                    out.write_str(s)?;
                }
                out.write_str(" mask ")?;
                if let Some(s) = self.format_hex_u32(f.mask, &mut hexbuf) {
                    out.write_str(s)?;
                }
                out.write_str("\r\n")?;
            }
        }
        if !any_active {
            out.write_str("    (none active)\r\n")?;
        }

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

/// Computes a CAN bit-timing value (packed into the format bxcan's
/// `set_bit_timing()` expects, matching the CAN_BTR register layout minus
/// mode bits) for a target bitrate given the actual APB1 (PCLK1) frequency.
///
/// CAN bit time = 1 (sync segment, fixed) + TS1 + TS2 time quanta, where
/// each time quantum = (BRP+1) / PCLK1 seconds. We aim for a sample point
/// around 87.5% of the bit time (the conventional choice for CAN), trying
/// total quanta counts from 25 down to 8 and picking the first one that
/// divides PCLK1/bitrate exactly with BRP in bxcan's valid range.
///
/// Returns `None` if no valid (BRP, TS1, TS2) combination exactly reaches
/// the requested bitrate from this clock — e.g. if pclk1_hz is 0 (CAN not
/// yet initialized) or the bitrate doesn't evenly divide the clock for any
/// allowed quanta count.
fn compute_bit_timing(pclk1_hz: u32, bitrate: u32) -> Option<u32> {
    if pclk1_hz == 0 || bitrate == 0 {
        return None;
    }

    // bxcan/STM32 BTR field ranges: BRP 1..=1024, TS1 1..=16, TS2 1..=8,
    // SJW 1..=4. Total time quanta (1 sync + TS1 + TS2) must be 8..=25 for
    // a sane CAN bit time.
    for total_tq in (8u32..=25).rev() {
        let denom = bitrate as u64 * total_tq as u64;
        if denom == 0 {
            continue;
        }
        if (pclk1_hz as u64) % denom != 0 {
            continue;
        }
        let brp = (pclk1_hz as u64) / denom;
        if brp < 1 || brp > 1024 {
            continue;
        }

        // Target ~87.5% sample point: sample point = (1 + TS1) / total_tq.
        // => TS1 = round(0.875 * total_tq) - 1, clamped to bxcan's range.
        let raw_ts1 = ((total_tq * 7 + 4) / 8).saturating_sub(1); // round(0.875*tq) - 1
        let ts1 = raw_ts1.clamp(1, 16);
        let ts2 = (total_tq - 1).saturating_sub(ts1).clamp(1, 8);

        // Re-validate the segments actually sum to total_tq; if clamping
        // pushed them off, this tq doesn't give a clean split — try the
        // next one down rather than emit slightly-wrong timing.
        if 1 + ts1 + ts2 != total_tq {
            continue;
        }

        let sjw: u32 = 1; // conservative default, valid 1..=4

        let brp_field = (brp as u32) - 1;
        let ts1_field = ts1 - 1;
        let ts2_field = ts2 - 1;
        let sjw_field = sjw - 1;

        let btr = (sjw_field << 24) | (ts2_field << 20) | (ts1_field << 16) | brp_field;
        return Some(btr);
    }

    None
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