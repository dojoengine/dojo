use std::collections::BTreeMap;

use num_bigint::BigUint;
use num_traits::{FromPrimitive, One, ToPrimitive};

use crate::class::{ClassHash, CompiledClassHash};
use crate::contract::{ContractAddress, StorageKey, StorageValue};
use crate::state::StateUpdates;
use crate::Felt;

use super::eip4844::BLOB_LEN;

pub fn encode_state_updates(value: StateUpdates) -> Vec<BigUint> {
    let mut contract_updates = BTreeMap::<ContractAddress, ContractUpdate>::new();

    for (addr, nonce) in &value.nonce_updates {
        let entry = contract_updates.entry(*addr).or_default();
        entry.metadata.new_nonce = Some(*nonce);
    }

    for (addr, class_hash) in &value.deployed_contracts {
        let entry = contract_updates.entry(*addr).or_default();
        entry.metadata.class_information_flag = true;
        entry.class_hash = Some(*class_hash);
    }

    for (addr, storages) in &value.storage_updates {
        let entry = contract_updates.entry(*addr).or_default();
        entry.metadata.total_storage_updates = storages.len();
        entry.storage_updates = storages.clone();
    }

    let mut buffer = Vec::new();

    // Encode the contract updates
    let total_updates = BigUint::from_usize(contract_updates.len()).unwrap();
    buffer.push(total_updates);

    for (addr, value) in contract_updates {
        buffer.push(addr.to_biguint());
        value.encode(&mut buffer);
    }

    // Encode the class declarations
    let total_declarations = BigUint::from_usize(value.declared_classes.len()).unwrap();
    buffer.push(total_declarations);

    for (hash, compiled_hash) in &value.declared_classes {
        buffer.push(hash.to_biguint());
        buffer.push(compiled_hash.to_biguint());
    }

    buffer
}

pub fn decode_state_updates<'a>(value: impl IntoIterator<Item = &'a BigUint>) -> StateUpdates {
    let mut state_updates = StateUpdates::default();
    let mut iter = value.into_iter();

    let total_contract_updates = iter.next().and_then(|v| v.to_usize()).expect("valid usize");

    for _ in 0..total_contract_updates {
        let address: ContractAddress = iter.next().map(Felt::from).expect("valid address").into();
        let metadata = iter.next().map(Metadata::decode).expect("valid metadata");

        let class_hash = if metadata.class_information_flag {
            iter.next().map(Felt::from).map(Some).expect("valid class hash")
        } else {
            None
        };

        let mut storages = BTreeMap::new();
        for _ in 0..metadata.total_storage_updates {
            let key = iter.next().map(StorageKey::from).expect("valid storage key");
            if let Some(value) = iter.next().map(StorageValue::from) {
                storages.insert(key, value);
            } else {
                return state_updates;
            }
        }

        if !storages.is_empty() {
            state_updates.storage_updates.insert(address, storages);
        }

        if let Some(nonce) = metadata.new_nonce {
            state_updates.nonce_updates.insert(address, nonce);
        }

        if let Some(hash) = class_hash {
            state_updates.deployed_contracts.insert(address, hash);
        }
    }

    let total_declared_classes = iter.next().and_then(|v| v.to_usize()).expect("valid usize");

    for _ in 0..total_declared_classes {
        let class_hash = iter.next().map(ClassHash::from).expect("valid class hash");
        let compiled_class_hash =
            iter.next().map(CompiledClassHash::from).expect("valid compiled class hash");
        state_updates.declared_classes.insert(class_hash, compiled_class_hash);
    }

    state_updates
}

/// Metadata information about the contract update.
///
/// Encoding format:
///
/// |---padding---|---class flag---|---new nonce---|---no. storage updates---|
///     127 bits        1 bit           64 bits             64 bits
#[derive(Debug, Default)]
struct Metadata {
    /// Class information flag, whose value in the encoded format is one of the following:
    ///
    /// - 0: Storage updates only
    /// - 1: The contract was deployed or replaced in this state update.
    ///
    /// When this flag is set to 1, the new class hash occupies an additional word before the
    /// storage updates section.
    class_information_flag: bool,
    /// The new nonce value of the contract if it was updated. Otherwise, in the encoded form, it
    /// is set to 0.
    new_nonce: Option<Felt>,
    /// The number of storage updates of the contract in the state updates.
    total_storage_updates: usize,
}

