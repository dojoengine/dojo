//! Data availability encoding and decoding.
//!
//! The encoding format is based on the format of which Starknet's publishes its state diffs onto
//! the Ethereum blockchain, refer to the Starknet [docs](https://docs.starknet.io/architecture-and-concepts/network-architecture/data-availability/) for more information.
//!
//! Example of a Starknet's encoded state diff that might be published on onchain:
//!
//! ```
//! ┌───────┬─────────────────────────────────────────────────────────────────────┐
//! │ Index │                          Field Element                              │
//! ├───────┼─────────────────────────────────────────────────────────────────────┤
//! │  [0]  │ 1                                                                   │
//! │  [1]  │ 2019172390095051323869047481075102003731246132997057518965927979... │
//! │  [2]  │ 18446744073709551617                                                │
//! │  [3]  │ 100                                                                 │
//! │  [4]  │ 200                                                                 │
//! │  [5]  │ 1                                                                   │
//! │  [6]  │ 1351148242645005540004162531550805076995747746087542030095186557... │
//! │  [7]  │ 5584042735604047785084552540304580210136563524662166906885950118... │
//! └───────┴─────────────────────────────────────────────────────────────────────┘
//!
//! Explanation:-
//!
//! [0] The number of contracts whose state was updated.
//! [1] The address of the first, and only, contract whose state changed.
//! [2] Meta information regarding the update, see [Metadata] for more details.
//! [3] Key of the storage update
//! [4] Value of the storage update (value of key 100 is set to 200)
//! [5] New declare section: 1 declare v2 transaction in this state update
//! [6] Encoding of the class hash
//! [7] Encoding of the compiled class hash of the declared class
//! ```

use std::collections::BTreeMap;
use std::num::ParseIntError;

use num_bigint::BigUint;
use num_traits::{FromPrimitive, One, ToPrimitive};

use crate::class::{ClassHash, CompiledClassHash};
use crate::contract::{ContractAddress, StorageKey, StorageValue};
use crate::state::StateUpdates;
use crate::Felt;

#[derive(Debug, thiserror::Error)]
pub enum EncodingError {
    #[error("Missing contract updates entry count")]
    MissingUpdatesCount,
    #[error("Missing class declarations entry count")]
    MissingDeclarationsCount,
    #[error("Missing contract address")]
    MissingAddress,
    #[error("Missing contract update metadata")]
    MissingMetadata,
    #[error("Missing updated storage key")]
    MissingStorageKey,
    #[error("Missing updated storage value")]
    MissingStorageValue,
    #[error("Missing new updated class hash")]
    MissingNewClassHash,
    #[error("Missing class hash")]
    MissingClassHash,
    #[error("Missing compiled class hash")]
    MissingCompiledClassHash,
    #[error("Invalid value")]
    InvalidValue,
    #[error("Invalid metadata")]
    InvalidMetadata,
    #[error(transparent)]
    ParseInt(#[from] ParseIntError),
}

/// This function doesn't enforce that the resulting [Vec] is of a certain length.
///
/// In a scenario where the state diffs of a block corresponds to a single data availability's
/// blob object (eg an EIP4844 blob), it should be the sequencer's responsibility to ensure that
/// the state diffs should fit inside the single blob object.
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

/// Similar to the [encode_state_updates] function, this function doesn't enforce that the input
/// [BigUint] values are of a certain length either.
///
/// # Errors
///
/// Will return an error if the list is already exhausted while decoding the intermediary fields
/// that are expected to exist.
pub fn decode_state_updates(value: &[BigUint]) -> Result<StateUpdates, EncodingError> {
    let mut state_updates = StateUpdates::default();

    if value.is_empty() {
        return Ok(state_updates);
    }

    let mut iter = value.iter();

    let total_updates = iter.next().ok_or(EncodingError::MissingUpdatesCount)?;
    let total_updates = total_updates.to_usize().ok_or(EncodingError::InvalidValue)?;

    for _ in 0..total_updates {
        let address = iter.next().ok_or(EncodingError::MissingAddress)?;
        let address: ContractAddress = Felt::from(address).into();

        let metadata = iter.next().ok_or(EncodingError::MissingMetadata)?;
        let metadata = Metadata::decode(metadata)?;

        let class_hash = if metadata.class_information_flag {
            let hash = iter.next().ok_or(EncodingError::MissingNewClassHash)?;
            Some(Felt::from(hash))
        } else {
            None
        };

        let mut storages = BTreeMap::new();

        for _ in 0..metadata.total_storage_updates {
            let key = iter.next().ok_or(EncodingError::MissingStorageKey)?;
            let key = StorageKey::from(key);

            let value = iter.next().ok_or(EncodingError::MissingStorageValue)?;
            let value = StorageValue::from(value);

            storages.insert(key, value);
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

    let total_declarations = iter.next().ok_or(EncodingError::MissingDeclarationsCount)?;
    let total_declarations = total_declarations.to_usize().ok_or(EncodingError::InvalidValue)?;

    for _ in 0..total_declarations {
        let class_hash = iter.next().ok_or(EncodingError::MissingClassHash)?;
        let class_hash = ClassHash::from(class_hash);

        let compiled_class_hash = iter.next().ok_or(EncodingError::MissingCompiledClassHash)?;
        let compiled_class_hash = CompiledClassHash::from(compiled_class_hash);

        state_updates.declared_classes.insert(class_hash, compiled_class_hash);
    }

    Ok(state_updates)
}

/// Metadata information about the contract update.
// Encoding format:
//
// ┌───────────────┬───────────────┬───────────────┬───────────────────────────┐
// │    padding    │  class flag   │   new nonce   │   no. storage updates     │
// ├───────────────┼───────────────┼───────────────┼───────────────────────────┤
// │    127 bits   │    1 bit      │    64 bits    │         64 bits           │
// └───────────────┴───────────────┴───────────────┴───────────────────────────┘
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
    // TODO: improve errors?
    fn decode(word: &BigUint) -> Result<Self, EncodingError> {
        // expand to 256 bits if needed
        let bits = format!("{word:0>256b}");

        let flag = bits.get(127..(127 + 1)).ok_or(EncodingError::InvalidMetadata)?;
        let flag = u8::from_str_radix(flag, 2)?;
        let class_information_flag = flag == 1;

        let nonce = bits.get(128..(128 + 64)).ok_or(EncodingError::InvalidMetadata)?;
        let nonce = u64::from_str_radix(nonce, 2)?;
        let nonce = Felt::from_u64(nonce).ok_or(EncodingError::InvalidMetadata)?;
        let new_nonce = if nonce == Felt::ZERO { None } else { Some(nonce) };

        let total = bits.get(192..(192 + 64)).ok_or(EncodingError::InvalidMetadata)?;
        let total_storage_updates = usize::from_str_radix(total, 2)?;

        Ok(Self { class_information_flag, new_nonce, total_storage_updates })
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

        let encoded = Metadata::decode(&metadata).unwrap();
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

        let state_updates = super::decode_state_updates(&input).unwrap();

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
