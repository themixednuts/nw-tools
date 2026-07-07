#[derive(Debug, Clone, Copy)]
pub(super) struct ManagerTableInputSpec {
    pub(super) rust_type: &'static str,
    pub(super) table_name: &'static str,
    pub(super) row_type_name: &'static str,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct ManagerDependencyInputSpec {
    pub(super) rust_type: &'static str,
    pub(super) resource: &'static str,
}

mod dependencies;
mod families;
mod manager_tables_00;
mod manager_tables_01;
mod manager_tables_02;
mod manager_tables_03;
mod manager_tables_04;
mod manager_tables_05;
mod manager_tables_06;
mod manager_tables_07;
mod manager_tables_08;
mod manager_tables_09;

pub(super) use dependencies::MANAGER_DEPENDENCY_INPUT_SPECS;
pub(in crate::manager::specs) use families::{
    ABILITY_DATA_MANAGER_TABLES, OBJECTIVE_TASKS_DATA_MANAGER_TABLES,
    OBJECTIVES_DATA_MANAGER_TABLES,
};
pub(in crate::manager::specs) use families::{
    MISSION_DATA_MANAGER_TABLES, SPELL_DATA_MANAGER_TABLES,
};

pub(super) fn manager_table_input_specs() -> impl Iterator<Item = &'static ManagerTableInputSpec> {
    [
        manager_tables_00::SPECS,
        manager_tables_01::SPECS,
        manager_tables_02::SPECS,
        manager_tables_03::SPECS,
        manager_tables_04::SPECS,
        manager_tables_05::SPECS,
        manager_tables_06::SPECS,
        manager_tables_07::SPECS,
        manager_tables_08::SPECS,
        manager_tables_09::SPECS,
    ]
    .into_iter()
    .flat_map(|specs| specs.iter())
}
