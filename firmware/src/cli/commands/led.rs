use crate::cli::Command;
use crate::io_interface::serial::SerialIO;
use crate::cli::CliConfig;
use usb_device::UsbError;
use stm32f4xx_hal as hal;
use hal::gpio::{Pin, Output, PushPull};
use cortex_m::interrupt::Mutex;
use core::cell::RefCell;

// Enum to hold any of our LED pins
enum LedPin {
    Pin4(Pin<'D', 4, Output<PushPull>>),
    Pin5(Pin<'D', 5, Output<PushPull>>),
    Pin6(Pin<'D', 6, Output<PushPull>>),
    Pin7(Pin<'D', 7, Output<PushPull>>),
}

impl LedPin {
    fn set_high(&mut self) {
        match self {
            LedPin::Pin4(pin) => pin.set_high(),
            LedPin::Pin5(pin) => pin.set_high(),
            LedPin::Pin6(pin) => pin.set_high(),
            LedPin::Pin7(pin) => pin.set_high(),
        }
    }
    
    fn set_low(&mut self) {
        match self {
            LedPin::Pin4(pin) => pin.set_low(),
            LedPin::Pin5(pin) => pin.set_low(),
            LedPin::Pin6(pin) => pin.set_low(),
            LedPin::Pin7(pin) => pin.set_low(),
        }
    }
}

// Static storage for LED pins
static LED_PINS: Mutex<RefCell<Option<[LedPin; 4]>>> = Mutex::new(RefCell::new(None));

#[derive(Clone)]
pub struct LedCommand;

impl LedCommand {
    pub fn new() -> Self {
        Self
    }
    
    /// Initialize LED pins - call this from main with the GPIO pins
    pub fn init_pins(
        pd4: Pin<'D', 4>,
        pd5: Pin<'D', 5>,
        pd6: Pin<'D', 6>,
        pd7: Pin<'D', 7>,
    ) {
        let led1 = LedPin::Pin4(pd4.into_push_pull_output());
        let led2 = LedPin::Pin5(pd5.into_push_pull_output());
        let led3 = LedPin::Pin6(pd6.into_push_pull_output());
        let led4 = LedPin::Pin7(pd7.into_push_pull_output());
        
        cortex_m::interrupt::free(|cs| {
            *LED_PINS.borrow(cs).borrow_mut() = Some([led1, led2, led3, led4]);
        });
    }
    
    /// Parse LED number from string (1-4)
    fn parse_led_number(&self, arg: &str) -> Result<usize, &'static str> {
        match arg {
            "1" => Ok(0),
            "2" => Ok(1),
            "3" => Ok(2),
            "4" => Ok(3),
            _ => Err("LED number must be 1, 2, 3, or 4"),
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
    
    /// Control LED
    fn control_led(&self, led_num: usize, state: bool) -> Result<(), &'static str> {
        cortex_m::interrupt::free(|cs| {
            if let Some(pins) = LED_PINS.borrow(cs).borrow_mut().as_mut() {
                if led_num < pins.len() {
                    if state {
                        pins[led_num].set_high();
                    } else {
                        pins[led_num].set_low();
                    }
                    Ok(())
                } else {
                    Err("Invalid LED number")
                }
            } else {
                Err("LED pins not initialized")
            }
        })
    }
}

impl Command for LedCommand {
    fn name(&self) -> &'static str {
        "led"
    }

    fn initialize(&mut self) -> Result<(), &'static str> {
        Ok(())
    }
    
    fn execute(&mut self, args: &[&str], output: &mut SerialIO, config: &mut CliConfig) -> Result<(), UsbError> {
        // Check argument count
        if args.len() != 2 {
            output.write_str("Usage: led <1-4> <on|off>\r\n")?;
            return Ok(());
        }
        
        // Parse LED number
        let led_num = match self.parse_led_number(args[0]) {
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
        
        // Control the LED
        match self.control_led(led_num, state) {
            Ok(()) => {
                if config.is_short_output() {
                    output.write_str("OK\r\n")?;
                } else {
                    output.write_str("LED ")?;
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
        output.write_str("led <1-4> <on|off> - Control LEDs on PD4-PD7\r\n")
    }
}