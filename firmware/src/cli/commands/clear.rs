/*
* This is the clear command. All it does is clears the console by moving the cursor.

TODO Change the error colors to RED

? Author: Krzysztof Sikora, 16.02.2026
*/

use crate::cli::Command;
use crate::io_interface::serial::SerialIO;
use crate::cli::CliConfig;
use usb_device::UsbError;

#[derive(Clone)]
pub struct ClearCommand;

impl ClearCommand {
    pub fn new() -> Self {
        Self
    }
}

impl Command for ClearCommand {
    fn name(&self) -> &'static str {
        "clear"
    }

    fn initialize(&mut self) -> Result<(), &'static str> {
        Ok(())
    }
    
    fn execute(&mut self, args: &[&str], output: &mut SerialIO, _config: &mut CliConfig) -> Result<(), UsbError> {
        // Clear screen and move cursor to top-left
        output.write_str("\x1b[2J\x1b[H")?;
        
        // Optional: Show a message if there are args
        if !args.is_empty() {
            output.write_str("Screen cleared!\r\n")?;
        }
        
        Ok(())
    }
    
    fn print_help(&self, output: &mut SerialIO) -> Result<(), UsbError> {
        output.write_str("clear - Clear the terminal screen\r\n")
    }
}