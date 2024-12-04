use cairo_lang_macro::TokenStream;
use regex::Regex;

/// Asserts that the output token stream is as expected.
///
/// #Arguments
///   `output` - the output token stream
///   `expected` - the expected output
pub(crate) fn assert_output_stream(output: &TokenStream, expected: &str) {
    // to avoid differences due to formatting, we remove all the whitespaces
    // and newlines.
    fn trim_whitespaces_and_newlines(s: &str) -> String {
        s.replace(" ", "").replace("\n", "")
    }

    // the `ensure_unique` function contains a randomly generated
    // hash, so we have to remove it to be able to compare.
    let re = Regex::new(r"let _hash =.*;").unwrap();
    let output = output.to_string();
    let output = re.replace(&output, "");

    let output = trim_whitespaces_and_newlines(&output);
    let expected = trim_whitespaces_and_newlines(expected);

    assert_eq!(output, expected);
}
