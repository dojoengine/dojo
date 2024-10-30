use dojo::{model::{Model, IModel, ModelDef}, meta::{Layout, Ty}};

#[starknet::embeddable]
pub impl IModelImpl<TContractState, M, +Model<M>> of IModel<TContractState> {
    fn dojo_name(self: @TContractState) -> ByteArray {
        Model::<M>::name()
    }

    fn version(self: @TContractState) -> u8 {
        Model::<M>::version()
    }

    fn schema(self: @TContractState) -> Ty {
        Model::<M>::schema()
    }

    fn layout(self: @TContractState) -> Layout {
        Model::<M>::layout()
    }

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
