use crate::cli::Command;
use crate::io_interface::serial::SerialIO;
use crate::cli::CliConfig;
use usb_device::UsbError;
use stm32f4xx_hal as hal;
use hal::gpio::{Pin, Output, PushPull, Input};
use cortex_m::interrupt::Mutex;
use core::cell::RefCell;

// Enum to hold any of our GPIO pins in either mode (PD12-PD15)
enum GpioPin {
    // Output variants
    OutputPin12(Pin<'D', 12, Output<PushPull>>),
    OutputPin13(Pin<'D', 13, Output<PushPull>>),
    OutputPin14(Pin<'D', 14, Output<PushPull>>),
    OutputPin15(Pin<'D', 15, Output<PushPull>>),
    
    // Input variants
    InputPin12(Pin<'D', 12, Input<>>),
    InputPin13(Pin<'D', 13, Input<>>),
    InputPin14(Pin<'D', 14, Input<>>),
    InputPin15(Pin<'D', 15, Input<>>),
}

impl GpioPin {
    // Set pin high (only works for output pins)
    fn set_high(&mut self) -> Result<(), &'static str> {
        match self {
            GpioPin::OutputPin12(pin) => { pin.set_high(); Ok(()) }
            GpioPin::OutputPin13(pin) => { pin.set_high(); Ok(()) }
            GpioPin::OutputPin14(pin) => { pin.set_high(); Ok(()) }
            GpioPin::OutputPin15(pin) => { pin.set_high(); Ok(()) }
            _ => Err("Cannot set: pin is configured as input"),
        }
    }
    
    // Set pin low (only works for output pins)
    fn set_low(&mut self) -> Result<(), &'static str> {
        match self {
            GpioPin::OutputPin12(pin) => { pin.set_low(); Ok(()) }
            GpioPin::OutputPin13(pin) => { pin.set_low(); Ok(()) }
            GpioPin::OutputPin14(pin) => { pin.set_low(); Ok(()) }
            GpioPin::OutputPin15(pin) => { pin.set_low(); Ok(()) }
            _ => Err("Cannot set: pin is configured as input"),
        }
    }
    
    // Read pin state
    // For input pins: reads actual pin state
    // For output pins: reads current output state
    fn read_state(&self) -> bool {
        match self {
            // Output pins - return output state
            GpioPin::OutputPin12(pin) => pin.is_set_high(),
            GpioPin::OutputPin13(pin) => pin.is_set_high(),
            GpioPin::OutputPin14(pin) => pin.is_set_high(),
            GpioPin::OutputPin15(pin) => pin.is_set_high(),
            
            // Input pins - return actual pin state
            GpioPin::InputPin12(pin) => pin.is_high(),
            GpioPin::InputPin13(pin) => pin.is_high(),
            GpioPin::InputPin14(pin) => pin.is_high(),
            GpioPin::InputPin15(pin) => pin.is_high(),
        }
    }
}

// Structure to hold pin configuration and current mode
struct PinConfig {
    pin: GpioPin,
    mode: PinMode,
}

#[derive(Clone, Copy, PartialEq)]
enum PinMode {
    Input,
    Output,
}

// Static storage for GPIO pins
static GPIO_PINS: Mutex<RefCell<Option<[PinConfig; 4]>>> = Mutex::new(RefCell::new(None));

#[derive(Clone)]
pub struct GpioCommand;

impl GpioCommand {
    pub fn new() -> Self {
        Self
    }
    
