/*
* This is the main application configuration struct
* It holds all the settings that can be changed by the user

? Author: Krzysztof Sikora, 16.02.2026
*/


/// Output length enum
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnswerLength {
    Short,
    Long,
}

impl Default for AnswerLength {
    fn default() -> Self {
        Self::Long
    }
}

/// ANSI color codes as static strings
pub struct AnsiColor;

impl AnsiColor {
//    pub const GREEN: &'static str = "\x1b[32m";
//    pub const BLUE: &'static str = "\x1b[34m";
//    pub const WHITE: &'static str = "\x1b[37m";

    pub const FALLOUT_GREEN: &'static str = "\x1b[38;5;106m";  // Muted green (original)
    pub const FALLOUT_DARK_GREEN: &'static str = "\x1b[38;5;28m"; // Dark green (new)
    pub const FALLOUT_NEON_GREEN: &'static str = "\x1b[38;5;46m"; // Bright neon green (new)
//    pub const FALLOUT_BRIGHT_GREEN: &'static str = "\x1b[38;5;118m"; // Bright green (original)
    pub const FALLOUT_RED: &'static str = "\x1b[38;5;160m";    // Alert red
}

/// CLI Configuration
#[derive(Debug, Clone)]
pub struct CliConfig {
    pub answer_length: AnswerLength,
    pub colored_output: bool,
    pub prompt_character: bool,
    pub prompt_character_char: char,
    pub prompt_character_color: Option<&'static str>, // ANSI color code
    pub command_color: Option<&'static str>, // ANSI color code
    pub answer_color: Option<&'static str>, // ANSI color code
    pub error_color: Option<&'static str>, // ANSI color code
}

impl Default for CliConfig {
    fn default() -> Self {
        Self {
            answer_length: AnswerLength::Long,
            colored_output: true,
            prompt_character: true,
            prompt_character_char: '>',
            prompt_character_color: Some(AnsiColor::FALLOUT_GREEN),
            command_color: Some(AnsiColor::FALLOUT_NEON_GREEN),
            answer_color: Some(AnsiColor::FALLOUT_DARK_GREEN),
            error_color: Some(AnsiColor::FALLOUT_RED),
        }
    }
}

impl CliConfig {
    pub fn new() -> Self {
        Self::default()
    }
    
    /// Get the prompt character (returns the character, not a String)
    pub fn get_prompt_char(&self) -> Option<char> {
        if self.prompt_character {
            Some(self.prompt_character_char)
        } else {
            None
        }
    }
    
    /// Check if output should be short
    pub fn is_short_output(&self) -> bool {
        matches!(self.answer_length, AnswerLength::Short)
    }
    
    /// Set answer length
    pub fn set_answer_length(&mut self, length: AnswerLength) {
        self.answer_length = length;
    }

    /// Enable/disable colored output
    pub fn set_colored_output(&mut self, enabled: bool) {
        self.colored_output = enabled;
    }
    
    /// Enable/disable prompt character
    pub fn set_prompt_character_enabled(&mut self, enabled: bool) {
        self.prompt_character = enabled;
    }

    /// Set prompt character
    pub fn set_prompt_char(&mut self, c: char) {
        self.prompt_character_char = c;
    }
}