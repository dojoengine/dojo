/// Handle data (de)serialization to be stored into the world storage.
pub trait DojoStore<T> {
    fn serialize(self: @T, ref serialized: Array<felt252>);
    fn deserialize(ref values: Span<felt252>) -> Option<T>;
}

/// The default implementation of DojoStore uses Serde.
mod default_impl {
    pub impl SerdeBasedDojoStore<T, +Serde<T>> of super::DojoStore<T> {
        #[inline(always)]
        fn serialize(self: @T, ref serialized: Array<felt252>) {
            Serde::serialize(self, ref serialized);
        }
        #[inline(always)]
        fn deserialize(ref values: Span<felt252>) -> Option<T> {
            Serde::<T>::deserialize(ref values)
        }
    }
}

pub impl DojoStore_felt252 = default_impl::SerdeBasedDojoStore<felt252>;
pub impl DojoStore_bool = default_impl::SerdeBasedDojoStore<bool>;
pub impl DojoStore_u8 = default_impl::SerdeBasedDojoStore<u8>;
pub impl DojoStore_u16 = default_impl::SerdeBasedDojoStore<u16>;
pub impl DojoStore_u32 = default_impl::SerdeBasedDojoStore<u32>;
pub impl DojoStore_u64 = default_impl::SerdeBasedDojoStore<u64>;
pub impl DojoStore_u128 = default_impl::SerdeBasedDojoStore<u128>;
pub impl DojoStore_u256 = default_impl::SerdeBasedDojoStore<u256>;
pub impl DojoStore_i8 = default_impl::SerdeBasedDojoStore<i8>;
pub impl DojoStore_i16 = default_impl::SerdeBasedDojoStore<i16>;
pub impl DojoStore_i32 = default_impl::SerdeBasedDojoStore<i32>;
pub impl DojoStore_i64 = default_impl::SerdeBasedDojoStore<i64>;
pub impl DojoStore_i128 = default_impl::SerdeBasedDojoStore<i128>;
pub impl DojoStore_ContractAddress = default_impl::SerdeBasedDojoStore<starknet::ContractAddress>;
pub impl DojoStore_ClassHash = default_impl::SerdeBasedDojoStore<starknet::ClassHash>;
pub impl DojoStore_EthAddress = default_impl::SerdeBasedDojoStore<starknet::EthAddress>;
pub impl DojoStore_ByteArray = default_impl::SerdeBasedDojoStore<ByteArray>;

/// Specific implementation of DojoStore for Option<T>.
impl DojoStore_option<T, +DojoStore<T>> of DojoStore<Option<T>> {
    fn serialize(self: @Option<T>, ref serialized: Array<felt252>) {
        match self {
            Option::Some(x) => {
                serialized.append(1);
                DojoStore::serialize(x, ref serialized);
            },
            Option::None => { serialized.append(2); },
        }
    }

    fn deserialize(ref values: Span<felt252>) -> Option<Option<T>> {
        if let Option::Some(x) = values.pop_front() {
            return match *x {
                0 => Option::Some(Default::default()),
                1 => Option::Some(DojoStore::<T>::deserialize(ref values)),
                2 => Option::Some(Option::None),
                _ => Option::None,
            };
        }

        Option::None
    }
}

fn serialize_array_helper<T, +DojoStore<T>, +Drop<T>>(
    mut input: Span<T>, ref output: Array<felt252>,
) {
    if let Some(value) = input.pop_front() {
        DojoStore::serialize(value, ref output);
        serialize_array_helper(input, ref output);
    }
}

fn deserialize_array_helper<T, +DojoStore<T>, +Drop<T>>(
    ref serialized: Span<felt252>, mut curr_output: Array<T>, remaining: felt252,
) -> Option<Array<T>> {
    if remaining == 0 {
        return Option::Some(curr_output);
    }
    curr_output.append(DojoStore::deserialize(ref serialized)?);
    deserialize_array_helper(ref serialized, curr_output, remaining - 1)
}

/// Specific implementation of DojoStore for Array<T>,
/// to call DojoStore for array items instead of Serde directly.
impl DojoStore_array<T, +Drop<T>, +DojoStore<T>> of DojoStore<Array<T>> {
    fn serialize(self: @Array<T>, ref serialized: Array<felt252>) {
        DojoStore::serialize(@self.len(), ref serialized);
        serialize_array_helper(self.span(), ref serialized);
    }

    fn deserialize(ref values: Span<felt252>) -> Option<Array<T>> {
        let length = *values.pop_front()?;
        let mut arr = array![];
        deserialize_array_helper(ref values, arr, length)
    }
}

impl DojoStore_span<T, +Drop<T>, +DojoStore<T>> of DojoStore<Span<T>> {
    fn serialize(self: @Span<T>, ref serialized: Array<felt252>) {
        DojoStore::serialize(@(*self).len(), ref serialized);
        serialize_array_helper(*self, ref serialized);
    }

    fn deserialize(ref values: Span<felt252>) -> Option<Span<T>> {
        let length = *values.pop_front()?;
        let mut arr = array![];
        Option::Some(deserialize_array_helper(ref values, arr, length)?.span())
    }
}
