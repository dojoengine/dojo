use std::env;
use std::error::Error;

use vergen::{BuildBuilder, Emitter};
use vergen_gitcl::GitclBuilder;

fn main() -> Result<(), Box<dyn Error>> {
    let build = BuildBuilder::default().build_timestamp(true).build()?;
    let gitcl =
        GitclBuilder::default().describe(true, false, None).dirty(true).sha(true).build()?;

    // Emit the instructions
    Emitter::default().add_instructions(&build)?.add_instructions(&gitcl)?.emit_and_set()?;

    let sha = env::var("VERGEN_GIT_SHA")?;
    let is_dirty = env::var("VERGEN_GIT_DIRTY")? == "true";

    // > git describe --always --tags
    // if not on a tag: v0.2.0-beta.3-82-g1939939b
    // if on a tag: v0.2.0-beta.3
    let not_on_tag = env::var("VERGEN_GIT_DESCRIBE")?.ends_with(&format!("-g{sha}"));
    let is_dev = is_dirty || not_on_tag;
    println!("cargo:rustc-env=DEV_BUILD_SUFFIX={}", if is_dev { "-dev" } else { "" });

    Ok(())
}
