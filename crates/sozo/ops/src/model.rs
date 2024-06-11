use anyhow::Result;
use cainome::cairo_serde::{ByteArray, CairoSerde};
use dojo_types::schema::Ty;
use dojo_world::contracts::model::ModelReader;
use dojo_world::contracts::world::WorldContractReader;
use starknet::core::types::{BlockId, BlockTag, FieldElement};
use starknet::core::utils::get_selector_from_name;
use starknet::providers::jsonrpc::HttpTransport;
use starknet::providers::JsonRpcClient;

const INDENT: &str = "    ";

pub async fn model_class_hash(
    name: String,
    world_address: FieldElement,
    provider: JsonRpcClient<HttpTransport>,
) -> Result<()> {
    let mut world_reader = WorldContractReader::new(world_address, &provider);
    world_reader.set_block(BlockId::Tag(BlockTag::Pending));

    let model = world_reader.model_reader(&name).await?;

    println!("{:#x}", model.class_hash());

    Ok(())
}

pub async fn model_contract_address(
    name: String,
    world_address: FieldElement,
    provider: JsonRpcClient<HttpTransport>,
) -> Result<()> {
    let mut world_reader = WorldContractReader::new(world_address, &provider);
    world_reader.set_block(BlockId::Tag(BlockTag::Pending));

    let model = world_reader.model_reader(&name).await?;

    println!("{:#x}", model.contract_address());

    Ok(())
}

pub async fn model_layout(
    name: String,
    world_address: FieldElement,
    provider: JsonRpcClient<HttpTransport>,
) -> Result<()> {
    let mut world_reader = WorldContractReader::new(world_address, &provider);
    world_reader.set_block(BlockId::Tag(BlockTag::Pending));

    let model = world_reader.model_reader(&name).await?;
    let layout = match model.layout().await {
        Ok(x) => x,
        Err(_) => anyhow::bail!(
            "[Incorrect layout]\nThe model is packed but contains at least one custom type field \
             which is not packed.\nPlease check your model to fix this."
        ),
    };
    let schema = model.schema().await?;

    deep_print_layout(&name, &layout, &schema);

    Ok(())
}

pub async fn model_schema(
    name: String,
    world_address: FieldElement,
    provider: JsonRpcClient<HttpTransport>,
    to_json: bool,
) -> Result<()> {
    let mut world_reader = WorldContractReader::new(world_address, &provider);
    world_reader.set_block(BlockId::Tag(BlockTag::Pending));

    let model = world_reader.model_reader(&name).await?;
    let schema = model.schema().await?;

    if to_json {
        println!("{}", serde_json::to_string_pretty(&schema)?)
    } else {
        deep_print_ty(schema);
    }

    Ok(())
}

pub async fn model_get(
    name: String,
    keys: Vec<FieldElement>,
    world_address: FieldElement,
    provider: JsonRpcClient<HttpTransport>,
) -> Result<()> {
    if keys.is_empty() {
        anyhow::bail!("Models always have at least one key. Please provide it (or them).");
    }

    let mut world_reader = WorldContractReader::new(world_address, &provider);
    world_reader.set_block(BlockId::Tag(BlockTag::Pending));

    let model = world_reader.model_reader(&name).await?;
    let schema = model.schema().await?;
    let values = model.entity_storage(&keys).await?;

    deep_print_record(&schema, &keys, &values);

    Ok(())
}

#[derive(Clone, Debug)]
struct LayoutInfo {
    layout_type: LayoutInfoType,
    name: String,
    fields: Vec<FieldLayoutInfo>,
}

#[derive(Clone, Debug)]
enum LayoutInfoType {
    Struct,
    Enum,
    Tuple,
    Array,
}

#[derive(Clone, Debug)]
struct FieldLayoutInfo {
    selector: String,
    name: String,
    layout: String,
}

fn format_fixed(layout: &[u8]) -> String {
    format!("[{}]", layout.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(", "))
}

fn format_layout_ref(type_name: &str) -> String {
    format!("layout({type_name})")
}

fn format_selector(selector: String) -> String {
    if selector.starts_with("0x") { format!("[{}]", selector) } else { selector }
}

fn format_name(name: String) -> String {
    if !name.is_empty() { format!(" {} ", name) } else { name }
}

fn format_field(selector: String, name: String, layout: String) -> String {
    let layout = if layout.eq("[]") { "".to_string() } else { format!(": {layout}") };

    format!("{INDENT}{:<20}{:<18}{}", format_selector(selector), format_name(name), layout)
}

