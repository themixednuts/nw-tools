use super::*;

mod table_inputs;

pub(in crate::manager::specs) use table_inputs::{
    ABILITY_DATA_MANAGER_TABLES, MISSION_DATA_MANAGER_TABLES, OBJECTIVE_TASKS_DATA_MANAGER_TABLES,
    OBJECTIVES_DATA_MANAGER_TABLES, SPELL_DATA_MANAGER_TABLES,
};

#[derive(Debug, Clone, Copy)]
struct RuntimeManifestManagerSpec {
    ghidra_class: &'static str,
    rust_type: &'static str,
}

const RUNTIME_MANIFEST_MANAGER_SPECS: &[RuntimeManifestManagerSpec] = &[];

pub(super) fn runtime_manifest_manager_specs() -> Vec<NativeManagerSpec> {
    RUNTIME_MANIFEST_MANAGER_SPECS
        .iter()
        .map(|spec| {
            NativeManagerSpec::runtime_manifest(
                GhidraClassPath::new(spec.ghidra_class).expect("validated Ghidra class"),
                rust_type(spec.rust_type),
                manager_inputs_for_manifest(
                    spec.rust_type,
                    manager_dependency_inputs(spec.rust_type),
                ),
            )
            .expect("validated runtime manager manifest spec")
        })
        .collect()
}

fn manager_table_inputs(rust_type_path: &'static str) -> Vec<NativeManagerInput> {
    let inputs = table_inputs::manager_table_input_specs()
        .filter(|input| input.rust_type == rust_type_path)
        .map(|input| table_input(input.table_name, input.row_type_name))
        .collect::<Vec<_>>();

    inputs
}

pub(super) fn manager_table_family_specs(
    rust_type_path: &'static str,
) -> Vec<TableFamilyTableSpec> {
    table_inputs::manager_table_input_specs()
        .filter(|input| input.rust_type == rust_type_path)
        .map(|input| TableFamilyTableSpec {
            variant: to_upper_camel_ident(input.table_name, "Table"),
            table_name: input.table_name,
            row_type_name: input.row_type_name,
        })
        .collect()
}

fn manager_dependency_inputs(rust_type_path: &'static str) -> Vec<NativeManagerInput> {
    table_inputs::MANAGER_DEPENDENCY_INPUT_SPECS
        .iter()
        .filter(|input| input.rust_type == rust_type_path)
        .map(|input| NativeManagerInput::manager(rust_type(input.resource)))
        .collect()
}

pub(super) fn manager_inputs_for_manifest(
    rust_type_path: &'static str,
    inputs: Vec<NativeManagerInput>,
) -> Vec<NativeManagerInput> {
    let mut inputs = inputs;
    for dependency in manager_dependency_inputs(rust_type_path) {
        if !inputs.contains(&dependency) {
            inputs.push(dependency);
        }
    }

    let mut table_inputs = manager_table_inputs(rust_type_path);
    if table_inputs.is_empty() {
        return inputs;
    }
    table_inputs.extend(
        inputs
            .into_iter()
            .filter(|input| !matches!(input, NativeManagerInput::Table(_))),
    );
    table_inputs
}