    /// Initialize GPIO pins - call this from main with the GPIO pins (PD12-PD15)
    /// By default, pins are initialized as inputs for safety
    pub fn init_pins(
        pd12: Pin<'D', 12>,
        pd13: Pin<'D', 13>,
        pd14: Pin<'D', 14>,
        pd15: Pin<'D', 15>,
    ) {
        // Initialize all pins as inputs by default (safer)
        let pin1 = PinConfig {
            pin: GpioPin::InputPin12(pd12.into_floating_input()),
            mode: PinMode::Input,
        };
        let pin2 = PinConfig {
            pin: GpioPin::InputPin13(pd13.into_floating_input()),
            mode: PinMode::Input,
        };
        let pin3 = PinConfig {
            pin: GpioPin::InputPin14(pd14.into_floating_input()),
            mode: PinMode::Input,
        };
        let pin4 = PinConfig {
            pin: GpioPin::InputPin15(pd15.into_floating_input()),
            mode: PinMode::Input,
        };
        
        cortex_m::interrupt::free(|cs| {
            *GPIO_PINS.borrow(cs).borrow_mut() = Some([pin1, pin2, pin3, pin4]);
        });
    }
    
    /// Parse pin number from string (12-15)
    fn parse_pin_number(&self, arg: &str) -> Result<usize, &'static str> {
        match arg {
            "12" => Ok(0),
            "13" => Ok(1),
            "14" => Ok(2),
            "15" => Ok(3),
            _ => Err("Pin number must be 12, 13, 14, or 15"),
        }
    }
    
    /// Parse mode (input/output)
    fn parse_mode(&self, arg: &str) -> Result<PinMode, &'static str> {
        match arg {
            "input" => Ok(PinMode::Input),
            "output" => Ok(PinMode::Output),
            _ => Err("Mode must be 'input' or 'output'"),
        }
    }
    
    /// Parse state (high/low)
    fn parse_state(&self, arg: &str) -> Result<bool, &'static str> {
        match arg {
            "high" => Ok(true),
            "low" => Ok(false),
            _ => Err("State must be 'high' or 'low'"),
        }
    }
    
    /// Set pin mode
    fn set_mode(&self, pin_num: usize, mode: PinMode) -> Result<(), &'static str> {
        cortex_m::interrupt::free(|cs| {
            if let Some(pins) = GPIO_PINS.borrow(cs).borrow_mut().as_mut() {
                if pin_num >= pins.len() {
                    return Err("Invalid pin number");
                }
                
                // Check if mode is already set to desired value
                if pins[pin_num].mode == mode {
                    return Ok(());
                }
                
                // Need to recreate the pin with new mode
                // This is a simplified approach - in reality, you'd need to
                // properly reconfigure the pin. For now, we'll return an error
                // as dynamic reconfiguration is complex with the HAL
                Err("Mode change requires reinitialization. Set mode during initialization.")
            } else {
                Err("GPIO pins not initialized")
            }
        })
    }
    
    /// Set pin state (only works if pin is in output mode)
    fn set_pin_state(&self, pin_num: usize, state: bool) -> Result<(), &'static str> {
        cortex_m::interrupt::free(|cs| {
            if let Some(pins) = GPIO_PINS.borrow(cs).borrow_mut().as_mut() {
                if pin_num >= pins.len() {
                    return Err("Invalid pin number");
                }
                
                if pins[pin_num].mode != PinMode::Output {
                    return Err("Cannot set: pin is configured as input");
                }
                
                if state {
                    pins[pin_num].pin.set_high()
                } else {
                    pins[pin_num].pin.set_low()
                }
            } else {
                Err("GPIO pins not initialized")
            }
        })
    }
    
    /// Read pin state (works for both input and output)
    fn read_pin_state(&self, pin_num: usize) -> Result<bool, &'static str> {
        cortex_m::interrupt::free(|cs| {
            if let Some(pins) = GPIO_PINS.borrow(cs).borrow().as_ref() {
                if pin_num < pins.len() {
                    Ok(pins[pin_num].pin.read_state())
                } else {
                    Err("Invalid pin number")
                }
            } else {
                Err("GPIO pins not initialized")
            }
        })
    }
}