fn format_field_layout(
    layout: &dojo_world::contracts::model::abigen::model::Layout,
    schema: &dojo_types::schema::Ty,
) -> String {
    match layout {
        dojo_world::contracts::model::abigen::model::Layout::Fixed(x) => format_fixed(x),
        dojo_world::contracts::model::abigen::model::Layout::ByteArray => {
            "layout(ByteArray)".to_string()
        }
        _ => format_layout_ref(&get_name_from_schema(schema)),
    }
}

fn is_layout_in_list(list: &[LayoutInfo], name: &String) -> bool {
    list.iter().any(|x| x.name.eq(name))
}

fn get_name_from_schema(schema: &dojo_types::schema::Ty) -> String {
    match schema {
        dojo_types::schema::Ty::Struct(s) => s.name.clone(),
        dojo_types::schema::Ty::Enum(e) => e.name.clone(),
        dojo_types::schema::Ty::Primitive(p) => match p {
            dojo_types::primitive::Primitive::U8(_) => "u8".to_string(),
            dojo_types::primitive::Primitive::U16(_) => "u16".to_string(),
            dojo_types::primitive::Primitive::U32(_) => "u32".to_string(),
            dojo_types::primitive::Primitive::U64(_) => "u64".to_string(),
            dojo_types::primitive::Primitive::U128(_) => "u128".to_string(),
            dojo_types::primitive::Primitive::U256(_) => "u256".to_string(),
            dojo_types::primitive::Primitive::USize(_) => "usize".to_string(),
            dojo_types::primitive::Primitive::Bool(_) => "bool".to_string(),
            dojo_types::primitive::Primitive::Felt252(_) => "felt252".to_string(),
            dojo_types::primitive::Primitive::ClassHash(_) => "ClassHash".to_string(),
            dojo_types::primitive::Primitive::ContractAddress(_) => "ContractAddress".to_string(),
        },
        dojo_types::schema::Ty::Tuple(t) => {
            format!("({})", t.iter().map(get_name_from_schema).collect::<Vec<_>>().join(", "))
        }
        dojo_types::schema::Ty::Array(a) => format!("Array<{}>", get_name_from_schema(&a[0])),
        _ => "".to_string(),
    }
}

fn get_printable_layout_list_from_struct(
    field_layouts: &[dojo_world::contracts::model::abigen::model::FieldLayout],
    schema: &dojo_types::schema::Ty,
    layout_list: &mut Vec<LayoutInfo>,
) {
    if let dojo_types::schema::Ty::Struct(ss) = schema {
        let name = get_name_from_schema(schema);

        // process main struct
        if !is_layout_in_list(layout_list, &name) {
            layout_list.push(LayoutInfo {
                layout_type: LayoutInfoType::Struct,
                name,
                fields: field_layouts
                    .iter()
                    .zip(ss.children.iter().filter(|x| !x.key))
                    .map(|(l, m)| FieldLayoutInfo {
                        selector: format!("{:#x}", l.selector),
                        name: m.name.clone(),
                        layout: format_field_layout(&l.layout, &m.ty),
                    })
                    .collect::<Vec<_>>(),
            });
        }

        // process members
        for (member_layout, member) in
            field_layouts.iter().zip(ss.children.iter().filter(|x| !x.key))
        {
            get_printable_layout_list(&member_layout.layout, &member.ty, layout_list);
        }
    };
}

fn get_printable_layout_list_from_enum(
    field_layouts: &[dojo_world::contracts::model::abigen::model::FieldLayout],
    schema: &dojo_types::schema::Ty,
    layout_list: &mut Vec<LayoutInfo>,
) {
    if let dojo_types::schema::Ty::Enum(se) = schema {
        let name = get_name_from_schema(schema);

        // proces main enum
        if !is_layout_in_list(layout_list, &name) {
            layout_list.push(LayoutInfo {
                layout_type: LayoutInfoType::Enum,
                name,
                fields: field_layouts
                    .iter()
                    .zip(se.options.iter())
                    .map(|(l, o)| FieldLayoutInfo {
                        selector: format!("{:#x}", l.selector),
                        name: o.name.to_string(),
                        layout: format_field_layout(&l.layout, &o.ty),
                    })
                    .collect::<Vec<_>>(),
            });
        }

        // process variants
        for (variant_layout, variant) in field_layouts.iter().zip(se.options.iter()) {
            get_printable_layout_list(&variant_layout.layout, &variant.ty, layout_list);
        }
    }
}

