#[derive(Introspect)]
struct SimpleModel {
    #[key]
    k: u32,
    v: u32
}

#[derive(Drop, Serde)]
pub struct SimpleModelValue {
    pub v: u32,
}

type SimpleModelKeyType = u32;

pub impl SimpleModelKeyParser of dojo::model::model::KeyParser<SimpleModel, SimpleModelKeyType> {
    #[inline(always)]
    fn parse_key(self: @SimpleModel) -> SimpleModelKeyType {
        *self.k
    }
}

impl SimpleModelModelValueKey of dojo::model::model_value::ModelValueKey<
    SimpleModelValue, SimpleModelKeyType
> {}

// Impl to get the static definition of a model
pub mod m_SimpleModel_definition {
    use super::SimpleModel;
    pub impl SimpleModelDefinitionImpl<T> of dojo::model::ModelDefinition<T> {
        #[inline(always)]
        fn name() -> ByteArray {
            "SimpleModel"
        }

        #[inline(always)]
        fn layout() -> dojo::meta::Layout {
            dojo::meta::Introspect::<SimpleModel>::layout()
        }

        #[inline(always)]
        fn schema() -> dojo::meta::introspect::Struct {
            if let dojo::meta::introspect::Ty::Struct(s) =
                dojo::meta::Introspect::<SimpleModel>::ty() {
                s
            } else {
                panic!("Model `SimpleModel`: invalid schema.")
            }
        }

        #[inline(always)]
        fn size() -> Option<usize> {
            dojo::meta::Introspect::<SimpleModel>::size()
        }
    }
}

pub impl SimpleModelDefinition = m_SimpleModel_definition::SimpleModelDefinitionImpl<SimpleModel>;
pub impl SimpleModelModelValueDefinition =
    m_SimpleModel_definition::SimpleModelDefinitionImpl<SimpleModelValue>;

pub impl SimpleModelModelParser of dojo::model::model::ModelParser<SimpleModel> {
    fn serialize_keys(self: @SimpleModel) -> Span<felt252> {
        let mut serialized = core::array::ArrayTrait::new();
        core::serde::Serde::serialize(self.k, ref serialized);

        core::array::ArrayTrait::span(@serialized)
    }
    fn serialize_values(self: @SimpleModel) -> Span<felt252> {
        let mut serialized = core::array::ArrayTrait::new();
        core::serde::Serde::serialize(self.v, ref serialized);

        core::array::ArrayTrait::span(@serialized)
    }
}

pub impl SimpleModelModelValueParser of dojo::model::model_value::ModelValueParser<
    SimpleModelValue
> {
    fn serialize_values(self: @SimpleModelValue) -> Span<felt252> {
        let mut serialized = core::array::ArrayTrait::new();
        core::serde::Serde::serialize(self.v, ref serialized);

        core::array::ArrayTrait::span(@serialized)
    }
}

pub impl SimpleModelModelImpl = dojo::model::model::ModelImpl<SimpleModel>;
pub impl SimpleModelModelValueImpl = dojo::model::model_value::ModelValueImpl<SimpleModelValue>;

#[starknet::contract]
pub mod m_SimpleModel {
    use super::SimpleModel;
    use super::SimpleModelValue;

    #[storage]
    struct Storage {}

    #[abi(embed_v0)]
    impl SimpleModel__DojoDeployedModelImpl =
        dojo::model::component::IDeployedModelImpl<ContractState, SimpleModel>;

    #[abi(embed_v0)]
    impl SimpleModel__DojoStoredModelImpl =
        dojo::model::component::IStoredModelImpl<ContractState, SimpleModel>;

    #[abi(embed_v0)]
    impl SimpleModel__DojoModelImpl =
        dojo::model::component::IModelImpl<ContractState, SimpleModel>;

    #[abi(per_item)]
    #[generate_trait]
    impl SimpleModelImpl of ISimpleModel {
        // Ensures the ABI contains the Model struct, even if never used
        // into as a system input.
        #[external(v0)]
        fn ensure_abi(self: @ContractState, model: SimpleModel) {
            let _model = model;
        }

        // Outputs ModelValue to allow a simple diff from the ABI compared to the
        // model to retrieved the keys of a model.
        #[external(v0)]
        fn ensure_values(self: @ContractState, value: SimpleModelValue) {
            let _value = value;
        }

        // Ensures the generated contract has a unique classhash, using
        // a hardcoded hash computed on model and member names.
        #[external(v0)]
        fn ensure_unique(self: @ContractState) {}
    }
}
