use anyhow::Result;
use nw_datasheet::game_system::GameSystemDataTables as GameSystemCatalog;

use crate::game_system_schema::GameSystemDataTablesSchemaReport as GameSystemCatalogSchemaReport;
use crate::manager::ManagerCodegenPlan;
use crate::plan::GameDataCodegenPlan;
use crate::schema::{GameDataCompileMode, schema_report_for_mode};
use crate::system::SystemCodegenPlan;
use crate::target::{GameDataTargetLanguage, GameDataTargetPlan, GameDataTargetPlanError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameDataCompiler {
    options: GameDataCompilerOptions,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GameDataCompileUnit {
    schema_report: GameSystemCatalogSchemaReport,
    strict_schema_report: GameSystemCatalogSchemaReport,
    codegen_plan: GameDataCodegenPlan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameDataCompilerOptions {
    mode: GameDataCompileMode,
    targets: Vec<GameDataTargetPlan>,
    managers: ManagerCodegenPlan,
    systems: SystemCodegenPlan,
}

impl Default for GameDataCompiler {
    fn default() -> Self {
        Self::source_format()
    }
}

impl GameDataCompiler {
    #[must_use]
    pub fn new(mode: GameDataCompileMode) -> Self {
        Self {
            options: GameDataCompilerOptions::new(mode),
        }
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
    pub fn with_options(options: GameDataCompilerOptions) -> Self {
        Self { options }
    }

    #[must_use]
    pub const fn mode(&self) -> GameDataCompileMode {
        self.options.mode
    }

    #[must_use]
    pub const fn options(&self) -> &GameDataCompilerOptions {
        &self.options
    }

    #[must_use]
    pub fn compile_unit(&self, catalog: &GameSystemCatalog) -> GameDataCompileUnit {
        let schema_report = schema_report_for_mode(catalog, self.options.mode);
        let strict_schema_report = if self.options.mode == GameDataCompileMode::Strict {
            schema_report.clone()
        } else {
            schema_report_for_mode(catalog, GameDataCompileMode::Strict)
        };
        let managers = if self.options.managers.is_empty()
            && self.options.targets.iter().any(|target| {
                target.supports_product(crate::target::GameDataProduct::SemanticManagers)
            }) {
            ManagerCodegenPlan::validated_native_for_schema(&schema_report)
        } else {
            self.options.managers.clone()
        };
        let codegen_plan = GameDataCodegenPlan::from_schema_report(
            self.options.mode,
            &schema_report,
            self.options.targets.clone(),
        )
        .with_managers(managers)
        .with_systems(self.options.systems.clone());
        GameDataCompileUnit::new(schema_report, strict_schema_report, codegen_plan)
    }

    #[must_use]
    pub fn codegen_plan(&self, catalog: &GameSystemCatalog) -> GameDataCodegenPlan {
        self.compile_unit(catalog).into_codegen_plan()
    }
}

impl GameDataCompilerOptions {
    #[must_use]
    pub fn new(mode: GameDataCompileMode) -> Self {
        let targets = vec![GameDataTargetPlan::standalone(GameDataTargetLanguage::Rust)];
        Self {
            mode,
            targets,
            managers: ManagerCodegenPlan::new(),
            systems: SystemCodegenPlan::new(),
        }
    }

    #[must_use]
    pub fn standalone(mode: GameDataCompileMode, language: GameDataTargetLanguage) -> Self {
        Self {
            mode,
            targets: vec![GameDataTargetPlan::standalone(language)],
            managers: ManagerCodegenPlan::new(),
            systems: SystemCodegenPlan::new(),
        }
    }

    #[must_use]
    pub fn with_targets(
        mode: GameDataCompileMode,
        targets: impl IntoIterator<Item = GameDataTargetPlan>,
    ) -> Result<Self, GameDataTargetPlanError> {
        let targets = targets.into_iter().collect::<Vec<_>>();
        for target in &targets {
            target.validate()?;
        }
        Ok(Self {
            mode,
            targets,
            managers: ManagerCodegenPlan::new(),
            systems: SystemCodegenPlan::new(),
        })
    }

    #[must_use]
    pub const fn mode(&self) -> GameDataCompileMode {
        self.mode
    }

    #[must_use]
    pub fn targets(&self) -> &[GameDataTargetPlan] {
        &self.targets
    }

    #[must_use]
    pub const fn managers(&self) -> &ManagerCodegenPlan {
        &self.managers
    }

    #[must_use]
    pub const fn systems(&self) -> &SystemCodegenPlan {
        &self.systems
    }

    #[must_use]
    pub fn with_managers(mut self, managers: ManagerCodegenPlan) -> Self {
        self.managers = managers;
        self
    }

    #[must_use]
    pub fn with_systems(mut self, systems: SystemCodegenPlan) -> Self {
        self.systems = systems;
        self
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
