use console::{pad_str, Alignment, Style, StyledObject};
use scarb_ui::Ui;

pub trait MigrationUi {
    fn print_step(&self, step: usize, icon: &str, message: &str);

    fn print_header(&self, message: impl AsRef<str>);

    fn print_sub(&self, message: impl AsRef<str>);

    fn print_hidden_sub(&self, message: impl AsRef<str>);
}

impl MigrationUi for Ui {
    fn print_step(&self, step: usize, icon: &str, message: &str) {
        self.print(format!("{} {icon} {message}.", dimmed_message(format!("[{step}]"))));
    }

    fn print_header(&self, message: impl AsRef<str>) {
        self.print(bold_message(message.as_ref()).to_string())
    }

    fn print_sub(&self, message: impl AsRef<str>) {
        self.print(subtitle(message));
    }

    fn print_hidden_sub(&self, message: impl AsRef<str>) {
        self.verbose(subtitle(message));
    }
}

fn subtitle<D: AsRef<str>>(message: D) -> String {
    dimmed_message(format!("{} {}", pad_str(">", 3, Alignment::Right, None), message.as_ref()))
        .to_string()
}

pub(super) fn dimmed_message<D>(message: D) -> StyledObject<D> {
    Style::new().dim().apply_to(message)
}

pub(super) fn bold_message<D>(message: D) -> StyledObject<D> {
    Style::new().bold().apply_to(message)
}

pub(super) fn italic_message<D>(message: D) -> StyledObject<D> {
    Style::new().italic().apply_to(message)
}
