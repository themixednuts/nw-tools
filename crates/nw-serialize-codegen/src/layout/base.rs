use std::collections::{BTreeMap, BTreeSet};

use uuid::Uuid;

use crate::ir::{SerializeCodegenItem, SerializeCodegenUnit};
use crate::types::ResolvedType;

use super::LayoutBaseEdge;

pub(crate) fn primary_base_chains_by_type_id(
    items_by_type_id: &BTreeMap<Uuid, &SerializeCodegenItem>,
) -> BTreeMap<Uuid, Vec<LayoutBaseEdge>> {
    items_by_type_id
        .values()
        .copied()
        .map(|item| {
            (
                item.source_type_id,
                primary_base_chain_edges(item, items_by_type_id),
            )
        })
        .collect()
}

#[must_use]
pub fn reflected_base_type_ids(unit: &SerializeCodegenUnit) -> BTreeSet<Uuid> {
    let index = unit.index();
    let items_by_type_id = index.items_by_type_id();
    unit.items
        .iter()
        .flat_map(|item| reflected_base_type_ids_for_item(item, items_by_type_id))
        .collect()
}

pub(crate) fn primary_base_chain(
    item: &SerializeCodegenItem,
    items_by_type_id: &BTreeMap<Uuid, &SerializeCodegenItem>,
) -> Vec<String> {
    primary_base_chain_edges(item, items_by_type_id)
        .into_iter()
        .map(|edge| edge.source_name)
        .collect()
}

pub(crate) fn primary_base_chain_edges(
    item: &SerializeCodegenItem,
    items_by_type_id: &BTreeMap<Uuid, &SerializeCodegenItem>,
) -> Vec<LayoutBaseEdge> {
    let mut chain = Vec::new();
    let mut seen = BTreeSet::new();
    let mut current = item;
    loop {
        if !seen.insert(current.source_type_id) {
            break;
        }

        let Some((base_type_id, base_name)) = primary_base(current) else {
            break;
        };

        let matching_base_item = items_by_type_id
            .get(&base_type_id)
            .filter(|base_item| base_item.source_name == base_name);
        chain.push(LayoutBaseEdge {
            type_id: base_type_id,
            source_name: base_name,
            matches_reflected_type: matching_base_item.is_some(),
        });

        let Some(base_item) = matching_base_item else {
            break;
        };
        current = base_item;
    }
    chain.reverse();
    chain
}

fn primary_base(item: &SerializeCodegenItem) -> Option<(Uuid, String)> {
    serialized_primary_base(item).or_else(|| rtti_primary_base(item))
}

fn serialized_primary_base(item: &SerializeCodegenItem) -> Option<(Uuid, String)> {
    item.fields
        .iter()
        .filter(|field| field.is_base_class)
        .find_map(|field| match &field.resolved_type {
            ResolvedType::Named {
                type_id,
                source_name,
            } => Some((*type_id, source_name.clone())),
            ResolvedType::Unknown { .. }
            | ResolvedType::Scalar(_)
            | ResolvedType::Sequence { .. }
            | ResolvedType::Map { .. }
            | ResolvedType::RangedInteger { .. }
            | ResolvedType::ByteStream
            | ResolvedType::Pair { .. }
            | ResolvedType::Pointer { .. }
            | ResolvedType::Optional { .. }
            | ResolvedType::Asset { .. }
            | ResolvedType::Uid { .. }
            | ResolvedType::ReplicatedField { .. }
            | ResolvedType::Tuple { .. } => None,
        })
}

fn rtti_primary_base(item: &SerializeCodegenItem) -> Option<(Uuid, String)> {
    item.rtti_base_chain
        .last()
        .map(|base| (base.type_id, base.source_name.clone()))
}

