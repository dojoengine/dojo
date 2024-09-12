use super::introspect::Ty;
use super::layout::Layout;

#[derive(Drop, Serde, Debug, PartialEq)]
pub struct ModelDefinition {
    pub name: ByteArray,
    pub namespace: ByteArray,
    pub namespace_selector: felt252,
    pub version: u8,
    pub layout: Layout,
    pub schema: Ty,
    pub packed_size: Option<u32>,
    pub unpacked_size: Option<u32>
}
