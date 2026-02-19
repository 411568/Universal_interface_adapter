/*
* This is the main CLI Command trait
* All new commands have to implement this trait to be registered

? Author: Krzysztof Sikora, 16.02.2026
*/

use crate::io_interface::serial::SerialIO;
use crate::cli::CliConfig;
use usb_device::UsbError;

/// Trait for all CLI commands
pub trait Command {
    /// Get the command name (what user types)
    fn name(&self) -> &'static str;

    /// Initialize the required peripherals etc
    fn initialize(&mut self) -> Result<(), &'static str>;
    
    /// Execute the command with given arguments
    fn execute(&mut self, args: &[&str], output: &mut SerialIO, config: &mut CliConfig) -> Result<(), UsbError>;
    
    /// Print help information for this command
    fn print_help(&self, output: &mut SerialIO) -> Result<(), UsbError>;
}