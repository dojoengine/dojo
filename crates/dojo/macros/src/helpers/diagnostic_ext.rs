use cairo_lang_macro::{Diagnostic, Severity};

pub trait DiagnosticsExt {
    fn with_error(message: String) -> Self;
    fn push_error(&mut self, message: String);
}

impl DiagnosticsExt for Vec<Diagnostic> {
    fn with_error(message: String) -> Self {
        vec![Diagnostic::error(message)]
    }
    fn push_error(&mut self, message: String) {
        self.push(Diagnostic::error(message));
    }
}

pub trait DiagnosticExt {
    fn to_pretty_string(&self) -> String;
}

impl DiagnosticExt for Diagnostic {
    fn to_pretty_string(&self) -> String {
        let severity = match self.severity() {
            Severity::Error => "error",
            Severity::Warning => "warning",
        };

        format!("[{severity}] {}", self.message())
    }
}
