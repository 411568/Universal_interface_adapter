// TODO 
// Add the analog format command to switch between raw and millivolt output
// Add error handling for when the user types value out of range or invalid resistor values

use crate::cli::{CliConfig, Command};
use crate::io_interface::serial::SerialIO;
use usb_device::UsbError;
use stm32f4xx_hal as hal;
use hal::adc::{Adc, config::{SampleTime}};
use hal::dac::DacOut;
use hal::gpio::Analog;
use cortex_m::interrupt::Mutex;
use core::cell::RefCell;

type Adc1Type = Adc<hal::pac::ADC1>;
type AdcPin1 = hal::gpio::PA0<Analog>;
type AdcPin2 = hal::gpio::PA1<Analog>;
type DacCh1 = hal::dac::C1;

// Format: 0 = raw (0-4095), 1 = millivolts
static ANALOG_FORMAT: Mutex<RefCell<u8>> = Mutex::new(RefCell::new(0));

// ADC channel amplification (attenuation) settings - resistor to ground
#[derive(Clone, Copy)]
enum AdcAmp {
    R1_5K,  // 1.5k - divider ratio 11.5k/1.5k ≈ 7.67
    R3_3K,  // 3.3k - divider ratio 13.3k/3.3k ≈ 4.03
    R10K,   // 10k  - divider ratio 20k/10k = 2.0
}

// DAC amplification settings - feedback resistor
#[derive(Clone, Copy)]
enum DacAmp {
    R30K,   // 30k - gain 4x
    R15K,   // 15k - gain 2.5x
    R0,     // 0 (buffer) - gain 1x
}


fn format_u32(mut n: u32, buf: &mut [u8; 10]) -> Option<&str> {
    if n == 0 {
        buf[0] = b'0';
        return core::str::from_utf8(&buf[..1]).ok();
    }
    
    let mut pos = 10;
    while n > 0 {
        pos -= 1;
        buf[pos] = b'0' + (n % 10) as u8;
        n /= 10;
    }
    
    core::str::from_utf8(&buf[pos..]).ok()
}



static ADC1_AMP: Mutex<RefCell<AdcAmp>> = Mutex::new(RefCell::new(AdcAmp::R10K));
static ADC2_AMP: Mutex<RefCell<AdcAmp>> = Mutex::new(RefCell::new(AdcAmp::R10K));
static DAC_AMP: Mutex<RefCell<DacAmp>> = Mutex::new(RefCell::new(DacAmp::R0));

static ADC1: Mutex<RefCell<Option<Adc1Type>>> = Mutex::new(RefCell::new(None));
static ADC_PIN1: Mutex<RefCell<Option<AdcPin1>>> = Mutex::new(RefCell::new(None));
static ADC_PIN2: Mutex<RefCell<Option<AdcPin2>>> = Mutex::new(RefCell::new(None));
static DAC_CH1: Mutex<RefCell<Option<DacCh1>>> = Mutex::new(RefCell::new(None));

#[derive(Clone)]
pub struct AnalogCommand;

impl AnalogCommand {
    pub fn new() -> Self {
        Self
    }

    pub fn init(adc: Adc1Type, adc_pin1: AdcPin1, adc_pin2: AdcPin2, dac_ch1: DacCh1) {
        cortex_m::interrupt::free(|cs| {
            *ADC1.borrow(cs).borrow_mut() = Some(adc);
            *ADC_PIN1.borrow(cs).borrow_mut() = Some(adc_pin1);
            *ADC_PIN2.borrow(cs).borrow_mut() = Some(adc_pin2);
            *DAC_CH1.borrow(cs).borrow_mut() = Some(dac_ch1);
        });
    }

