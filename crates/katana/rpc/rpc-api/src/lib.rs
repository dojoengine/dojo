pub mod dev;
pub mod katana;
pub mod starknet;
pub mod torii;

/// List of APIs supported by Katana.
#[derive(Debug, Copy, Clone)]
pub enum ApiKind {
    Starknet,
    Katana,
    Torii,
    Dev,
}
