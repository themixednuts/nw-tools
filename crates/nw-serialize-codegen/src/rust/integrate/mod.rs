use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use thiserror::Error;
use uuid::Uuid;

pub mod identity;
pub mod source_index;

use crate::CodegenContext;
use crate::naming::rust_field_ident;
use crate::rust::analyze::{RustItemUpdatePlan, RustSourceAnalyzeError, RustSourceFile};
use crate::rust::integrate::source_index::RustSourceTypeIndex;
use crate::rust::item_plan::{RustCodegenUnit, RustItemPlan};
use crate::rust::source::{RustSourceEmitError, RustSourceEmitter};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RustSourceInventory {
    files: Vec<RustSourceInventoryFile>,
    items_by_name: BTreeMap<String, Vec<RustSourceInventoryItem>>,
    items_by_type_id: BTreeMap<Uuid, Vec<RustSourceInventoryItem>>,
    source_types: RustSourceTypeIndex,
}

impl RustSourceInventory {
    pub fn from_root(
        root: impl AsRef<Path>,
        context: &CodegenContext,
    ) -> Result<Self, RustIntegrationError> {
        let root = root.as_ref();
        let mut paths = Vec::new();
        collect_rust_files(root, &mut paths)?;
        paths.sort();
        let scans = context.runner().try_map(&paths, |path| {
            let source = fs::read_to_string(path).map_err(|source| RustIntegrationError::Read {
                path: path.clone(),
                source,
            })?;
            let file = parse_inventory_file(path.clone(), &source)?;
            let source_types = RustSourceTypeIndex::from_source(root, path, &source)?;
            Ok(RustSourceInventoryFileScan { file, source_types })
        })?;

        let mut files = Vec::with_capacity(scans.len());
        let mut source_types = RustSourceTypeIndex::default();
        for scan in scans {
            files.push(scan.file);
            source_types.merge(scan.source_types);
        }
        Ok(Self::from_files_with_source_types(files, source_types))
    }

    pub fn from_paths(
        paths: impl IntoIterator<Item = PathBuf>,
        context: &CodegenContext,
    ) -> Result<Self, RustIntegrationError> {
        let mut paths = paths.into_iter().collect::<Vec<_>>();
        paths.sort();
        let files = context.runner().try_map(&paths, |path| {
            let source = fs::read_to_string(path).map_err(|source| RustIntegrationError::Read {
                path: path.clone(),
                source,
            })?;
            parse_inventory_file(path.clone(), &source)
        })?;
        Ok(Self::from_files(files))
    }

