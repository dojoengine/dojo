use super::{ModelDefinition, Model};
use super::storage::MemberModelStorage;

/// The `MemberStore` trait.
///
/// It provides a standardized way to interact with members of a model.
///
/// # Template Parameters
/// - `M`: The type of the model.
/// - `S`: The type of the storage.
/// - `T`: The type of the member.
pub trait MemberStore<S, M, T> {
    /// Retrieves a member of type `T` from a model of type `M` using the provided member id and key
    /// of type `K`.
    fn get_member(self: @S, entity_id: felt252, member_id: felt252) -> T;
    /// Updates a member of type `T` within a model of type `M` using the provided member id, key of
    /// type `K`, and new value of type `T`.
    fn update_member(ref self: S, entity_id: felt252, member_id: felt252, value: T);
}

pub impl MemberStoreImpl<
    S, M, T, +Model<M>, +MemberModelStorage<S, M, T>, +ModelDefinition<M>, +Serde<T>, +Drop<T>
> of MemberStore<S, M, T> {
    fn get_member(self: @S, entity_id: felt252, member_id: felt252) -> T {
        MemberModelStorage::<S, M, T>::get_member(self, entity_id, member_id)
    }

    fn update_member(ref self: S, entity_id: felt252, member_id: felt252, value: T) {
        MemberModelStorage::<S, M, T>::update_member(ref self, entity_id, member_id, value)
    }
}
