use anyhow::Result;
use clap::Args;
use dojo_bindgen::{Backend, BindingManager};
use dojo_lang::scarb_internal::compile_workspace;
use scarb::core::{Config, TargetKind};
use scarb::ops::CompileOpts;

#[derive(Args, Debug)]
pub struct BuildArgs {
    #[arg(long)]
    #[arg(help = "Generate Typescript bindings.")]
    pub typescript: bool,

    #[arg(long)]
    #[arg(help = "Generate Unity bindings.")]
    pub unity: bool,
}

impl BuildArgs {
    pub fn run(self, config: &Config) -> Result<()> {
        let compile_info = compile_workspace(
            config,
            CompileOpts { include_targets: vec![], exclude_targets: vec![TargetKind::TEST] },
        )?;

        let mut backends = vec![];
        if self.typescript {
            backends.push(Backend::Typescript);
        }

        if self.unity {
            backends.push(Backend::Unity);
        }

        let bindgen = BindingManager { artifacts_path: compile_info.target_dir, backends };

        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(bindgen.generate())
            .expect("Error generating bindings");

        Ok(())
    }
}