    pub fn from_sources<'a>(
        sources: impl IntoIterator<Item = (PathBuf, &'a str)>,
    ) -> Result<Self, RustIntegrationError> {
        let files = sources
            .into_iter()
            .map(|(path, source)| parse_inventory_file(path, source))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self::from_files(files))
    }

    fn from_files(files: Vec<RustSourceInventoryFile>) -> Self {
        Self::from_files_with_source_types(files, RustSourceTypeIndex::default())
    }

    fn from_files_with_source_types(
        files: Vec<RustSourceInventoryFile>,
        source_types: RustSourceTypeIndex,
    ) -> Self {
        let mut items_by_name: BTreeMap<String, Vec<RustSourceInventoryItem>> = BTreeMap::new();
        let mut items_by_type_id: BTreeMap<Uuid, Vec<RustSourceInventoryItem>> = BTreeMap::new();
        for inventory_file in &files {
            for item in inventory_file.file.items() {
                let inventory_item = RustSourceInventoryItem {
                    path: inventory_file.path.clone(),
                    item: item.clone(),
                };
                items_by_name
                    .entry(item.name.clone())
                    .or_default()
                    .push(inventory_item.clone());
                for type_id in item
                    .identity_attrs
                    .iter()
                    .filter_map(|attr| attr.type_id)
                    .collect::<std::collections::BTreeSet<_>>()
                {
                    items_by_type_id
                        .entry(type_id)
                        .or_default()
                        .push(inventory_item.clone());
                }
            }
        }
        for items in items_by_name.values_mut() {
            items.sort_by(|left, right| left.path.cmp(&right.path));
        }
        for items in items_by_type_id.values_mut() {
            items.sort_by(|left, right| {
                left.path
                    .cmp(&right.path)
                    .then_with(|| left.item.name.cmp(&right.item.name))
            });
        }
        Self {
            files,
            items_by_name,
            items_by_type_id,
            source_types,
        }
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    #[must_use]
    pub fn files(&self) -> &[RustSourceInventoryFile] {
        &self.files
    }

    #[must_use]
    pub fn candidates_for(&self, item_name: &str) -> &[RustSourceInventoryItem] {
        self.items_by_name
            .get(item_name)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    #[must_use]
    pub fn candidates_for_type_id(&self, type_id: Uuid) -> &[RustSourceInventoryItem] {
        self.items_by_type_id
            .get(&type_id)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    #[must_use]
    pub const fn source_type_index(&self) -> &RustSourceTypeIndex {
        &self.source_types
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RustSourceInventoryFile {
    pub path: PathBuf,
    pub file: RustSourceFile,
}

struct RustSourceInventoryFileScan {
    file: RustSourceInventoryFile,
    source_types: RustSourceTypeIndex,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RustSourceInventoryItem {
    pub path: PathBuf,
    pub item: crate::rust::analyze::RustSourceItem,
}

pub trait RustItemPathResolver: Sync {
    fn path_for(&self, item: &RustItemPlan) -> PathBuf;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlatRustItemPathResolver {
    root: PathBuf,
}

impl FlatRustItemPathResolver {
    #[must_use]
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }
}

impl RustItemPathResolver for FlatRustItemPathResolver {
    fn path_for(&self, item: &RustItemPlan) -> PathBuf {
        self.root
            .join(format!("{}.rs", rust_field_ident(&item.rust_name)))
    }
}

#[derive(Debug, Default)]
pub struct RustIntegrationPlanner {
    emitter: RustSourceEmitter,
}

impl RustIntegrationPlanner {
    #[must_use]
    pub fn new() -> Self {
        Self {
            emitter: RustSourceEmitter::default(),
        }
    }

    pub fn plan(
        &self,
        unit: &RustCodegenUnit,
        inventory: &RustSourceInventory,
        paths: &impl RustItemPathResolver,
        context: &CodegenContext,
    ) -> Result<RustIntegrationPlan, RustIntegrationError> {
        let items =
            context
                .runner()
                .try_map_until_cancelled(&unit.items, context.cancel(), |desired| {
                    let candidates = inventory.candidates_for(&desired.rust_name);
                    let action = match candidates {
                        [] => RustIntegrationAction::Create {
                            target_path: paths.path_for(desired),
                            source: self.emit_single_item(desired, context)?,
                        },
                        [candidate] => {
                            let update = candidate.item.plan_update(desired).map_err(|source| {
                                RustIntegrationError::Analyze {
                                    path: candidate.path.clone(),
                                    source: Box::new(source),
                                }
                            })?;
                            if update.is_current() {
                                RustIntegrationAction::Current {
                                    path: candidate.path.clone(),
                                }
                            } else {
                                RustIntegrationAction::Update {
                                    path: candidate.path.clone(),
                                    update: Box::new(update),
                                }
                            }
                        }
                        candidates => RustIntegrationAction::Ambiguous {
                            item_name: desired.rust_name.clone(),
                            candidate_paths: candidates
                                .iter()
                                .map(|candidate| candidate.path.clone())
                                .collect(),
                        },
                    };
                    Ok(RustIntegrationItemPlan {
                        item: desired.clone(),
                        action,
                    })
                })?;
        if items.was_cancelled() {
            return Err(RustIntegrationError::Emit(Box::new(
                RustSourceEmitError::Cancelled,
            )));
        }
        Ok(RustIntegrationPlan {
            items: items.into_completed(),
        })
    }

    fn emit_single_item(
        &self,
        item: &RustItemPlan,
        context: &CodegenContext,
    ) -> Result<String, RustIntegrationError> {
        self.emitter
            .emit(
                &RustCodegenUnit {
                    items: vec![item.clone()],
                },
                context,
            )
            .map_err(|source| RustIntegrationError::Emit(Box::new(source)))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RustIntegrationPlan {
    pub items: Vec<RustIntegrationItemPlan>,
}

impl RustIntegrationPlan {
    #[must_use]
    pub fn is_current(&self) -> bool {
        self.items
            .iter()
            .all(|item| matches!(item.action, RustIntegrationAction::Current { .. }))
    }

    pub fn creates(&self) -> impl Iterator<Item = &RustIntegrationItemPlan> {
        self.items
            .iter()
            .filter(|item| matches!(item.action, RustIntegrationAction::Create { .. }))
    }

    pub fn updates(&self) -> impl Iterator<Item = &RustIntegrationItemPlan> {
        self.items
            .iter()
            .filter(|item| matches!(item.action, RustIntegrationAction::Update { .. }))
    }

    pub fn ambiguities(&self) -> impl Iterator<Item = &RustIntegrationItemPlan> {
        self.items
            .iter()
            .filter(|item| matches!(item.action, RustIntegrationAction::Ambiguous { .. }))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RustIntegrationItemPlan {
    pub item: RustItemPlan,
    pub action: RustIntegrationAction,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RustIntegrationAction {
    Current {
        path: PathBuf,
    },
    Create {
        target_path: PathBuf,
        source: String,
    },
    Update {
        path: PathBuf,
        update: Box<RustItemUpdatePlan>,
    },
    Ambiguous {
        item_name: String,
        candidate_paths: Vec<PathBuf>,
    },
}

#[derive(Debug, Error)]
pub enum RustIntegrationError {
    #[error("failed to walk Rust source root {path}")]
    Walk {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("failed to read Rust source at {path}")]
    Read {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("failed to parse Rust source at {path}")]
    Parse {
        path: PathBuf,
        #[source]
        source: Box<RustSourceAnalyzeError>,
    },
    #[error("failed to analyze Rust source at {path}")]
    Analyze {
        path: PathBuf,
        #[source]
        source: Box<RustSourceAnalyzeError>,
    },
    #[error("failed to emit Rust source")]
    Emit(#[source] Box<RustSourceEmitError>),
}

fn parse_inventory_file(
    path: PathBuf,
    source: &str,
) -> Result<RustSourceInventoryFile, RustIntegrationError> {
    let file = RustSourceFile::parse(source).map_err(|source| RustIntegrationError::Parse {
        path: path.clone(),
        source: Box::new(source),
    })?;
    Ok(RustSourceInventoryFile { path, file })
}

pub(super) fn collect_rust_files(
    root: &Path,
    paths: &mut Vec<PathBuf>,
) -> Result<(), RustIntegrationError> {
    let entries = fs::read_dir(root).map_err(|source| RustIntegrationError::Walk {
        path: root.to_path_buf(),
        source,
    })?;
    for entry in entries {
        let entry = entry.map_err(|source| RustIntegrationError::Walk {
            path: root.to_path_buf(),
            source,
        })?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|source| RustIntegrationError::Walk {
                path: path.clone(),
                source,
            })?;
        if file_type.is_dir() {
            collect_rust_files(&path, paths)?;
        } else if file_type.is_file() && path.extension().and_then(|ext| ext.to_str()) == Some("rs")
        {
            paths.push(path);
        }
    }
    Ok(())
}

trait PlanExistingItem {
    fn plan_update(
        &self,
        desired: &RustItemPlan,
    ) -> Result<RustItemUpdatePlan, RustSourceAnalyzeError>;
}

impl PlanExistingItem for crate::rust::analyze::RustSourceItem {
    fn plan_update(
        &self,
        desired: &RustItemPlan,
    ) -> Result<RustItemUpdatePlan, RustSourceAnalyzeError> {
        let file = RustSourceFile::from_item_for_planning(self.clone());
        file.plan_item_update(desired)
    }
}

impl RustSourceFile {
    fn from_item_for_planning(item: crate::rust::analyze::RustSourceItem) -> Self {
        Self::from_items_for_planning([item])
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    use uuid::uuid;

    use super::*;
    use crate::rust::enum_plan::RustVariantPlan;
    use crate::rust::identity::RustTypeIdentityPlan;
    use crate::rust::item_plan::{RustFieldPlan, RustItemKind, RustItemPlan};

    #[test]
    fn plans_current_existing_source_without_generating_changes() {
        let desired = health_component_plan();
        let source = RustSourceEmitter::emit_unit(
            &RustCodegenUnit {
                items: vec![desired.clone()],
            },
            &CodegenContext::inline(),
        )
        .expect("source");
        let inventory = RustSourceInventory::from_sources([(
            PathBuf::from("components/health.rs"),
            source.as_str(),
        )])
        .expect("inventory");
        let plan = RustIntegrationPlanner::default()
            .plan(
                &RustCodegenUnit {
                    items: vec![desired],
                },
                &inventory,
                &FlatRustItemPathResolver::new("components"),
                &CodegenContext::inline(),
            )
            .expect("integration plan");

        assert!(plan.is_current());
        assert!(matches!(
            &plan.items[0].action,
            RustIntegrationAction::Current { path } if path == Path::new("components/health.rs")
        ));
    }

    #[test]
    fn plans_missing_item_as_create_at_resolved_path() {
        let desired = health_component_plan();
        let inventory = RustSourceInventory::from_sources([]).expect("inventory");
        let plan = RustIntegrationPlanner::default()
            .plan(
                &RustCodegenUnit {
                    items: vec![desired],
                },
                &inventory,
                &FlatRustItemPathResolver::new("components"),
                &CodegenContext::inline(),
            )
            .expect("integration plan");

        assert_eq!(plan.creates().count(), 1);
        assert!(matches!(
            &plan.items[0].action,
            RustIntegrationAction::Create { target_path, source }
                if target_path == Path::new("components/health_component.rs")
                    && source.contains("pub struct HealthComponent")
                    && source.contains("#[az_rtti(")
        ));
    }

    #[test]
    fn plans_existing_item_update_from_syn_delta() {
        let desired = health_component_plan();
        let source = r#"
use bevy::prelude::Component;

#[derive(Component, Debug)]
pub struct HealthComponent {
    pub value: i16,
}
"#;
        let inventory =
            RustSourceInventory::from_sources([(PathBuf::from("components/health.rs"), source)])
                .expect("inventory");
        let plan = RustIntegrationPlanner::default()
            .plan(
                &RustCodegenUnit {
                    items: vec![desired],
                },
                &inventory,
                &FlatRustItemPathResolver::new("components"),
                &CodegenContext::inline(),
            )
            .expect("integration plan");

        assert_eq!(plan.updates().count(), 1);
        let RustIntegrationAction::Update { path, update } = &plan.items[0].action else {
            panic!("expected update action");
        };
        assert_eq!(path, Path::new("components/health.rs"));
        assert_eq!(
            update.missing_derives,
            vec![
                "AzRtti",
                "Default",
                "Clone",
                "PartialEq",
                "Serialize",
                "Deserialize",
                "Reflect"
            ]
        );
        assert_eq!(update.field_type_mismatches[0].existing_type, "i16");
        assert_eq!(update.field_type_mismatches[0].expected_type, "u32");
    }

    #[test]
    fn reports_ambiguous_existing_items_instead_of_guessing() {
        let desired = health_component_plan();
        let source = r#"
#[derive(Debug)]
pub struct HealthComponent;
"#;
        let inventory = RustSourceInventory::from_sources([
            (PathBuf::from("components/a.rs"), source),
            (PathBuf::from("components/b.rs"), source),
        ])
        .expect("inventory");
        let plan = RustIntegrationPlanner::default()
            .plan(
                &RustCodegenUnit {
                    items: vec![desired],
                },
                &inventory,
                &FlatRustItemPathResolver::new("components"),
                &CodegenContext::inline(),
            )
            .expect("integration plan");

        assert_eq!(plan.ambiguities().count(), 1);
        assert!(matches!(
            &plan.items[0].action,
            RustIntegrationAction::Ambiguous { candidate_paths, .. }
                if candidate_paths == &vec![
                    PathBuf::from("components/a.rs"),
                    PathBuf::from("components/b.rs")
                ]
        ));
    }

    #[test]
    fn builds_inventory_from_nested_rust_source_root() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path().join("components");
        let nested = root.join("nested");
        fs::create_dir_all(&nested).expect("create nested source directory");
        fs::write(
            root.join("health.rs"),
            r#"
#[derive(Debug)]
pub struct HealthComponent;
"#,
        )
        .expect("write health source");
        fs::write(
            nested.join("mana.rs"),
            r#"
#[derive(Debug)]
pub struct ManaComponent;
"#,
        )
        .expect("write mana source");
        fs::write(root.join("notes.txt"), "not rust").expect("write ignored file");

        let inventory = RustSourceInventory::from_root(&root, &crate::CodegenContext::inline())
            .expect("inventory");

        assert_eq!(inventory.files().len(), 2);
        assert_eq!(inventory.candidates_for("HealthComponent").len(), 1);
        assert_eq!(inventory.candidates_for("ManaComponent").len(), 1);
        assert!(inventory.candidates_for("Ignored").is_empty());
    }

    fn health_component_plan() -> RustItemPlan {
        RustItemPlan {
            source_type_id: uuid!("BBBBBBBB-BBBB-BBBB-BBBB-BBBBBBBBBBBB"),
            source_name: "Example::HealthComponent".to_owned(),
            is_reflected_base: false,
            is_slot_owner: false,
            has_layout_family_descendants: false,
            is_bevy_component: true,
            file_stem_override: None,
            scope_path: vec!["example".to_owned()],
            family_scope_path: vec!["example".to_owned(), "health_components".to_owned()],
            rust_name: "HealthComponent".to_owned(),
            kind: RustItemKind::Struct,
            identity: RustTypeIdentityPlan::az_rtti(
                uuid!("BBBBBBBB-BBBB-BBBB-BBBB-BBBBBBBBBBBB"),
                Some("Example::HealthComponent".to_owned()),
            ),
            repr: None,
            raw_conversion: None,
            derives: vec![
                "Component".to_owned(),
                "AzRtti".to_owned(),
                "Debug".to_owned(),
                "Default".to_owned(),
                "Clone".to_owned(),
                "PartialEq".to_owned(),
                "Serialize".to_owned(),
                "Deserialize".to_owned(),
                "Reflect".to_owned(),
            ],
            rtti_bases: Vec::new(),
            fields: vec![RustFieldPlan {
                source_name: "m_value".to_owned(),
                rust_name: "value".to_owned(),
                source_type_id: uuid!("43DA906B-7DEF-4CA8-9790-854106D3F983"),
                rust_type: "u32".to_owned(),
                unresolved_type: None,
                integer_range: None,
                data_size: None,
                offset: None,
                flags: None,
                is_base_class: false,
            }],
            variants: Vec::<RustVariantPlan>::new(),
        }
    }
}
