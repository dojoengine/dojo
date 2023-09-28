#[cfg(target_arch = "wasm32")]
extern crate wasm_prost as prost;
#[cfg(target_arch = "wasm32")]
extern crate wasm_tonic as tonic;

pub mod conversion;

#[cfg(feature = "client")]
pub mod client;

#[cfg(feature = "server")]
pub mod server;

pub mod protos {
    pub mod world {
        tonic::include_proto!("world");
    }
    pub mod types {
        tonic::include_proto!("types");
    }
}
