use cairo_lang_macro::{Diagnostics, ProcMacroResult, TokenStream};

use crate::diagnostic_ext::DiagnosticsExt;

pub trait ProcMacroResultExt {
    fn error(message: String) -> Self;
}

impl ProcMacroResultExt for ProcMacroResult {
    fn error(message: String) -> Self {
        let mut diagnostics = vec![];
        diagnostics.push_error(message);
        ProcMacroResult::new(TokenStream::empty()).with_diagnostics(Diagnostics::new(diagnostics))
    }
}
