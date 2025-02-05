use async_graphql::dynamic::indexmap::IndexMap;
use async_graphql::dynamic::TypeRef;
use async_graphql::{Name, Value};
use dojo_types::primitive::Primitive;
use strum::IntoEnumIterator;
use strum_macros::{AsRefStr, Display, EnumIter, EnumString};

// ValueMapping is used to map the values of the fields of a model and TypeMapping their
// corresponding types. Both are used at runtime to dynamically build/resolve graphql
// queries/schema. `Value` from async-graphql supports nesting, but TypeRef does not. TypeData is
// used to support nesting.
pub type ValueMapping = IndexMap<Name, Value>;
pub type TypeMapping = IndexMap<Name, TypeData>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeData {
    Simple(TypeRef),
    Nested((TypeRef, IndexMap<Name, TypeData>)),
    List(Box<TypeData>),
    // Union can only  represent an object of objects
    // Enum((TypeRef, IndexMap<Name, TypeData>)),
}

impl TypeData {
    pub fn type_ref(&self) -> TypeRef {
        match self {
            TypeData::Simple(ty) | TypeData::Nested((ty, _)) => ty.clone(),
            TypeData::List(inner) => TypeRef::List(Box::new(inner.type_ref())),
            // TypeData::Enum((ty, _)) => ty.clone(),
        }
    }

    pub fn is_simple(&self) -> bool {
        matches!(self, TypeData::Simple(_))
    }

    pub fn is_nested(&self) -> bool {
        matches!(self, TypeData::Nested(_))
    }

    pub fn is_list(&self) -> bool {
        matches!(self, TypeData::List(_))
    }

    pub fn inner(&self) -> Option<&TypeData> {
        match self {
            TypeData::List(inner) => Some(inner),
            _ => None,
        }
    }

    // pub fn is_enum(&self) -> bool {
    //     matches!(self, TypeData::Enum(_))
    // }

    pub fn type_mapping(&self) -> Option<&IndexMap<Name, TypeData>> {
        match self {
            TypeData::Simple(_) => None,
            TypeData::Nested((_, type_mapping)) => Some(type_mapping),
            TypeData::List(_) => None,
            // TypeData::Enum((_, type_mapping)) => Some(type_mapping),
        }
    }
}

#[derive(Debug)]
pub enum ScalarType {
    Cairo(Primitive),
    Torii(GraphqlType),
}

// basic types like ID and Int are handled by async-graphql
#[derive(AsRefStr, Display, EnumIter, EnumString, Debug)]
pub enum GraphqlType {
    ByteArray,
    Enum,
    Cursor,
    DateTime,
}

impl ScalarType {
    pub fn all() -> Vec<String> {
        Primitive::iter()
            .map(|ty| ty.to_string())
            .chain(GraphqlType::iter().map(|ty| ty.to_string()))
            .collect()
    }
}
