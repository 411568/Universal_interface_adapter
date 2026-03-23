use crate::cli::Command;
use crate::io_interface::serial::SerialIO;
use crate::cli::CliConfig;
use usb_device::UsbError;
use stm32f4xx_hal as hal;
use hal::gpio::{Pin, Output, PushPull};
use cortex_m::interrupt::Mutex;
use core::cell::RefCell;

// Enum to hold any of our Relay pins (PE0-PE1)
enum RelayPin {
    Pin0(Pin<'E', 0, Output<PushPull>>),
    Pin1(Pin<'E', 1, Output<PushPull>>),
}

impl RelayPin {
    fn set_high(&mut self) {
        match self {
            RelayPin::Pin0(pin) => pin.set_high(),
            RelayPin::Pin1(pin) => pin.set_high(),
        }
    }
    
    fn set_low(&mut self) {
        match self {
            RelayPin::Pin0(pin) => pin.set_low(),
            RelayPin::Pin1(pin) => pin.set_low(),
        }
    }
}

// Static storage for Relay pins
static RELAY_PINS: Mutex<RefCell<Option<[RelayPin; 2]>>> = Mutex::new(RefCell::new(None));

#[derive(Clone)]
pub struct RelayCommand;

impl RelayCommand {
    pub fn new() -> Self {
        Self
    }
    
    /// Initialize Relay pins - call this from main with the GPIO pins (PE0-PE1)
    pub fn init_pins(
        pe0: Pin<'E', 0>,
        pe1: Pin<'E', 1>,
    ) {
        let relay1 = RelayPin::Pin0(pe0.into_push_pull_output());
        let relay2 = RelayPin::Pin1(pe1.into_push_pull_output());
        
        cortex_m::interrupt::free(|cs| {
            *RELAY_PINS.borrow(cs).borrow_mut() = Some([relay1, relay2]);
        });
    }
    
    /// Parse Relay number from string (1-2)
    fn parse_relay_number(&self, arg: &str) -> Result<usize, &'static str> {
        match arg {
            "1" => Ok(0),
            "2" => Ok(1),
            _ => Err("Relay number must be 1 or 2"),
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
    
    /// Control Relay
    fn control_relay(&self, relay_num: usize, state: bool) -> Result<(), &'static str> {
        cortex_m::interrupt::free(|cs| {
            if let Some(pins) = RELAY_PINS.borrow(cs).borrow_mut().as_mut() {
                if relay_num < pins.len() {
                    if state {
                        pins[relay_num].set_high();
                    } else {
                        pins[relay_num].set_low();
                    }
                    Ok(())
                } else {
                    Err("Invalid relay number")
                }
            } else {
                Err("Relay pins not initialized")
            }
        })
    }
}

impl Command for RelayCommand {
    fn name(&self) -> &'static str {
        "relay"
    }

    fn initialize(&mut self) -> Result<(), &'static str> {
        Ok(())
    }
    
    fn execute(&mut self, args: &[&str], output: &mut SerialIO, config: &mut CliConfig) -> Result<(), UsbError> {
        // Check argument count
        if args.len() != 2 {
            output.write_str("Usage: relay <1-2> <on|off>\r\n")?;
            return Ok(());
        }
        
        // Parse relay number
        let relay_num = match self.parse_relay_number(args[0]) {
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
        
        // Control the relay
        match self.control_relay(relay_num, state) {
            Ok(()) => {
                if config.is_short_output() {
                    output.write_str("OK\r\n")?;
                } else {
                    output.write_str("Relay ")?;
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
        output.write_str("relay <1-2> <on|off> - Control relays on PE0-PE1\r\n")
    }
}