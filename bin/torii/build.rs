use std::{env, error::Error};

use vergen::{BuildBuilder, Emitter};
use vergen_gitcl::GitclBuilder;

fn main() -> Result<(), Box<dyn Error>> {
    let build = BuildBuilder::default().build_timestamp(true).build()?;
    let gitcl =
        GitclBuilder::default().describe(true, true, None).branch(true).sha(true).build()?;

    // Emit the instructions
    Emitter::default().add_instructions(&build)?.add_instructions(&gitcl)?.emit_and_set()?;

    let version = env!("CARGO_PKG_VERSION");
    let git_branch = env::var("VERGEN_GIT_BRANCH").unwrap_or("unknown".to_string());
    let git_sha = env::var("VERGEN_GIT_SHA").unwrap_or("unknown".to_string());
    let git_describe = env::var("VERGEN_GIT_DESCRIBE").unwrap_or("unknown".to_string());

    let dev = git_describe.contains(&git_sha) || git_describe.contains("dirty");

    let version = if dev {
        format!("{} dev ({} {})", version, git_branch, git_sha)
    } else {
        format!("{} ({})", version, git_sha)
    };
    println!("cargo:rustc-env=TORII_VERSION_SPEC={}", version);

    Ok(())
}
