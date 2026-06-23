use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use uuid::Uuid;

use super::api::ComponentScaffoldError;
use super::existing_patch::ExistingComponentPatchShape;
use crate::rust::integrate::identity;
use crate::{CodegenContext, RustSourceTypeIndex};

#[derive(Debug)]
pub(super) struct ExistingComponents {
    pub(super) by_type_id: BTreeMap<Uuid, ExistingComponent>,
    pub(super) source_types: RustSourceTypeIndex,
}

#[derive(Debug)]
pub(super) struct ExistingComponent {
    pub(super) component_name: String,
    pub(super) file_path: PathBuf,
    pub(super) field_names: BTreeSet<String>,
    pub(super) field_names_in_order: Vec<String>,
    insert_offset: Option<usize>,
    unit_struct_semicolon: Option<usize>,
}

impl ExistingComponent {
    pub(super) fn can_receive_fields(&self) -> bool {
        self.insert_offset.is_some() || self.unit_struct_semicolon.is_some()
    }

    pub(super) fn patch_shape(&self) -> ExistingComponentPatchShape {
        if let Some(insert_offset) = self.insert_offset {
            ExistingComponentPatchShape::Braced { insert_offset }
        } else if let Some(semicolon_offset) = self.unit_struct_semicolon {
            ExistingComponentPatchShape::Unit { semicolon_offset }
        } else {
            unreachable!("checked by can_receive_fields")
        }
    }
}

pub(super) fn discover_existing_components(
    root: &Path,
    context: &CodegenContext,
) -> Result<ExistingComponents, ComponentScaffoldError> {
    let mut by_type_id = BTreeMap::new();
    let mut source_types = RustSourceTypeIndex::default();
    let mut files = Vec::new();
    collect_rust_files(root, &mut files)?;
    files.sort();

    let scanned_files = context
        .runner()
        .map(
            &files,
            |file_path| -> Result<ExistingRustFileScan, ComponentScaffoldError> {
                let text = fs::read_to_string(file_path).map_err(|source| {
                    ComponentScaffoldError::Read {
                        path: file_path.to_path_buf(),
                        source,
                    }
                })?;
                let components = identity::scan_component_structs(&text, file_path)
                    .into_iter()
                    .filter_map(|scanned| {
                        scanned.type_id.map(|type_id| {
                            (
                                type_id,
                                ExistingComponent {
                                    component_name: scanned.component_name,
                                    file_path: scanned.file_path,
                                    field_names: scanned.field_names,
                                    field_names_in_order: scanned.field_names_in_order,
                                    insert_offset: scanned.insert_offset,
                                    unit_struct_semicolon: scanned.unit_struct_semicolon,
                                },
                            )
                        })
                    })
                    .collect::<Vec<_>>();
                let source_types = RustSourceTypeIndex::from_source(root, file_path, &text)?;
                Ok(ExistingRustFileScan {
                    components,
                    source_types,
                })
            },
        )
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?;

    for file_scan in scanned_files {
        source_types.merge(file_scan.source_types);
        for (type_id, component) in file_scan.components {
            by_type_id.insert(type_id, component);
        }
    }

    Ok(ExistingComponents {
        by_type_id,
        source_types,
    })
}

#[derive(Debug)]
struct ExistingRustFileScan {
    components: Vec<(Uuid, ExistingComponent)>,
    source_types: RustSourceTypeIndex,
}

fn collect_rust_files(root: &Path, files: &mut Vec<PathBuf>) -> Result<(), ComponentScaffoldError> {
    let entries = fs::read_dir(root).map_err(|source| ComponentScaffoldError::Walk {
        path: root.to_path_buf(),
        source,
    })?;

    for entry in entries {
        let entry = entry.map_err(|source| ComponentScaffoldError::Walk {
            path: root.to_path_buf(),
            source,
        })?;
        let path = entry.path();
        if path.is_dir() {
            collect_rust_files(&path, files)?;
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
            files.push(path);
        }
    }

    Ok(())
}
