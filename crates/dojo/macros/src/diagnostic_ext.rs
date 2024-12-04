use cairo_lang_macro::Diagnostic;

pub trait DiagnosticsExt {
    fn push_error(&mut self, message: String);
    fn push_warn(&mut self, message: String);
}

impl DiagnosticsExt for Vec<Diagnostic> {
    fn push_error(&mut self, message: String) {
        self.push(Diagnostic::error(message));
    }

    fn push_warn(&mut self, message: String) {
        self.push(Diagnostic::warn(message));
    }
}
