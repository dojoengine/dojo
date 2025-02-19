/// Handle data (de)serialization to be stored into
/// the world storage.
/// 
/// The default implementation of this trait uses Serde.
pub trait DojoStore<T, +Serde<T>> {
    fn serialize(self: @T, ref serialized: Array<felt252>) {
        Serde::<T>::serialize(self, ref serialized);
    }
    fn deserialize(ref values: Span<felt252>) -> Option<T> {
        Serde::<T>::deserialize(ref values)
    }
}

impl DojoStore_felt252 of DojoStore<felt252>;
impl DojoStore_u8 of DojoStore<u8>;
impl DojoStore_u16 of DojoStore<u16>;
impl DojoStore_u32 of DojoStore<u32>;
impl DojoStore_u64 of DojoStore<u64>;
impl DojoStore_u128 of DojoStore<u128>;
impl DojoStore_u256 of DojoStore<u256>;
impl DojoStore_i8 of DojoStore<i8>;
impl DojoStore_i16 of DojoStore<i16>;
impl DojoStore_i32 of DojoStore<i32>;
impl DojoStore_i64 of DojoStore<i64>;
impl DojoStore_i128 of DojoStore<i128>;
impl DojoStore_ContractAddress of DojoStore<starknet::ContractAddress>;
impl DojoStore_ClassHash of DojoStore<starknet::ClassHash>;
impl DojoStore_EthAddress of DojoStore<starknet::EthAddress>;
impl DojoStore_ByteArray of DojoStore<ByteArray>;

/// Specific implementation of DojoStore for Option<T>.
/// 
/// 'None' is stored as the first variant (instead of 'Some').
/// The variant index is incremented by 1 to be able to detect
/// unitialized variant.
impl DojoStore_option<T, +Serde<T>, +DojoStore<T>, +Serde<Option<T>>> of DojoStore<Option<T>> {
    fn serialize(self: @Option<T>, ref serialized: Array<felt252>) {
        match self {
            Option::Some(x) => {
                serialized.append(2);
                DojoStore::serialize(x, ref serialized);
            },
            Option::None => {
                serialized.append(1);
            }
        }
    }

    fn deserialize(ref values: Span<felt252>) -> Option<Option<T>> {
        if let Option::Some(x) = values.pop_front() {
            return match *x {
                0 | 1 => Option::Some(Option::None),
                2 => Option::Some(DojoStore::<T>::deserialize(ref values)),
                _ => Option::None
            };
        }

        Option::None
    }
}

fn serialize_array_helper<T, +Serde<T>, +DojoStore<T>, +Drop<T>>(mut input: Span<T>, ref output: Array<felt252>) {
    match input.pop_front() {
        Option::Some(value) => {
            DojoStore::serialize(value, ref output);
            serialize_array_helper(input, ref output);
        },
        Option::None => {},
    }
}

fn deserialize_array_helper<T, +Serde<T>, +DojoStore<T>, +Drop<T>>(
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
impl DojoStore_array<T, +Drop<T>, +Serde<T>, +DojoStore<T>> of DojoStore<Array<T>> {
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

// TODO RBA: specific implementation for tuples.

/// Specific implementation of DojoStore for Span<T>,
/// to call DojoStore for span items instead of Serde directly.
//impl DojoStore_span<T, +Drop<T>, +Serde<T>, +DojoStore<T>> of DojoStore<Span<T>> {
//    fn serialize(self: @Span<T>, ref serialized: Array<felt252>) {
//        DojoStore::serialize(@self.len(), ref serialized);
//        serialize_array_helper(*self, ref serialized);
//    }
//
//    fn deserialize(ref values: Span<felt252>) -> Option<Span<T>> {
//        let length = *values.pop_front()?;
//        let mut arr = ArrayTrait::new();
//        Some(deserialize_array_helper(ref values, arr, length)?.span())
//    }
//}

