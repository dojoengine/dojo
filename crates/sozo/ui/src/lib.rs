//! Manages the UI for the Sozo CLI.

use colored::{Color, ColoredString, Colorize};

/// The length of the indent for each section level.
const SECTION_INDENT_LENGTH: usize = 3;

// The colors of the section levels.
const SOZO_SECTION_COLORS: [Color; 2] = [Color::Blue, Color::BrightBlue];

/// The prefixes for titles according to the section level.
const SOZO_TITLE_PREFIX: [&str; 2] = ["> ", "- "];

/// The color for warnings.
const WARNING_COLOR: Color = Color::BrightYellow;

/// The color for errors.
const ERROR_COLOR: Color = Color::BrightRed;

/// The color for results.
const RESULT_COLOR: Color = Color::BrightGreen;

#[derive(Debug, Clone, Default)]
pub enum SozoVerbosity {
    Quiet,
    #[default]
    Normal,
    Verbose,
    Debug,
    Trace,
}

// trait to handle message output.
// Allow us to accept both &str and String.
pub trait Message {
    fn text(self) -> String;
}

impl Message for String {
    fn text(self) -> String {
        self
    }
}

impl Message for &str {
    fn text(self) -> String {
        self.to_string()
    }
}

#[derive(Debug)]
pub struct SozoUi {
    section_level: usize,
    verbosity: SozoVerbosity,
}

impl Default for SozoUi {
    fn default() -> Self {
        Self::new(SozoVerbosity::default())
    }
}

impl SozoUi {
    /// Returns a new instance of SozoUi.
    pub fn new(verbosity: SozoVerbosity) -> Self {
        SozoUi { section_level: 0, verbosity }
    }

    /// Returns a new SozoUi instance to handle a subsection of the current one.
    pub fn subsection(&self) -> Self {
        SozoUi { section_level: self.section_level + 1, verbosity: self.verbosity.clone() }
    }

    /// Prints a new line.
    pub fn new_line(&self) {
        println!();
    }

    /// Prints a block of text surrounded by newlines.
    pub fn print_block<T: Message>(&self, block: T) {
        self.print_colored_block(block.text().into());
    }

    /// Prints a block of text surrounded by newlines. Could provide a specific color for the text.
    pub fn print_colored_block(&self, block: ColoredString) {
        println!();
        self.print_with_indentation(block);
        println!();
    }

    /// Prints a block of text surrounded by newlines with a specific warning color.
    pub fn print_warning_block<T: Message>(&self, block: T) {
        self.print_colored_block(block.text().color(WARNING_COLOR));
    }

    /// Prints a title for the current section. Use the color set for the section.
    pub fn print_title<T: Message>(&self, title: T) {
        println!();
        self.print_with_section_color(format!("{}{}", self.get_title_prefix(), title.text()));
    }

    /// Prints a text with the indentation for the current section.
    pub fn print<T: Message>(&self, text: T) {
        self.print_with_indentation(text.text().into());
    }

    /// Prints a warning without taking the indentation into account.
    pub fn warn<T: Message>(&self, text: T) {
        println!("{}", text.text().color(WARNING_COLOR));
    }

    /// Prints an error without taking the indentation into account.
    pub fn error<T: Message>(&self, text: T) {
        println!("error: {}", text.text().trim().color(ERROR_COLOR));
    }

    /// Prints a text without any indentation or color.
    pub fn print_raw<T: Message>(&self, text: T) {
        println!("{}", text.text());
    }

    /// Prints a text with the indentation for the current section.
    pub fn print_colored(&self, text: ColoredString) {
        self.print_with_indentation(text);
    }

    /// Prints a result for the current section.
    pub fn print_result<T: Message>(&self, result: T) {
        self.print_with_indentation(result.text().color(RESULT_COLOR));
    }

    /// Returns the prefix for the title of the current section.
    fn get_title_prefix(&self) -> String {
        if self.section_level < SOZO_TITLE_PREFIX.len() {
            SOZO_TITLE_PREFIX[self.section_level].to_string()
        } else {
            "".to_string()
        }
    }

    /// Returns the indentation for the current section.
    fn get_section_indent(&self) -> String {
        " ".repeat(self.section_level * SECTION_INDENT_LENGTH)
    }

    /// Prints a text with the indentation for the current section.
    fn print_with_indentation(&self, text: ColoredString) {
        println!("{}{}", self.get_section_indent(), text);
    }

    /// Prints a text with the color and the indentation set for the current section.
    fn print_with_section_color(&self, text: String) {
        let text = if self.section_level < SOZO_SECTION_COLORS.len() {
            let mut text = text.color(SOZO_SECTION_COLORS[self.section_level]);

            // main titles are always bold
            if self.section_level == 0 {
                text = text.bold()
            }

            text
        } else {
            text.into()
        };

        self.print_with_indentation(text);
    }
}
