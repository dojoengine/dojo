use cairo_lang_macro::{Diagnostic, ProcMacroResult, TokenStream};
use cairo_lang_parser::utils::SimpleParserDatabase;
use cairo_lang_syntax::node::ast::{ItemEnum, OptionTypeClause, Variant};
use cairo_lang_syntax::node::helpers::QueryAttrs;
use cairo_lang_syntax::node::Terminal;

use crate::helpers::{DiagnosticsExt, DojoChecker, ProcMacroResultExt};

#[derive(Debug)]
pub struct DojoEnumIntrospect {
    pub diagnostics: Vec<Diagnostic>,
}

impl DojoEnumIntrospect {
    pub fn new() -> Self {
        Self {
            diagnostics: vec![],
        }
    }

    pub fn process(
        db: &SimpleParserDatabase,
        enum_ast: &ItemEnum,
        is_packed: bool,
    ) -> ProcMacroResult {
        let mut introspect = DojoEnumIntrospect::new();

        let derive_attrs = enum_ast.attributes(db).query_attr(db, "derive");

        DojoChecker::check_derive_conflicts(db, &mut introspect.diagnostics, derive_attrs);

        let token = introspect.generate(db, enum_ast, is_packed);

        ProcMacroResult::finalize(token, introspect.diagnostics)
    }

    /// Generate the introspect of a Enum
    pub fn generate(
        &mut self,
        db: &SimpleParserDatabase,
        enum_ast: &ItemEnum,
        is_packed: bool,
    ) -> TokenStream {
        let enum_name = enum_ast.name(db).text(db).into();
        let variant_sizes = self.compute_enum_variant_sizes(db, enum_ast);

        let layout = if is_packed {
            if self.is_enum_packable(&variant_sizes) {
                self.build_packed_enum_layout(db, enum_ast)
            } else {
                self.diagnostics.push_error(
                    "To be packed, all variants must have fixed layout of same size.".to_string(),
                );
                "".to_string()
            }
        } else {
            format!(
                "dojo::meta::Layout::Enum(
                array![
                {}
                ].span()
            )",
                self.build_variant_layouts(db, enum_ast)
            )
        };

        let (gen_types, gen_impls) =
            super::generics::build_generic_types_and_impls(db, enum_ast.generic_params(db));
        let enum_size = self.compute_enum_layout_size(&variant_sizes, is_packed);
        let ty = self.build_enum_ty(db, &enum_name, enum_ast);

