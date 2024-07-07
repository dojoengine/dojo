use std::fs;

use cairo_lang_filesystem::ids::Directory;
use cairo_lang_syntax::node::db::SyntaxGroup;
use regex::Regex;
use toml::Table;

/// Check if the provided name follows the format rules.
pub fn is_name_valid(name: &str) -> bool {
    Regex::new(r"^[a-zA-Z0-9_]+$").unwrap().is_match(name)
}

// parse the configuration file of the first crate to extract
// the main package Id (so the name field of the package section of the Scarb.toml file)
// TODO: Ask to Scarb team to expose this package Id information with the new macro system.
pub fn get_package_id(db: &dyn SyntaxGroup) -> Option<String> {
    let crates = db.crates();

    if crates.is_empty() {
        return Option::None;
    }

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

        if config.contains_key("package")
            && config["package"].as_table().unwrap().contains_key("name")
        {
            return Some(config["package"]["name"].as_str().unwrap().to_string());
        };
    }

    Option::None
}
