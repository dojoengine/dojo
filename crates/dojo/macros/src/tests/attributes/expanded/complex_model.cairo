#[derive(Introspect)]
struct ComplexModel {
    #[key]
    k1: u8,
    #[key]
    k2: u32,
    v1: u256,
    v2: Option<u128>
}

#[derive(Drop, Serde)]
pub struct ComplexModelValue {
    pub v1: u256,
    pub v2: Option<u128>,
}

type ComplexModelKeyType = (u8, u32);

pub impl ComplexModelKeyParser of dojo::model::model::KeyParser<ComplexModel, ComplexModelKeyType> {
    #[inline(always)]
    fn parse_key(self: @ComplexModel) -> ComplexModelKeyType {
        (*self.k1, *self.k2)
    }
}

impl ComplexModelModelValueKey of dojo::model::model_value::ModelValueKey<
    ComplexModelValue, ComplexModelKeyType
> {}

// Impl to get the static definition of a model
pub mod m_ComplexModel_definition {
    use super::ComplexModel;
    pub impl ComplexModelDefinitionImpl<T> of dojo::model::ModelDefinition<T> {
        #[inline(always)]
        fn name() -> ByteArray {
            "ComplexModel"
        }

        #[inline(always)]
        fn layout() -> dojo::meta::Layout {
            dojo::meta::Introspect::<ComplexModel>::layout()
        }

        #[inline(always)]
        fn schema() -> dojo::meta::introspect::Struct {
            if let dojo::meta::introspect::Ty::Struct(s) =
                dojo::meta::Introspect::<ComplexModel>::ty() {
                s
            } else {
                panic!("Model `ComplexModel`: invalid schema.")
            }
        }

        #[inline(always)]
        fn size() -> Option<usize> {
            dojo::meta::Introspect::<ComplexModel>::size()
        }
    }
}

pub impl ComplexModelDefinition =
    m_ComplexModel_definition::ComplexModelDefinitionImpl<ComplexModel>;
pub impl ComplexModelModelValueDefinition =
    m_ComplexModel_definition::ComplexModelDefinitionImpl<ComplexModelValue>;

pub impl ComplexModelModelParser of dojo::model::model::ModelParser<ComplexModel> {
    fn serialize_keys(self: @ComplexModel) -> Span<felt252> {
        let mut serialized = core::array::ArrayTrait::new();
        core::serde::Serde::serialize(self.k1, ref serialized);
        core::serde::Serde::serialize(self.k2, ref serialized);

        core::array::ArrayTrait::span(@serialized)
    }
    fn serialize_values(self: @ComplexModel) -> Span<felt252> {
        let mut serialized = core::array::ArrayTrait::new();
        core::serde::Serde::serialize(self.v1, ref serialized);
        core::serde::Serde::serialize(self.v2, ref serialized);

        core::array::ArrayTrait::span(@serialized)
    }
}

pub impl ComplexModelModelValueParser of dojo::model::model_value::ModelValueParser<
    ComplexModelValue
> {
    fn serialize_values(self: @ComplexModelValue) -> Span<felt252> {
        let mut serialized = core::array::ArrayTrait::new();
        core::serde::Serde::serialize(self.v1, ref serialized);
        core::serde::Serde::serialize(self.v2, ref serialized);

        core::array::ArrayTrait::span(@serialized)
    }
}

pub impl ComplexModelModelImpl = dojo::model::model::ModelImpl<ComplexModel>;
pub impl ComplexModelModelValueImpl = dojo::model::model_value::ModelValueImpl<ComplexModelValue>;

#[starknet::contract]
pub mod m_ComplexModel {
    use super::ComplexModel;
    use super::ComplexModelValue;

    #[storage]
    struct Storage {}

    #[abi(embed_v0)]
    impl ComplexModel__DojoDeployedModelImpl =
        dojo::model::component::IDeployedModelImpl<ContractState, ComplexModel>;

    #[abi(embed_v0)]
    impl ComplexModel__DojoStoredModelImpl =
        dojo::model::component::IStoredModelImpl<ContractState, ComplexModel>;

    #[abi(embed_v0)]
    impl ComplexModel__DojoModelImpl =
        dojo::model::component::IModelImpl<ContractState, ComplexModel>;

    #[abi(per_item)]
    #[generate_trait]
    impl ComplexModelImpl of IComplexModel {
        // Ensures the ABI contains the Model struct, even if never used
        // into as a system input.
        #[external(v0)]
        fn ensure_abi(self: @ContractState, model: ComplexModel) {
            let _model = model;
        }

        // Outputs ModelValue to allow a simple diff from the ABI compared to the
        // model to retrieved the keys of a model.
        #[external(v0)]
        fn ensure_values(self: @ContractState, value: ComplexModelValue) {
            let _value = value;
        }

        // Ensures the generated contract has a unique classhash, using
        // a hardcoded hash computed on model and member names.
        #[external(v0)]
        fn ensure_unique(self: @ContractState) {}
    }
}
