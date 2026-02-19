/*
* The CommandRegistry stores all the cli commands in a HashMap (commands)
* To create a new command you use the register function

? Author: Krzysztof Sikora, 16.02.2026
*/

mod echo;
mod setup;
mod clear;

pub use echo::EchoCommand;
pub use setup::SetupCommand;
pub use clear::ClearCommand;

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
}

impl CommandType {
    pub fn name(&self) -> &'static str {
        match self {
            CommandType::Echo(cmd) => cmd.name(),
            CommandType::Setup(cmd) => cmd.name(),
            CommandType::Clear(cmd) => cmd.name(),
        }
    }
    
    pub fn initialize(&mut self) -> Result<(), &'static str> {
        match self {
            CommandType::Echo(cmd) => cmd.initialize(),
            CommandType::Setup(cmd) => cmd.initialize(),
            CommandType::Clear(cmd) => cmd.initialize(),
        }
    }
    
    pub fn execute(&mut self, args: &[&str], output: &mut SerialIO, config: &mut CliConfig) -> Result<(), UsbError> {
        match self {
            CommandType::Echo(cmd) => cmd.execute(args, output, config),
            CommandType::Setup(cmd) => cmd.execute(args, output, config),
            CommandType::Clear(cmd) => cmd.execute(args, output, config),
        }
    }
    
    pub fn print_help(&self, output: &mut SerialIO) -> Result<(), UsbError> {
        match self {
            CommandType::Echo(cmd) => cmd.print_help(output),
            CommandType::Setup(cmd) => cmd.print_help(output),
            CommandType::Clear(cmd) => cmd.print_help(output),
        }
    }
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
    
    pub fn register_setup(&mut self, command: SetupCommand) -> Result<(), &'static str> {
        self.register(CommandType::Setup(command))
    }
    
    pub fn register_clear(&mut self, command: ClearCommand) -> Result<(), &'static str> {
        self.register(CommandType::Clear(command))
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