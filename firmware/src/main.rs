//! CDC-ACM serial port example with CLI interface
//! Target board: any STM32F4 with a OTG FS peripheral and a 8MHz HSE crystal
#![no_std]
#![no_main]

// #![allow(unused)]  // Allows all unused code in this file
// #![allow(unused_variables)]  // Only allows unused variables
// #![allow(unused_imports)]    // Only allows unused imports
#![allow(dead_code)]         // Only allows unused functions/struct

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
use cli::commands::{EchoCommand, ClearCommand, SetupCommand, LedCommand};


// Statically allocate memory for USB endpoint buffers
static EP_MEMORY: ConstStaticCell<[u32; 1024]> = ConstStaticCell::new([0; 1024]);
// Statically allocate memory for the USB bus allocator
static USB_BUS: StaticCell<Option<UsbBusAllocator<UsbBus<USB>>>> = StaticCell::new();


fn initialize_serial() -> SerialIO {
    let dp = pac::Peripherals::take().unwrap();
    // let cp = cortex_m::peripheral::Peripherals::take().unwrap();

    // Set the main clock
    let mut rcc = dp
        .RCC
        .freeze(Config::hse(8.MHz()).sysclk(48.MHz()).require_pll48clk());

    // Split the GPIO
    let gpioa = dp.GPIOA.split(&mut rcc);
    let gpiod = dp.GPIOD.split(&mut rcc);

    // Initialize LED pins
    LedCommand::init_pins(
        gpiod.pd4,
        gpiod.pd5,
        gpiod.pd6,
        gpiod.pd7,
    );

    // Create the USB device
    let usb = USB::new(
        (dp.OTG_FS_GLOBAL, dp.OTG_FS_DEVICE, dp.OTG_FS_PWRCLK),
        (gpioa.pa11, gpioa.pa12),
        &rcc.clocks,
    );

    // Store the USB bus allocator
    let usb_bus = USB_BUS.init(Some(UsbBus::new(usb, EP_MEMORY.take())));
    let usb_bus = usb_bus.as_ref().unwrap();

    let serial = usbd_serial::SerialPort::new(usb_bus);

    let usb_dev = UsbDeviceBuilder::new(usb_bus, UsbVidPid(0x16c0, 0x27dd))
        .device_class(usbd_serial::USB_CLASS_CDC)
        .strings(&[StringDescriptors::default()
            .manufacturer("Boards & Bits")
            .product("Universal Interface Adapter v1.0")
            .serial_number("v1.0")])
        .unwrap()
        .build();

    SerialIO::new(serial, usb_dev)
}



#[entry]
fn main() -> ! {
    // Initialize serial interface (also sets up LEDs)
    let mut serial_io = initialize_serial();
    
    // Small delay for USB enumeration
    for _ in 0..100_000 {
        cortex_m::asm::nop();
    }

    // Create command registry
    let mut registry = CommandRegistry::new();
    let mut config = CliConfig::new(); 
    
    // Create and register commands
    let _ = registry.register_echo(EchoCommand::new());
    let _ = registry.register_setup(SetupCommand::new());
    let _ = registry.register_clear(ClearCommand::new());
    let _ = registry.register_led(LedCommand::new());

    // Initialize all commands
    if let Err(e) = registry.initialize_all_commands() {
        let _ = serial_io.write_str("Initialization error: ");
        let _ = serial_io.write_str(e);
        let _ = serial_io.write_str("\r\n");
    }

    // Start CLI loop
    cli::run_cli(&mut serial_io, &mut registry, &mut config);
}