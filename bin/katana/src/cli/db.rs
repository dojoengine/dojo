use std::path::{self};

use anyhow::{Context, Result};
use clap::{Args, Subcommand};
use comfy_table::modifiers::UTF8_ROUND_CORNERS;
use comfy_table::presets::UTF8_FULL;
use comfy_table::Table;
use katana_db::abstraction::Database;
use katana_db::mdbx::{DbEnv, DbEnvKind};
use katana_db::tables::NUM_TABLES;

/// Create a human-readable byte unit string (eg. 16.00 KiB)
macro_rules! byte_unit {
    ($size:expr) => {
        format!(
            "{:.2}",
            byte_unit::Byte::from_u64($size as u64)
                .get_appropriate_unit(byte_unit::UnitType::Binary)
        )
    };
}

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

                let mut table = table();
                let mut rows = Vec::with_capacity(NUM_TABLES);
                // total size of all tables (incl. freelist)
                let mut total_size = 0;

                table.set_header(vec![
                    "Table",
                    "Entries",
                    "Depth",
                    "Branch Pages",
                    "Leaf Pages",
                    "Overflow Pages",
                    "Size",
                ]);

                // page size is equal across all tables, so we can just get it from the first table
                // and use it to calculate for the freelist table.
                let mut pagesize: usize = 0;

                for (name, stat) in stats.table_stats().iter() {
                    let entries = stat.entries();
                    let depth = stat.depth();
                    let branch_pages = stat.branch_pages();
                    let leaf_pages = stat.leaf_pages();
                    let overflow_pages = stat.overflow_pages();
                    let size = stat.total_size();

                    rows.push(vec![
                        name.to_string(),
                        entries.to_string(),
                        depth.to_string(),
                        branch_pages.to_string(),
                        leaf_pages.to_string(),
                        overflow_pages.to_string(),
                        byte_unit!(size),
                    ]);

                    // increment the size of all tables
                    total_size += size;

                    if pagesize == 0 {
                        pagesize = stat.page_size() as usize;
                    }
                }

                // sort the rows by the table name
                rows.sort_by(|a, b| a[0].cmp(&b[0]));
                table.add_rows(rows);

                // add special row for the freelist table
                let freelist_size = stats.freelist() * pagesize;
                total_size += freelist_size;

                table.add_row(vec![
                    "Freelist".to_string(),
                    stats.freelist().to_string(),
                    "-".to_string(),
                    "-".to_string(),
                    "-".to_string(),
                    "-".to_string(),
                    byte_unit!(freelist_size),
                ]);

                // add the last row for the total size
                table.add_row(vec![
                    "Total Size".to_string(),
                    "-".to_string(),
                    "-".to_string(),
                    "-".to_string(),
                    "-".to_string(),
                    "-".to_string(),
                    byte_unit!(total_size),
                ]);

                println!("{table}");
            }
        }

        Ok(())
    }
}

/// Open the database at `path` in read-only mode.
///
/// The path is expanded and resolved to an absolute path before opening the database for clearer
/// error messages.
fn open_db_ro(path: &str) -> Result<DbEnv> {
    let path = path::absolute(shellexpand::full(path)?.into_owned())?;
    DbEnv::open(&path, DbEnvKind::RO).with_context(|| {
        format!("Opening database file in read-only mode at path {}", path.display())
    })
}

/// Create a table with the default UTF-8 full border and rounded corners.
fn table() -> Table {
    let mut table = Table::new();
    table.load_preset(UTF8_FULL).apply_modifier(UTF8_ROUND_CORNERS);
    table
}
