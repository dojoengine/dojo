use std::cmp::Reverse;

use anyhow::Result;
use clap::{Args, Parser};
use colored::{ColoredString, Colorize};
use dojo_bindgen::{BuiltinPlugins, PluginManager};
use dojo_world::ResourceType;
use dojo_world::local::{ResourceLocal, WorldLocal};
use scarb_interop::{self, Scarb, MetadataDojoExt};
use scarb_metadata::Metadata;
use tabled::settings::Style;
use tabled::{Table, Tabled};
use tracing::debug;

//use crate::commands::check_package_dojo_version;

#[derive(Debug, Clone, Args)]
pub struct BuildArgs {
    #[arg(long)]
    #[arg(help = "Generate Typescript bindings.")]
    pub typescript: bool,

    #[arg(long)]
    #[arg(help = "Generate Typescript bindings.")]
    pub typescript_v2: bool,

    #[arg(long)]
    #[arg(help = "Generate Recs bindings.")]
    pub recs: bool,

    #[arg(long)]
    #[arg(help = "Generate Unity bindings.")]
    pub unity: bool,

    #[arg(long)]
    #[arg(help = "Generate Unreal Engine bindings.")]
    pub unrealengine: bool,

    #[arg(long)]
    #[arg(help = "Output directory.", default_value = "bindings")]
    pub bindings_output: String,

    /* TODO RBA

    --
    For those two, let's copy the structs from Scarb
    without the associated logic. And we should implement a to string
    method to them pass this to `Scarb::build`.

    Or we can even makes it simpler, since by default we build the workspace (except if a list of packages is provided).

    And for the features, we still need those 3, so let's re-use the
    same struct as scarb.
    - features: a list of features.
    - all-features: to avoid listing all.
    - no-default-features: to avoid listing none and deactivating all features.
    --

       /// Specify the features to activate.
       #[command(flatten)]
       pub features: FeaturesSpec,

       /// Specify packages to build.
       #[command(flatten)]
       pub packages: Option<PackagesFilter>,
    */
    /// Display statistics about the compiled contracts.
    #[command(flatten)]
    pub stats: StatOptions,
}

#[derive(Debug, Clone, Args, Default, PartialEq)]
#[command(next_help_heading = "Statistics options")]
pub struct StatOptions {
    #[arg(long = "stats.by-tag")]
    #[arg(help = "Sort the stats by tag.")]
    #[arg(conflicts_with_all = ["stats.by-sierra-mb", "stats.by-sierra-felts", "stats.by-casm-felts"])]
    #[arg(default_value_t = false)]
    pub sort_by_tag: bool,

    #[arg(long = "stats.by-sierra-mb")]
    #[arg(help = "Sort the stats by Sierra file size in MB.")]
    #[arg(conflicts_with_all = ["stats.by-tag", "stats.by-sierra-felts", "stats.by-casm-felts"])]
    #[arg(default_value_t = false)]
    pub sort_by_sierra_mb: bool,

    #[arg(long = "stats.by-sierra-felts")]
    #[arg(help = "Sort the stats by Sierra program size in felts.")]
    #[arg(conflicts_with_all = ["stats.by-tag", "stats.by-sierra-mb", "stats.by-casm-felts"])]
    #[arg(default_value_t = false)]
    pub sort_by_sierra_felts: bool,

    #[arg(long = "stats.by-casm-felts")]
    #[arg(help = "Sort the stats by Casm bytecode size in felts.")]
    #[arg(conflicts_with_all = ["stats.by-tag", "stats.by-sierra-mb", "stats.by-sierra-felts"])]
    #[arg(default_value_t = false)]
    pub sort_by_casm_felts: bool,
}

