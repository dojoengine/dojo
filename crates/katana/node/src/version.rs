/// The latest version from Cargo.toml.
pub const CARGO_PKG_VERSION: &str = env!("CARGO_PKG_VERSION");

/// The SHA of the latest commit.
pub const VERGEN_GIT_SHA: &str = env!("VERGEN_GIT_SHA");

// > 1.0.0-alpha.19 (77d4800)
// > if on dev (ie dirty):  1.0.0-alpha.19-dev (77d4800)
pub const VERSION: &str = const_format::concatcp!(
    env!("CARGO_PKG_VERSION"),
    env!("DEV_BUILD_SUFFIX"),
    " (",
    VERGEN_GIT_SHA,
    ")"
);
