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
use stm32f4xx_hal::serial;
use usb_device::prelude::*;
use usb_device::bus::UsbBusAllocator;

mod io_interface;
mod cli;

use io_interface::serial::SerialIO;
use cli::CommandRegistry;
use cli::CliConfig;
use cli::commands::{EchoCommand, ClearCommand, SetupCommand, LedCommand, MosfetCommand, RelayCommand, IsolatedInputCommand, GpioCommand, UartCommand};


// Statically allocate memory for USB endpoint buffers
static EP_MEMORY: ConstStaticCell<[u32; 1024]> = ConstStaticCell::new([0; 1024]);
// Statically allocate memory for the USB bus allocator
static USB_BUS: StaticCell<Option<UsbBusAllocator<UsbBus<USB>>>> = StaticCell::new();


fn initialize_peripherals() -> SerialIO {
    let dp = pac::Peripherals::take().unwrap();
    // let cp = cortex_m::peripheral::Peripherals::take().unwrap();

    // Set the main clock
    let mut rcc = dp
        .RCC
        .freeze(Config::hse(8.MHz()).sysclk(48.MHz()).require_pll48clk());

    // Split the GPIO
    let gpioa = dp.GPIOA.split(&mut rcc);
    let gpiod = dp.GPIOD.split(&mut rcc);
    let gpioc = dp.GPIOC.split(&mut rcc);
    let gpioe = dp.GPIOE.split(&mut rcc);

    // Initialize LED pins
    LedCommand::init_pins(
        gpiod.pd4,
        gpiod.pd5,
        gpiod.pd6,
        gpiod.pd7,
    );

    // Initialize the MOSFET pins
    MosfetCommand::init_pins(
        gpioc.pc6,
        gpioc.pc7,
        gpioc.pc8,
        gpioc.pc9,
    );

    // Initialize the Relay pins
    RelayCommand::init_pins(
        gpioe.pe0,
        gpioe.pe1,
    );

    // Initialize the Isolated Input pins
    IsolatedInputCommand::init_pins(
        gpioe.pe2,
        gpioe.pe3,
        gpioe.pe4,
    );

    // Initialize the GPIO pins
    GpioCommand::init_pins(
        gpiod.pd12,
        gpiod.pd13,
        gpiod.pd14,
        gpiod.pd15,
    );

     // Initialize UART1 (USART2) - Example pins PA2/PA3
    let uart1_tx = gpioa.pa2.into_alternate();
    let uart1_rx = gpioa.pa3.into_alternate();
    let uart1 = dp.USART2.serial(
        (uart1_tx, uart1_rx),
        serial::config::Config::default().baudrate(9600.bps()),
        &mut rcc,
    ).unwrap();
    UartCommand::init_uart1(uart1);

    // Initialize UART2 (USART3) - Example pins PB10/PB11 or PC10/PC11
    let uart2_tx = gpioc.pc10.into_alternate(); 
    let uart2_rx = gpioc.pc11.into_alternate(); 
    let uart2 = dp.USART3.serial(
        (uart2_tx, uart2_rx),
        serial::config::Config::default().baudrate(9600.bps()),
        &mut rcc,
    ).unwrap();
    UartCommand::init_uart2(uart2);

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
    let mut serial_io = initialize_peripherals();
    
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
    let _ = registry.register_mosfets(MosfetCommand::new());
    let _ = registry.register_relays(RelayCommand::new());
    let _ = registry.register_isolated_inputs(IsolatedInputCommand::new());
    let _ = registry.register_gpios(GpioCommand::new());
    let _ = registry.register_uart(UartCommand::new());

    // Initialize all commands
    if let Err(e) = registry.initialize_all_commands() {
        let _ = serial_io.write_str("Initialization error: ");
        let _ = serial_io.write_str(e);
        let _ = serial_io.write_str("\r\n");
    }

    // Start CLI loop
    cli::run_cli(&mut serial_io, &mut registry, &mut config);
}