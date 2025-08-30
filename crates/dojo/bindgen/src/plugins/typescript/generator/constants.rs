pub const CAIRO_FELT252: &str = "felt252";
pub const CAIRO_CONTRACT_ADDRESS: &str = "ContractAddress";
pub const CAIRO_BYTE_ARRAY: &str = "ByteArray";
pub const CAIRO_U8: &str = "u8";
pub const CAIRO_U16: &str = "u16";
pub const CAIRO_U32: &str = "u32";
pub const CAIRO_U64: &str = "u64";
pub const CAIRO_U128: &str = "u128";
pub const CAIRO_U256: &str = "u256";
pub const CAIRO_U256_STRUCT: &str = "U256";
pub const CAIRO_I8: &str = "i8";
pub const CAIRO_I16: &str = "i16";
pub const CAIRO_I32: &str = "i32";
pub const CAIRO_I64: &str = "i64";
pub const CAIRO_I128: &str = "i128";
pub const CAIRO_BOOL: &str = "bool";
pub const CAIRO_OPTION: &str = "Option";
pub const CAIRO_UNIT_TYPE: &str = "()";

pub(crate) const CAIRO_OPTION_DEFAULT_VALUE: &str = "new CairoOption(CairoOptionVariant.None)";

pub const JS_BOOLEAN: &str = "boolean";
pub const JS_STRING: &str = "string";
pub const JS_BIGNUMBERISH: &str = "BigNumberish";

pub(crate) const BIGNUMBERISH_IMPORT: &str = "import { BigNumberish } from 'starknet';\n";
pub(crate) const CAIRO_OPTION_IMPORT: &str = "import { CairoOption } from 'starknet';\n";
pub(crate) const CAIRO_ENUM_IMPORT: &str = "import { CairoCustomEnum } from 'starknet';\n";
pub(crate) const CAIRO_OPTION_TYPE_PATH: &str = "core::option::Option";
pub(crate) const SN_IMPORT_SEARCH: &str = "} from 'starknet';";
pub(crate) const CAIRO_OPTION_TOKEN: &str = "CairoOption, CairoOptionVariant,";
pub(crate) const CAIRO_ENUM_TOKEN: &str = "CairoCustomEnum,";
