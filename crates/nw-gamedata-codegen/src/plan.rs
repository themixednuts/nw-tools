use std::collections::BTreeSet;

use crate::asset_access::GameAssetAccessCodegenPlan;
use crate::game_system_schema::GameSystemDataTablesSchemaReport;
use crate::manager::ManagerCodegenPlan;
use crate::project::StandaloneProjectCodegenPlan;
use crate::schema::GameDataCompileMode;
use crate::system::SystemCodegenPlan;
use crate::target::{GameDataProduct, GameDataTargetPlan};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameDataCodegenPlan {
    mode: GameDataCompileMode,
    targets: Vec<GameDataTargetPlan>,
    tables: TableCodegenPlan,
    managers: ManagerCodegenPlan,
    systems: SystemCodegenPlan,
    game_asset_access: GameAssetAccessCodegenPlan,
    standalone_projects: StandaloneProjectCodegenPlan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableCodegenPlan {
    table_count: usize,
    row_type_count: usize,
    source_table_count: usize,
}

impl GameDataCodegenPlan {
    #[must_use]
    pub fn from_schema_report(
        mode: GameDataCompileMode,
        schema_report: &GameSystemDataTablesSchemaReport,
        targets: Vec<GameDataTargetPlan>,
    ) -> Self {
        let game_asset_access = GameAssetAccessCodegenPlan::from_targets(&targets);
        let standalone_projects =
            StandaloneProjectCodegenPlan::from_targets(&targets, &game_asset_access);
        Self {
            mode,
            targets,
            tables: TableCodegenPlan::from_schema_report(schema_report),
            managers: ManagerCodegenPlan::new(),
            systems: SystemCodegenPlan::new(),
            game_asset_access,
            standalone_projects,
        }
    }

    #[must_use]
    pub fn target_plans_for(&self, product: GameDataProduct) -> Vec<&GameDataTargetPlan> {
        self.targets
            .iter()
            .filter(|target| target.supports_product(product))
            .collect()
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
    pub const fn tables(&self) -> &TableCodegenPlan {
        &self.tables
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
    pub const fn game_asset_access(&self) -> &GameAssetAccessCodegenPlan {
        &self.game_asset_access
    }

    #[must_use]
    pub const fn standalone_projects(&self) -> &StandaloneProjectCodegenPlan {
        &self.standalone_projects
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

    #[must_use]
    pub fn standalone_targets_are_self_contained(&self) -> bool {
        self.targets
            .iter()
            .all(GameDataTargetPlan::is_self_contained)
    }
}

impl TableCodegenPlan {
    #[must_use]
    pub fn from_schema_report(schema_report: &GameSystemDataTablesSchemaReport) -> Self {
        let mut row_types = BTreeSet::new();
        for table in &schema_report.tables {
            row_types.insert((table.row_type_crc, table.row_type_name.as_str()));
        }
        Self {
            table_count: schema_report.tables.len(),
            row_type_count: row_types.len(),
            source_table_count: schema_report.tables.len(),
        }
    }

    #[must_use]
    pub const fn table_count(&self) -> usize {
        self.table_count
    }

    #[must_use]
    pub const fn row_type_count(&self) -> usize {
        self.row_type_count
    }

    #[must_use]
    pub const fn source_table_count(&self) -> usize {
        self.source_table_count
    }
}