impl BuildArgs {
    pub fn run(self, scarb_metadata: &Metadata) -> Result<()> {
        /* TODO RBA
                let ws = scarb::ops::read_workspace(config.manifest_path(), config)?;
                ws.profile_check()?;

                // Ensure we don't have old contracts in the build dir, since the local artifacts
                // guides the migration.
                ws.clean_dir_profile();

                let packages: Vec<Package> = if let Some(filter) = self.packages {
                    filter.match_many(&ws)?.into_iter().collect()
                } else {
                    ws.members().collect()
                };

                for p in &packages {
                    check_package_dojo_version(&ws, p)?;
                }

                debug!(?packages);
        */
        scarb_metadata.clean_dir_profile();

        // TODO: pass arguments to scarb build based on the one exposed into Sozo CLI.
        Scarb::build(&scarb_metadata.workspace.manifest_path)?;
        /* TODO RBA
                scarb::ops::compile(
                    packages.iter().map(|p| p.id).collect(),
                    CompileOpts {
                        include_target_names: vec![],
                        include_target_kinds: vec![],
                        exclude_target_kinds: vec![TargetKind::TEST],
                        features: self.features.try_into()?,
                        ignore_cairo_version: false,
                    },
                    &ws,
                )?;
        */
        let mut builtin_plugins = vec![];

        if self.typescript {
            builtin_plugins.push(BuiltinPlugins::Typescript);
        }

        if self.typescript_v2 {
            builtin_plugins.push(BuiltinPlugins::TypeScriptV2);
        }

        if self.recs {
            builtin_plugins.push(BuiltinPlugins::Recs);
        }

        if self.unity {
            builtin_plugins.push(BuiltinPlugins::Unity);
        }

        if self.unrealengine {
            builtin_plugins.push(BuiltinPlugins::UnrealEngine);
        }

        /* TODO RBA
                // Custom plugins are always empty for now.
                let bindgen = PluginManager {
                    profile_name: ws.current_profile().expect("Profile expected").to_string(),
                    root_package_name: ws
                        .root_package()
                        .map(|p| p.id.name.to_string())
                        .unwrap_or("NO_ROOT_PACKAGE".to_string()),
                    output_path: self.bindings_output.into(),
                    manifest_path: config.manifest_path().to_path_buf(),
                    plugins: vec![],
                    builtin_plugins,
                };

                // TODO: check about the skip migration as now we process the metadata
                // directly during the compilation to get the data we need from it.
                config.tokio_handle().block_on(bindgen.generate(None)).expect("Error generating bindings");

                if self.stats != StatOptions::default() {
                    let world = WorldLocal::from_directory(
                        ws.target_dir_profile().to_string(),
                        ws.load_profile_config().unwrap(),
                    )?;

                    let world_stat = world.to_stat_item();
                    let mut stats = vec![world_stat];

                    for r in world.resources.values() {
                        if r.resource_type() != ResourceType::Namespace {
                            stats.push(r.to_stat_item());
                        }
                    }

                    if self.stats.sort_by_tag {
                        stats.sort_by_key(|s| s.tag.clone());
                    } else if self.stats.sort_by_sierra_mb {
                        stats.sort_by_key(|s| Reverse(s.sierra_file_size));
                    } else if self.stats.sort_by_sierra_felts {
                        stats.sort_by_key(|s| Reverse(s.sierra_program_size));
                    } else if self.stats.sort_by_casm_felts {
                        stats.sort_by_key(|s| Reverse(s.casm_bytecode_size));
                    }

                    let mut table = Table::new(stats.iter().map(StatItemPrint::from).collect::<Vec<_>>());
                    table.with(Style::psql());

                    println!();
                    println!("{table}");

                    if stats.iter().all(|s| s.casm_bytecode_size == 0) {
                        println!(
                            "{}",
                            r#"
All the casm bytecode sizes are 0, did you forget to enable casm compilation?
To enable casm compilation, add the following to your Scarb.toml:

[[target.starknet-contract]]
sierra = true
casm = true
            "#
                            .bright_yellow()
                        );
                    }

                    println!(
                        "\nRefer to https://docs.starknet.io/tools/limits-and-triggers/ for more \
                         information about the public networks limits."
                    );
                }
        */
        Ok(())
    }
}

