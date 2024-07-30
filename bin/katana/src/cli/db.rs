use std::path::{self};

use anyhow::{Context, Result};
use byte_unit::UnitType;
use clap::{Args, Subcommand};
use comfy_table::modifiers::UTF8_ROUND_CORNERS;
use comfy_table::presets::UTF8_FULL;
use comfy_table::Table;
use katana_db::abstraction::Database;
use katana_db::mdbx::{DbEnv, DbEnvKind};

#[derive(Args)]
pub struct DbArgs {
    #[arg(short, long)]
    #[arg(global = true)]
    #[arg(help = "Path to the database directory")]
    #[arg(default_value = "~/.katana/db")]
    path: String,

    #[command(subcommand)]
    commands: Commands,
}

#[derive(Subcommand)]
enum Commands {
    #[command(about = "Retrieves database statistics")]
    Stats,
}

impl DbArgs {
    pub(crate) fn execute(self) -> Result<()> {
        match self.commands {
            Commands::Stats => {
                let db = open_db_ro(&self.path)?;
                let stats = db.stats()?;

                let mut table = Table::new();
                table.load_preset(UTF8_FULL).apply_modifier(UTF8_ROUND_CORNERS).set_header(vec![
                    "Table",
                    "Entries",
                    "Depth",
                    "Branch Pages",
                    "Leaf Pages",
                    "Overflow Pages",
                    "Total Size",
                ]);

                for (name, stat) in stats.table_stats() {
                    let entries = stat.entries();
                    let depth = stat.depth();
                    let branch_pages = stat.branch_pages();
                    let leaf_pages = stat.leaf_pages();
                    let overflow_pages = stat.overflow_pages();
                    let size = byte_unit::Byte::from_u64(stat.total_size() as u64)
                        .get_appropriate_unit(UnitType::Decimal);

                    table.add_row(vec![
                        name.to_string(),
                        entries.to_string(),
                        depth.to_string(),
                        branch_pages.to_string(),
                        leaf_pages.to_string(),
                        overflow_pages.to_string(),
                        format!("{size:.2}"),
                    ]);
                }

                println!("{table}");
            }
        }

        Ok(())
    }
}

// Open the database at `path` in read-only mode.
//
// The path is expanded and resolved to an absolute path before opening the database for clearer
// error messages.
fn open_db_ro(path: &str) -> Result<DbEnv> {
    let path = path::absolute(shellexpand::full(path)?.into_owned())?;
    DbEnv::open(&path, DbEnvKind::RO).with_context(|| {
        format!("Opening database file in read-only mode at path {}", path.display())
    })
}
