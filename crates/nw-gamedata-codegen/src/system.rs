//! Planning surface for future native GameData system codegen.
//!
//! System codegen follows the same rule as manager codegen: emit only validated
//! native classes with fully mapped behavior. This module is a typed plan
//! boundary, not a place for guessed system surfaces.

use crate::native::{
    NativeClassSpec, NativeClassSpecError, NativeCodegenFile, NativeCodegenOutput,
    NativeCodegenPlan, validate_native_class_spec_inputs,
};
use crate::symbols::{GhidraClassPath, GhidraFunctionPath, RustTypePath};
use crate::target::GameDataTargetLanguage;

pub type SystemCodegenFile = NativeCodegenFile;
pub type SystemCodegenOutput = NativeCodegenOutput;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeSystemSpec {
    native: NativeClassSpec<RustTypePath>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SystemCodegenPlan {
    native: NativeCodegenPlan<NativeSystemSpec>,
}

impl SystemCodegenPlan {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            native: NativeCodegenPlan::new(),
        }
    }

    #[must_use]
    pub fn from_native_systems(systems: Vec<NativeSystemSpec>) -> Self {
        Self {
            native: NativeCodegenPlan::from_specs(systems),
        }
    }

    #[must_use]
    pub fn systems(&self) -> &[NativeSystemSpec] {
        self.native.specs()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.native.is_empty()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.native.len()
    }
}

impl NativeSystemSpec {
    #[must_use]
    pub fn new(
        ghidra_class: GhidraClassPath,
        rust_type: RustTypePath,
        managers: Vec<RustTypePath>,
        ghidra_functions: Vec<GhidraFunctionPath>,
    ) -> Result<Self, NativeClassSpecError> {
        validate_native_class_spec_inputs(
            "native system",
            "manager",
            &ghidra_class,
            &rust_type,
            &managers,
            &ghidra_functions,
        )?;
        Ok(Self {
            native: NativeClassSpec::new(ghidra_class, rust_type, managers, ghidra_functions),
        })
    }

    #[must_use]
    pub const fn ghidra_class(&self) -> &GhidraClassPath {
        self.native.ghidra_class()
    }

    #[must_use]
    pub const fn rust_type(&self) -> &RustTypePath {
        self.native.rust_type()
    }

    #[must_use]
    pub fn managers(&self) -> &[RustTypePath] {
        self.native.references()
    }

    #[must_use]
    pub fn ghidra_functions(&self) -> &[GhidraFunctionPath] {
        self.native.ghidra_functions()
    }
}

pub trait SystemEmitter {
    fn target_language(&self) -> GameDataTargetLanguage;

    fn emit_systems(&self, plan: &SystemCodegenPlan) -> anyhow::Result<SystemCodegenOutput>;
}
