//! CLI main loop

use crate::io_interface::serial::SerialIO;
use crate::cli::CommandRegistry;
use crate::cli::CliConfig;

/// This is the actual cli loop that reads the input etc.
pub fn run_cli(serial: &mut SerialIO, registry: &mut CommandRegistry, config: &mut CliConfig) -> ! {
    let mut line_buffer = [0u8; 64];
    
    loop {
        // Poll USB
        serial.poll();

        // Set command color for user input BEFORE printing prompt
        if config.colored_output {
            if let Some(color) = config.command_color {
                let _ = serial.write_str(color);
            }
        }
        
        // Print prompt (will be in command color unless we override it)
        if config.colored_output {
            if let Some(prompt_color) = config.prompt_character_color {
                // If prompt has its own color, use it just for the character
                let _ = serial.write_str(prompt_color);
                let mut char_buf = [0u8; 4];
                let char_str = config.prompt_character_char.encode_utf8(&mut char_buf);
                let _ = serial.write_str(char_str);
                let _ = serial.write_str("\x1b[0m");
                
                // Re-apply command color after prompt character reset
                if let Some(cmd_color) = config.command_color {
                    let _ = serial.write_str(cmd_color);
                }
                let _ = serial.write_str(" ");
            } else {
                // No separate prompt color, everything in command color
                let mut char_buf = [0u8; 4];
                let char_str = config.prompt_character_char.encode_utf8(&mut char_buf);
                let _ = serial.write_str(char_str);
                let _ = serial.write_str(" ");
            }
        } else {
            let mut char_buf = [0u8; 4];
            let char_str = config.prompt_character_char.encode_utf8(&mut char_buf);
            let _ = serial.write_str(char_str);
            let _ = serial.write_str(" ");
        }
        
        // Read line character by character
        let mut buffer_pos = 0;
        loop {
            serial.poll();
            
            if let Some(byte) = serial.read_byte() {
                // Echo character back
                if byte == b'\r' || byte == b'\n' {
                    // End of line
                    let _ = serial.write_bytes(b"\r\n");
                    break;
                } else if byte == 0x7F || byte == 0x08 { // Backspace or DEL
                    if buffer_pos > 0 {
                        buffer_pos -= 1;
                        let _ = serial.write_bytes(b"\x08 \x08"); // Backspace, space, backspace
                    }
                } else if byte >= 0x20 && byte <= 0x7E { // Printable characters
                    if buffer_pos < line_buffer.len() - 1 {
                        line_buffer[buffer_pos] = byte;
                        buffer_pos += 1;
                        let _ = serial.write_bytes(&[byte]);
                    }
                }
            }
        }

        // Process the line
        if buffer_pos > 0 {
            // Convert buffer to string
            if let Ok(line) = core::str::from_utf8(&line_buffer[..buffer_pos]) {
                let line = line.trim();
                
                // Parse command and arguments
                let mut parts = [""; 10];
                let mut part_count = 0;
                
                for word in line.split_whitespace() {
                    if part_count < parts.len() {
                        parts[part_count] = word;
                        part_count += 1;
                    }
                }
                
                if part_count > 0 {
                    let cmd_name = parts[0];
                    let args = &parts[1..part_count];

                    // Set answer color before command output
                    if config.colored_output {
                        if let Some(color) = config.answer_color {
                            let _ = serial.write_str(color);
                        }
                    }

                    // Get help for a specific command
                    if cmd_name == "help" {
                        if part_count == 1 {
                            let _ = registry.print_command_list(serial);
                        } else {
                            // Show help for specific command
                            let _ = registry.print_help(args[0], serial);
                        }
                    }
                    // Execute command
                    else if let Some(command) = registry.get_command_mut(cmd_name) {
                        if let Err(_e) = command.execute(args, serial, config) {
                            let _ = serial.write_str("Error executing command\r\n");
                        }
                    } else {
                        let _ = serial.write_str("Unknown command: ");
                        let _ = serial.write_str(cmd_name);
                        let _ = serial.write_str(". Type 'help' for available commands.\r\n");
                    }
                }
            }
        }
        
        // Reset color
        if config.colored_output {
            let _ = serial.write_str("\x1b[0m");
        }
    }
}