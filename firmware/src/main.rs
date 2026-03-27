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
use stm32f4xx_hal::i2c::I2c;
use stm32f4xx_hal::spi::{Spi, Mode, Phase, Polarity};
use stm32f4xx_hal::can::CanExt;
use stm32f4xx_hal::adc::{Adc, config::AdcConfig};
use stm32f4xx_hal::dac::{dac, DacPin};
use usb_device::prelude::*;
use usb_device::bus::UsbBusAllocator;

mod io_interface;
mod cli;

use io_interface::serial::SerialIO;
use cli::CommandRegistry;
use cli::CliConfig;
use cli::commands::{EchoCommand, ClearCommand, SetupCommand, LedCommand, MosfetCommand, RelayCommand, IsolatedInputCommand, GpioCommand, UartCommand, Rs422Command, Rs232Command, AnalogCommand, I2cCommand, SpiCommand, CanCommand};


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
    let gpiob = dp.GPIOB.split(&mut rcc);
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

    // Initialize UART2 (UART4) - Example pins PC10/PC11
    let uart2_tx = gpioc.pc10.into_alternate(); 
    let uart2_rx = gpioc.pc11.into_alternate(); 
    let uart2 = dp.UART4.serial(
        (uart2_tx, uart2_rx),
        serial::config::Config::default().baudrate(9600.bps()),
        &mut rcc,
    ).unwrap();
    UartCommand::init_uart2(uart2);

    // Initialize RS422 (USART1) with control pins
    let rs422_tx = gpioe.pe8.into_alternate();
    let rs422_rx = gpioe.pe7.into_alternate();
    let rs422_re = gpioe.pe9.into_push_pull_output(); // RE pin (active LOW)
    let rs422_de = gpiob.pb2.into_push_pull_output(); // DE pin (active HIGH)
    let rs422 = dp.UART5.serial(
        (rs422_tx, rs422_rx),
        serial::config::Config::default().baudrate(9600.bps()),
        &mut rcc,
    ).unwrap();
    Rs422Command::init(rs422, rs422_re, rs422_de);

    // Initialize RS232 (UART4)
    let rs232_tx = gpiod.pd8.into_alternate();
    let rs232_rx = gpioc.pc5.into_alternate();
    let rs232 = dp.USART3.serial(
        (rs232_tx, rs232_rx),
        serial::config::Config::default().baudrate(9600.bps()),
        &mut rcc,
    ).unwrap();
    Rs232Command::init(rs232);

    // Initialize ADC and DAC for analog operations
    let adc_config = AdcConfig::default();
    let adc = Adc::new(dp.ADC1, true, adc_config, &mut rcc);
    let adc_pin1 = gpioa.pa0.into_analog();
    let adc_pin2 = gpioa.pa1.into_analog();
    
    // Initialize DAC - PA4 is DAC channel 1
    let dac_pin = gpioa.pa4.into_analog();
    let mut dac_ch1 = dac(dp.DAC, dac_pin, &mut rcc);
    dac_ch1.enable();
    
    AnalogCommand::init(adc, adc_pin1, adc_pin2, dac_ch1);

    // Initialize I2C1 - PB6 (SCL), PB7 (SDA)
    let i2c1_scl = gpiob.pb6.into_alternate_open_drain();
    let i2c1_sda = gpiob.pb7.into_alternate_open_drain();
    let i2c1 = I2c::new(
        dp.I2C1,
        (i2c1_scl, i2c1_sda),
        100.kHz(),
        &mut rcc,
    );
    I2cCommand::init_i2c1(i2c1);

    // Initialize I2C2 - PB10 (SCL), PC12 (SDA)
    let i2c2_scl = gpiob.pb10.into_alternate_open_drain();
    let i2c2_sda = gpioc.pc12.into_alternate_open_drain();
    let i2c2 = I2c::new(
        dp.I2C2,
        (i2c2_scl, i2c2_sda),
        100.kHz(),
        &mut rcc,
    );
    I2cCommand::init_i2c2(i2c2);

    // Initialize SPI1 (labeled as SPI 2 in documentation) - PA5 (SCK), PA6 (MISO), PA7 (MOSI)
    let spi1_sck = gpioa.pa5.into_alternate();
    let spi1_miso = gpioa.pa6.into_alternate();
    let spi1_mosi = gpioa.pa7.into_alternate();
    let spi1 = Spi::new(
        dp.SPI1,
        (Some(spi1_sck), Some(spi1_miso), Some(spi1_mosi)),
        Mode {
            polarity: Polarity::IdleLow,
            phase: Phase::CaptureOnFirstTransition,
        },
        1.MHz(),
        &mut rcc,
    );
    SpiCommand::init_spi1(spi1);

    // Initialize SPI2 (labeled as SPI 1 in documentation) - PB13 (SCK), PB14 (MISO), PB15 (MOSI)
    let spi2_sck = gpiob.pb13.into_alternate();
    let spi2_miso = gpiob.pb14.into_alternate();
    let spi2_mosi = gpiob.pb15.into_alternate();
    let spi2 = Spi::new(
        dp.SPI2,
        (Some(spi2_sck), Some(spi2_miso), Some(spi2_mosi)),
        Mode {
            polarity: Polarity::IdleLow,
            phase: Phase::CaptureOnFirstTransition,
        },
        1.MHz(),
        &mut rcc,
    );
    SpiCommand::init_spi2(spi2);

    // Initialize CAN1 (PD0 = CAN_RX, PD1 = CAN_TX)
    let can_rx = gpiod.pd0.into_alternate();
    let can_tx = gpiod.pd1.into_alternate();
    
    let can1 = dp.CAN1.can((can_tx, can_rx), &mut rcc);
    CanCommand::init_can(can1);

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
    let _ = registry.register_rs422(Rs422Command::new());
    let _ = registry.register_rs232(Rs232Command::new());
    let _ = registry.register_analog(AnalogCommand::new());
    let _ = registry.register_i2c(I2cCommand::new());
    let _ = registry.register_spi(SpiCommand::new());
    let _ = registry.register_can(CanCommand::new());

    // Initialize all commands
    if let Err(e) = registry.initialize_all_commands() {
        let _ = serial_io.write_str("Initialization error: ");
        let _ = serial_io.write_str(e);
        let _ = serial_io.write_str("\r\n");
    }

    // Start CLI loop
    cli::run_cli(&mut serial_io, &mut registry, &mut config);
}