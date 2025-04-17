use cairo_lang_macro::{Diagnostic, Diagnostics, ProcMacroResult, TokenStream};

use crate::helpers::DiagnosticsExt;

pub trait ProcMacroResultExt {
    fn fail(message: String) -> Self;
    fn fail_with_diagnostics(diagnostics: Vec<Diagnostic>) -> Self;
    fn finalize(token_stream: TokenStream, diagnostics: Vec<Diagnostic>) -> Self;
}

impl ProcMacroResultExt for ProcMacroResult {
    fn fail(message: String) -> Self {
        Self::fail_with_diagnostics(Vec::<Diagnostic>::with_error(message))
    }
    fn fail_with_diagnostics(diagnostics: Vec<Diagnostic>) -> Self {
        ProcMacroResult::new(TokenStream::empty()).with_diagnostics(Diagnostics::new(diagnostics))
    }
    fn finalize(token_stream: TokenStream, diagnostics: Vec<Diagnostic>) -> Self {
        ProcMacroResult::new(token_stream).with_diagnostics(Diagnostics::new(diagnostics))
    }
}
