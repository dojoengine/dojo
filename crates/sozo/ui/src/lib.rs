//! Manages the UI for the Sozo CLI.

use colored::{Color, ColoredString, Colorize};

#[derive(Debug, Clone, Default, PartialEq, PartialOrd)]
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

#[derive(Debug, Clone)]
pub struct SozoUiTheme {
    pub indent_length: usize,
    pub section_colors: Vec<Color>,
    pub step_color: Color,
    pub default_color: Color,
    pub title_prefixes: Vec<String>,
    pub warning_color: Color,
    pub error_color: Color,
    pub debug_color: Color,
    pub trace_color: Color,
    pub result_color: Color,
}

impl SozoUiTheme {
    pub fn light() -> SozoUiTheme {
        SozoUiTheme {
            indent_length: 3,
            section_colors: vec![Color::Blue, Color::Blue],
            step_color: Color::Magenta,
            default_color: Color::Black,
            title_prefixes: vec!["> ".to_string(), "- ".to_string()],
            warning_color: Color::Yellow,
            error_color: Color::Red,
            debug_color: Color::BrightBlack,
            trace_color: Color::BrightBlack,
            result_color: Color::Green,
        }
    }

    pub fn dark() -> SozoUiTheme {
        SozoUiTheme {
            indent_length: 3,
            section_colors: vec![Color::BrightBlue, Color::Blue],
            step_color: Color::BrightMagenta,
            default_color: Color::White,
            title_prefixes: vec!["> ".to_string(), "- ".to_string()],
            warning_color: Color::BrightYellow,
            error_color: Color::BrightRed,
            debug_color: Color::BrightBlack,
            trace_color: Color::BrightBlack,
            result_color: Color::BrightGreen,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SozoUi {
    section_level: usize,
    verbosity: SozoVerbosity,
    theme: SozoUiTheme,
}

impl Default for SozoUi {
    fn default() -> Self {
        Self::new(SozoUiTheme::dark(), SozoVerbosity::default())
    }
}

impl SozoUi {
    /// Returns a new instance of SozoUi.
    pub fn new(theme: SozoUiTheme, verbosity: SozoVerbosity) -> Self {
        SozoUi { section_level: 0, verbosity, theme }
    }

    /// Returns a new SozoUi instance to handle a subsection of the current one.
    pub fn subsection(&self) -> Self {
        let mut ui = self.clone();
        ui.section_level += 1;
        ui
    }

    /// Prints a new line.
    pub fn new_line(&self) {
        if self.verbosity > SozoVerbosity::Quiet {
            println!();
        }
    }

    /// Indent a text using the indentation config.
    pub fn indent<T: Message>(&self, level: usize, text: T) -> String {
        format!("{}{}", self.get_indent(level), text.text())
    }

    /// Prints a title for the current section. Use the color set for the section.
    pub fn title<T: Message>(&self, title: T) {
        if self.verbosity > SozoVerbosity::Quiet {
            self.new_line();
            self.print_with_section_color(format!("{}{}", self.get_title_prefix(), title.text()));
        }
    }

    /// Prints a step withthe current section.
    pub fn step<T: Message>(&self, step: T) {
        if self.verbosity > SozoVerbosity::Quiet {
            self.new_line();
            self.print_with_indentation(format!("◦ {}", step.text()).color(self.theme.step_color));
        }
    }

    /// Prints a text with the indentation for the current section.
    pub fn print<T: Message>(&self, text: T) {
        self.do_print(text, SozoVerbosity::Normal, self.theme.default_color);
    }

    /// Prints a text without any indentation or color.
    pub fn print_raw<T: Message>(&self, text: T) {
        if self.verbosity > SozoVerbosity::Quiet {
            println!("{}", text.text());
        }
    }

    /// Prints a result for the current section.
    pub fn result<T: Message>(&self, result: T) {
        self.do_print(result, SozoVerbosity::Normal, self.theme.result_color);
    }

    /// Prints a warning without taking the indentation into account.
    pub fn warn<T: Message>(&self, text: T) {
        self.do_print(text, SozoVerbosity::Quiet, self.theme.warning_color);
    }

    /// Prints an error without taking the indentation into account.
    pub fn error<T: Message>(&self, text: T) {
        self.do_print(
            format!("error: {}", text.text().trim()),
            SozoVerbosity::Quiet,
            self.theme.error_color,
        );
    }

    pub fn verbose<T: Message>(&self, text: T) {
        self.do_print(text, SozoVerbosity::Verbose, self.theme.default_color);
    }

    pub fn debug<T: Message>(&self, text: T) {
        self.do_print(text, SozoVerbosity::Debug, self.theme.debug_color);
    }

    pub fn trace<T: Message>(&self, text: T) {
        self.do_print(text, SozoVerbosity::Trace, self.theme.trace_color);
    }

    /// Prints a block of text surrounded by newlines.
    pub fn block<T: Message>(&self, block: T) {
        self.do_block(block, SozoVerbosity::Normal, self.theme.default_color);
    }

    /// Prints a block of text surrounded by newlines with a specific warning color.
    pub fn warn_block<T: Message>(&self, block: T) {
        self.new_line();
        self.do_block(block, SozoVerbosity::Normal, self.theme.warning_color);
        self.new_line();
    }

    /// Prints a block of text surrounded by newlines with a specific erorr color.
    pub fn error_block<T: Message>(&self, block: T) {
        self.new_line();
        self.do_block(block, SozoVerbosity::Normal, self.theme.error_color);
        self.new_line();
    }

    pub fn verbose_block<T: Message>(&self, block: T) {
        self.do_block(block, SozoVerbosity::Verbose, self.theme.default_color);
    }

    /// Prints a block of text surrounded by newlines with a specific debug color.
    pub fn debug_block<T: Message>(&self, block: T) {
        self.do_block(block, SozoVerbosity::Debug, self.theme.debug_color);
    }

    /// Prints a block of text surrounded by newlines with a specific trace color.
    pub fn trace_block<T: Message>(&self, block: T) {
        self.do_block(block, SozoVerbosity::Trace, self.theme.trace_color);
    }

    fn do_print<T: Message>(&self, text: T, verbosity: SozoVerbosity, color: Color) {
        if self.verbosity >= verbosity {
            self.print_with_indentation(text.text().color(color));
        }
    }

    fn do_block<T: Message>(&self, multiline_text: T, verbosity: SozoVerbosity, color: Color) {
        if self.verbosity >= verbosity {
            for line in multiline_text.text().lines() {
                self.print_with_indentation(line.color(color));
            }
        }
    }

    /// Returns the prefix for the title of the current section.
    fn get_title_prefix(&self) -> String {
        if self.section_level < self.theme.title_prefixes.len() {
            self.theme.title_prefixes[self.section_level].to_string()
        } else {
            "".to_string()
        }
    }

    /// Returns the indentation for the current section.
    fn get_indent(&self, level: usize) -> String {
        " ".repeat(level * self.theme.indent_length)
    }

    /// Prints a text with the indentation for the current section.
    fn print_with_indentation(&self, text: ColoredString) {
        println!("{}{}", self.get_indent(self.section_level), text);
    }

    /// Prints a text with the color and the indentation set for the current section.
    fn print_with_section_color(&self, text: String) {
        let text = if self.section_level < self.theme.section_colors.len() {
            let mut text = text.color(self.theme.section_colors[self.section_level]);

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
