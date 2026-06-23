use std::borrow::Cow;

use crate::compiler::SerializeCodegenView;
use crate::go::layout::GoStandaloneLayoutReport;
use crate::go::source::{GoSourceEmitError, GoSourceEmitter, GoSourceOptions, GoStandaloneProject};
use crate::ir::{SerializeCodegenSelection, SerializeCodegenUnit};
use crate::layout::LayoutAnalysisReport;
use crate::rust::integrate::source_index::RustSourceTypeIndex;
use crate::rust::layout::RustStandaloneLayoutReport;
use crate::rust::source::{RustSourceEmitError, RustSourceEmitter, RustStandaloneProject};
use crate::typescript::layout::TypeScriptStandaloneLayoutReport;
use crate::typescript::source::{
    TypeScriptSourceEmitError, TypeScriptSourceEmitter, TypeScriptSourceOptions,
    TypeScriptStandaloneProject, TypeScriptStandaloneProjectOptions,
};
use crate::{CodegenContext, RustCodegenPlanner, RustCodegenUnit};

impl<'a> SerializeCodegenView<'a> {
    #[must_use]
    pub fn new(
        context_unit: &'a SerializeCodegenUnit,
        selection: SerializeCodegenSelection,
    ) -> Self {
        match selection {
            SerializeCodegenSelection::All => Self::all(context_unit),
            SerializeCodegenSelection::Components
            | SerializeCodegenSelection::ComponentFamilies
            | SerializeCodegenSelection::RuntimeRoots => Self {
                emitted_unit: Cow::Owned(context_unit.select(selection)),
                context_unit,
            },
        }
    }

    #[must_use]
    pub fn all(context_unit: &'a SerializeCodegenUnit) -> Self {
        Self {
            emitted_unit: Cow::Borrowed(context_unit),
            context_unit,
        }
    }

    #[must_use]
    pub fn emitted_unit(&self) -> &SerializeCodegenUnit {
        self.emitted_unit.as_ref()
    }

    #[must_use]
    pub const fn context_unit(&self) -> &SerializeCodegenUnit {
        self.context_unit
    }

    #[must_use]
    pub fn layout_analysis_report(&self) -> LayoutAnalysisReport {
        LayoutAnalysisReport::from_codegen_unit_with_context(
            self.emitted_unit(),
            self.context_unit(),
        )
    }

    #[must_use]
    pub fn rust_codegen_unit(&self) -> RustCodegenUnit {
        RustCodegenPlanner::default()
            .plan_serialize_codegen_unit_with_context(self.emitted_unit(), self.context_unit())
    }

    #[must_use]
    pub fn rust_codegen_unit_with_source_types(
        &self,
        source_types: RustSourceTypeIndex,
    ) -> RustCodegenUnit {
        RustCodegenPlanner::default()
            .with_source_type_index(source_types)
            .plan_serialize_codegen_unit_with_context(self.emitted_unit(), self.context_unit())
    }

    #[must_use]
    pub fn standalone_rust_codegen_unit(&self) -> RustCodegenUnit {
        RustCodegenPlanner::standalone()
            .plan_serialize_codegen_unit_with_context(self.emitted_unit(), self.context_unit())
    }

    #[must_use]
    pub fn standalone_rust_layout_report(&self) -> RustStandaloneLayoutReport {
        RustStandaloneLayoutReport::from_codegen_unit(&self.standalone_rust_codegen_unit())
    }

    pub fn emit_rust_source(
        &self,
        context: &CodegenContext,
    ) -> Result<String, RustSourceEmitError> {
        RustSourceEmitter::emit_unit(&self.rust_codegen_unit(), context)
    }

    pub fn emit_rust_source_with_source_types(
        &self,
        source_types: RustSourceTypeIndex,
        context: &CodegenContext,
    ) -> Result<String, RustSourceEmitError> {
        RustSourceEmitter::emit_unit(
            &self.rust_codegen_unit_with_source_types(source_types),
            context,
        )
    }

    pub fn emit_standalone_rust_project(
        &self,
        context: &CodegenContext,
    ) -> Result<RustStandaloneProject, RustSourceEmitError> {
        RustSourceEmitter::emit_standalone_project(&self.standalone_rust_codegen_unit(), context)
    }

    pub fn emit_typescript_source(&self) -> Result<String, TypeScriptSourceEmitError> {
        TypeScriptSourceEmitter::emit_unit(self.emitted_unit())
    }

    pub fn emit_typescript_source_with_options(
        &self,
        options: &TypeScriptSourceOptions,
    ) -> Result<String, TypeScriptSourceEmitError> {
        TypeScriptSourceEmitter.emit_with_options(self.emitted_unit(), options)
    }

    #[must_use]
    pub fn standalone_typescript_layout_report(&self) -> TypeScriptStandaloneLayoutReport {
        TypeScriptStandaloneLayoutReport::from_codegen_unit_with_context(
            self.emitted_unit(),
            self.context_unit(),
        )
    }

    pub fn emit_standalone_typescript_project(
        &self,
        context: &CodegenContext,
    ) -> Result<TypeScriptStandaloneProject, TypeScriptSourceEmitError> {
        TypeScriptSourceEmitter.emit_standalone_project_with_context(
            self.emitted_unit(),
            self.context_unit(),
            context,
        )
    }

    pub fn emit_standalone_typescript_project_with_options(
        &self,
        options: &TypeScriptStandaloneProjectOptions,
        context: &CodegenContext,
    ) -> Result<TypeScriptStandaloneProject, TypeScriptSourceEmitError> {
        TypeScriptSourceEmitter.emit_standalone_project_with_options_and_context(
            self.emitted_unit(),
            self.context_unit(),
            options,
            context,
        )
    }

    pub fn emit_go_source(&self) -> Result<String, GoSourceEmitError> {
        GoSourceEmitter::emit_unit(self.emitted_unit())
    }

    pub fn emit_go_source_with_options(
        &self,
        options: &GoSourceOptions,
    ) -> Result<String, GoSourceEmitError> {
        GoSourceEmitter.emit(self.emitted_unit(), options)
    }

    #[must_use]
    pub fn standalone_go_layout_report(&self) -> GoStandaloneLayoutReport {
        GoStandaloneLayoutReport::from_codegen_unit_with_context(
            self.emitted_unit(),
            self.context_unit(),
        )
    }

    pub fn emit_standalone_go_project(
        &self,
        module_path: &str,
        package_name: &str,
        context: &CodegenContext,
    ) -> Result<GoStandaloneProject, GoSourceEmitError> {
        GoSourceEmitter::default().emit_standalone_project_with_context(
            self.emitted_unit(),
            self.context_unit(),
            module_path,
            package_name,
            context,
        )
    }
}
