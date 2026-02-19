//! CDC-ACM serial port example with CLI interface
//! Target board: any STM32F4 with a OTG FS peripheral and a 8MHz HSE crystal
#![no_std]
#![no_main]

use panic_halt as _;

use cortex_m_rt::entry;
use static_cell::ConstStaticCell;
use static_cell::StaticCell;
use stm32f4xx_hal::otg_fs::{UsbBus, USB};
use stm32f4xx_hal::rcc::Config;
use stm32f4xx_hal::{pac, prelude::*};
use usb_device::prelude::*;
use usb_device::bus::UsbBusAllocator;

mod io_interface;
mod cli;

use io_interface::serial::SerialIO;
use cli::CommandRegistry;
use cli::CliConfig;
use cli::commands::{EchoCommand, ClearCommand, SetupCommand};

// Statically allocate memory for USB endpoint buffers
static EP_MEMORY: ConstStaticCell<[u32; 1024]> = ConstStaticCell::new([0; 1024]);
// Statically allocate memory for the USB bus allocator
static USB_BUS: StaticCell<Option<UsbBusAllocator<UsbBus<USB>>>> = StaticCell::new();

#[entry]
fn main() -> ! {
    let dp = pac::Peripherals::take().unwrap();
    let cp = cortex_m::peripheral::Peripherals::take().unwrap();


    // Set the main clock (keep your working USB initialization)
    let mut rcc = dp
        .RCC
        .freeze(Config::hse(8.MHz()).sysclk(48.MHz()).require_pll48clk());

    // Create the delay function
    let mut delay = cp.SYST.delay(&rcc.clocks);

    let gpioa = dp.GPIOA.split(&mut rcc);

    let usb = USB::new(
        (dp.OTG_FS_GLOBAL, dp.OTG_FS_DEVICE, dp.OTG_FS_PWRCLK),
        (gpioa.pa11, gpioa.pa12),
        &rcc.clocks,
    );

    // Store the USB bus allocator in a static variable to give it 'static lifetime
    let usb_bus = USB_BUS.init(Some(UsbBus::new(usb, EP_MEMORY.take())));
    let usb_bus = usb_bus.as_ref().unwrap();

    let serial = usbd_serial::SerialPort::new(usb_bus);

    let usb_dev = UsbDeviceBuilder::new(usb_bus, UsbVidPid(0x16c0, 0x27dd))
        .device_class(usbd_serial::USB_CLASS_CDC)
        .strings(&[StringDescriptors::default()
            .manufacturer("Boards & Bits")
            .product("Universal Interface Adapter")
            .serial_number("v1.0")])
        .unwrap()
        .build();

    // Create our SerialIO wrapper
    let mut serial_io = SerialIO::new(serial, usb_dev);

    // Small delay for USB enumeration
    for _ in 0..100_000 {
        cortex_m::asm::nop();
    }

    // Create command registry
    let mut registry = CommandRegistry::new();
    let mut config = CliConfig::new(); 
    
    // Create and register commands
    let echo_cmd = EchoCommand::new();
    let _ = registry.register_echo(echo_cmd);

    let setup_cmd = SetupCommand::new();
    let _ = registry.register_setup(setup_cmd);

    let clear_cmd = ClearCommand::new();
    let _ = registry.register_clear(clear_cmd);

    // Initialize all commands
    if let Err(e) = registry.initialize_all_commands() {
        let _ = serial_io.write_str("Initialization error: ");
        let _ = serial_io.write_str(e);
        let _ = serial_io.write_str("\r\n");
    }

    // Start CLI loop
    run_cli(&mut serial_io, &mut registry, &mut config);
}



// This is the actual cli loop that reads the input etc.
fn run_cli(serial: &mut SerialIO, registry: &mut CommandRegistry, config: &mut CliConfig) -> ! {
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