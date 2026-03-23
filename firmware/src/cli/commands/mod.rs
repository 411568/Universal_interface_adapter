/*
* The CommandRegistry stores all the cli commands in a HashMap (commands)
* To create a new command you use the register function

? Author: Krzysztof Sikora, 16.02.2026
*/

mod echo;
mod setup;
mod clear;
mod led;
mod mosfets;
mod relays;
mod isolated_inputs;
mod gpios; 

pub use echo::EchoCommand;
pub use setup::SetupCommand;
pub use clear::ClearCommand;
pub use led::LedCommand;
pub use mosfets::MosfetCommand;
pub use relays::RelayCommand;
pub use isolated_inputs::IsolatedInputCommand;
pub use gpios::GpioCommand;

use crate::cli::Command;
use crate::io_interface::serial::SerialIO;
use crate::cli::CliConfig;
use usb_device::UsbError;

/// Maximum number of commands that can be registered
const MAX_COMMANDS: usize = 10;

/// Enum of all possible command types
pub enum CommandType {
    Echo(EchoCommand),
    Setup(SetupCommand),
    Clear(ClearCommand),
    Led(LedCommand),
    Mosfets(MosfetCommand),
    Relays(RelayCommand),
    IsolatedInputs(IsolatedInputCommand),
    Gpios(GpioCommand),
}

macro_rules! impl_command_dispatch {
    ($self:expr, $method:ident, $($variant:ident),+) => {
        match $self {
            $(CommandType::$variant(cmd) => cmd.$method(),)+
        }
    };
}

macro_rules! impl_command_execute {
    ($self:expr, $args:expr, $output:expr, $config:expr, $($variant:ident),+) => {
        match $self {
            $(CommandType::$variant(cmd) => cmd.execute($args, $output, $config),)+
        }
    };
}

macro_rules! impl_command_print_help {
    ($self:expr, $output:expr, $($variant:ident),+) => {
        match $self {
            $(CommandType::$variant(cmd) => cmd.print_help($output),)+
        }
    };
}


impl CommandType {
    pub fn name(&self) -> &'static str {
        impl_command_dispatch!(self, name, Echo, Setup, Clear, Led, Mosfets, Relays, IsolatedInputs, Gpios)
    }

    // pub fn name(&self) -> &'static str {
    //     match self {
    //         CommandType::Echo(cmd) => cmd.name(),
    //         CommandType::Setup(cmd) => cmd.name(),
    //         CommandType::Clear(cmd) => cmd.name(),
    //         CommandType::Led(cmd) => cmd.name(),
    //         CommandType::Mosfets(cmd) => cmd.name(),
    //         CommandType::Relays(cmd) => cmd.name(),
    //         CommandType::IsolatedInputs(cmd) => cmd.name(),
    //         CommandType::Gpios(cmd) => cmd.name(),
    //     }
    // }

    pub fn initialize(&mut self) -> Result<(), &'static str> {
        impl_command_dispatch!(self, initialize, Echo, Setup, Clear, Led, Mosfets, Relays, IsolatedInputs, Gpios)
    }
    
    // pub fn initialize(&mut self) -> Result<(), &'static str> {
    //     match self {
    //         CommandType::Echo(cmd) => cmd.initialize(),
    //         CommandType::Setup(cmd) => cmd.initialize(),
    //         CommandType::Clear(cmd) => cmd.initialize(),
    //         CommandType::Led(cmd) => cmd.initialize(),
    //         CommandType::Mosfets(cmd) => cmd.initialize(),
    //         CommandType::Relays(cmd) => cmd.initialize(),
    //         CommandType::IsolatedInputs(cmd) => cmd.initialize(),
    //         CommandType::Gpios(cmd) => cmd.initialize(),
    //     }
    // }

    pub fn execute(&mut self, args: &[&str], output: &mut SerialIO, config: &mut CliConfig) -> Result<(), UsbError> {
        impl_command_execute!(self, args, output, config, Echo, Setup, Clear, Led, Mosfets, Relays, IsolatedInputs, Gpios)
    }

    // pub fn execute(&mut self, args: &[&str], output: &mut SerialIO, config: &mut CliConfig) -> Result<(), UsbError> {
    //     match self {
    //         CommandType::Echo(cmd) => cmd.execute(args, output, config),
    //         CommandType::Setup(cmd) => cmd.execute(args, output, config),
    //         CommandType::Clear(cmd) => cmd.execute(args, output, config),
    //         CommandType::Led(cmd) => cmd.execute(args, output, config),
    //         CommandType::Mosfets(cmd) => cmd.execute(args, output, config),
    //         CommandType::Relays(cmd) => cmd.execute(args, output, config),
    //         CommandType::IsolatedInputs(cmd) => cmd.execute(args, output, config),
    //         CommandType::Gpios(cmd) => cmd.execute(args, output, config),
    //     }
    // }

    pub fn print_help(&self, output: &mut SerialIO) -> Result<(), UsbError> {
        impl_command_print_help!(self, output, Echo, Setup, Clear, Led, Mosfets, Relays, IsolatedInputs, Gpios)
    }
    
    // pub fn print_help(&self, output: &mut SerialIO) -> Result<(), UsbError> {
    //     match self {
    //         CommandType::Echo(cmd) => cmd.print_help(output),
    //         CommandType::Setup(cmd) => cmd.print_help(output),
    //         CommandType::Clear(cmd) => cmd.print_help(output),
    //         CommandType::Led(cmd) => cmd.print_help(output),
    //         CommandType::Mosfets(cmd) => cmd.print_help(output),
    //         CommandType::Relays(cmd) => cmd.print_help(output),
    //         CommandType::IsolatedInputs(cmd) => cmd.print_help(output),
    //         CommandType::Gpios(cmd) => cmd.print_help(output),
    //     }
    // }
}

