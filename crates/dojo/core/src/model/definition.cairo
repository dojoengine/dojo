use dojo::meta::{Layout, introspect::Ty};

/// The `ModelIndex` provides encapsulation for different ways to access
/// a model's data.
///
/// - `Keys`: Access by keys, where each individual key is known, and can be hashed.
/// - `Id`: Access by id, where only the id of the entity is known (keys already hashed).
/// - `MemberId`: Access by member id, where the member id and entity id are known.
#[derive(Copy, Drop, Serde, Debug, PartialEq)]
pub enum ModelIndex {
    Keys: Span<felt252>,
    Id: felt252,
    // (entity_id, member_id)
    MemberId: (felt252, felt252)
}

/// The `ModelDefinition` trait.
///
/// Definition of the model containing all the fields that makes up a model.
pub trait ModelDefinition<T> {
    fn name() -> ByteArray;
    fn version() -> u8;
    fn layout() -> Layout;
    fn schema() -> Ty;
    fn size() -> Option<usize>;
}

/// A plain struct with all the fields of a model definition.
#[derive(Drop, Serde, Debug, PartialEq)]
pub struct ModelDef {
    pub name: ByteArray,
    pub version: u8,
    pub layout: Layout,
    pub schema: Ty,
    pub packed_size: Option<usize>,
    pub unpacked_size: Option<usize>,
}
