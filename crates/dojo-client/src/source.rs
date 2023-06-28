use starknet::core::types::FieldElement;

use crate::storage::Storage;

pub trait Source {
    type Error;

    /// Load the World state from the remote source.
    fn load<S: Storage>(&self, address: FieldElement, state: &mut S) -> Result<(), Self::Error>;

    // /// Get the current head of the remote source to determine
    // /// how much it has synced with the remote state.
    // fn head(&self) -> Result<u64, Self::Error>;
    // /// Set the current head of the remote source.
    // fn set_head(&self, head: u64) -> Result<(), Self::Error>;
}

// pub struct JsonRpcSource<T> {
//     client: JsonRpcClient<T>,
// }

// impl<T> Source for JsonRpcSource<T>
// where
//     T: JsonRpcTransport,
// {
//     type Error = JsonRpcClientError<T>;

//     fn load<S: Storage>(&self, address: FieldElement, state: &mut S) -> Result<(), Self::Error> {
//         unimplemented!()
//     }
// }