/// Registry to store and manage commands
pub struct CommandRegistry {
    commands: [Option<CommandType>; MAX_COMMANDS],
    count: usize,
}

impl CommandRegistry {
    pub fn new() -> Self {
        Self {
            // Use array initialization with const to avoid Copy trait requirement
            commands: [
                None, None, None, None, None,
                None, None, None, None, None,
            ],
            count: 0,
        }
    }
    
    pub fn register_echo(&mut self, command: EchoCommand) -> Result<(), &'static str> {
        self.register(CommandType::Echo(command))
    }

    pub fn register_led(&mut self, command: LedCommand) -> Result<(), &'static str> {
        self.register(CommandType::Led(command))
    }
    
    pub fn register_setup(&mut self, command: SetupCommand) -> Result<(), &'static str> {
        self.register(CommandType::Setup(command))
    }
    
    pub fn register_clear(&mut self, command: ClearCommand) -> Result<(), &'static str> {
        self.register(CommandType::Clear(command))
    }

    pub fn register_mosfets(&mut self, command: MosfetCommand) -> Result<(), &'static str> {
        self.register(CommandType::Mosfets(command))
    }

    pub fn register_relays(&mut self, command: RelayCommand) -> Result<(), &'static str> {
        self.register(CommandType::Relays(command))
    }

    pub fn register_isolated_inputs(&mut self, command: IsolatedInputCommand) -> Result<(), &'static str> {
        self.register(CommandType::IsolatedInputs(command))
    }

    pub fn register_gpios(&mut self, command: GpioCommand) -> Result<(), &'static str> {
        self.register(CommandType::Gpios(command))
    }
    
    fn register(&mut self, command: CommandType) -> Result<(), &'static str> {
        if self.count >= MAX_COMMANDS {
            return Err("Command registry full");
        }
        
        // Check for duplicate names
        let name = command.name();
        for i in 0..self.count {
            if let Some(cmd) = &self.commands[i] {
                if cmd.name() == name {
                    return Err("Command with this name already exists");
                }
            }
        }
        
        self.commands[self.count] = Some(command);
        self.count += 1;
        
        Ok(())
    }
    
    pub fn get_command_mut(&mut self, name: &str) -> Option<&mut CommandType> {
        // Use a simple loop with index to avoid multiple mutable borrows
        for i in 0..self.count {
            // First check the name without borrowing mutably
            let name_matches = if let Some(cmd) = &self.commands[i] {
                cmd.name() == name
            } else {
                false
            };
            
            if name_matches {
                // Now we can safely return a mutable reference
                return self.commands[i].as_mut();
            }
        }
        None
    }

    pub fn initialize_all_commands(&mut self) -> Result<(), &'static str> {
        for i in 0..self.count {
            if let Some(cmd) = &mut self.commands[i] {
                cmd.initialize()?;
            }
        }
        Ok(())
    }

    pub fn print_help(&self, name: &str, output: &mut SerialIO) -> Result<bool, UsbError> {
        for i in 0..self.count {
            if let Some(cmd) = &self.commands[i] {
                if cmd.name() == name {
                    cmd.print_help(output)?;
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }

    pub fn print_command_list(&self, output: &mut SerialIO) -> Result<(), UsbError> {
        // Collect names
        let mut names = [""; MAX_COMMANDS];
        let mut name_count = 0;
        
        for i in 0..self.count {
            if let Some(cmd) = &self.commands[i] {
                names[name_count] = cmd.name();
                name_count += 1;
            }
        }
        
        // Simple bubble sort
        for i in 0..name_count {
            for j in 0..name_count - i - 1 {
                if names[j] > names[j + 1] {
                    names.swap(j, j + 1);
                }
            }
        }
        
        // Print sorted names
        for i in 0..name_count {
            output.write_str("  ")?;
            output.write_str(names[i])?;
            output.write_str("\r\n")?;
        }
        
        Ok(())
    }
}