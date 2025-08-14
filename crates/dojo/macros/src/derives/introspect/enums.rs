use cairo_lang_macro::{Diagnostic, ProcMacroResult, TokenStream};
use cairo_lang_parser::utils::SimpleParserDatabase;
use cairo_lang_syntax::node::Terminal;
use cairo_lang_syntax::node::ast::{ItemEnum, OptionTypeClause, Variant};
use cairo_lang_syntax::node::helpers::QueryAttrs;

use crate::helpers::{DiagnosticsExt, DojoChecker, DojoFormatter, ProcMacroResultExt};

#[derive(Debug)]
pub struct DojoEnumIntrospect {
    pub diagnostics: Vec<Diagnostic>,
}

impl DojoEnumIntrospect {
    pub fn new() -> Self {
        Self { diagnostics: vec![] }
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
        let (variant_sizes, identical_variants) = self.compute_enum_variant_sizes(db, enum_ast);

        let layout = if is_packed {
            if identical_variants {
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

        let gen_types = DojoFormatter::build_generic_types(db, enum_ast.generic_params(db));

        let inspect_gen_impls = DojoFormatter::build_generic_impls(
            &gen_types,
            &["+dojo::meta::introspect::Introspect".to_string()],
            &[],
        );

        let enum_size = self.compute_enum_layout_size(&variant_sizes, identical_variants);
        let ty = self.build_enum_ty(db, &enum_name, enum_ast);

        super::generate_introspect(
            &enum_name,
            &enum_size,
            &gen_types,
            inspect_gen_impls,
            &layout,
            &ty,
        )
    }

    pub fn compute_enum_variant_sizes(
        &self,
        db: &SimpleParserDatabase,
        enum_ast: &ItemEnum,
    ) -> (Vec<Vec<String>>, bool) {
        let variant_sizes = enum_ast
            .variants(db)
            .elements(db)
            .iter()
            .map(|v| match v.type_clause(db) {
                OptionTypeClause::Empty(_) => vec![],
                OptionTypeClause::TypeClause(type_clause) => {
                    super::size::get_field_size_from_type_clause(db, &type_clause)
                }
            })
            .collect::<Vec<_>>();

        if variant_sizes.is_empty() {
            (vec![], true)
        } else {
            let v0 = variant_sizes[0].clone();
            let identical_variants = variant_sizes.iter().all(|vs| *vs == v0);

            (variant_sizes, identical_variants)
        }
    }

    pub fn compute_enum_layout_size(
        &self,
        variant_sizes: &[Vec<String>],
        identical_variants: bool,
    ) -> String {
        if variant_sizes.is_empty() {
            return "Option::None".to_string();
        }

        let mut sizes = if identical_variants {
            // 1 felt252 for the variant identifier
            let mut sizes = vec!["Some(1)".to_string()];
            sizes.extend(variant_sizes[0].clone());
            sizes
        } else {
            vec![]
        };

        super::size::build_size_function_body(&mut sizes)
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

        if variant_layout.iter().any(|v| super::layout::is_custom_layout(v.as_str())) {
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
            // with the new `DojoStore`` trait, variants start from 1, to be able to use
            // 0 as uninitialized variant.
            let selector = i + 1;

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
        println!("building enum: {}", name);

        let variants = enum_ast.variants(db).elements(db);

        let variants_ty = if variants.is_empty() {
            "".to_string()
        } else {
            variants.iter().map(|v| self.build_variant_ty(db, v)).collect::<Vec<_>>().join(",\n")
        };

        println!("finish building enum: {name}");
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

        println!("building enum variant: {}", name);

        match variant.type_clause(db) {
            OptionTypeClause::Empty(_) => {
                println!("it's giving empty");
                // use an empty tuple if the variant has no data
                format!("('{name}', dojo::meta::introspect::Ty::Tuple(array![].span()))")
            }
            OptionTypeClause::TypeClause(type_clause) => {
                println!("it's giving type clause");
                format!("('{name}', {})", super::ty::build_ty_from_type_clause(db, &type_clause))
            }
        }
    }
}
