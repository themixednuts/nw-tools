use std::collections::BTreeSet;

use crate::game_system_schema::GameSystemDataTablesSchemaReport;
use crate::manager::ManagerCodegenPlan;
use crate::schema::GameDataCompileMode;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameDataCodegenPlan {
    mode: GameDataCompileMode,
    tables: TableCodegenPlan,
    managers: ManagerCodegenPlan,
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
    ) -> Self {
        Self {
            mode,
            tables: TableCodegenPlan::from_schema_report(schema_report),
            managers: ManagerCodegenPlan::validated_native_for_schema(schema_report),
        }
    }

    #[must_use]
    pub const fn mode(&self) -> GameDataCompileMode {
        self.mode
    }

    #[must_use]
    pub const fn tables(&self) -> &TableCodegenPlan {
        &self.tables
    }

    #[must_use]
    pub const fn managers(&self) -> &ManagerCodegenPlan {
        &self.managers
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
