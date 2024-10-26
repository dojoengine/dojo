use std::fs;
use std::str::FromStr;

use anyhow::{Error, Result};
use camino::Utf8PathBuf;
use dojo_world::config::ProfileConfig;
use scarb::core::{Config, TomlManifest};
use semver::Version;

/// Loads the profile config from the Scarb workspace configuration.
pub fn load_profile_config(config: &Config) -> Result<(String, ProfileConfig), Error> {
    let ws = scarb::ops::read_workspace(config.manifest_path(), config)?;
    // Safe to unwrap since manifest is a file.
    let manifest_dir = ws.manifest_path().parent().unwrap().to_path_buf();
    let profile_str =
        ws.current_profile().expect("Scarb profile expected to be defined.").to_string();

    let dev_config_path = manifest_dir.join("dojo_dev.toml");
    let config_path = manifest_dir.join(format!("dojo_{}.toml", &profile_str));

    if !dev_config_path.exists() {
        return Err(anyhow::anyhow!(
            "Profile configuration file not found for profile `{}`. Expected at {}.",
            &profile_str,
            dev_config_path
        ));
    }

    // If the profile file is not found, default to `dev.toml` file that must exist.
    let config_path = if !config_path.exists() { dev_config_path } else { config_path };

    let content = fs::read_to_string(&config_path)?;
    let config: ProfileConfig = toml::from_str(&content)?;

    Ok((profile_str.to_string(), config))
}

pub fn verify_cairo_version_compatibility(manifest_path: &Utf8PathBuf) -> Result<()> {
    let scarb_cairo_version = scarb::version::get().cairo;
    // When manifest file doesn't exists ignore it. Would be the case during `sozo init`
    let Ok(manifest) = TomlManifest::read_from_path(manifest_path) else { return Ok(()) };

    // For any kind of error, like package not specified, cairo version not specified return
    // without an error
    let Some(package) = manifest.package else { return Ok(()) };

    let Some(cairo_version) = package.cairo_version else { return Ok(()) };

    // only when cairo version is found in manifest file confirm that it matches
    let version_req = cairo_version.as_defined().unwrap();
    let version = Version::from_str(scarb_cairo_version.version).unwrap();
    if !version_req.matches(&version) {
        anyhow::bail!(
            "Cairo version {} found in {} is not supported by dojo (expecting {}). Please change \
             the Cairo version in your manifest or update dojo.",
            version_req,
            manifest_path,
            version,
        );
    };

    Ok(())
}

pub fn generate_version() -> String {
    const DOJO_VERSION: &str = env!("CARGO_PKG_VERSION");
    let scarb_version = scarb::version::get().version;
    let scarb_sierra_version = scarb::version::get().sierra.version;
    let scarb_cairo_version = scarb::version::get().cairo.version;

    let version_string = format!(
        "{}\nscarb: {}\ncairo: {}\nsierra: {}",
        DOJO_VERSION, scarb_version, scarb_cairo_version, scarb_sierra_version,
    );
    version_string
}

pub fn is_address(tag_or_address: &str) -> bool {
    tag_or_address.starts_with("0x")
}