    // Convert ADC raw value to millivolts considering attenuation
    fn adc_to_mv(&self, raw: u16, amp: AdcAmp) -> u32 {
        // Vref = 3300mV, 12-bit ADC
        // Vadc_mv = (raw * 3300) / 4095
        let adc_mv = (raw as u32 * 3300) / 4095;
        
        // Apply attenuation multiplier
        // Vin = Vadc * (R_top + R_bottom) / R_bottom
        // R_top = 10k always
        match amp {
            AdcAmp::R1_5K => (adc_mv * 115) / 15,  // 11.5k / 1.5k = 7.67
            AdcAmp::R3_3K => (adc_mv * 133) / 33,  // 13.3k / 3.3k = 4.03
            AdcAmp::R10K => adc_mv * 2,          // 20k / 10k = 2.0
        }
    }

    // Convert millivolts to DAC raw value considering amplification
    fn mv_to_dac(&self, mv: u32, amp: DacAmp) -> u16 {
        // Apply amplification - input voltage to DAC needs to be divided by gain
        let dac_mv = match amp {
            DacAmp::R30K => (mv * 10) / 40,  // gain 4x, so divide by 4
            DacAmp::R15K => (mv * 10) / 25,  // gain 2.5x, so divide by 2.5
            DacAmp::R0 => mv,                // gain 1x (buffer)
        };
        
        // Convert mV to DAC value: raw = (dac_mv * 4095) / 3300
        let raw = (dac_mv * 4095) / 3300;
        if raw > 4095 {
            4095
        } else {
            raw as u16
        }
    }