        super::generate_introspect(&enum_name, &enum_size, &gen_types, gen_impls, &layout, &ty)
    }

    pub fn compute_enum_variant_sizes(
        &self,
        db: &SimpleParserDatabase,
        enum_ast: &ItemEnum,
    ) -> Vec<(Vec<String>, u32, bool)> {
        enum_ast
            .variants(db)
            .elements(db)
            .iter()
            .map(|v| match v.type_clause(db) {
                OptionTypeClause::Empty(_) => (vec![], 0, false),
                OptionTypeClause::TypeClause(type_clause) => {
                    super::size::get_field_size_from_type_clause(db, &type_clause)
                }
            })
            .collect::<Vec<_>>()
    }

    pub fn is_enum_packable(&self, variant_sizes: &[(Vec<String>, u32, bool)]) -> bool {
        if variant_sizes.is_empty() {
            return true;
        }

        let v0_sizes = variant_sizes[0].0.clone();
        let v0_fixed_size = variant_sizes[0].1;

        variant_sizes.iter().all(|vs| {
            vs.0.len() == v0_sizes.len()
                && vs.0.iter().zip(v0_sizes.iter()).all(|(a, b)| a == b)
                && vs.1 == v0_fixed_size
                && !vs.2
        })
    }

    pub fn compute_enum_layout_size(
        &self,
        variant_sizes: &[(Vec<String>, u32, bool)],
        is_packed: bool,
    ) -> String {
        if variant_sizes.is_empty() {
            return "Option::None".to_string();
        }

        let v0 = variant_sizes[0].clone();
        let identical_variants = variant_sizes
            .iter()
            .all(|vs| vs.0 == v0.0 && vs.1 == v0.1 && vs.2 == v0.2);

        if identical_variants {
            let (mut sizes, mut cumulated_sizes, is_dynamic_size) = v0;

            // add one felt252 to store the variant identifier
            cumulated_sizes += 1;

            super::size::build_size_function_body(
                &mut sizes,
                cumulated_sizes,
                is_dynamic_size,
                is_packed,
            )
        } else {
            "Option::None".to_string()
        }
    }

    //
    pub fn build_packed_enum_layout(
        &mut self,
        db: &SimpleParserDatabase,
        enum_ast: &ItemEnum,
    ) -> String {
        // to be packable, all variants data must have the same size.
        // as this point has already been checked before calling `build_packed_enum_layout`,
        // just use the first variant to generate the fixed layout.
        let elements = enum_ast.variants(db).elements(db);
        let mut variant_layout = if elements.is_empty() {
            vec![]
        } else {
            match elements.first().unwrap().type_clause(db) {
                OptionTypeClause::Empty(_) => vec![],
                OptionTypeClause::TypeClause(type_clause) => {
                    super::layout::get_packed_field_layout_from_type_clause(
                        db,
                        &mut self.diagnostics,
                        &type_clause,
                    )
                }
            }
        };

        // don't forget the store the variant value
        variant_layout.insert(0, "8".to_string());

        if variant_layout
            .iter()
            .any(|v| super::layout::is_custom_layout(v.as_str()))
        {
            super::layout::generate_cairo_code_for_fixed_layout_with_custom_types(&variant_layout)
        } else {
            format!(
                "dojo::meta::Layout::Fixed(
                array![
                {}
                ].span()
            )",
                variant_layout.join(",")
            )
        }
    }

    /// build the full layout for every variant in the Enum.
    /// Note that every variant may have a different associated data type.
    pub fn build_variant_layouts(
        &mut self,
        db: &SimpleParserDatabase,
        enum_ast: &ItemEnum,
    ) -> String {
        let mut layouts = vec![];

        for (i, v) in enum_ast.variants(db).elements(db).iter().enumerate() {
            let selector = format!("{i}");

            let variant_layout = match v.type_clause(db) {
                OptionTypeClause::Empty(_) => {
                    "dojo::meta::Layout::Fixed(array![].span())".to_string()
                }
                OptionTypeClause::TypeClause(type_clause) => {
                    super::layout::get_layout_from_type_clause(
                        db,
                        &mut self.diagnostics,
                        &type_clause,
                    )
                }
            };

            layouts.push(format!(
                "dojo::meta::FieldLayout {{
                    selector: {selector},
                    layout: {variant_layout}
                }}"
            ));
        }

        layouts.join(",\n")
    }

    pub fn build_enum_ty(
        &self,
        db: &SimpleParserDatabase,
        name: &String,
        enum_ast: &ItemEnum,
    ) -> String {
        let variants = enum_ast.variants(db).elements(db);

        let variants_ty = if variants.is_empty() {
            "".to_string()
        } else {
            variants
                .iter()
                .map(|v| self.build_variant_ty(db, v))
                .collect::<Vec<_>>()
                .join(",\n")
        };

        format!(
            "dojo::meta::introspect::Ty::Enum(
            dojo::meta::introspect::Enum {{
                name: '{name}',
                attrs: array![].span(),
                children: array![
                {variants_ty}\n
                ].span()
            }}
        )"
        )
    }

    pub fn build_variant_ty(&self, db: &SimpleParserDatabase, variant: &Variant) -> String {
        let name = variant.name(db).text(db).to_string();
        match variant.type_clause(db) {
            OptionTypeClause::Empty(_) => {
                // use an empty tuple if the variant has no data
                format!("('{name}', dojo::meta::introspect::Ty::Tuple(array![].span()))")
            }
            OptionTypeClause::TypeClause(type_clause) => {
                format!(
                    "('{name}', {})",
                    super::ty::build_ty_from_type_clause(db, &type_clause)
                )
            }
        }
    }
}
