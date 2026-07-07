//! Shared model for validated native GameData codegen outputs.
//!
//! The typed IR for mapped native classes ([`NativeClassSpec`]) lives in
//! `newworld-gamedata-manager-specs` and is re-exported here; this module
//! keeps the codegen-side plan and output containers.

use std::path::PathBuf;

use crate::target::GameDataTargetLanguage;

pub use nw_gamedata_manager_specs::native::{
    NativeClassSpec, NativeClassSpecError, validate_native_class_spec_inputs,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeCodegenPlan<Spec> {
    specs: Vec<Spec>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeCodegenOutput {
    target_language: GameDataTargetLanguage,
    files: Vec<NativeCodegenFile>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeCodegenFile {
    path: PathBuf,
    contents: String,
}

impl<Spec> NativeCodegenPlan<Spec> {
    #[must_use]
    pub const fn new() -> Self {
        Self { specs: Vec::new() }
    }

    #[must_use]
    pub fn from_specs(specs: Vec<Spec>) -> Self {
        Self { specs }
    }

    #[must_use]
    pub fn specs(&self) -> &[Spec] {
        &self.specs
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.specs.is_empty()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.specs.len()
    }
}

impl<Spec> Default for NativeCodegenPlan<Spec> {
    fn default() -> Self {
        Self::new()
    }
}

impl NativeCodegenOutput {
    #[must_use]
    pub fn new(target_language: GameDataTargetLanguage, files: Vec<NativeCodegenFile>) -> Self {
        Self {
            target_language,
            files,
        }
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    #[must_use]
    pub const fn target_language(&self) -> GameDataTargetLanguage {
        self.target_language
    }

    #[must_use]
    pub fn files(&self) -> &[NativeCodegenFile] {
        &self.files
    }

    #[must_use]
    pub fn into_files(self) -> Vec<NativeCodegenFile> {
        self.files
    }
}

impl NativeCodegenFile {
    #[must_use]
    pub fn new(path: impl Into<PathBuf>, contents: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            contents: contents.into(),
        }
    }

    #[must_use]
    pub fn path(&self) -> &std::path::Path {
        &self.path
    }

    #[must_use]
    pub fn contents(&self) -> &str {
        &self.contents
    }

    #[must_use]
    pub fn into_parts(self) -> (PathBuf, String) {
        (self.path, self.contents)
    }
}
