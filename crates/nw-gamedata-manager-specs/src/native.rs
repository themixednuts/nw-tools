//! Shared model for validated native GameData codegen inputs.
//!
//! This is intentionally a small typed IR layer. It describes native classes
//! that have already been mapped; it does not discover classes or invent
//! manager/system surfaces.

use crate::symbols::{GhidraClassPath, GhidraFunctionPath, RustTypePath};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeClassSpec<Reference> {
    ghidra_class: GhidraClassPath,
    rust_type: RustTypePath,
    references: Vec<Reference>,
    ghidra_functions: Vec<GhidraFunctionPath>,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum NativeClassSpecError {
    #[error(
        "{kind} `{rust_type}` for `{ghidra_class}` must reference at least one {reference_kind}"
    )]
    MissingReferences {
        kind: &'static str,
        reference_kind: &'static str,
        ghidra_class: GhidraClassPath,
        rust_type: RustTypePath,
    },

    #[error("{kind} `{rust_type}` for `{ghidra_class}` must include validated Ghidra functions")]
    MissingGhidraFunctions {
        kind: &'static str,
        ghidra_class: GhidraClassPath,
        rust_type: RustTypePath,
    },
}

impl<Reference> NativeClassSpec<Reference> {
    #[must_use]
    pub fn new(
        ghidra_class: GhidraClassPath,
        rust_type: RustTypePath,
        references: Vec<Reference>,
        ghidra_functions: Vec<GhidraFunctionPath>,
    ) -> Self {
        Self {
            ghidra_class,
            rust_type,
            references,
            ghidra_functions,
        }
    }

    #[must_use]
    pub const fn ghidra_class(&self) -> &GhidraClassPath {
        &self.ghidra_class
    }

    #[must_use]
    pub const fn rust_type(&self) -> &RustTypePath {
        &self.rust_type
    }

    #[must_use]
    pub fn references(&self) -> &[Reference] {
        &self.references
    }

    #[must_use]
    pub fn ghidra_functions(&self) -> &[GhidraFunctionPath] {
        &self.ghidra_functions
    }
}

pub fn validate_native_class_spec_inputs<Reference>(
    kind: &'static str,
    reference_kind: &'static str,
    ghidra_class: &GhidraClassPath,
    rust_type: &RustTypePath,
    references: &[Reference],
    ghidra_functions: &[GhidraFunctionPath],
) -> Result<(), NativeClassSpecError> {
    if references.is_empty() {
        return Err(NativeClassSpecError::MissingReferences {
            kind,
            reference_kind,
            ghidra_class: ghidra_class.clone(),
            rust_type: rust_type.clone(),
        });
    }
    if ghidra_functions.is_empty() {
        return Err(NativeClassSpecError::MissingGhidraFunctions {
            kind,
            ghidra_class: ghidra_class.clone(),
            rust_type: rust_type.clone(),
        });
    }
    Ok(())
}
