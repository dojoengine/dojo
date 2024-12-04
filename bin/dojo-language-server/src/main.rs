use clap::Parser;

/// Dojo Language Server
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {}

fn main() {
    cairo_lang_language_server::start();
}
