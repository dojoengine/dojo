use std::collections::HashSet;

use cairo_lang_macro::{quote, Diagnostic, ProcMacroResult, TokenStream};
use cairo_lang_parser::utils::SimpleParserDatabase;
use cairo_lang_syntax::node::helpers::QueryAttrs;
use cairo_lang_syntax::node::{ast, TypedSyntaxNode};

use crate::constants::{
    DOJO_INTROSPECT_DERIVE, DOJO_LEGACY_STORAGE_DERIVE, DOJO_PACKED_DERIVE, DOJO_STORE_DERIVE,
    EXPECTED_DERIVE_ATTR_NAMES,
};
use crate::helpers::{
    self, get_serialization_path_and_prefix, DiagnosticsExt, DojoChecker, DojoFormatter,
    DojoParser, DojoTokenizer, Member, ProcMacroResultExt,
};

#[derive(Debug)]
pub struct DojoModel {
    diagnostics: Vec<Diagnostic>,
    model_type: String,
    model_value_derive_attr_names: Vec<String>,
    members_values: Vec<String>,
    key_type: String,
    keys_to_tuple: String,
    serialized_keys: Vec<String>,
    serialized_values: Vec<String>,
    unique_hash: String,
    model_layout: String,
    use_legacy_storage: bool,
    model_deserialize_path: String,
    model_deserialize_prefix: String,
    deserialized_keys: Vec<String>,
    deserialized_values: Vec<String>,
    deserialized_modelvalue: String,
    deserialize_body: String,
}

impl DojoModel {
    pub fn new() -> Self {
        Self {
            diagnostics: vec![],
            model_type: String::default(),
            model_value_derive_attr_names: vec![],
            members_values: vec![],
            key_type: String::default(),
            keys_to_tuple: String::default(),
            serialized_keys: vec![],
            serialized_values: vec![],
            unique_hash: String::default(),
            model_layout: String::default(),
            use_legacy_storage: false,
            model_deserialize_path: String::default(),
            model_deserialize_prefix: String::default(),
            deserialized_keys: vec![],
            deserialized_values: vec![],
            deserialized_modelvalue: String::default(),
            deserialize_body: String::default(),
        }
    }
    pub fn process(token_stream: TokenStream) -> ProcMacroResult {
        let db = SimpleParserDatabase::default();

        if let Some(struct_ast) = DojoParser::parse_and_find_struct(&db, &token_stream) {
            return DojoModel::process_ast(&db, &struct_ast);
        }

        ProcMacroResult::fail("'dojo::model' must be used on struct only.".to_string())
    }

