use crate::cli::Command;
use crate::io_interface::serial::SerialIO;
use crate::cli::CliConfig;
use usb_device::UsbError;
use stm32f4xx_hal as hal;
use hal::gpio::{Pin, Input};
use cortex_m::interrupt::Mutex;
use core::cell::RefCell;

// Enum to hold any of our isolated input pins (PE2-PE4)
enum IsolatedInputPin {
    Pin2(Pin<'E', 2, Input<>>),
    Pin3(Pin<'E', 3, Input<>>),
    Pin4(Pin<'E', 4, Input<>>),
}

impl IsolatedInputPin {
    fn is_high(&self) -> bool {
        match self {
            IsolatedInputPin::Pin2(pin) => pin.is_high(),
            IsolatedInputPin::Pin3(pin) => pin.is_high(),
            IsolatedInputPin::Pin4(pin) => pin.is_high(),
        }
    }
    
    fn is_low(&self) -> bool {
        match self {
            IsolatedInputPin::Pin2(pin) => pin.is_low(),
            IsolatedInputPin::Pin3(pin) => pin.is_low(),
            IsolatedInputPin::Pin4(pin) => pin.is_low(),
        }
    }
}

// Static storage for isolated input pins
static ISOLATED_INPUT_PINS: Mutex<RefCell<Option<[IsolatedInputPin; 3]>>> = Mutex::new(RefCell::new(None));

#[derive(Clone)]
pub struct IsolatedInputCommand;

impl IsolatedInputCommand {
    pub fn new() -> Self {
        Self
    }
    
    /// Initialize isolated input pins - call this from main with the GPIO pins (PE2-PE4)
    pub fn init_pins(
        pe2: Pin<'E', 2>,
        pe3: Pin<'E', 3>,
        pe4: Pin<'E', 4>,
    ) {
        let input1 = IsolatedInputPin::Pin2(pe2.into_floating_input());
        let input2 = IsolatedInputPin::Pin3(pe3.into_floating_input());
        let input3 = IsolatedInputPin::Pin4(pe4.into_floating_input());
        
        cortex_m::interrupt::free(|cs| {
            *ISOLATED_INPUT_PINS.borrow(cs).borrow_mut() = Some([input1, input2, input3]);
        });
    }
    
    /// Parse input number from string (1-3)
    fn parse_input_number(&self, arg: &str) -> Result<usize, &'static str> {
        match arg {
            "1" => Ok(0),
            "2" => Ok(1),
            "3" => Ok(2),
            _ => Err("Input number must be 1, 2, or 3"),
        }
    }
    
    /// Read isolated input state
    fn read_input(&self, input_num: usize) -> Result<bool, &'static str> {
        cortex_m::interrupt::free(|cs| {
            if let Some(pins) = ISOLATED_INPUT_PINS.borrow(cs).borrow().as_ref() {
                if input_num < pins.len() {
                    Ok(pins[input_num].is_high())
                } else {
                    Err("Invalid input number")
                }
            } else {
                Err("Isolated input pins not initialized")
            }
        })
    }
}

impl Command for IsolatedInputCommand {
    fn name(&self) -> &'static str {
        "isolated_input"
    }

    fn initialize(&mut self) -> Result<(), &'static str> {
        Ok(())
    }
    
    fn execute(&mut self, args: &[&str], output: &mut SerialIO, config: &mut CliConfig) -> Result<(), UsbError> {
        // Check argument count (should be exactly 1 argument - the input number)
        if args.len() != 1 {
            output.write_str("Usage: isolated_input <1-3>\r\n")?;
            return Ok(());
        }
        
        // Parse input number
        let input_num = match self.parse_input_number(args[0]) {
            Ok(num) => num,
            Err(e) => {
                output.write_str(e)?;
                output.write_str("\r\n")?;
                return Ok(());
            }
        };
        
        // Read the input state
        match self.read_input(input_num) {
            Ok(is_high) => {
                if config.is_short_output() {
                    if is_high {
                        output.write_str("1\r\n")?;
                    } else {
                        output.write_str("0\r\n")?;
                    }
                } else {
                    output.write_str("Input ")?;
                    output.write_str(args[0])?;
                    output.write_str(" is ")?;
                    if is_high {
                        output.write_str("HIGH\r\n")?;
                    } else {
                        output.write_str("LOW\r\n")?;
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
        output.write_str("isolated_input <1-3> - Read isolated inputs on PE2-PE4 (returns HIGH/LOW)\r\n")
    }
}