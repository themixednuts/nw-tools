use std::collections::BTreeMap;

use uuid::Uuid;

use crate::ir::{SerializeCodegenIndex, SerializeCodegenItem, SerializeCodegenUnit};

#[must_use]
pub fn dependency_ordered_codegen_items(unit: &SerializeCodegenUnit) -> Vec<&SerializeCodegenItem> {
    let index = unit.index();
    let items_by_type_id = index.items_by_type_id();
    let dependencies_by_type_id = dependency_type_ids_by_type_id(unit, &index);
    let mut roots = unit.items.iter().collect::<Vec<_>>();
    roots.sort_by(|left, right| {
        left.source_name
            .cmp(&right.source_name)
            .then_with(|| left.source_type_id.cmp(&right.source_type_id))
    });

    let mut states = BTreeMap::<Uuid, DependencyVisitState>::new();
    let mut ordered = Vec::with_capacity(unit.items.len());
    for root in roots {
        visit_dependency_ordered_item(
            root.source_type_id,
            items_by_type_id,
            &dependencies_by_type_id,
            &mut states,
            &mut ordered,
        );
    }
    ordered
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DependencyVisitState {
    Visiting,
    Visited,
}

fn visit_dependency_ordered_item<'a>(
    root_type_id: Uuid,
    items_by_type_id: &BTreeMap<Uuid, &'a SerializeCodegenItem>,
    dependencies_by_type_id: &BTreeMap<Uuid, Vec<Uuid>>,
    states: &mut BTreeMap<Uuid, DependencyVisitState>,
    ordered: &mut Vec<&'a SerializeCodegenItem>,
) {
    if states.get(&root_type_id) == Some(&DependencyVisitState::Visited) {
        return;
    }

    let mut stack = vec![(root_type_id, false)];
    while let Some((type_id, expanded)) = stack.pop() {
        match (expanded, states.get(&type_id).copied()) {
            (_, Some(DependencyVisitState::Visited)) => continue,
            (false, Some(DependencyVisitState::Visiting)) => continue,
            (true, _) => {
                states.insert(type_id, DependencyVisitState::Visited);
                if let Some(item) = items_by_type_id.get(&type_id) {
                    ordered.push(*item);
                }
            }
            (false, None) => {
                states.insert(type_id, DependencyVisitState::Visiting);
                stack.push((type_id, true));
                if let Some(dependencies) = dependencies_by_type_id.get(&type_id) {
                    for dependency_type_id in dependencies.iter().rev() {
                        if states
                            .get(dependency_type_id)
                            .is_none_or(|state| *state != DependencyVisitState::Visited)
                        {
                            stack.push((*dependency_type_id, false));
                        }
                    }
                }
            }
        }
    }
}

fn dependency_type_ids_by_type_id(
    unit: &SerializeCodegenUnit,
    index: &SerializeCodegenIndex<'_>,
) -> BTreeMap<Uuid, Vec<Uuid>> {
    unit.items
        .iter()
        .map(|item| {
            (
                item.source_type_id,
                index.known_direct_dependency_type_ids(item),
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use uuid::uuid;

    use crate::ir::{
        SerializeCodegenItem, SerializeCodegenItemKind, SerializeCodegenRttiBase,
        SerializeCodegenUnit,
    };
    use crate::role::ReflectedTypeRole;

    use super::*;

    #[test]
    fn dependency_order_uses_rtti_base_chain_edges() {
        let base_id = uuid!("AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA");
        let derived_id = uuid!("BBBBBBBB-BBBB-BBBB-BBBB-BBBBBBBBBBBB");
        let unit = SerializeCodegenUnit {
            items: vec![
                struct_item(
                    derived_id,
                    "A::Derived",
                    vec![SerializeCodegenRttiBase {
                        type_id: base_id,
                        source_name: "Z::Base".to_owned(),
                    }],
                ),
                struct_item(base_id, "Z::Base", Vec::new()),
            ],
        };

        let ordered_ids = dependency_ordered_codegen_items(&unit)
            .into_iter()
            .map(|item| item.source_type_id)
            .collect::<Vec<_>>();

        assert_eq!(ordered_ids, vec![base_id, derived_id]);
    }

    fn struct_item(
        source_type_id: Uuid,
        source_name: &str,
        rtti_base_chain: Vec<SerializeCodegenRttiBase>,
    ) -> SerializeCodegenItem {
        SerializeCodegenItem {
            source_type_id,
            source_name: source_name.to_owned(),
            role: ReflectedTypeRole::SupportType,
            is_reflection_marker: false,
            is_abstract: Some(false),
            factory: None,
            rtti_base_chain,
            kind: SerializeCodegenItemKind::Struct,
            enum_underlying_type: None,
            fields: Vec::new(),
            variants: Vec::new(),
        }
    }
}
