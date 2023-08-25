use crate::commands::Command;
use std::{fs, io};
use std::path::{Path, PathBuf};
use scarb::core::Config;

#[derive(Args, Debug)]
pub struct AddArgs {};

impl AddArgs {

  fn run(self, config : &Config) -> Result<()> {

    let world_path = &config.world_path;

    // Generate path to executor file
    let executor_path = world_path.join("Executor.cairo");

    // Check if Executor.cairo already exists
    // if Path::new("Executor.cairo").exists() {
    //   return Err(eyre::eyre!("Executor.cairo already exists"));
    // }

    // Check if executor already exists
    if executor_path.exists() {
        io::Error::new(io::ErrorKind::Other, "Executor.cairo already exists",)
      }

    // Initialize Executor.cairo 
    fs::write(executor_path, DEFAULT_EXECUTOR_CODE)?;

    // Print message
    config.ui().print("Initialized Executor.cairo");

    Ok(())
  }

}

// Default executor code
const DEFAULT_EXECUTOR_CODE: &str = r#"
use starknet::ClassHash;

use dojo::world::Context;

#[starknet::interface]
trait IExecutor<T> {
    fn execute(self: @T, class_hash: ClassHash, calldata: Span<felt252>) -> Span<felt252>;
    fn call(
        self: @T, class_hash: ClassHash, entrypoint: felt252, calldata: Span<felt252>
    ) -> Span<felt252>;
}

#[starknet::contract]
mod executor {
    use array::{ArrayTrait, SpanTrait};
    use option::OptionTrait;
    use starknet::ClassHash;

    use super::IExecutor;

    const EXECUTE_ENTRYPOINT: felt252 =
        0x0240060cdb34fcc260f41eac7474ee1d7c80b7e3607daff9ac67c7ea2ebb1c44;

    #[storage]
    struct Storage {}

    #[external(v0)]
    impl Executor of IExecutor<ContractState> {
        /// Executes a System by calling its execute entrypoint.
        ///
        /// # Arguments
        ///
        /// * `class_hash` - Class Hash of the System.
        /// * `calldata` - Calldata to pass to the System.
        ///
        /// # Returns
        ///
        /// The return value of the System's execute entrypoint.
        fn execute(
            self: @ContractState, class_hash: ClassHash, calldata: Span<felt252>
        ) -> Span<felt252> {
            starknet::syscalls::library_call_syscall(class_hash, EXECUTE_ENTRYPOINT, calldata)
                .unwrap_syscall()
        }

        /// Call the provided `entrypoint` method on the given `class_hash`.
        ///
        /// # Arguments
        ///
        /// * `class_hash` - Class Hash to call.
        /// * `entrypoint` - Entrypoint to call.
        /// * `calldata` - The calldata to pass.
        ///
        /// # Returns
        ///
        /// The return value of the call.
        fn call(
            self: @ContractState,
            class_hash: ClassHash,
            entrypoint: felt252,
            calldata: Span<felt252>
        ) -> Span<felt252> {
            starknet::syscalls::library_call_syscall(class_hash, entrypoint, calldata)
                .unwrap_syscall()
        }
    }
}
"#;