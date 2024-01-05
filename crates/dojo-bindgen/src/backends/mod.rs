pub mod typescript;
pub mod unity;

#[derive(Debug)]
pub enum Backend {
    Typescript,
    Unity,
}