/* TODO RBA
impl Default for BuildArgs {
    fn default() -> Self {
        // use the clap defaults
        let features = FeaturesSpec::parse_from([""]);

        Self {
            features,
            typescript: false,
            typescript_v2: false,
            recs: false,
            unity: false,
            unrealengine: false,
            bindings_output: "bindings".to_string(),
            stats: StatOptions::default(),
            packages: None,
        }
    }
}

trait ContractStats {
    fn to_stat_item(&self) -> StatItem;
    fn sierra_file_size(&self) -> Result<usize>;
    fn sierra_program_felt_size(&self) -> usize;
    fn casm_program_felt_size(&self) -> usize;
}

#[derive(Debug, Tabled)]
struct StatItemPrint {
    #[tabled(rename = "")]
    tag: ColoredString,
    #[tabled(rename = "Sierra size (byte)")]
    sierra_file_size: ColoredString,
    #[tabled(rename = "Sierra felts")]
    sierra_program_size: ColoredString,
    #[tabled(rename = "Casm felts")]
    casm_bytecode_size: ColoredString,
}

impl From<&StatItem> for StatItemPrint {
    fn from(item: &StatItem) -> Self {
        const MAX_SIERRA_SIZE_BYTES: usize = 4_089_446;
        const MAX_CASM_FELTS: usize = 81_290;

        let tag = if item.tag == "world" {
            "World".to_string().bright_magenta()
        } else {
            item.tag.to_string().bright_blue()
        };

        let sierra_file_size = if item.sierra_file_size > MAX_SIERRA_SIZE_BYTES {
            item.sierra_file_size.to_string().bright_red()
        } else {
            item.sierra_file_size.to_string().bright_green()
        };

        let sierra_program_size = item.sierra_program_size.to_string().bright_black();

        let casm_bytecode_size = if item.casm_bytecode_size > MAX_CASM_FELTS {
            item.casm_bytecode_size.to_string().bright_red()
        } else {
            item.casm_bytecode_size.to_string().bright_green()
        };

        Self { tag, sierra_file_size, sierra_program_size, casm_bytecode_size }
    }
}

#[derive(Debug)]
struct StatItem {
    tag: String,
    sierra_file_size: usize,
    sierra_program_size: usize,
    casm_bytecode_size: usize,
}

impl ContractStats for ResourceLocal {
    fn to_stat_item(&self) -> StatItem {
        StatItem {
            tag: self.tag(),
            sierra_file_size: self.sierra_file_size().unwrap(),
            sierra_program_size: self.sierra_program_felt_size(),
            casm_bytecode_size: self.casm_program_felt_size(),
        }
    }

    fn sierra_file_size(&self) -> Result<usize> {
        // Easiest way to get the file size if by reserializing into the original json
        // the class file.
        Ok(serde_json::to_string(&self.common().class)?.len())
    }

    fn sierra_program_felt_size(&self) -> usize {
        self.common().class.sierra_program.len()
    }

    fn casm_program_felt_size(&self) -> usize {
        self.common().casm_class.as_ref().map_or(0, |casm| casm.bytecode.len())
    }
}

impl ContractStats for WorldLocal {
    fn to_stat_item(&self) -> StatItem {
        StatItem {
            tag: "world".to_string(),
            sierra_file_size: self.sierra_file_size().unwrap(),
            sierra_program_size: self.sierra_program_felt_size(),
            casm_bytecode_size: self.casm_program_felt_size(),
        }
    }

    fn sierra_file_size(&self) -> Result<usize> {
        // Easiest way to get the file size if by reserializing into the original json
        // the class file.
        Ok(serde_json::to_string(&self.class)?.len())
    }

    fn sierra_program_felt_size(&self) -> usize {
        self.class.sierra_program.len()
    }

    fn casm_program_felt_size(&self) -> usize {
        self.casm_class.as_ref().map_or(0, |casm| casm.bytecode.len())
    }
}
*/