fn reflected_base_type_ids_for_item(
    item: &SerializeCodegenItem,
    items_by_type_id: &BTreeMap<Uuid, &SerializeCodegenItem>,
) -> BTreeSet<Uuid> {
    let mut base_type_ids = BTreeSet::new();
    for field in &item.fields {
        if !field.is_base_class {
            continue;
        }
        let ResolvedType::Named {
            type_id,
            source_name,
        } = &field.resolved_type
        else {
            continue;
        };
        if items_by_type_id
            .get(type_id)
            .is_some_and(|base_item| base_item.source_name == *source_name)
        {
            base_type_ids.insert(*type_id);
        }
    }
    for base in &item.rtti_base_chain {
        if items_by_type_id
            .get(&base.type_id)
            .is_some_and(|base_item| base_item.source_name == base.source_name)
        {
            base_type_ids.insert(base.type_id);
        }
    }
    base_type_ids
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use uuid::uuid;

    use crate::ir::{
        SerializeCodegenField, SerializeCodegenItem, SerializeCodegenItemKind,
        SerializeCodegenRttiBase, SerializeCodegenVariant,
    };
    use crate::role::ReflectedTypeRole;
    use crate::types::ResolvedType;

    use super::primary_base_chain_edges;

    #[test]
    fn primary_base_chain_edges_order_root_base_before_direct_base() {
        let root_id = uuid!("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa");
        let middle_id = uuid!("bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb");
        let leaf_id = uuid!("cccccccc-cccc-cccc-cccc-cccccccccccc");
        let root = item(root_id, "Example::Root", Vec::new());
        let middle = item(
            middle_id,
            "Example::Middle",
            vec![base_field(root_id, "Example::Root")],
        );
        let leaf = item(
            leaf_id,
            "Example::Leaf",
            vec![base_field(middle_id, "Example::Middle")],
        );
        let items_by_type_id = BTreeMap::from([
            (root.source_type_id, &root),
            (middle.source_type_id, &middle),
            (leaf.source_type_id, &leaf),
        ]);

        let chain = primary_base_chain_edges(&leaf, &items_by_type_id);

        assert_eq!(
            chain
                .into_iter()
                .map(|edge| edge.source_name)
                .collect::<Vec<_>>(),
            vec!["Example::Root", "Example::Middle"]
        );
    }

    #[test]
    fn primary_base_chain_edges_stop_cycles_without_recursive_walks() {
        let left_id = uuid!("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa");
        let right_id = uuid!("bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb");
        let left = item(
            left_id,
            "Example::Left",
            vec![base_field(right_id, "Example::Right")],
        );
        let right = item(
            right_id,
            "Example::Right",
            vec![base_field(left_id, "Example::Left")],
        );
        let items_by_type_id =
            BTreeMap::from([(left.source_type_id, &left), (right.source_type_id, &right)]);

        let chain = primary_base_chain_edges(&left, &items_by_type_id);

        assert_eq!(
            chain
                .into_iter()
                .map(|edge| edge.source_name)
                .collect::<Vec<_>>(),
            vec!["Example::Left", "Example::Right"]
        );
    }

    fn item(
        source_type_id: uuid::Uuid,
        source_name: &str,
        fields: Vec<SerializeCodegenField>,
    ) -> SerializeCodegenItem {
        SerializeCodegenItem {
            source_type_id,
            source_name: source_name.to_owned(),
            role: ReflectedTypeRole::SupportType,
            is_reflection_marker: false,
            is_abstract: Some(false),
            factory: None,
            rtti_base_chain: Vec::<SerializeCodegenRttiBase>::new(),
            kind: SerializeCodegenItemKind::Struct,
            enum_underlying_type: None,
            fields,
            variants: Vec::<SerializeCodegenVariant>::new(),
        }
    }

    fn base_field(source_type_id: uuid::Uuid, source_name: &str) -> SerializeCodegenField {
        SerializeCodegenField {
            source_name: "BaseClass1".to_owned(),
            source_type_id,
            resolved_type: ResolvedType::Named {
                type_id: source_type_id,
                source_name: source_name.to_owned(),
            },
            data_size: None,
            offset: None,
            flags: None,
            is_base_class: true,
            is_pointer: false,
            is_dynamic_field: false,
        }
    }
}
