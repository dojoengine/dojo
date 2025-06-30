use cairo_lang_macro::{Diagnostic, ProcMacroResult, TokenStream};
use cairo_lang_parser::utils::SimpleParserDatabase;
use cairo_lang_syntax::node::ast::{ItemEnum, OptionTypeClause, Variant};
use cairo_lang_syntax::node::helpers::QueryAttrs;
use cairo_lang_syntax::node::{Terminal, TypedSyntaxNode};

use crate::helpers::{debug_store_expand, DiagnosticsExt, DojoChecker, ProcMacroResultExt};

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

        let gen_types = super::generics::build_generic_types(db, enum_ast.generic_params(db));
        let gen_joined_types = gen_types.join(", ");

        let enum_name_with_generics = format!("{enum_name}<{gen_joined_types}>");

        let inspect_gen_impls = super::generics::build_generic_impls(
            &gen_types,
            &["+dojo::meta::introspect::Introspect".to_string()],
            &[],
        );
        let dojo_store_gen_impls = super::generics::build_generic_impls(
            &gen_types,
            &["+dojo::storage::DojoStore".to_string(), "+core::serde::Serde".to_string()],
            &[format!("+core::traits::Default<{enum_name_with_generics}>")],
        );

        let enum_size = self.compute_enum_layout_size(&variant_sizes, is_packed);
        let ty = self.build_enum_ty(db, &enum_name, enum_ast);

        let dojo_store = Self::build_enum_dojo_store(
            db,
            &enum_name,
            enum_ast,
            &gen_types,
            &dojo_store_gen_impls,
        );

        debug_store_expand(&format!("DOJO_STORE ENUM::{enum_name}"), &dojo_store);

        super::generate_introspect(
            &enum_name,
            &enum_size,
            &gen_types,
            inspect_gen_impls,
            &layout,
            &ty,
            &dojo_store,
        )
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
        let identical_variants =
            variant_sizes.iter().all(|vs| vs.0 == v0.0 && vs.1 == v0.1 && vs.2 == v0.2);

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
        let variants = enum_ast.variants(db).elements(db);

        let variants_ty = if variants.is_empty() {
            "".to_string()
        } else {
            variants.iter().map(|v| self.build_variant_ty(db, v)).collect::<Vec<_>>().join(",\n")
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
                format!("('{name}', {})", super::ty::build_ty_from_type_clause(db, &type_clause))
            }
        }
    }

    pub fn build_enum_dojo_store(
        db: &SimpleParserDatabase,
        name: &String,
        enum_ast: &ItemEnum,
        generic_types: &[String],
        generic_impls: &String,
    ) -> String {
        let mut serialized_variants = vec![];
        let mut deserialized_variants = vec![];

        for (index, variant) in enum_ast.variants(db).elements(db).iter().enumerate() {
            let variant_name = variant.name(db).text(db).to_string();
            let full_variant_name = format!("{name}::{variant_name}");
            let variant_index = index + 1;

            let (serialized_variant, deserialized_variant) = match variant.type_clause(db) {
                OptionTypeClause::TypeClause(ty) => {
                    let ty = ty.ty(db).as_syntax_node().get_text_without_trivia(db);

                    let serialized = format!(
                        "{full_variant_name}(d) => {{
                            serialized.append({variant_index});
                            dojo::storage::DojoStore::serialize(d, ref serialized);
                        }},"
                    );

                    let deserialized = format!(
                        "{variant_index} => {{
                            let variant_data = dojo::storage::DojoStore::<{ty}>::deserialize(ref \
                         values)?;
                            Option::Some({full_variant_name}(variant_data))
                        }},",
                    );

                    (serialized, deserialized)
                }
                OptionTypeClause::Empty(_) => {
                    let serialized = format!(
                        "{full_variant_name} => {{ serialized.append({variant_index}); }},"
                    );
                    let deserialized =
                        format!("{variant_index} => Option::Some({full_variant_name}),",);

                    (serialized, deserialized)
                }
            };

            serialized_variants.push(serialized_variant);
            deserialized_variants.push(deserialized_variant);
        }

        let serialized_variants = serialized_variants.join("\n");
        let deserialized_variants = deserialized_variants.join("\n");

        let generic_params = if generic_types.is_empty() {
            "".to_string()
        } else {
            format!("<{}>", generic_types.join(", "))
        };

        let impl_decl = if generic_types.is_empty() {
            format!("impl {name}DojoStore of dojo::storage::DojoStore<{name}>")
        } else {
            format!(
                "impl {name}DojoStore<{generic_impls}> of \
                 dojo::storage::DojoStore<{name}{generic_params}>"
            )
        };

        format!(
            "{impl_decl} {{
        fn serialize(self: @{name}{generic_params}, ref serialized: Array<felt252>) {{
            match self {{
                {serialized_variants}
            }};
        }}
        fn deserialize(ref values: Span<felt252>) -> Option<{name}{generic_params}> {{
            let variant = *values.pop_front()?;
            match variant {{
                0 => Option::Some(Default::<{name}{generic_params}>::default()),
                {deserialized_variants}
                _ => Option::None,
            }}
        }}
    }}"
        )
    }
}