    fn process_ast(db: &SimpleParserDatabase, struct_ast: &ast::ItemStruct) -> ProcMacroResult {
        let mut model = DojoModel::new();

        model.model_type = struct_ast.name(db).as_syntax_node().get_text(db).trim().to_string();

        if let Some(failure) = DojoChecker::is_name_valid("model", &model.model_type) {
            return failure;
        }

        // generic models are not allowed
        if let Some(failure) = DojoChecker::is_struct_generic("model", db, struct_ast) {
            return failure;
        }

        let derive_attr_names = DojoParser::extract_derive_attr_names(
            db,
            &mut model.diagnostics,
            struct_ast.attributes(db).query_attr(db, "derive"),
        );

        model.use_legacy_storage =
            derive_attr_names.contains(&DOJO_LEGACY_STORAGE_DERIVE.to_string());

        let mut values: Vec<Member> = vec![];
        let mut keys: Vec<Member> = vec![];
        let mut key_types: Vec<String> = vec![];
        let mut key_attrs: Vec<String> = vec![];

        // The impl constraint for a model `MemberStore` must be defined for each member type.
        // To avoid double, we keep track of the processed types to skip the double impls.
        let mut model_member_store_impls_processed: HashSet<String> = HashSet::new();
        let mut model_member_store_impls: Vec<String> = vec![];

        let members = DojoParser::parse_members(
            db,
            &struct_ast.members(db).elements(db),
            &mut model.diagnostics,
        );

        DojoFormatter::serialize_keys_and_values(
            db,
            &struct_ast.members(db).elements(db),
            &mut model.serialized_keys,
            &mut model.serialized_values,
            model.use_legacy_storage,
        );

        struct_ast.members(db).elements(db).iter().for_each(|member_ast| {
            if member_ast.has_attr(db, "key") {
                model
                    .deserialized_keys
                    .push(DojoFormatter::deserialize_member_ty(db, member_ast, true, "keys"));
            } else {
                model.deserialized_values.push(DojoFormatter::deserialize_member_ty(
                    db,
                    member_ast,
                    model.use_legacy_storage,
                    "values",
                ));
            }
        });

        members.iter().for_each(|member| {
            if member.key {
                keys.push(member.clone());
                key_types.push(member.ty.clone());
                key_attrs.push(format!("*self.{}", member.name.clone()));
            } else {
                values.push(member.clone());
                model
                    .members_values
                    .push(DojoFormatter::get_member_declaration(&member.name, &member.ty));

                if !model_member_store_impls_processed.contains(&member.ty.to_string()) {
                    model_member_store_impls.extend(vec![
                        format!(
                            "+dojo::model::storage::MemberModelStorage<S, {}, {}>",
                            model.model_type, member.ty
                        ),
                        format!(
                            "+dojo::model::storage::MemberModelStorage<S, {}Value, {}>",
                            model.model_type, member.ty
                        ),
                        format!(
                            "+dojo::model::members::MemberStore::<S, {}Value, {}>",
                            model.model_type, member.ty
                        ),
                    ]);

                    model_member_store_impls_processed.insert(member.ty.to_string());
                }
            }
        });

        if keys.is_empty() {
            model.diagnostics.push_error("Model must define at least one #[key] attribute".into());
        }

        if values.is_empty() {
            model
                .diagnostics
                .push_error("Model must define at least one member that is not a key".into());
        }

        if !model.diagnostics.is_empty() {
            return ProcMacroResult::fail_with_diagnostics(model.diagnostics);
        }

        (model.keys_to_tuple, model.key_type) = if keys.len() > 1 {
            (format!("({})", key_attrs.join(", ")), format!("({})", key_types.join(", ")))
        } else {
            (key_attrs.first().unwrap().to_string(), key_types.first().unwrap().to_string())
        };

        // Build the list of derive attributes to set on "ModelValue" struct.
        model.model_value_derive_attr_names = derive_attr_names
            .iter()
            .map(|d| d.to_string())
            .filter(|d| d != DOJO_INTROSPECT_DERIVE && d != DOJO_PACKED_DERIVE)
            .collect::<Vec<String>>();

        let mut missing_derive_attr_names = vec![];

        // If Introspect or IntrospectPacked derive attribute is not set for the model,
        // use Introspect by default.
        if derive_attr_names.contains(&DOJO_PACKED_DERIVE.to_string()) {
            missing_derive_attr_names.push(DOJO_PACKED_DERIVE.to_string());
        } else {
            missing_derive_attr_names.push(DOJO_INTROSPECT_DERIVE.to_string());
        }

        // If DojoStore derive attribute is not set for the model, add it.
        if !derive_attr_names.contains(&DOJO_STORE_DERIVE.to_string()) {
            missing_derive_attr_names.push(DOJO_STORE_DERIVE.to_string());
        }

        // Add missing expected derive attributes for "Model" struct.
        EXPECTED_DERIVE_ATTR_NAMES.iter().for_each(|expected_attr| {
            let attr = expected_attr.to_string();

            if !derive_attr_names.contains(&attr) {
                missing_derive_attr_names.push(attr.clone());
                model.model_value_derive_attr_names.push(attr);
            }
        });

        let is_packed = derive_attr_names.contains(&DOJO_PACKED_DERIVE.to_string());

        model.unique_hash = helpers::compute_unique_hash(
            db,
            &model.model_type,
            is_packed,
            &struct_ast.members(db).elements(db),
        )
        .to_string();

        let value_names = values.iter().map(|v| v.name.clone()).collect::<Vec<_>>().join(",\n");

        model.deserialized_modelvalue = format!(
            "Option::Some({}Value {{
                {value_names}
            }})",
            model.model_type
        );

        (model.model_deserialize_path, model.model_deserialize_prefix) =
            get_serialization_path_and_prefix(model.use_legacy_storage);

        model.model_layout = if model.use_legacy_storage {
            format!(
                "dojo::meta::layout::build_legacy_layout(
                    dojo::meta::Introspect::<{}>::layout()
                )",
                model.model_type
            )
        } else {
            format!("dojo::meta::Introspect::<{}>::layout()", model.model_type)
        };

        model.deserialize_body = if model.use_legacy_storage {
            format!(
                "
                let mut serialized: Array<felt252> = keys.into();
                serialized.append_span(values);
                let mut data = serialized.span();

                core::serde::Serde::<{model_type}>::deserialize(ref data)
                ",
                model_type = model.model_type,
            )
        } else {
            format!(
                "
            {deserialized_keys}
            {deserialized_values}

            Some({model_type} {{
                {keys},
                {values},
            }})
            ",
                deserialized_keys = model.deserialized_keys.join("\n"),
                deserialized_values = model.deserialized_values.join("\n"),
                model_type = model.model_type,
                keys = keys.iter().map(|k| k.name.clone()).collect::<Vec<_>>().join(",\n"),
                values = values.iter().map(|v| v.name.clone()).collect::<Vec<_>>().join(",\n"),
            )
        };

        let model_code = model.generate_model_code();

        let original_struct = DojoTokenizer::rebuild_original_struct(db, struct_ast);

        let missing_derive_attr = if missing_derive_attr_names.is_empty() {
            DojoTokenizer::tokenize("")
        } else {
            DojoTokenizer::tokenize(&format!("#[derive({})]", missing_derive_attr_names.join(", ")))
        };

        ProcMacroResult::finalize(
            quote! {
                // original struct with missing derive attributes
                #missing_derive_attr
                #original_struct

                // model
                #model_code
            },
            model.diagnostics,
        )
    }

    fn generate_model_code(&self) -> TokenStream {
        let (
            model_type,
            model_value_derive_attr_names,
            members_values,
            key_type,
            keys_to_tuple,
            serialized_keys,
            serialized_values,
            unique_hash,
            model_layout,
            use_legacy_storage,
            model_deserialize_path,
            model_deserialize_prefix,
            deserialized_values,
            deserialized_modelvalue,
            deserialize_body,
        ) = (
            &self.model_type,
            format!("#[derive({})]", self.model_value_derive_attr_names.join(", ")),
            self.members_values.join(""),
            &self.key_type,
            &self.keys_to_tuple,
            self.serialized_keys.join(""),
            self.serialized_values.join(""),
            &self.unique_hash,
            &self.model_layout,
            self.use_legacy_storage,
            &self.model_deserialize_path,
            &self.model_deserialize_prefix,
            &self.deserialized_values.join(""),
            &self.deserialized_modelvalue,
            &self.deserialize_body,
        );

        let content = format!(
            "{model_value_derive_attr_names}
pub struct {model_type}Value {{
    {members_values}
}}

type {model_type}KeyType = {key_type};

pub impl {model_type}KeyParser of dojo::model::model::KeyParser<{model_type}, {model_type}KeyType> \
             {{
    #[inline(always)]
    fn parse_key(self: @{model_type}) -> {model_type}KeyType {{
        {keys_to_tuple}
    }}
}}

impl {model_type}ModelValueKey of dojo::model::model_value::ModelValueKey<{model_type}Value, \
             {model_type}KeyType> {{
}}

// Impl to get the static definition of a model
pub mod m_{model_type}_definition {{
    use super::{model_type};
    pub impl {model_type}DefinitionImpl<T> of dojo::model::ModelDefinition<T>{{
        #[inline(always)]
        fn name() -> ByteArray {{
            \"{model_type}\"
        }}

        #[inline(always)]
        fn layout() -> dojo::meta::Layout {{
            {model_layout}
        }}

        #[inline(always)]
        fn use_legacy_storage() -> bool {{
            {use_legacy_storage}
        }}

        #[inline(always)]
        fn schema() -> dojo::meta::introspect::Struct {{
            if let dojo::meta::introspect::Ty::Struct(s) = \
             dojo::meta::Introspect::<{model_type}>::ty() {{
                s
            }}
            else {{
                panic!(\"Model {model_type}: invalid schema.\")
            }}
        }}

        #[inline(always)]
        fn size() -> Option<usize> {{
            dojo::meta::Introspect::<{model_type}>::size()
        }}
    }}
}}

pub impl {model_type}Definition = \
             m_{model_type}_definition::{model_type}DefinitionImpl<{model_type}>;
pub impl {model_type}ModelValueDefinition = \
             m_{model_type}_definition::{model_type}DefinitionImpl<{model_type}Value>;

pub impl {model_type}ModelParser of dojo::model::model::ModelParser<{model_type}> {{
    fn deserialize(ref keys: Span<felt252>, ref values: Span<felt252>) -> Option<{model_type}> {{
        {deserialize_body}
    }}
    fn serialize_keys(self: @{model_type}) -> Span<felt252> {{
        let mut serialized = core::array::ArrayTrait::new();
        {serialized_keys}
        core::array::ArrayTrait::span(@serialized)
    }}
    fn serialize_values(self: @{model_type}) -> Span<felt252> {{
        let mut serialized = core::array::ArrayTrait::new();
        {serialized_values}
        core::array::ArrayTrait::span(@serialized)
    }}
}}

pub impl {model_type}ModelValueParser of \
             dojo::model::model_value::ModelValueParser<{model_type}Value> {{
    fn deserialize(ref values: Span<felt252>) -> Option<{model_type}Value> {{
    {model_deserialize_path}::<{model_type}Value>::{model_deserialize_prefix}deserialize(ref \
             values)
    }}
    fn serialize_values(self: @{model_type}Value) -> Span<felt252> {{
        let mut serialized = core::array::ArrayTrait::new();
        {serialized_values}
        core::array::ArrayTrait::span(@serialized)
    }}
}}

// Note that {model_type}DojoStore is implemented through the DojoStore derive attribute
// as any structs.

pub impl {model_type}ValueDojoStore of dojo::storage::DojoStore<{model_type}Value> {{
    fn dojo_serialize(self: @{model_type}Value, ref serialized: Array<felt252>) {{
        {serialized_values}
    }}
    fn dojo_deserialize(ref values: Span<felt252>) -> Option<{model_type}Value> {{
        {deserialized_values}
        {deserialized_modelvalue}
    }}
}}

pub impl {model_type}ModelImpl = dojo::model::model::ModelImpl<{model_type}>;
pub impl {model_type}ModelValueImpl = dojo::model::model_value::ModelValueImpl<{model_type}Value>;

#[starknet::contract]
pub mod m_{model_type} {{
    use super::{model_type};
    use super::{model_type}Value;

    #[storage]
    struct Storage {{}}

    #[abi(embed_v0)]
    impl {model_type}__DojoDeployedModelImpl = \
             dojo::model::component::IDeployedModelImpl<ContractState, {model_type}>;

    #[abi(embed_v0)]
    impl {model_type}__DojoStoredModelImpl = \
             dojo::model::component::IStoredModelImpl<ContractState, {model_type}>;

    #[abi(embed_v0)]
    impl {model_type}__DojoModelImpl = dojo::model::component::IModelImpl<ContractState, \
             {model_type}>;

    #[abi(per_item)]
    #[generate_trait]
    impl {model_type}Impl of I{model_type} {{
        // Ensures the ABI contains the Model struct, even if never used
        // into as a system input.
        #[external(v0)]
        fn ensure_abi(self: @ContractState, model: {model_type}) {{
            let _model = model;
        }}

        // Outputs ModelValue to allow a simple diff from the ABI compared to the
        // model to retrieved the keys of a model.
        #[external(v0)]
        fn ensure_values(self: @ContractState, value: {model_type}Value) {{
            let _value = value;
        }}

        // Ensures the generated contract has a unique classhash, using
        // a hardcoded hash computed on model and member names.
        #[external(v0)]
        fn ensure_unique(self: @ContractState) {{
            let _hash = {unique_hash};
        }}
    }}
}}"
        );

        TokenStream::new(vec![DojoTokenizer::tokenize(&content)])
    }
}
