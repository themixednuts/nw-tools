use std::collections::{BTreeMap, BTreeSet};

use uuid::Uuid;

use crate::ir::SerializeCodegenItem;
use crate::naming::{rust_module_ident, rust_type_ident};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct RustNamePlan {
    names_by_type_id: BTreeMap<Uuid, String>,
    absolute_paths_by_type_id: BTreeMap<Uuid, String>,
    ambiguous_type_names: BTreeSet<String>,
}

impl RustNamePlan {
    pub(super) fn scoped_candidates_with_root<const N: usize>(
        types: impl IntoIterator<Item = (Uuid, Vec<String>, String)>,
        scopes_by_type_id: BTreeMap<Uuid, Vec<String>>,
        root: [&'static str; N],
    ) -> Self {
        let names_by_type_id = scoped_rust_candidate_type_names_by_id(types);
        Self::from_names_and_scopes(
            names_by_type_id,
            scopes_by_type_id,
            root.into_iter().map(str::to_owned).collect(),
        )
    }

    fn from_names_and_scopes(
        names_by_type_id: BTreeMap<Uuid, String>,
        scopes_by_type_id: BTreeMap<Uuid, Vec<String>>,
        absolute_root: Vec<String>,
    ) -> Self {
        let mut type_name_counts = BTreeMap::<String, usize>::new();
        for name in names_by_type_id.values() {
            *type_name_counts.entry(name.clone()).or_default() += 1;
        }
        let ambiguous_type_names = type_name_counts
            .into_iter()
            .filter_map(|(name, count)| (count > 1).then_some(name))
            .collect::<BTreeSet<_>>();

        let absolute_paths_by_type_id = names_by_type_id
            .iter()
            .filter_map(|(type_id, name)| {
                let scope = scopes_by_type_id.get(type_id)?;
                Some((
                    *type_id,
                    rust_absolute_type_path(&absolute_root, scope, name),
                ))
            })
            .collect();

        Self {
            names_by_type_id,
            absolute_paths_by_type_id,
            ambiguous_type_names,
        }
    }

    pub(super) fn definition_name(&self, item: &SerializeCodegenItem) -> String {
        self.names_by_type_id
            .get(&item.source_type_id)
            .cloned()
            .unwrap_or_else(|| rust_type_ident(&item.source_name))
    }

    pub(super) fn reference_name(&self, type_id: Uuid, source_name: &str) -> String {
        let name = self
            .names_by_type_id
            .get(&type_id)
            .cloned()
            .unwrap_or_else(|| rust_type_ident(source_name));
        if self.ambiguous_type_names.contains(&name) {
            self.absolute_paths_by_type_id
                .get(&type_id)
                .cloned()
                .unwrap_or(name)
        } else {
            name
        }
    }

    pub(super) fn names_by_type_id(&self) -> &BTreeMap<Uuid, String> {
        &self.names_by_type_id
    }
}

fn rust_absolute_type_path(root: &[String], scope: &[String], rust_name: &str) -> String {
    let mut path = root.to_vec();
    path.extend(scope.iter().map(|segment| rust_module_ident(segment)));
    path.push(rust_name.to_owned());
    path.join("::")
}

fn scoped_rust_candidate_type_names_by_id(
    types: impl IntoIterator<Item = (Uuid, Vec<String>, String)>,
) -> BTreeMap<Uuid, String> {
    let mut candidate_ids = BTreeMap::<(Vec<String>, String), Vec<(Uuid, String)>>::new();
    for (type_id, scope, candidate) in types {
        if type_id.is_nil() {
            continue;
        }
        candidate_ids
            .entry((scope, candidate.clone()))
            .or_default()
            .push((type_id, candidate));
    }

    let mut names_by_id = BTreeMap::new();
    for ((_, candidate), mut entries) in candidate_ids {
        entries.sort_by(|(left_id, left_name), (right_id, right_name)| {
            left_id
                .cmp(right_id)
                .then_with(|| left_name.cmp(right_name))
        });
        if entries.len() == 1 {
            names_by_id.insert(entries[0].0, candidate);
            continue;
        }
        for (type_id, _) in entries {
            names_by_id.insert(type_id, format!("{candidate}{}", short_type_id(type_id)));
        }
    }
    names_by_id
}

fn short_type_id(type_id: Uuid) -> String {
    type_id
        .simple()
        .to_string()
        .chars()
        .take(8)
        .collect::<String>()
        .to_ascii_uppercase()
}