    fn parse_int(&self, arg: &str) -> Result<u16, &'static str> {
        arg.parse::<u16>().map_err(|_| "Invalid integer value")
    }

    fn parse_channel(&self, arg: &str) -> Result<u8, &'static str> {
        match arg {
            "1" => Ok(1),
            "2" => Ok(2),
            _ => Err("Channel must be 1 or 2"),
        }
    }

    fn read_adc(&self, channel: u8, out: &mut SerialIO, cfg: &CliConfig) -> Result<(), UsbError> {
        let value = cortex_m::interrupt::free(|cs| {
            let mut adc_opt = ADC1.borrow(cs).borrow_mut();
            
            if let Some(adc) = adc_opt.as_mut() {
                match channel {
                    1 => {
                        let mut pin_opt = ADC_PIN1.borrow(cs).borrow_mut();
                        if let Some(pin) = pin_opt.as_mut() {
                            Some(adc.convert(pin, SampleTime::Cycles_480))
                        } else {
                            None
                        }
                    }
                    2 => {
                        let mut pin_opt = ADC_PIN2.borrow(cs).borrow_mut();
                        if let Some(pin) = pin_opt.as_mut() {
                            Some(adc.convert(pin, SampleTime::Cycles_480))
                        } else {
                            None
                        }
                    }
                    _ => None,
                }
            } else {
                None
            }
        });

        if let Some(raw_val) = value {
            let format = cortex_m::interrupt::free(|cs| *ANALOG_FORMAT.borrow(cs).borrow());
            
            if format == 0 {
                // Raw format
                if cfg.is_short_output() {
                    let mut buf = [0u8; 6];
                    if let Some(s) = format_u16(raw_val, &mut buf) {
                        out.write_str(s)?;
                        out.write_str("\r\n")?;
                    }
                } else {
                    out.write_str("ADC")?;
                    out.write_str(if channel == 1 { "1" } else { "2" })?;
                    out.write_str(" value: ")?;
                    let mut buf = [0u8; 6];
                    if let Some(s) = format_u16(raw_val, &mut buf) {
                        out.write_str(s)?;
                    }
                    out.write_str("\r\n")?;
                }
            } else {
                // Millivolt format
                let amp = cortex_m::interrupt::free(|cs| {
                    if channel == 1 {
                        *ADC1_AMP.borrow(cs).borrow()
                    } else {
                        *ADC2_AMP.borrow(cs).borrow()
                    }
                });
                
                let mv = self.adc_to_mv(raw_val, amp);
                
                if cfg.is_short_output() {
                    let mut buf = [0u8; 10];
                    if let Some(s) = format_u32(mv, &mut buf) {
                        out.write_str(s)?;
                        out.write_str(" mV\r\n")?;
                    }
                } else {
                    out.write_str("ADC")?;
                    out.write_str(if channel == 1 { "1" } else { "2" })?;
                    out.write_str(": ")?;
                    let mut buf = [0u8; 10];
                    if let Some(s) = format_u32(mv, &mut buf) {
                        out.write_str(s)?;
                    }
                    out.write_str(" mV\r\n")?;
                }
            }
        } else {
            out.write_str("Error: Failed to read ADC\r\n")?;
        }
        
        Ok(())
    }

    fn continuous_read(&self, channel: u8, out: &mut SerialIO) -> Result<(), UsbError> {
        out.write_str("Continuous ADC")?;
        out.write_str(if channel == 1 { "1" } else { "2" })?;
        out.write_str(" reading. Press ESC to exit\r\n")?;

        loop {
            out.poll();

            // Check for ESC key
            while let Some(b) = out.read_byte() {
                if b == 0x1B { // ESC key
                    out.write_str("\r\nExiting continuous mode\r\n")?;
                    return Ok(());
                }
            }

            // Read ADC value
            let value = cortex_m::interrupt::free(|cs| {
                let mut adc_opt = ADC1.borrow(cs).borrow_mut();
                
                if let Some(adc) = adc_opt.as_mut() {
                    match channel {
                        1 => {
                            let mut pin_opt = ADC_PIN1.borrow(cs).borrow_mut();
                            if let Some(pin) = pin_opt.as_mut() {
                                Some(adc.convert(pin, SampleTime::Cycles_480))
                            } else {
                                None
                            }
                        }
                        2 => {
                            let mut pin_opt = ADC_PIN2.borrow(cs).borrow_mut();
                            if let Some(pin) = pin_opt.as_mut() {
                                Some(adc.convert(pin, SampleTime::Cycles_480))
                            } else {
                                None
                            }
                        }
                        _ => None,
                    }
                } else {
                    None
                }
            });

            if let Some(raw_val) = value {
                let format = cortex_m::interrupt::free(|cs| *ANALOG_FORMAT.borrow(cs).borrow());
                
                if format == 0 {
                    // Raw format
                    let mut buf = [0u8; 6];
                    if let Some(s) = format_u16(raw_val, &mut buf) {
                        out.write_str(s)?;
                        out.write_str("\r\n")?;
                    }
                } else {
                    // Millivolt format
                    let amp = cortex_m::interrupt::free(|cs| {
                        if channel == 1 {
                            *ADC1_AMP.borrow(cs).borrow()
                        } else {
                            *ADC2_AMP.borrow(cs).borrow()
                        }
                    });
                    
                    let mv = self.adc_to_mv(raw_val, amp);
                    let mut buf = [0u8; 10];
                    if let Some(s) = format_u32(mv, &mut buf) {
                        out.write_str(s)?;
                        out.write_str(" mV\r\n")?;
                    }
                }
            }

            // Small delay between readings
            for _ in 0..100_000 {
                cortex_m::asm::nop();
            }
        }
    }

    fn set_adc_amp(&self, channel: u8, resistor: &str, out: &mut SerialIO, cfg: &CliConfig) -> Result<(), UsbError> {
        // Parse resistor value
        let amp = match resistor {
            "1.5k" | "1500" => AdcAmp::R1_5K,
            "3.3k" | "3300" => AdcAmp::R3_3K,
            "10k" | "10000" => AdcAmp::R10K,
            _ => {
                out.write_str("Error: Invalid resistor value. Use 1.5k, 3.3k, or 10k\r\n")?;
                return Ok(());
            }
        };

        // Store amplification setting
        cortex_m::interrupt::free(|cs| {
            if channel == 1 {
                *ADC1_AMP.borrow(cs).borrow_mut() = amp;
            } else {
                *ADC2_AMP.borrow(cs).borrow_mut() = amp;
            }
        });

        if cfg.is_short_output() {
            out.write_str("OK\r\n")?;
        } else {
            out.write_str("ADC")?;
            out.write_str(if channel == 1 { "1" } else { "2" })?;
            out.write_str(" amplification set to ")?;
            out.write_str(resistor)?;
            out.write_str("\r\n")?;
        }
        Ok(())
    }

    fn set_dac_value(&self, value: u16, out: &mut SerialIO, cfg: &CliConfig) -> Result<(), UsbError> {
        if value > 4095 {
            out.write_str("Error: Value must be 0-4095 (12-bit)\r\n")?;
            return Ok(());
        }

        cortex_m::interrupt::free(|cs| {
            if let Some(dac) = DAC_CH1.borrow(cs).borrow_mut().as_mut() {
                dac.set_value(value);
            }
        });

        if cfg.is_short_output() {
            out.write_str("OK\r\n")?;
        } else {
            out.write_str("DAC value set to ")?;
            let mut buf = [0u8; 6];
            if let Some(s) = format_u16(value, &mut buf) {
                out.write_str(s)?;
            }
            out.write_str("\r\n")?;
        }
        Ok(())
    }

    fn set_dac_amp(&self, resistor: &str, out: &mut SerialIO, cfg: &CliConfig) -> Result<(), UsbError> {
        // Parse resistor value
        let _resistor_val = match resistor {
            "30k" | "30000" => 30000,
            "15k" | "15000" => 15000,
            "0" => 0,
            _ => {
                out.write_str("Error: Invalid resistor value. Use 30k, 15k, or 0\r\n")?;
                return Ok(());
            }
        };

        // TODO: Set amplification via GPIO or similar hardware control
        // This depends on your hardware design (analog switches, etc.)

        if cfg.is_short_output() {
            out.write_str("OK\r\n")?;
        } else {
            out.write_str("DAC amplification set to ")?;
            out.write_str(resistor)?;
            out.write_str("\r\n")?;
        }
        Ok(())
    }

    fn set_format(&self, format_str: &str, out: &mut SerialIO, cfg: &CliConfig) -> Result<(), UsbError> {
        let format_val = match format_str {
            "raw" | "0" => 0,
            "mv" | "millivolt" | "millivolts" | "1" => 1,
            _ => {
                out.write_str("Error: Invalid format. Use 'raw' or 'mv'\r\n")?;
                return Ok(());
            }
        };

        cortex_m::interrupt::free(|cs| {
            *ANALOG_FORMAT.borrow(cs).borrow_mut() = format_val;
        });

        if cfg.is_short_output() {
            out.write_str("OK\r\n")?;
        } else {
            out.write_str("Analog format set to ")?;
            out.write_str(if format_val == 0 { "raw" } else { "millivolts" })?;
            out.write_str("\r\n")?;
        }
        Ok(())
    }
}

