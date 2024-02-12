#[cfg(target_arch = "wasm32")]
extern crate wasm_prost as prost;
#[cfg(target_arch = "wasm32")]
extern crate wasm_tonic as tonic;

pub mod types;

#[cfg(feature = "client")]
pub mod client;

#[cfg(feature = "server")]
pub mod server;

pub mod proto {
    pub mod world {
        tonic::include_proto!("world");

        #[cfg(feature = "server")]
        pub(crate) const FILE_DESCRIPTOR_SET: &[u8] =
            tonic::include_file_descriptor_set!("world_descriptor");
    }
    pub mod types {
        tonic::include_proto!("types");
    }
}
