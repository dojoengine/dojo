pub mod dev;
pub mod saya;
pub mod starknet;
pub mod torii;

/// List of APIs supported by Katana.
#[derive(Debug, Copy, Clone)]
pub enum ApiKind {
    Starknet,
    Torii,
    Dev,
    Saya,
}
