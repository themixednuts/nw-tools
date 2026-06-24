use std::path::Path;

use anyhow::{Result, bail};

use crate::{
    CodegenContext, Diagnostic, MissingReflectedBodyPlaceholder, RustCodegenPlanner,
    RustSourceEmitter, RustStandaloneProjectFile, SerializeCodegenRootMode,
    SerializeCodegenRootSelection, SerializeContextCompiler, Severity,
    complete_known_missing_reflected_bodies, resolve_codegen_root_type_ids,
};

/// Request for emitting generated Rust modules inside an existing crate.
#[derive(Debug, Clone)]
pub struct IntegratedRustProjectRequest<'a> {
    serialize_context: &'a Path,
    module_descriptors: Option<&'a Path>,
    roots: Vec<String>,
    native_default_roots: Vec<String>,
}

impl<'a> IntegratedRustProjectRequest<'a> {
    #[must_use]
    pub fn new(serialize_context: &'a Path) -> Self {
        Self {
            serialize_context,
            module_descriptors: None,
            roots: Vec::new(),
            native_default_roots: Vec::new(),
        }
    }

    #[must_use]
    pub fn module_descriptors(mut self, module_descriptors: Option<&'a Path>) -> Self {
        self.module_descriptors = module_descriptors;
        self
    }

    #[must_use]
    pub fn roots(mut self, roots: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.roots = roots.into_iter().map(Into::into).collect();
        self
    }

    #[must_use]
    pub fn native_default_roots(
        mut self,
        native_default_roots: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        self.native_default_roots = native_default_roots.into_iter().map(Into::into).collect();
        self
    }

    pub fn generate(&self, context: &CodegenContext) -> Result<IntegratedRustProject> {
        let compile_unit = SerializeContextCompiler::compile_from_paths(
            self.serialize_context,
            self.module_descriptors,
            None::<&Path>,
            context,
        )?;
        reject_compile_errors(&compile_unit.diagnostics)?;

        let root_type_ids = resolve_codegen_root_type_ids(
            &compile_unit.codegen_unit,
            self.roots.iter().map(String::as_str),
        )?;
        let native_default_type_ids = resolve_codegen_root_type_ids(
            &compile_unit.codegen_unit,
            self.native_default_roots.iter().map(String::as_str),
        )?;
        let selected = SerializeCodegenRootSelection::new(SerializeCodegenRootMode::Runtime)
            .with_explicit_roots(root_type_ids)
            .select_unit(&compile_unit.codegen_unit);
        let completed =
            complete_known_missing_reflected_bodies(selected, compile_unit.codegen_unit.clone());
        let rust_unit = RustCodegenPlanner::default()
            .without_default_derive_for(native_default_type_ids)
            .plan_serialize_codegen_units(&completed.emitted, &completed.context, context);
        let project = RustSourceEmitter::emit_integrated_project(&rust_unit, context)?;

        Ok(IntegratedRustProject {
            files: project.files,
            placeholders: completed.placeholders,
            diagnostics: compile_unit.diagnostics,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntegratedRustProject {
    pub files: Vec<RustStandaloneProjectFile>,
    pub placeholders: Vec<MissingReflectedBodyPlaceholder>,
    pub diagnostics: Vec<Diagnostic>,
}

fn reject_compile_errors(diagnostics: &[Diagnostic]) -> Result<()> {
    let errors = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.severity == Severity::Error)
        .collect::<Vec<_>>();
    if errors.is_empty() {
        return Ok(());
    }

    for diagnostic in errors.iter().take(16) {
        eprintln!("error: {}", diagnostic.message);
    }
    if errors.len() > 16 {
        eprintln!("... {} more error(s)", errors.len() - 16);
    }
    bail!("SerializeContext compile emitted errors; refusing to generate source");
}
