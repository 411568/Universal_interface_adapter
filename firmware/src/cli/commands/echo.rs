/*
* This is the echo command, as basic as it can get. It just echoes the text back to the terminal.

TODO Change the error colors to RED

? Author: Krzysztof Sikora, 16.02.2026
*/

use crate::cli::Command;
use crate::io_interface::serial::SerialIO;
use crate::cli::CliConfig;
use usb_device::UsbError;

#[derive(Clone)]
pub struct EchoCommand;

impl EchoCommand {
    pub fn new() -> Self {
        Self
    }
}

impl Command for EchoCommand {
    fn name(&self) -> &'static str {
        "echo"
    }

    fn initialize(&mut self) -> Result<(), &'static str> {
        // Nothing to initialize
        Ok(())
    }
    
    fn execute(&mut self, args: &[&str], output: &mut SerialIO, config: &mut CliConfig) -> Result<(), UsbError> 
    {
        if args.is_empty() {
            output.write_str("No message provided. Type 'help echo' for more options.\r\n")?;
            return Ok(());
        }

        // TESTING: Use config to determine output format
        if config.is_short_output() {
            // Write each argument manually instead of using join
            for (i, arg) in args.iter().enumerate() {
                if i > 0 {
                    output.write_str(" ")?;
                }
                output.write_str(arg)?;
            }
            output.write_str("\r\n")?;
        } else {
            output.write_str("Echo output: ")?;
            for (i, arg) in args.iter().enumerate() {
                if i > 0 {
                    output.write_str(" ")?;
                }
                output.write_str(arg)?;
            }
            output.write_str("\r\n")?;
        }
        Ok(())
    } 
    
    fn print_help(&self, output: &mut SerialIO) -> Result<(), UsbError> {
        output.write_str("echo <text>     - Echo back the provided text\r\n")
    }
}