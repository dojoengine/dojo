use std::error::Error;

use vergen::{BuildBuilder, Emitter};
use vergen_gitcl::GitclBuilder;

fn main() -> Result<(), Box<dyn Error>> {
    let build = BuildBuilder::default().build_timestamp(true).build()?;
    let gitcl =
        GitclBuilder::default().describe(true, true, None).branch(true).sha(true).build()?;

    // Emit the instructions
    Emitter::default().add_instructions(&build)?.add_instructions(&gitcl)?.emit_and_set()?;

    Ok(())
}