fn get_printable_layout_list_from_tuple(
    item_layouts: &[dojo_world::contracts::model::abigen::model::Layout],
    schema: &dojo_types::schema::Ty,
    layout_list: &mut Vec<LayoutInfo>,
) {
    if let dojo_types::schema::Ty::Tuple(st) = schema {
        let name = get_name_from_schema(schema);

        // process tuple
        if !is_layout_in_list(layout_list, &name) {
            layout_list.push(LayoutInfo {
                layout_type: LayoutInfoType::Tuple,
                name,
                fields: item_layouts
                    .iter()
                    .enumerate()
                    .zip(st.iter())
                    .map(|((i, l), s)| FieldLayoutInfo {
                        selector: format!("{:#x}", i),
                        name: "".to_string(),
                        layout: format_field_layout(l, s),
                    })
                    .collect::<Vec<_>>(),
            });
        }

        // process tuple items
        for (item_layout, item_schema) in item_layouts.iter().zip(st.iter()) {
            get_printable_layout_list(item_layout, item_schema, layout_list);
        }
    }
}

fn get_printable_layout_list_from_array(
    item_layout: &dojo_world::contracts::model::abigen::model::Layout,
    schema: &dojo_types::schema::Ty,
    layout_list: &mut Vec<LayoutInfo>,
) {
    if let dojo_types::schema::Ty::Array(sa) = schema {
        let name = get_name_from_schema(schema);

        // process array
        if !is_layout_in_list(layout_list, &name) {
            layout_list.push(LayoutInfo {
                layout_type: LayoutInfoType::Array,
                name,
                fields: vec![FieldLayoutInfo {
                    selector: "[ItemIndex]".to_string(),
                    name: "".to_string(),
                    layout: format_field_layout(item_layout, &sa[0]),
                }],
            });
        }

        // process array item
        get_printable_layout_list(item_layout, &sa[0], layout_list);
    }
}

fn get_printable_layout_list(
    root_layout: &dojo_world::contracts::model::abigen::model::Layout,
    schema: &dojo_types::schema::Ty,
    layout_list: &mut Vec<LayoutInfo>,
) {
    match root_layout {
        dojo_world::contracts::model::abigen::model::Layout::Struct(ls) => {
            get_printable_layout_list_from_struct(ls, schema, layout_list);
        }
        dojo_world::contracts::model::abigen::model::Layout::Enum(le) => {
            get_printable_layout_list_from_enum(le, schema, layout_list);
        }
        dojo_world::contracts::model::abigen::model::Layout::Tuple(lt) => {
            get_printable_layout_list_from_tuple(lt, schema, layout_list);
        }
        dojo_world::contracts::model::abigen::model::Layout::Array(la) => {
            get_printable_layout_list_from_array(&la[0], schema, layout_list);
        }
        _ => {}
    };
}

fn print_layout_info(layout_info: LayoutInfo) {
    let fields = layout_info
        .fields
        .into_iter()
        .map(|f| format_field(f.selector, f.name, f.layout))
        .collect::<Vec<_>>();
    let layout_title = match layout_info.layout_type {
        LayoutInfoType::Struct => format!("Struct {} {{", layout_info.name),

        LayoutInfoType::Enum => {
            format!("{:<42}: [251] (variant id)", format!("Enum {} {{", layout_info.name))
        }
        LayoutInfoType::Tuple => format!("{} (", layout_info.name),
        LayoutInfoType::Array => {
            format!("{:<42}: [32] (length)", format!("{} (", layout_info.name))
        }
    };
    let end_token = match layout_info.layout_type {
        LayoutInfoType::Struct => '}',
        LayoutInfoType::Enum => '}',
        LayoutInfoType::Tuple => ')',
        LayoutInfoType::Array => ')',
    };

    println!(
        "{layout_title}
{}
{end_token}\n",
        fields.join("\n")
    );
}

// print the full Layout tree
fn deep_print_layout(
    name: &String,
    layout: &dojo_world::contracts::model::abigen::model::Layout,
    schema: &dojo_types::schema::Ty,
) {
    if let dojo_world::contracts::model::abigen::model::Layout::Fixed(lf) = layout {
        println!("\n{} (packed)", name);
        println!("    selector : {:#x}", get_selector_from_name(name).unwrap());
        println!("    layout   : {}", format_fixed(lf));
    } else {
        let mut layout_list = vec![];
        get_printable_layout_list(layout, schema, &mut layout_list);

        println!("\n{} selector: {:#x}\n", name, get_selector_from_name(name).unwrap());

        for l in layout_list {
            print_layout_info(l);
        }
    }
}

