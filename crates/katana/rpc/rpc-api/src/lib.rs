pub mod katana;
pub mod saya;
pub mod starknet;

/// List of APIs supported by Katana.
#[derive(Debug, Copy, Clone)]
pub enum ApiKind {
    Starknet,
    Katana,
    Saya,
}
