use std::collections::{HashMap, HashSet};

use cairo_lang_defs::ids::{
    FunctionWithBodyId, ImplDefId, ImplFunctionId, LanguageElementId, ModuleId, ModuleItemId,
    SubmoduleId, TopLevelLanguageElementId, TraitFunctionId, TraitId,
};
use cairo_lang_diagnostics::{DiagnosticAdded, Maybe};
use cairo_lang_semantic::corelib::core_submodule;
use cairo_lang_semantic::db::SemanticGroup;
// use cairo_lang_semantic::items::attribute::SemanticQueryAttrs;
use cairo_lang_semantic::items::enm::SemanticEnumEx;
use cairo_lang_semantic::items::structure::SemanticStructEx;
use cairo_lang_semantic::plugin::DynPluginAuxData;
use cairo_lang_semantic::types::{ConcreteEnumLongId, ConcreteStructLongId};
use cairo_lang_semantic::{
    ConcreteTypeId, GenericArgumentId, GenericParam, Mutability, TypeId, TypeLongId,
};
use cairo_lang_utils::{extract_matches, try_extract_matches};
use itertools::zip_eq;
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;
use thiserror::Error;

#[cfg(test)]
#[path = "deps_test.rs"]
mod test;

#[derive(Default)]
pub struct DepsExtractor {
    /// List of type that were included abi.
    /// Used to avoid redundancy.
    types: HashSet<TypeId>,
}

impl DepsExtractor {}

fn get_type_name(db: &dyn SemanticGroup, ty: TypeId) -> Option<SmolStr> {
    let concrete_ty = try_extract_matches!(db.lookup_intern_type(ty), TypeLongId::Concrete)?;
    Some(concrete_ty.generic_type(db).name(db.upcast()))
}
