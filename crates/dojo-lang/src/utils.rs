use std::fs;

use cairo_lang_filesystem::ids::Directory;
use cairo_lang_syntax::node::db::SyntaxGroup;
use regex::Regex;
use toml::Table;

/// Check if the provided name follows the format rules.
pub fn is_name_valid(name: &str) -> bool {
    Regex::new(r"^[a-zA-Z0-9_]+$").unwrap().is_match(name)
}

// Parses the configuration file of the first crate to extract the default namespace.
// TODO: Ask to Scarb team to expose this information with the new macro system.
pub fn get_default_namespace(db: &dyn SyntaxGroup) -> Option<String> {
    let crates = db.crates();

    if crates.is_empty() {
        return Option::None;
    }

    // Crates[0] is always the root crate that triggered the build origin.
    // In case of a library, crates[0] refers to the lib itself if compiled directly,
    // or the crate using the library otherwise.
    let configuration = match db.crate_config(crates[0]) {
        Option::Some(cfg) => cfg,
        Option::None => return Option::None,
    };

    if let Directory::Real(path) = configuration.root {
        let config_path = path.parent().unwrap().join("Scarb.toml");
        let config_content = match fs::read_to_string(config_path) {
            Ok(x) => x,
            Err(_) => return Option::None,
        };
        let config = match config_content.parse::<Table>() {
            Ok(x) => x,
            Err(_) => return Option::None,
        };

        if let Some(tool) = config.get("tool").and_then(|t| t.as_table()) {
            if let Some(dojo) = tool.get("dojo").and_then(|d| d.as_table()) {
                if let Some(world) = dojo.get("world").and_then(|w| w.as_table()) {
                    if let Some(namespace) = world.get("namespace").and_then(|n| n.as_str()) {
                        return Some(namespace.to_string());
                    }
                }
            }
        }
    }

    Option::None
}