impl Command for GpioCommand {
    fn name(&self) -> &'static str {
        "gpio"
    }

    fn initialize(&mut self) -> Result<(), &'static str> {
        Ok(())
    }
    
    fn execute(&mut self, args: &[&str], output: &mut SerialIO, config: &mut CliConfig) -> Result<(), UsbError> {
        if args.is_empty() {
            output.write_str("Usage: gpio <pin> <command> [parameters]\r\n")?;
            output.write_str("Commands:\r\n")?;
            output.write_str("  mode <input|output> - Set pin mode\r\n")?;
            output.write_str("  set <high|low>      - Set pin state (output mode only)\r\n")?;
            output.write_str("  read                 - Read pin state\r\n")?;
            return Ok(());
        }
        
        // Parse pin number (first argument)
        let pin_num = match self.parse_pin_number(args[0]) {
            Ok(num) => num,
            Err(e) => {
                output.write_str(e)?;
                output.write_str("\r\n")?;
                return Ok(());
            }
        };
        
        // Check if we have a command
        if args.len() < 2 {
            output.write_str("Missing command. Use: gpio <pin> <mode|set|read>\r\n")?;
            return Ok(());
        }
        
        match args[1] {
            "mode" => {
                // Set mode: gpio <pin> mode <input|output>
                if args.len() != 3 {
                    output.write_str("Usage: gpio <pin> mode <input|output>\r\n")?;
                    return Ok(());
                }
                
                let mode = match self.parse_mode(args[2]) {
                    Ok(m) => m,
                    Err(e) => {
                        output.write_str(e)?;
                        output.write_str("\r\n")?;
                        return Ok(());
                    }
                };
                
                match self.set_mode(pin_num, mode) {
                    Ok(()) => {
                        if config.is_short_output() {
                            output.write_str("OK\r\n")?;
                        } else {
                            output.write_str("Pin ")?;
                            output.write_str(args[0])?;
                            output.write_str(" mode set to ")?;
                            output.write_str(args[2])?;
                            output.write_str("\r\n")?;
                        }
                    }
                    Err(e) => {
                        output.write_str(e)?;
                        output.write_str("\r\n")?;
                    }
                }
            }
            
            "set" => {
                // Set state: gpio <pin> set <high|low>
                if args.len() != 3 {
                    output.write_str("Usage: gpio <pin> set <high|low>\r\n")?;
                    return Ok(());
                }
                
                let state = match self.parse_state(args[2]) {
                    Ok(s) => s,
                    Err(e) => {
                        output.write_str(e)?;
                        output.write_str("\r\n")?;
                        return Ok(());
                    }
                };
                
                match self.set_pin_state(pin_num, state) {
                    Ok(()) => {
                        if config.is_short_output() {
                            output.write_str("OK\r\n")?;
                        } else {
                            output.write_str("Pin ")?;
                            output.write_str(args[0])?;
                            output.write_str(" set to ")?;
                            output.write_str(args[2])?;
                            output.write_str("\r\n")?;
                        }
                    }
                    Err(e) => {
                        output.write_str(e)?;
                        output.write_str("\r\n")?;
                    }
                }
            }
            
            "read" => {
                // Read state: gpio <pin> read
                if args.len() != 2 {
                    output.write_str("Usage: gpio <pin> read\r\n")?;
                    return Ok(());
                }
                
                match self.read_pin_state(pin_num) {
                    Ok(state) => {
                        if config.is_short_output() {
                            if state {
                                output.write_str("1\r\n")?;
                            } else {
                                output.write_str("0\r\n")?;
                            }
                        } else {
                            output.write_str("Pin ")?;
                            output.write_str(args[0])?;
                            output.write_str(" is ")?;
                            if state {
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
            }
            
            _ => {
                output.write_str("Unknown command. Use: mode, set, or read\r\n")?;
            }
        }
        
        Ok(())
    }
    
    fn print_help(&self, output: &mut SerialIO) -> Result<(), UsbError> {
        output.write_str("gpio <12-15> mode <input|output> - Set pin mode (NOTE: mode changes require reinit)\r\n")?;
        output.write_str("gpio <12-15> set <high|low>       - Set pin state (output mode only)\r\n")?;
        output.write_str("gpio <12-15> read                 - Read pin state (works for both modes)\r\n")?;
        output.write_str("Controls GPIO pins PD12-PD15\r\n")
    }
}