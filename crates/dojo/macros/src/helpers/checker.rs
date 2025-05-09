use cairo_lang_macro::{Diagnostic, ProcMacroResult};
use cairo_lang_parser::utils::SimpleParserDatabase;
use cairo_lang_syntax::node::ast::Attribute;
use dojo_types::naming;

use crate::constants::{DOJO_INTROSPECT_DERIVE, DOJO_PACKED_DERIVE};
use crate::helpers::{DiagnosticsExt, DojoParser, ProcMacroResultExt};

pub struct DojoChecker {}

/// DojoChecker groups common verifications that should be done while
/// generating Dojo code.
impl DojoChecker {
    /// Be sure there is no conflict among `derive` attributes
    /// set on a Cairo element.
    pub fn check_derive_conflicts(
        db: &SimpleParserDatabase,
        diagnostics: &mut Vec<Diagnostic>,
        attrs: Vec<Attribute>,
    ) {
        let attr_names = DojoParser::extract_derive_attr_names(db, diagnostics, attrs);

        if attr_names.contains(&DOJO_INTROSPECT_DERIVE.to_string())
            && attr_names.contains(&DOJO_PACKED_DERIVE.to_string())
        {
            diagnostics.push_error(
                format!("{DOJO_INTROSPECT_DERIVE} and {DOJO_PACKED_DERIVE} attributes cannot be used at a same time.")
            );
        }
    }

    /// Check if the name of a Dojo element is valid.
    pub fn is_name_valid(element: &str, name: &str) -> Option<ProcMacroResult> {
        if !naming::is_name_valid(name) {
            return Some(ProcMacroResult::fail(format!(
                "The {element} name '{name}' can only contain characters (a-z/A-Z), \
                digits (0-9) and underscore (_)."
            )));
        }

        None
    }
}