fn _start_indent(level: usize, start_indent: bool) -> String {
    if start_indent { INDENT.repeat(level) } else { "".to_string() }
}

fn format_primitive(
    p: &dojo_types::primitive::Primitive,
    values: &mut Vec<FieldElement>,
    level: usize,
    start_indent: bool,
) -> String {
    let mut _p = *p;
    let _ = _p.deserialize(values);

    format!("{}{}", _start_indent(level, start_indent), _p.to_sql_value().unwrap())
}

fn format_byte_array(values: &mut Vec<FieldElement>, level: usize, start_indent: bool) -> String {
    let bytearray = ByteArray::cairo_deserialize(values, 0).unwrap();
    values.drain(0..ByteArray::cairo_serialized_size(&bytearray));

    format!("{}{}", _start_indent(level, start_indent), ByteArray::to_string(&bytearray).unwrap())
}

fn format_field_value(
    member: &dojo_types::schema::Member,
    values: &mut Vec<FieldElement>,
    level: usize,
) -> String {
    let field_repr = format_record_value(&member.ty, values, level, false);
    format!("{}{:<16}: {field_repr}", INDENT.repeat(level), member.name)
}

fn format_array(
    item: &dojo_types::schema::Ty,
    values: &mut Vec<FieldElement>,
    level: usize,
    start_indent: bool,
) -> String {
    let length: u32 = values.remove(0).try_into().unwrap();
    let mut items = vec![];

    for _ in 0..length {
        items.push(format_record_value(item, values, level + 1, true));
    }

    format!(
        "{}[\n{}\n{}]",
        _start_indent(level, start_indent),
        items.join(",\n"),
        INDENT.repeat(level)
    )
}

fn format_tuple(
    items: &[dojo_types::schema::Ty],
    values: &mut Vec<FieldElement>,
    level: usize,
    start_indent: bool,
) -> String {
    if items.is_empty() {
        return "".to_string();
    }

    let items_repr = items
        .iter()
        .map(|x| format_record_value(x, values, level + 1, true))
        .collect::<Vec<_>>()
        .join(",\n");

    format!("{}(\n{}\n{})", _start_indent(level, start_indent), items_repr, INDENT.repeat(level))
}

fn format_struct(
    schema: &dojo_types::schema::Struct,
    values: &mut Vec<FieldElement>,
    level: usize,
    start_indent: bool,
) -> String {
    let fields = schema
        .children
        .iter()
        .map(|m| format_field_value(m, values, level + 1))
        .collect::<Vec<_>>();

    format!(
        "{}{{\n{}\n{}}}",
        _start_indent(level, start_indent),
        fields.join(",\n"),
        INDENT.repeat(level)
    )
}

fn format_enum(
    schema: &dojo_types::schema::Enum,
    values: &mut Vec<FieldElement>,
    level: usize,
    start_indent: bool,
) -> String {
    let variant_index: u8 = values.remove(0).try_into().unwrap();
    let variant_index: usize = variant_index.into();
    let variant_name = format!("{}::{}", schema.name, schema.options[variant_index].name);
    let variant_data =
        format_record_value(&schema.options[variant_index].ty, values, level + 1, true);

    if variant_data.is_empty() {
        format!("{}{variant_name}", _start_indent(level, start_indent),)
    } else {
        format!(
            "{}{variant_name}(\n{}\n{})",
            _start_indent(level, start_indent),
            variant_data,
            INDENT.repeat(level)
        )
    }
}

fn format_record_value(
    schema: &dojo_types::schema::Ty,
    values: &mut Vec<FieldElement>,
    level: usize,
    start_indent: bool,
) -> String {
    match schema {
        dojo_types::schema::Ty::Primitive(p) => format_primitive(p, values, level, start_indent),
        dojo_types::schema::Ty::ByteArray(_) => format_byte_array(values, level, start_indent),
        dojo_types::schema::Ty::Struct(s) => format_struct(s, values, level, start_indent),
        dojo_types::schema::Ty::Enum(e) => format_enum(e, values, level, start_indent),
        dojo_types::schema::Ty::Array(a) => format_array(&a[0], values, level, start_indent),
        dojo_types::schema::Ty::Tuple(t) => format_tuple(t, values, level, start_indent),
    }
}

