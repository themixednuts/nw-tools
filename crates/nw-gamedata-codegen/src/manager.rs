//! GameData manager codegen mechanism.
//!
//! The manager vocabulary (`Native*` spec types) and the project's ported
//! native-manager facts (manager shapes, table family lists, key policies)
//! live in `newworld-gamedata-manager-specs` and are re-exported here so
//! call sites keep their `crate::manager` paths. This module keeps only the
//! codegen-side mechanism over those project-owned facts: plan construction,
//! emission context, and the emitter trait.

use std::collections::BTreeSet;

use crate::game_system_schema::GameSystemDataTablesSchemaReport;
use crate::native::{NativeCodegenFile, NativeCodegenOutput, NativeCodegenPlan};
use crate::schema::GameDataCompileMode;
use crate::target::{GameDataTargetLanguage, GameDataTargetPlan};

pub use nw_gamedata_manager_specs::manager::*;

pub type ManagerCodegenFile = NativeCodegenFile;
pub type ManagerCodegenOutput = NativeCodegenOutput;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ManagerCodegenPlan {
    native: NativeCodegenPlan<NativeManagerSpec>,
}

#[derive(Debug, Clone, Copy)]
pub struct ManagerEmissionContext<'a> {
    mode: GameDataCompileMode,
    target: &'a GameDataTargetPlan,
    plan: &'a ManagerCodegenPlan,
}

pub fn validated_native_manager_plan() -> ManagerCodegenPlan {
    ManagerCodegenPlan::from_native_managers(validated_native_manager_specs())
}

pub fn validated_native_manager_plan_for_schema(
    schema_report: &GameSystemDataTablesSchemaReport,
) -> ManagerCodegenPlan {
    let available_tables = schema_report
        .tables
        .iter()
        .map(|table| (table.table_name.as_str(), table.row_type_name.as_str()))
        .collect::<BTreeSet<_>>();
    let managers = validated_native_manager_specs()
        .into_iter()
        .filter(|manager| manager_table_inputs_available(manager, &available_tables))
        .collect();
    ManagerCodegenPlan::from_native_managers(managers)
}

fn manager_table_inputs_available(
    manager: &NativeManagerSpec,
    available_tables: &BTreeSet<(&str, &str)>,
) -> bool {
    manager.inputs().iter().all(|input| match input {
        NativeManagerInput::Table(table) => available_tables
            .contains(&(table.table_name().as_str(), table.row_type_name().as_str())),
        NativeManagerInput::Product(_) | NativeManagerInput::Manager(_) => true,
    })
}

impl ManagerCodegenPlan {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            native: NativeCodegenPlan::new(),
        }
    }

    #[must_use]
    pub fn from_native_managers(managers: Vec<NativeManagerSpec>) -> Self {
        Self {
            native: NativeCodegenPlan::from_specs(managers),
        }
    }

    #[must_use]
    pub fn validated_native() -> Self {
        validated_native_manager_plan()
    }

    #[must_use]
    pub fn validated_native_for_schema(
        schema_report: &crate::game_system_schema::GameSystemDataTablesSchemaReport,
    ) -> Self {
        validated_native_manager_plan_for_schema(schema_report)
    }

    #[must_use]
    pub fn managers(&self) -> &[NativeManagerSpec] {
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

    #[must_use]
    pub fn contracts(&self) -> Vec<ManagerContract<'_>> {
        self.managers()
            .iter()
            .map(NativeManagerSpec::contract)
            .collect()
    }
}

impl<'a> ManagerEmissionContext<'a> {
    #[must_use]
    pub const fn new(
        mode: GameDataCompileMode,
        target: &'a GameDataTargetPlan,
        plan: &'a ManagerCodegenPlan,
    ) -> Self {
        Self { mode, target, plan }
    }

    #[must_use]
    pub const fn mode(&self) -> GameDataCompileMode {
        self.mode
    }

    #[must_use]
    pub const fn target(&self) -> &'a GameDataTargetPlan {
        self.target
    }

    #[must_use]
    pub const fn plan(&self) -> &'a ManagerCodegenPlan {
        self.plan
    }
}

pub trait ManagerEmitter {
    fn target_language(&self) -> GameDataTargetLanguage;

    fn emit_managers(
        &self,
        context: ManagerEmissionContext<'_>,
    ) -> anyhow::Result<ManagerCodegenOutput>;
}
