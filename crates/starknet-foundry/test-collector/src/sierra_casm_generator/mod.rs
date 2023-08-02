//! Basic runner for running a Sierra program on the vm.

use cairo_lang_sierra::extensions::core::{CoreLibfunc, CoreType};
use cairo_lang_sierra::extensions::ConcreteType;
use cairo_lang_sierra::program::Function;
use cairo_lang_sierra::program_registry::{ProgramRegistry, ProgramRegistryError};
use cairo_lang_sierra_ap_change::ApChangeError;
use cairo_lang_sierra_to_casm::compiler::CompilationError;
use cairo_vm::vm::errors::vm_errors::VirtualMachineError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum GeneratorError {
    #[error("Failed calculating gas usage, it is likely a call for `get_gas` is missing.")]
    FailedGasCalculation,
    #[error("Function with suffix `{suffix}` to run not found.")]
    MissingFunction { suffix: String },
    #[error("Function expects arguments of size {expected} and received {actual} instead.")]
    ArgumentsSizeMismatch { expected: usize, actual: usize },
    #[error(transparent)]
    ProgramRegistryError(#[from] Box<ProgramRegistryError>),
    #[error(transparent)]
    SierraCompilationError(#[from] CompilationError),
    #[error(transparent)]
    ApChangeError(#[from] ApChangeError),
    #[error(transparent)]
    VirtualMachineError(#[from] Box<VirtualMachineError>),
    #[error("At least one test expected but none detected.")]
    NoTestsDetected,
}

pub struct SierraCasmGenerator {
    /// The sierra program.
    sierra_program: cairo_lang_sierra::program::Program,
    /// Program registry for the Sierra program.
    sierra_program_registry: ProgramRegistry<CoreType, CoreLibfunc>,
}

#[allow(clippy::result_large_err)]
impl SierraCasmGenerator {
    pub fn new(
        sierra_program: cairo_lang_sierra::program::Program,
    ) -> Result<Self, GeneratorError> {
        let sierra_program_registry =
            ProgramRegistry::<CoreType, CoreLibfunc>::new(&sierra_program)?;
        Ok(Self {
            sierra_program,
            sierra_program_registry,
        })
    }

    // Copied from crates/cairo-lang-runner/src/lib.rs
    /// Finds first function ending with `name_suffix`.
    pub fn find_function(&self, name_suffix: &str) -> Result<&Function, GeneratorError> {
        self.sierra_program
            .funcs
            .iter()
            .find(|f| {
                if let Some(name) = &f.id.debug_name {
                    name.ends_with(name_suffix)
                } else {
                    false
                }
            })
            .ok_or_else(|| GeneratorError::MissingFunction {
                suffix: name_suffix.to_owned(),
            })
    }

    #[must_use]
    pub fn get_info(
        &self,
        ty: &cairo_lang_sierra::ids::ConcreteTypeId,
    ) -> &cairo_lang_sierra::extensions::types::TypeInfo {
        self.sierra_program_registry.get_type(ty).unwrap().info()
    }
}