impl Metadata {
    // TODO: find a way to not use &str
    fn decode(word: &BigUint) -> Self {
        // expand to 256 bits if needed
        let bits = format!("{word:0>256b}");

        let flag = bits.get(127..(127 + 1)).unwrap();
        let flag = u8::from_str_radix(flag, 2).unwrap();
        let class_information_flag = if flag == 1 { true } else { false };

        let nonce = bits.get(128..(128 + 64)).unwrap();
        let nonce = u64::from_str_radix(nonce, 2).unwrap();
        let nonce = Felt::from_u64(nonce).unwrap();
        let new_nonce = if nonce == Felt::ZERO { None } else { Some(nonce) };

        let total = bits.get(192..(192 + 64)).unwrap();
        let total_storage_updates = usize::from_str_radix(total, 2).unwrap();

        Self { class_information_flag, new_nonce, total_storage_updates }
    }

    fn encode(&self) -> BigUint {
        let mut word = BigUint::ZERO;

        if self.class_information_flag {
            word |= BigUint::one() << 128;
        }

        if let Some(nonce) = self.new_nonce {
            word |= BigUint::from(nonce.to_u64().unwrap()) << 64;
        }

        word |= BigUint::from(self.total_storage_updates);

        word
    }
}

#[derive(Debug, Default)]
struct ContractUpdate {
    metadata: Metadata,
    class_hash: Option<ClassHash>,
    storage_updates: BTreeMap<StorageKey, StorageValue>,
}

impl ContractUpdate {
    // fn decode(encoded: &[BigUint]) -> Self {
    //     let address: ContractAddress = Felt::from(&encoded[0]).into();
    //     let metadata = Metadata::decode(&encoded[1]);

    //     let class_hash =
    //         if metadata.class_information_flag { Some(ClassHash::from(&encoded[2])) } else { None
    // };

    //     // The index of the first storage key in the encoded data.
    //     let storages_start_idx = if class_hash.is_some() { 3 } else { 2 };

    //     let mut storage_updates = BTreeMap::new();
    //     for i in 0..metadata.total_storage_updates {
    //         let idx = (storages_start_idx + i * 2) as usize;

    //         let key = StorageKey::from(&encoded[idx]);
    //         let value = StorageValue::from(&encoded[idx + 1]);
    //         storage_updates.insert(key, value);
    //     }

    //     Self { address, metadata, class_hash, storage_updates }
    // }

    fn encode(self, buffer: &mut Vec<BigUint>) {
        buffer.push(self.metadata.encode());

        if let Some(class_hash) = self.class_hash {
            buffer.push(class_hash.to_biguint());
        }

        for (key, value) in self.storage_updates {
            buffer.push(key.to_biguint());
            buffer.push(value.to_biguint());
        }
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use starknet::macros::felt;

    use super::*;

    macro_rules! biguint {
        ($s:expr) => {
            BigUint::from_str($s).unwrap()
        };
    }

    #[test]
    fn rt_metadata_encoding() {
        let metadata = felt!("0x10000000000000001").to_biguint();

        let encoded = Metadata::decode(&metadata);
        assert!(!encoded.class_information_flag);
        assert_eq!(encoded.new_nonce, Some(Felt::ONE));
        assert_eq!(encoded.total_storage_updates, 1);

        let encoded = encoded.encode();
        assert_eq!(encoded, metadata);
    }

    #[test]
    fn rt_state_updates_encoding() {
        let input = vec![
            biguint!("1"),
            biguint!(
                "2019172390095051323869047481075102003731246132997057518965927979101413600827"
            ),
            biguint!("18446744073709551617"),
            biguint!("100"),
            biguint!("200"),
            biguint!("1"),
            biguint!(
                "1351148242645005540004162531550805076995747746087542030095186557536641755046"
            ),
            biguint!("558404273560404778508455254030458021013656352466216690688595011803280448032"),
        ];

        let state_updates = super::decode_state_updates(&input);

        assert_eq!(state_updates.nonce_updates.len(), 1);
        assert_eq!(state_updates.storage_updates.len(), 1);
        assert_eq!(state_updates.declared_classes.len(), 1);
        assert_eq!(state_updates.deployed_contracts.len(), 0);

        let address: ContractAddress =
            felt!("2019172390095051323869047481075102003731246132997057518965927979101413600827")
                .into();

        assert_eq!(state_updates.nonce_updates.get(&address), Some(&Felt::ONE));

        let storage_updates = state_updates.storage_updates.get(&address).unwrap();
        assert_eq!(storage_updates.len(), 1);
        assert_eq!(storage_updates.get(&felt!("0x64")), Some(&felt!("0xc8")));

        let class_hash =
            felt!("1351148242645005540004162531550805076995747746087542030095186557536641755046");
        let compiled_class_hash =
            felt!("558404273560404778508455254030458021013656352466216690688595011803280448032");
        assert_eq!(state_updates.declared_classes.get(&class_hash), Some(&compiled_class_hash));

        let encoded = encode_state_updates(state_updates);
        similar_asserts::assert_eq!(encoded, input);
    }
}
