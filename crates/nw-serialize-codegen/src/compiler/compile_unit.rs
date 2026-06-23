use crate::compiler::{CompileUnit, SerializeCodegenView};
use crate::go::layout::GoStandaloneLayoutReport;
use crate::go::source::{GoSourceEmitError, GoSourceOptions, GoStandaloneProject};
use crate::ir::{SerializeCodegenSelection, SerializeCodegenUnit};
use crate::layout::LayoutAnalysisReport;
use crate::lint::Severity;
use crate::rust::integrate::{
    RustIntegrationError, RustIntegrationPlan, RustIntegrationPlanner, RustItemPathResolver,
    RustSourceInventory, source_index::RustSourceTypeIndex,
};
use crate::rust::layout::RustStandaloneLayoutReport;
use crate::rust::source::{RustSourceEmitError, RustStandaloneProject};
use crate::typescript::layout::TypeScriptStandaloneLayoutReport;
use crate::typescript::source::{
    TypeScriptSourceEmitError, TypeScriptSourceOptions, TypeScriptStandaloneProject,
    TypeScriptStandaloneProjectOptions,
};
use crate::{CodegenContext, RustCodegenUnit};

impl CompileUnit {
    #[must_use]
    pub fn codegen_view(&self, selection: SerializeCodegenSelection) -> SerializeCodegenView<'_> {
        SerializeCodegenView::new(&self.codegen_unit, selection)
    }

    #[must_use]
    pub fn full_codegen_view(&self) -> SerializeCodegenView<'_> {
        SerializeCodegenView::all(&self.codegen_unit)
    }

    #[must_use]
    pub fn selected_codegen_unit(
        &self,
        selection: SerializeCodegenSelection,
    ) -> SerializeCodegenUnit {
        self.codegen_view(selection).emitted_unit().clone()
    }

    #[must_use]
    pub fn layout_analysis_report(&self) -> LayoutAnalysisReport {
        self.full_codegen_view().layout_analysis_report()
    }

    #[must_use]
    pub fn selected_layout_analysis_report(
        &self,
        selection: SerializeCodegenSelection,
    ) -> LayoutAnalysisReport {
        self.codegen_view(selection).layout_analysis_report()
    }

    #[must_use]
    pub fn rust_codegen_unit(&self) -> RustCodegenUnit {
        self.full_codegen_view().rust_codegen_unit()
    }

    #[must_use]
    pub fn rust_codegen_unit_with_source_types(
        &self,
        source_types: RustSourceTypeIndex,
    ) -> RustCodegenUnit {
        self.full_codegen_view()
            .rust_codegen_unit_with_source_types(source_types)
    }

    #[must_use]
    pub fn standalone_rust_codegen_unit(&self) -> RustCodegenUnit {
        self.full_codegen_view().standalone_rust_codegen_unit()
    }

    #[must_use]
    pub fn selected_rust_codegen_unit(
        &self,
        selection: SerializeCodegenSelection,
    ) -> RustCodegenUnit {
        self.codegen_view(selection).rust_codegen_unit()
    }

    #[must_use]
    pub fn selected_rust_codegen_unit_with_source_types(
        &self,
        selection: SerializeCodegenSelection,
        source_types: RustSourceTypeIndex,
    ) -> RustCodegenUnit {
        self.codegen_view(selection)
            .rust_codegen_unit_with_source_types(source_types)
    }

    #[must_use]
    pub fn selected_standalone_rust_codegen_unit(
        &self,
        selection: SerializeCodegenSelection,
    ) -> RustCodegenUnit {
        self.codegen_view(selection).standalone_rust_codegen_unit()
    }

    #[must_use]
    pub fn standalone_rust_layout_report(&self) -> RustStandaloneLayoutReport {
        self.full_codegen_view().standalone_rust_layout_report()
    }

    #[must_use]
    pub fn selected_standalone_rust_layout_report(
        &self,
        selection: SerializeCodegenSelection,
    ) -> RustStandaloneLayoutReport {
        self.codegen_view(selection).standalone_rust_layout_report()
    }

    pub fn emit_rust_source(
        &self,
        context: &CodegenContext,
    ) -> Result<String, RustSourceEmitError> {
        self.full_codegen_view().emit_rust_source(context)
    }

    pub fn emit_rust_source_with_source_types(
        &self,
        source_types: RustSourceTypeIndex,
        context: &CodegenContext,
    ) -> Result<String, RustSourceEmitError> {
        self.full_codegen_view()
            .emit_rust_source_with_source_types(source_types, context)
    }

    pub fn emit_selected_rust_source(
        &self,
        selection: SerializeCodegenSelection,
        context: &CodegenContext,
    ) -> Result<String, RustSourceEmitError> {
        self.codegen_view(selection).emit_rust_source(context)
    }

    pub fn emit_selected_rust_source_with_source_types(
        &self,
        selection: SerializeCodegenSelection,
        source_types: RustSourceTypeIndex,
        context: &CodegenContext,
    ) -> Result<String, RustSourceEmitError> {
        self.codegen_view(selection)
            .emit_rust_source_with_source_types(source_types, context)
    }

    pub fn emit_standalone_rust_project(
        &self,
        context: &CodegenContext,
    ) -> Result<RustStandaloneProject, RustSourceEmitError> {
        self.full_codegen_view()
            .emit_standalone_rust_project(context)
    }

    pub fn emit_selected_standalone_rust_project(
        &self,
        selection: SerializeCodegenSelection,
        context: &CodegenContext,
    ) -> Result<RustStandaloneProject, RustSourceEmitError> {
        self.codegen_view(selection)
            .emit_standalone_rust_project(context)
    }

    pub fn emit_typescript_source(&self) -> Result<String, TypeScriptSourceEmitError> {
        self.full_codegen_view().emit_typescript_source()
    }

    #[must_use]
    pub fn standalone_typescript_layout_report(&self) -> TypeScriptStandaloneLayoutReport {
        self.full_codegen_view()
            .standalone_typescript_layout_report()
    }

    #[must_use]
    pub fn selected_standalone_typescript_layout_report(
        &self,
        selection: SerializeCodegenSelection,
    ) -> TypeScriptStandaloneLayoutReport {
        self.codegen_view(selection)
            .standalone_typescript_layout_report()
    }

    pub fn emit_selected_typescript_source(
        &self,
        selection: SerializeCodegenSelection,
    ) -> Result<String, TypeScriptSourceEmitError> {
        self.codegen_view(selection).emit_typescript_source()
    }

    pub fn emit_typescript_source_with_options(
        &self,
        options: &TypeScriptSourceOptions,
    ) -> Result<String, TypeScriptSourceEmitError> {
        self.full_codegen_view()
            .emit_typescript_source_with_options(options)
    }

    pub fn emit_selected_typescript_source_with_options(
        &self,
        selection: SerializeCodegenSelection,
        options: &TypeScriptSourceOptions,
    ) -> Result<String, TypeScriptSourceEmitError> {
        self.codegen_view(selection)
            .emit_typescript_source_with_options(options)
    }

    pub fn emit_standalone_typescript_project(
        &self,
    ) -> Result<TypeScriptStandaloneProject, TypeScriptSourceEmitError> {
        self.full_codegen_view()
            .emit_standalone_typescript_project()
    }

    pub fn emit_standalone_typescript_project_with_options(
        &self,
        options: &TypeScriptStandaloneProjectOptions,
    ) -> Result<TypeScriptStandaloneProject, TypeScriptSourceEmitError> {
        self.full_codegen_view()
            .emit_standalone_typescript_project_with_options(options)
    }

    pub fn emit_selected_standalone_typescript_project(
        &self,
        selection: SerializeCodegenSelection,
    ) -> Result<TypeScriptStandaloneProject, TypeScriptSourceEmitError> {
        self.codegen_view(selection)
            .emit_standalone_typescript_project()
    }

    pub fn emit_selected_standalone_typescript_project_with_options(
        &self,
        selection: SerializeCodegenSelection,
        options: &TypeScriptStandaloneProjectOptions,
    ) -> Result<TypeScriptStandaloneProject, TypeScriptSourceEmitError> {
        self.codegen_view(selection)
            .emit_standalone_typescript_project_with_options(options)
    }

    pub fn emit_go_source(&self) -> Result<String, GoSourceEmitError> {
        self.full_codegen_view().emit_go_source()
    }

    #[must_use]
    pub fn standalone_go_layout_report(&self) -> GoStandaloneLayoutReport {
        self.full_codegen_view().standalone_go_layout_report()
    }

    #[must_use]
    pub fn selected_standalone_go_layout_report(
        &self,
        selection: SerializeCodegenSelection,
    ) -> GoStandaloneLayoutReport {
        self.codegen_view(selection).standalone_go_layout_report()
    }

    pub fn emit_selected_go_source(
        &self,
        selection: SerializeCodegenSelection,
    ) -> Result<String, GoSourceEmitError> {
        self.codegen_view(selection).emit_go_source()
    }

    pub fn emit_go_source_with_options(
        &self,
        options: &GoSourceOptions,
    ) -> Result<String, GoSourceEmitError> {
        self.full_codegen_view()
            .emit_go_source_with_options(options)
    }

    pub fn emit_selected_go_source_with_options(
        &self,
        selection: SerializeCodegenSelection,
        options: &GoSourceOptions,
    ) -> Result<String, GoSourceEmitError> {
        self.codegen_view(selection)
            .emit_go_source_with_options(options)
    }

    pub fn emit_standalone_go_project(
        &self,
        module_path: &str,
        package_name: &str,
    ) -> Result<GoStandaloneProject, GoSourceEmitError> {
        self.full_codegen_view()
            .emit_standalone_go_project(module_path, package_name)
    }

    pub fn emit_selected_standalone_go_project(
        &self,
        selection: SerializeCodegenSelection,
        module_path: &str,
        package_name: &str,
    ) -> Result<GoStandaloneProject, GoSourceEmitError> {
        self.codegen_view(selection)
            .emit_standalone_go_project(module_path, package_name)
    }

    #[must_use]
    pub fn has_errors(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == Severity::Error)
    }

    pub fn plan_rust_integration(
        &self,
        inventory: &RustSourceInventory,
        paths: &impl RustItemPathResolver,
    ) -> Result<RustIntegrationPlan, RustIntegrationError> {
        let rust_unit =
            self.rust_codegen_unit_with_source_types(inventory.source_type_index().clone());
        RustIntegrationPlanner::default().plan(&rust_unit, inventory, paths)
    }
}
