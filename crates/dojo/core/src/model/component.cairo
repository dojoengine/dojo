use dojo::meta::Layout;
use dojo::meta::introspect::Struct;
use dojo::model::{IModel, Model, ModelDef};

#[starknet::embeddable]
pub impl IDeployedModelImpl<
    TContractState, M, +Model<M>,
> of dojo::meta::IDeployedResource<TContractState> {
    fn dojo_name(self: @TContractState) -> ByteArray {
        Model::<M>::name()
    }
}

#[starknet::embeddable]
pub impl IStoredModelImpl<
    TContractState, M, +Model<M>,
> of dojo::meta::IStoredResource<TContractState> {
    fn schema(self: @TContractState) -> Struct {
        Model::<M>::schema()
    }

    fn layout(self: @TContractState) -> Layout {
        Model::<M>::layout()
    }
}

#[starknet::embeddable]
pub impl IModelImpl<TContractState, M, +Model<M>> of IModel<TContractState> {
    fn unpacked_size(self: @TContractState) -> Option<usize> {
        Model::<M>::unpacked_size()
    }

    fn packed_size(self: @TContractState) -> Option<usize> {
        Model::<M>::packed_size()
    }

    fn definition(self: @TContractState) -> ModelDef {
        Model::<M>::definition()
    }
}