impl Command for AnalogCommand {
    fn name(&self) -> &'static str {
        "analog"
    }

    fn initialize(&mut self) -> Result<(), &'static str> {
        Ok(())
    }

    fn execute(
        &mut self,
        args: &[&str],
        out: &mut SerialIO,
        cfg: &mut CliConfig,
    ) -> Result<(), UsbError> {
        if args.is_empty() {
            out.write_str("Usage: analog <in|out> [args]\r\n")?;
            out.write_str("Type 'help analog' for more information\r\n")?;
            return Ok(());
        }

        match args[0] {
            "in" => {
                if args.len() < 2 {
                    out.write_str("Usage: analog in <channel|set_amp|continuous> [args]\r\n")?;
                    return Ok(());
                }

                match args[1] {
                    "set_amp" => {
                        if args.len() != 4 {
                            out.write_str("Usage: analog in set_amp <1|2> <1.5k|3.3k|10k>\r\n")?;
                            return Ok(());
                        }
                        let channel = match self.parse_channel(args[2]) {
                            Ok(c) => c,
                            Err(e) => {
                                out.write_str(e)?;
                                out.write_str("\r\n")?;
                                return Ok(());
                            }
                        };
                        self.set_adc_amp(channel, args[3], out, cfg)
                    }
                    "continuous" => {
                        if args.len() != 3 {
                            out.write_str("Usage: analog in continuous <1|2>\r\n")?;
                            return Ok(());
                        }
                        let channel = match self.parse_channel(args[2]) {
                            Ok(c) => c,
                            Err(e) => {
                                out.write_str(e)?;
                                out.write_str("\r\n")?;
                                return Ok(());
                            }
                        };
                        self.continuous_read(channel, out)
                    }
                    _ => {
                        // Try to parse as channel number
                        let channel = match self.parse_channel(args[1]) {
                            Ok(c) => c,
                            Err(_) => {
                                out.write_str("Unknown subcommand. Use: <1|2>, set_amp, or continuous\r\n")?;
                                return Ok(());
                            }
                        };
                        self.read_adc(channel, out, cfg)
                    }
                }
            }
            "out" => {
                if args.len() < 2 {
                    out.write_str("Usage: analog out <value|set_amp> [args]\r\n")?;
                    return Ok(());
                }

                match args[1] {
                    "value" => {
                        if args.len() != 3 {
                            out.write_str("Usage: analog out value <0-4095>\r\n")?;
                            return Ok(());
                        }
                        let value = match self.parse_int(args[2]) {
                            Ok(v) => v,
                            Err(e) => {
                                out.write_str(e)?;
                                out.write_str("\r\n")?;
                                return Ok(());
                            }
                        };
                        self.set_dac_value(value, out, cfg)
                    }
                    "set_amp" => {
                        if args.len() != 3 {
                            out.write_str("Usage: analog out set_amp <30k|15k|0>\r\n")?;
                            return Ok(());
                        }
                        self.set_dac_amp(args[2], out, cfg)
                    }
                    _ => {
                        out.write_str("Unknown subcommand. Use: value or set_amp\r\n")?;
                        Ok(())
                    }
                }
            }
            _ => {
                out.write_str("Unknown subcommand. Use: in or out\r\n")?;
                Ok(())
            }
        }
    }

    fn print_help(&self, out: &mut SerialIO) -> Result<(), UsbError> {
        out.write_str("analog <in|out> [args] - Analog ADC/DAC operations\r\n")?;
        out.write_str("ADC inputs: PA0 (ch1), PA1 (ch2) | DAC output: PA4\r\n")?;
        out.write_str("\r\nADC Input Commands:\r\n")?;
        out.write_str("  analog in <1|2>              - Read ADC channel (0-4095)\r\n")?;
        out.write_str("  analog in set_amp <1|2> <R>  - Set amplification (1.5k, 3.3k, 10k)\r\n")?;
        out.write_str("  analog in continuous <1|2>   - Continuous reading (ESC to exit)\r\n")?;
        out.write_str("\r\nDAC Output Commands:\r\n")?;
        out.write_str("  analog out value <0-4095>    - Set DAC output value (12-bit)\r\n")?;
        out.write_str("  analog out set_amp <R>       - Set amplification (30k, 15k, 0)\r\n")?;
        out.write_str("\r\nExamples:\r\n")?;
        out.write_str("  analog in 1                  - Read channel 1\r\n")?;
        out.write_str("  analog in set_amp 1 3.3k     - Set ch1 to 3.3k gain\r\n")?;
        out.write_str("  analog in continuous 2       - Continuous read ch2\r\n")?;
        out.write_str("  analog out value 2048        - Set DAC to mid-scale\r\n")?;
        out.write_str("  analog out set_amp 15k       - Set DAC amp to 15k\r\n")?;
        Ok(())
    }
}

fn format_u16(mut n: u16, buf: &mut [u8; 6]) -> Option<&str> {
    if n == 0 {
        buf[0] = b'0';
        return core::str::from_utf8(&buf[..1]).ok();
    }
    
    let mut pos = 6;
    while n > 0 {
        pos -= 1;
        buf[pos] = b'0' + (n % 10) as u8;
        n /= 10;
    }
    
    core::str::from_utf8(&buf[pos..]).ok()
}
