use std::collections::HashMap;
use std::env;
use std::sync::Arc;

use anyhow::{Context, Result};
use cairo_lang_compiler::db::RootDatabase;
use cairo_lang_diagnostics::ToOption;
use cairo_lang_filesystem::db::{get_originating_location, FilesGroup};
use cairo_lang_filesystem::ids::{FileId, FileLongId};
use cairo_lang_filesystem::span::TextSpan;
use cairo_lang_sierra_generator::db::SierraGenGroup;
use cairo_lang_sierra_generator::program_generator::{
    SierraProgramDebugInfo, SierraProgramWithDebug,
};
use cairo_lang_starknet::compile::{extract_semantic_entrypoints, SemanticEntryPoints};
use cairo_lang_starknet::contract::ContractDeclaration;
use itertools::{chain, Itertools};
use serde::Serialize;

pub fn compile_prepared_db_to_debug_info(
    db: &RootDatabase,
    contracts: &[&ContractDeclaration],
    // mut compiler_config: CompilerConfig<'_>,
) -> Result<Vec<SierraProgramDebugInfo>> {
    // compiler_config.diagnostics_reporter.ensure(db)?;

    contracts
        .iter()
        .map(|contract| compile_contract_with_prepared_and_checked_db_to_debug_info(db, contract))
        .try_collect()
}

/// Compile declared Starknet contract.
///
/// The `contract` value **must** come from `db`, for example as a result of calling
/// [`find_contracts`]. Does not check diagnostics, it is expected that they are checked by caller
/// of this function.
fn compile_contract_with_prepared_and_checked_db_to_debug_info(
    db: &RootDatabase,
    contract: &ContractDeclaration,
) -> Result<SierraProgramDebugInfo> {
    let SemanticEntryPoints { external, l1_handler, constructor } =
        extract_semantic_entrypoints(db, contract)?;
    let SierraProgramWithDebug { program: _sierra_program, debug_info } = Arc::unwrap_or_clone(
        db.get_sierra_program_for_functions(
            chain!(&external, &l1_handler, &constructor).map(|f| f.value).collect(),
        )
        .to_option()
        .with_context(|| "Compilation failed without any diagnostics.")?,
    );

    Ok(debug_info)
}

#[derive(Debug, Clone, Serialize)]
pub struct SierraToCairoDebugInfo {
    pub sierra_statements_to_cairo_info: HashMap<usize, SierraStatementToCairoDebugInfo>,
}

/// Human readable position inside a file, in lines and characters.
#[derive(Debug, Serialize, Clone)]
pub struct TextPosition {
    /// Line index, 0 based.
    pub line: usize,
    /// Character index inside the line, 0 based.
    pub col: usize,
}

#[derive(Debug, Serialize, Clone)]
pub struct Location {
    pub start: TextPosition,
    pub end: TextPosition,
    pub file_path: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SierraStatementToCairoDebugInfo {
    pub cairo_locations: Vec<Location>,
}

/// Returns a map from Sierra statement indexes to Cairo function names.
pub fn get_sierra_to_cairo_debug_info(
    sierra_program_debug_info: &SierraProgramDebugInfo,
    compiler_db: &RootDatabase,
) -> SierraToCairoDebugInfo {
    let mut sierra_statements_to_cairo_info: HashMap<usize, SierraStatementToCairoDebugInfo> =
        HashMap::new();

    for (statement_idx, locations) in
        sierra_program_debug_info.statements_locations.locations.iter_sorted()
    {
        let mut cairo_locations: Vec<Location> = Vec::new();
        for location in locations {
            let syntax_node = location.syntax_node(compiler_db);
            let file_id = syntax_node.stable_ptr().file_id(compiler_db);
            let _file_name = file_id.file_name(compiler_db);
            let syntax_node_location_span = syntax_node.span_without_trivia(compiler_db);

            let (originating_file_id, originating_text_span) =
                get_originating_location(compiler_db, file_id, syntax_node_location_span);
            let cairo_location = get_location_from_text_span(
                originating_text_span,
                originating_file_id,
                compiler_db,
            );
            if let Some(cl) = cairo_location {
                cairo_locations.push(cl);
            }
        }
        sierra_statements_to_cairo_info
            .insert(statement_idx.0, SierraStatementToCairoDebugInfo { cairo_locations });
    }

    SierraToCairoDebugInfo { sierra_statements_to_cairo_info }
}

pub fn get_location_from_text_span(
    text_span: TextSpan,
    file_id: FileId,
    compiler_db: &RootDatabase,
) -> Option<Location> {
    let current_dir = env::current_dir().expect("Failed to get current directory");
    // dbg!(&current_dir);
    // let file_path = match compiler_db.lookup_intern_file(file_id) {
    //     FileLongId::OnDisk(path) => {
    //         path.strip_prefix(current_dir).expect("Failed to get relative
    // path").to_path_buf().to_str().unwrap_or("<unknown>").to_string()     },
    //     FileLongId::Virtual(_) => file_id.full_path(compiler_db),
    // };
    let file_path = match compiler_db.lookup_intern_file(file_id) {
        FileLongId::OnDisk(path) => match path.strip_prefix(&current_dir) {
            Ok(relative_path) => relative_path.to_str().unwrap_or("<unknown>").to_string(),
            Err(_) => {
                return None;
            }
        },
        FileLongId::Virtual(_) => {
            return None;
        }
    };

    // let file_path = file_id.full_path(compiler_db);

    let start: Option<TextPosition> = text_span
        .start
        .position_in_file(compiler_db, file_id)
        .map(|s| TextPosition { line: s.line, col: s.col });

    let end = text_span
        .end
        .position_in_file(compiler_db, file_id)
        .map(|e| TextPosition { line: e.line, col: e.col });

    start.zip(end).map(|(start, end)| Location { start, end, file_path })
}
