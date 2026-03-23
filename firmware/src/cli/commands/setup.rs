/*
* The setup command allows the user to modify some of the cli_config parameters.

TODO Change the error colors to RED
TODO add a delay when writing long help to not overflow the buffer

? Author: Krzysztof Sikora, 16.02.2026
*/


use crate::cli::Command;
use crate::io_interface::serial::SerialIO;
use crate::cli::CliConfig;
use crate::cli::cli_config::AnswerLength;
use usb_device::UsbError;

#[derive(Clone)]
pub struct SetupCommand;

impl SetupCommand {
    pub fn new() -> Self {
        Self
    }
}

impl Command for SetupCommand {
    fn name(&self) -> &'static str {
        "setup"
    }

    fn initialize(&mut self) -> Result<(), &'static str> {
        // Nothing to initialize
        Ok(())
    }
    
    fn execute(&mut self, args: &[&str], output: &mut SerialIO, config: &mut CliConfig) -> Result<(), UsbError> {
        if args.is_empty() {
            output.write_str("Unknown setup option. Type 'help setup' for usage.\r\n")?;
            return Ok(());
        }
        
        // If the first argument is not recognized, print an error
        if args[0] != "colors" && args[0] != "answer_length" && args[0] != "prompt_character_mode" && args[0] != "prompt_character" {
            output.write_str("Unknown setup option: ")?;
            output.write_str(args[0])?;
            output.write_str(". Type 'help setup' for usage.\r\n")?;
        }

        // Set the prompt coloring mode
        if args[0] == "colors" && args.len() > 1 {
            if args[1] == "on" {
                config.set_colored_output(true);
                if config.answer_length != AnswerLength::Short {
                    output.write_str("Colored output enabled.\r\n")?;
                }
            }
            else if args[1] == "off" {
                config.set_colored_output(false);
                if config.answer_length != AnswerLength::Short {
                    output.write_str("Colored output disabled.\r\n")?;
                }
            }
            else {
                output.write_str("Invalid option for colors. Use 'on' or 'off'.\r\n")?;
            }
        }
        else if args[0] == "colors" {
            output.write_str("Unknown option. Usage: setup colors <on|off>\r\n")?;
        }


        // Set the answer length mode
        if args[0] == "answer_length" && args.len() > 1 {
            if args[1] == "short" {
                config.set_answer_length(AnswerLength::Short);
                if config.answer_length != AnswerLength::Short {
                    output.write_str("Answer length set to short.\r\n")?;
                }
            }
            else if args[1] == "long" {
                config.set_answer_length(AnswerLength::Long);
                if config.answer_length != AnswerLength::Short {
                    output.write_str("Answer length set to long.\r\n")?;
                }
            }
            else {
                output.write_str("Invalid option for answer_length. Use 'short' or 'long'.\r\n")?;
            }
        }
        else if args[0] == "answer_length" {
            output.write_str("Unknown option. Usage: setup answer_length <short|long>\r\n")?;
        }

        // Set the prompt character mode
        if args[0] == "prompt_character_mode" && args.len() > 1 {
            if args[1] == "on" {
                config.set_prompt_character_enabled(true);
                if config.answer_length != AnswerLength::Short {
                    output.write_str("Prompt character enabled.\r\n")?;
                }
            }
            else if args[1] == "off" {
                config.set_prompt_character_enabled(false);
                if config.answer_length != AnswerLength::Short {
                    output.write_str("Prompt character disabled.\r\n")?;
                }
            }
            else {
                output.write_str("Invalid option for prompt_character. Use 'on' or 'off'.\r\n")?;
            }
        }
        else if args[0] == "prompt_character_mode" {
            output.write_str("Unknown option. Usage: setup prompt_character <on|off>\r\n")?;
        }

        // Set the prompt character
        if args[0] == "prompt_character" && args.len() > 1 {
            if args[1].len() == 1 {
                if let Some(c) = args[1].chars().next() {
                    config.set_prompt_char(c);
                    if config.answer_length != AnswerLength::Short {
                        output.write_str("Prompt character set to '")?;
                        // Need to handle writing a single character
                        let mut char_buf = [0u8; 4];
                        let char_str = c.encode_utf8(&mut char_buf);
                        output.write_str(char_str)?;
                        output.write_str("'.\r\n")?;
                    }
                }
            }
            else {
                output.write_str("Invalid option for prompt_character. Use a single character.\r\n")?;
            }
        }
        else if args[0] == "prompt_character" {
            output.write_str("Unknown option. Usage: setup prompt_character <character>\r\n")?;
        }
        
        Ok(())
    }
    
    fn print_help(&self, output: &mut SerialIO) -> Result<(), UsbError> {
        output.write_str("setup - Configure CLI settings\r\n")?;
        output.write_str("Usage:\r\n")?;
        output.write_str("  setup colors <on|off>                - Enable/disable colored output\r\n")?;
        output.write_str("  setup answer_length <short|long>     - Set output verbosity\r\n")?;
        output.write_str("  setup prompt_character_mode <on|off> - Enable/disable prompt character\r\n")?;
        output.write_str("  setup prompt_character <char>        - Set the prompt character\r\n")
    }
}