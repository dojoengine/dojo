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

/// The color for results.
const RESULT_COLOR: Color = Color::BrightGreen;

#[derive(Debug)]
pub struct SozoUi {
    section_level: usize,
}

impl Default for SozoUi {
    fn default() -> Self {
        Self::new()
    }
}

impl SozoUi {
    /// Returns a new instance of SozoUi.
    pub fn new() -> Self {
        SozoUi { section_level: 0 }
    }

    /// Returns a new SozoUi instance to handle a subsection of the current one.
    pub fn subsection(&self) -> Self {
        SozoUi { section_level: self.section_level + 1 }
    }

    /// Prints a new line.
    pub fn new_line(&self) {
        println!();
    }

    /// Prints a block of text surrounded by newlines.
    pub fn print_block(&self, block: String) {
        self.print_colored_block(block.into());
    }

    /// Prints a block of text surrounded by newlines. Could provide a specific color for the text.
    pub fn print_colored_block(&self, block: ColoredString) {
        println!();
        self.print_with_indentation(block);
        println!();
    }

    /// Prints a block of text surrounded by newlines with a specific warning color.
    pub fn print_warning_block(&self, block: String) {
        self.print_colored_block(block.color(WARNING_COLOR));
    }

    /// Prints a title for the current section. Use the color set for the section.
    pub fn print_title(&self, title: String) {
        println!();
        self.print_with_section_color(format!("{}{}", self.get_title_prefix(), title));
    }

    /// Prints a text with the indentation for the current section.
    pub fn print(&self, text: String) {
        self.print_with_indentation(text.into());
    }

    /// Prints a text without any indentation or color.
    pub fn print_raw(&self, text: String) {
        println!("{}", text);
    }

    /// Prints a text with the indentation for the current section.
    pub fn print_colored(&self, text: ColoredString) {
        self.print_with_indentation(text);
    }

    /// Prints a result for the current section.
    pub fn print_result(&self, result: String) {
        self.print_with_indentation(result.color(RESULT_COLOR));
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
