//! A simple UI for the migration that can be used to display a spinner.

use std::fmt;

use sozo_voyager::VerificationUi;
use spinoff::spinners::SpinnerFrames;
use spinoff::{Spinner, spinner, spinners};

/// A simple UI for the migration that can be used to display a spinner.
pub struct MigrationUi {
    spinner: Spinner,
    default_frames: SpinnerFrames,
    silent: bool,
}

impl fmt::Debug for MigrationUi {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "spinner silent: {}", self.silent)
    }
}

impl MigrationUi {
    /// Returns a new instance with the default frames.
    pub fn new(text: Option<&'static str>) -> Self {
        if let Some(text) = text {
            let frames = spinner!(["⛩️ ", "🥷 ", "🗡️ "], 500);
            let spinner = Spinner::new(frames.clone(), text, None);
            Self { spinner, default_frames: frames, silent: false }
        } else {
            let frames = spinner!([""], 5000);
            let spinner = Spinner::new(frames.clone(), "", None);
            Self { spinner, default_frames: frames, silent: false }
        }
    }

    /// Returns a new instance with the given frames.
    pub fn new_with_frames(text: &'static str, frames: Vec<&'static str>) -> Self {
        let frames =
            spinners::SpinnerFrames { interval: 500, frames: frames.into_iter().collect() };

        let spinner = Spinner::new(frames.clone(), text, None);
        Self { spinner, default_frames: frames, silent: false }
    }

    /// Returns a new instance with the silent flag set.
    pub fn with_silent(mut self) -> Self {
        self.silent = true;
        self
    }

    /// Updates the text of the spinner.
    pub fn update_text(&mut self, text: &'static str) {
        if self.silent {
            return;
        }

        self.spinner.update_text(text);
    }

    /// Updates the text of the spinner with a boxed string.
    pub fn update_text_boxed(&mut self, text: String) {
        self.update_text(Box::leak(text.into_boxed_str()));
    }

    /// Stops the spinner and persists the text.
    pub fn stop_and_persist_boxed(&mut self, symbol: &'static str, text: String) {
        self.stop_and_persist(symbol, Box::leak(text.into_boxed_str()));
    }

    /// Stops the spinner and persists the text.
    pub fn stop_and_persist(&mut self, symbol: &'static str, text: &'static str) {
        if self.silent {
            return;
        }

        self.spinner.stop_and_persist(symbol, text);
    }

    /// Stops the spinner without additional text.
    pub fn stop(&mut self) {
        if self.silent {
            return;
        }

        self.spinner.stop_with_message("");
    }

    /// Restarts the spinner with the default frames if it has been stopped.
    pub fn restart(&mut self, text: &'static str) {
        if self.silent {
            return;
        }

        self.spinner = Spinner::new(self.default_frames.clone(), text, None);
    }
}

impl VerificationUi for MigrationUi {
    fn update_text_boxed(&mut self, text: String) {
        self.update_text_boxed(text);
    }
}
