use crate::cli::Command;
use crate::io_interface::serial::SerialIO;
use crate::cli::CliConfig;
use usb_device::UsbError;
use stm32f4xx_hal as hal;
use hal::gpio::{Pin, Output, PushPull};
use cortex_m::interrupt::Mutex;
use core::cell::RefCell;

// Enum to hold any of our MOSFET pins (PC6-PC9)
enum MosfetPin {
    Pin6(Pin<'C', 6, Output<PushPull>>),
    Pin7(Pin<'C', 7, Output<PushPull>>),
    Pin8(Pin<'C', 8, Output<PushPull>>),
    Pin9(Pin<'C', 9, Output<PushPull>>),
}

impl MosfetPin {
    fn set_high(&mut self) {
        match self {
            MosfetPin::Pin6(pin) => pin.set_high(),
            MosfetPin::Pin7(pin) => pin.set_high(),
            MosfetPin::Pin8(pin) => pin.set_high(),
            MosfetPin::Pin9(pin) => pin.set_high(),
        }
    }
    
    fn set_low(&mut self) {
        match self {
            MosfetPin::Pin6(pin) => pin.set_low(),
            MosfetPin::Pin7(pin) => pin.set_low(),
            MosfetPin::Pin8(pin) => pin.set_low(),
            MosfetPin::Pin9(pin) => pin.set_low(),
        }
    }
}

// Static storage for MOSFET pins
static MOSFET_PINS: Mutex<RefCell<Option<[MosfetPin; 4]>>> = Mutex::new(RefCell::new(None));

#[derive(Clone)]
pub struct MosfetCommand;

impl MosfetCommand {
    pub fn new() -> Self {
        Self
    }
    
    /// Initialize MOSFET pins - call this from main with the GPIO pins (PC6-PC9)
    pub fn init_pins(
        pc6: Pin<'C', 6>,
        pc7: Pin<'C', 7>,
        pc8: Pin<'C', 8>,
        pc9: Pin<'C', 9>,
    ) {
        let mosfet1 = MosfetPin::Pin6(pc6.into_push_pull_output());
        let mosfet2 = MosfetPin::Pin7(pc7.into_push_pull_output());
        let mosfet3 = MosfetPin::Pin8(pc8.into_push_pull_output());
        let mosfet4 = MosfetPin::Pin9(pc9.into_push_pull_output());
        
        cortex_m::interrupt::free(|cs| {
            *MOSFET_PINS.borrow(cs).borrow_mut() = Some([mosfet1, mosfet2, mosfet3, mosfet4]);
        });
    }
    
    /// Parse MOSFET number from string (1-4)
    fn parse_mosfet_number(&self, arg: &str) -> Result<usize, &'static str> {
        match arg {
            "1" => Ok(0),
            "2" => Ok(1),
            "3" => Ok(2),
            "4" => Ok(3),
            _ => Err("MOSFET number must be 1, 2, 3, or 4"),
        }
    }
    
    /// Parse on/off state
    fn parse_state(&self, arg: &str) -> Result<bool, &'static str> {
        match arg {
            "on" => Ok(true),
            "off" => Ok(false),
            _ => Err("State must be 'on' or 'off'"),
        }
    }
    
    /// Control MOSFET
    fn control_mosfet(&self, mosfet_num: usize, state: bool) -> Result<(), &'static str> {
        cortex_m::interrupt::free(|cs| {
            if let Some(pins) = MOSFET_PINS.borrow(cs).borrow_mut().as_mut() {
                if mosfet_num < pins.len() {
                    if state {
                        pins[mosfet_num].set_high();
                    } else {
                        pins[mosfet_num].set_low();
                    }
                    Ok(())
                } else {
                    Err("Invalid MOSFET number")
                }
            } else {
                Err("MOSFET pins not initialized")
            }
        })
    }
}

impl Command for MosfetCommand {
    fn name(&self) -> &'static str {
        "mosfet"
    }

    fn initialize(&mut self) -> Result<(), &'static str> {
        Ok(())
    }
    
    fn execute(&mut self, args: &[&str], output: &mut SerialIO, config: &mut CliConfig) -> Result<(), UsbError> {
        // Check argument count
        if args.len() != 2 {
            output.write_str("Usage: mosfet <1-4> <on|off>\r\n")?;
            return Ok(());
        }
        
        // Parse MOSFET number
        let mosfet_num = match self.parse_mosfet_number(args[0]) {
            Ok(num) => num,
            Err(e) => {
                output.write_str(e)?;
                output.write_str("\r\n")?;
                return Ok(());
            }
        };
        
        // Parse state
        let state = match self.parse_state(args[1]) {
            Ok(s) => s,
            Err(e) => {
                output.write_str(e)?;
                output.write_str("\r\n")?;
                return Ok(());
            }
        };
        
        // Control the MOSFET
        match self.control_mosfet(mosfet_num, state) {
            Ok(()) => {
                if config.is_short_output() {
                    output.write_str("OK\r\n")?;
                } else {
                    output.write_str("MOSFET ")?;
                    output.write_str(args[0])?;
                    if state {
                        output.write_str(" turned ON\r\n")?;
                    } else {
                        output.write_str(" turned OFF\r\n")?;
                    }
                }
            }
            Err(e) => {
                output.write_str(e)?;
                output.write_str("\r\n")?;
            }
        }
        
        Ok(())
    }
    
    fn print_help(&self, output: &mut SerialIO) -> Result<(), UsbError> {
        output.write_str("mosfet <1-4> <on|off> - Control MOSFETs on PC6-PC9\r\n")
    }
}