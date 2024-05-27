use cairo_lang_language_server::Tricks;
use dojo_lang::plugin::dojo_plugin_suite;

fn main() {
    let mut tricks = Tricks::default();
    tricks.extra_plugin_suites = Some(&|| vec![dojo_plugin_suite()]);
    cairo_lang_language_server::start_with_tricks(tricks);
}
