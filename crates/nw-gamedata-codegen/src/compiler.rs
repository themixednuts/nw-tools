use anyhow::Result;
use nw_datasheet::game_system::GameSystemDataTables as GameSystemCatalog;

use crate::game_system_schema::GameSystemDataTablesSchemaReport as GameSystemCatalogSchemaReport;
use crate::plan::GameDataCodegenPlan;
use crate::schema::{GameDataCompileMode, schema_report_for_mode};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameDataCompiler {
    mode: GameDataCompileMode,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GameDataCompileUnit {
    schema_report: GameSystemCatalogSchemaReport,
    strict_schema_report: GameSystemCatalogSchemaReport,
    codegen_plan: GameDataCodegenPlan,
}

impl Default for GameDataCompiler {
    fn default() -> Self {
        Self::source_format()
    }
}

impl GameDataCompiler {
    #[must_use]
    pub fn new(mode: GameDataCompileMode) -> Self {
        Self { mode }
    }

    #[must_use]
    pub fn strict() -> Self {
        Self::new(GameDataCompileMode::Strict)
    }

    #[must_use]
    pub fn source_format() -> Self {
        Self::new(GameDataCompileMode::SourceFormat)
    }

    #[must_use]
    pub const fn mode(&self) -> GameDataCompileMode {
        self.mode
    }

    #[must_use]
    pub fn compile_unit(&self, catalog: &GameSystemCatalog) -> GameDataCompileUnit {
        let schema_report = schema_report_for_mode(catalog, self.mode);
        let strict_schema_report = if self.mode == GameDataCompileMode::Strict {
            schema_report.clone()
        } else {
            schema_report_for_mode(catalog, GameDataCompileMode::Strict)
        };
        let codegen_plan = GameDataCodegenPlan::from_schema_report(self.mode, &schema_report);
        GameDataCompileUnit::new(schema_report, strict_schema_report, codegen_plan)
    }

    #[must_use]
    pub fn codegen_plan(&self, catalog: &GameSystemCatalog) -> GameDataCodegenPlan {
        self.compile_unit(catalog).into_codegen_plan()
    }
}

impl GameDataCompileUnit {
    #[must_use]
    pub const fn new(
        schema_report: GameSystemCatalogSchemaReport,
        strict_schema_report: GameSystemCatalogSchemaReport,
        codegen_plan: GameDataCodegenPlan,
    ) -> Self {
        Self {
            schema_report,
            strict_schema_report,
            codegen_plan,
        }
    }

    #[must_use]
    pub const fn schema_report(&self) -> &GameSystemCatalogSchemaReport {
        &self.schema_report
    }

    #[must_use]
    pub const fn strict_schema_report(&self) -> &GameSystemCatalogSchemaReport {
        &self.strict_schema_report
    }

    #[must_use]
    pub const fn codegen_plan_ref(&self) -> &GameDataCodegenPlan {
        &self.codegen_plan
    }

    #[must_use]
    pub fn into_codegen_plan(self) -> GameDataCodegenPlan {
        self.codegen_plan
    }

    #[must_use]
    pub fn has_schema_diagnostics(&self) -> bool {
        !self.schema_report.diagnostics.is_empty()
    }

    pub fn emit_with<E>(&self, emitter: &E) -> Result<crate::emit::GameDataCodegenOutput>
    where
        E: crate::emit::GameDataEmitter + ?Sized,
    {
        emitter.emit(self)
    }
}