// print the structured record values
fn deep_print_record(
    schema: &dojo_types::schema::Ty,
    keys: &[FieldElement],
    values: &[FieldElement],
) {
    let mut model_values = vec![];
    model_values.extend(keys);
    model_values.extend(values);

    println!("{}", format_record_value(schema, &mut model_values, 0, true));
}

fn get_ty_repr(ty: &Ty) -> String {
    match ty {
        Ty::Primitive(p) => p.to_string(),
        Ty::Struct(s) => s.name.clone(),
        Ty::Enum(e) => e.name.clone(),
        Ty::Tuple(items) => {
            if items.is_empty() {
                "".to_string()
            } else {
                format!("({},)", items.iter().map(get_ty_repr).collect::<Vec<_>>().join(", "))
            }
        }
        Ty::Array(items) => format!("Array<{}>", get_ty_repr(&items[0])),
        Ty::ByteArray(_) => "ByteArray".to_string(),
    }
}

// to verify if a Ty has already been processed (i.e is in the list),
// just compare their type representation.
fn is_ty_already_in_list(ty_list: &[Ty], ty: &Ty) -> bool {
    let ty_repr = get_ty_repr(ty);
    ty_list.iter().any(|t| get_ty_repr(t).eq(&ty_repr))
}

// parse the Ty tree from its root and extract Ty to print.
// (basically, structs and enums)
fn get_printable_ty_list(root_ty: &Ty, ty_list: &mut Vec<Ty>) {
    match root_ty {
        Ty::Primitive(_) => {}
        Ty::ByteArray(_) => {}
        Ty::Struct(s) => {
            if !is_ty_already_in_list(ty_list, root_ty) {
                ty_list.push(root_ty.clone());
            }

            for member in &s.children {
                if !is_ty_already_in_list(ty_list, &member.ty) {
                    get_printable_ty_list(&member.ty, ty_list);
                }
            }
        }
        Ty::Enum(e) => {
            if !ty_list.contains(root_ty) {
                ty_list.push(root_ty.clone());
            }

            for child in &e.options {
                if !is_ty_already_in_list(ty_list, &child.ty) {
                    get_printable_ty_list(&child.ty, ty_list);
                }
            }
        }
        Ty::Tuple(tuple) => {
            for item_ty in tuple {
                if !is_ty_already_in_list(ty_list, item_ty) {
                    get_printable_ty_list(item_ty, ty_list);
                }
            }
        }
        Ty::Array(items_ty) => {
            if !is_ty_already_in_list(ty_list, &items_ty[0]) {
                get_printable_ty_list(&items_ty[0], ty_list)
            }
        }
    };
}

pub fn format_ty_field(name: &String, ty: &Ty, is_key: bool) -> String {
    let ty_repr = get_ty_repr(ty);
    let ty_repr = if ty_repr.is_empty() { "".to_string() } else { format!(": {ty_repr}") };
    let key_repr = if is_key { "  #[key]\n".to_string() } else { "".to_string() };

    format! {"{key_repr}  {name}{ty_repr}"}
}

// print Ty representation if required.
// For example, there is no need to print any information about arrays or tuples
// as they are members of struct and their items will be printed.
pub fn print_ty(ty: &Ty) {
    let ty_repr = match ty {
        Ty::Struct(s) => {
            let mut struct_str = format!("struct {} {{\n", s.name);
            for member in &s.children {
                struct_str.push_str(&format!(
                    "{},\n",
                    format_ty_field(&member.name, &member.ty, member.key)
                ));
            }
            struct_str.push('}');
            Some(struct_str)
        }
        Ty::Enum(e) => {
            let mut enum_str = format!("enum {} {{\n", e.name);
            for child in &e.options {
                enum_str
                    .push_str(&format!("{},\n", format_ty_field(&child.name, &child.ty, false)));
            }
            enum_str.push('}');
            Some(enum_str)
        }
        _ => None,
    };

    if let Some(ty_repr) = ty_repr {
        println!("{}\n\n", ty_repr);
    }
}

// print the full Ty tree
pub fn deep_print_ty(root: Ty) {
    let mut ty_list = vec![];
    get_printable_ty_list(&root, &mut ty_list);

    for ty in ty_list {
        print_ty(&ty);
    }
}
