use std::collections::{BTreeMap, BTreeSet};

use uuid::Uuid;

use crate::model::{ReflectedClass, ReflectedMember, SerializeContextModel};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FieldOwnerEvidenceIndex {
    owner_fields_by_type_id: BTreeMap<Uuid, Vec<FieldOwnerEvidence>>,
    owners_by_exact_key: BTreeMap<FieldOwnerExactKey, Vec<FieldOwnerEvidence>>,
    owners_by_name_type_key: BTreeMap<FieldOwnerNameTypeKey, Vec<FieldOwnerEvidence>>,
    owners_by_type_offset_key: BTreeMap<FieldOwnerTypeOffsetKey, Vec<FieldOwnerEvidence>>,
    summary: FieldOwnerEvidenceSummary,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FieldOwnerEvidenceSummary {
    pub owner_type_count: usize,
    pub field_count: usize,
    pub exact_key_count: usize,
    pub name_type_key_count: usize,
    pub type_offset_key_count: usize,
    pub ambiguous_exact_key_count: usize,
    pub ambiguous_name_type_key_count: usize,
    pub ambiguous_type_offset_key_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct FieldOwnerEvidence {
    pub owner_type_id: Uuid,
    pub owner_name: String,
    pub field_name: String,
    pub field_type_id: Uuid,
    pub field_type_name: Option<String>,
    pub offset: Option<u32>,
    pub is_base_class: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldOwnerQuery {
    pub owner_type_id: Option<Uuid>,
    pub field_name: Option<String>,
    pub field_type_id: Option<Uuid>,
    pub offset: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FieldOwnerResolution {
    Found {
        kind: FieldOwnerResolutionKind,
        owner: FieldOwnerEvidence,
    },
    Ambiguous {
        kind: FieldOwnerResolutionKind,
        candidate_count: usize,
    },
    Missing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldOwnerResolutionKind {
    OwnerBody,
    ExactKey,
    NameTypeKey,
    TypeOffsetKey,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct FieldOwnerExactKey {
    field_name: String,
    field_type_id: Uuid,
    offset: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct FieldOwnerNameTypeKey {
    field_name: String,
    field_type_id: Uuid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct FieldOwnerTypeOffsetKey {
    field_type_id: Uuid,
    offset: u32,
}

impl FieldOwnerEvidenceIndex {
    #[must_use]
    pub fn from_model(model: &SerializeContextModel) -> Self {
        let mut index = Self::default();
        for class in model.classes.values() {
            index.insert_class_fields(model, class);
        }
        index.summary = index.build_summary();
        index
    }

    #[must_use]
    pub const fn summary(&self) -> FieldOwnerEvidenceSummary {
        self.summary
    }

    #[must_use]
    pub fn resolve(&self, query: &FieldOwnerQuery) -> FieldOwnerResolution {
        if let Some(owner_type_id) = query.owner_type_id
            && let Some(fields) = self.owner_fields_by_type_id.get(&owner_type_id)
            && let Some(owner) = unique_owner(fields.iter().filter(|field| field.matches(query)))
        {
            return FieldOwnerResolution::Found {
                kind: FieldOwnerResolutionKind::OwnerBody,
                owner,
            };
        }

        if let Some(key) = exact_key(
            query.field_name.as_deref(),
            query.field_type_id,
            query.offset,
        ) {
            let resolution = self.resolve_key(
                FieldOwnerResolutionKind::ExactKey,
                self.owners_by_exact_key.get(&key),
            );
            if !matches!(resolution, FieldOwnerResolution::Missing) {
                return resolution;
            }
        }

        if let Some(key) = name_type_key(query.field_name.as_deref(), query.field_type_id) {
            let resolution = self.resolve_key(
                FieldOwnerResolutionKind::NameTypeKey,
                self.owners_by_name_type_key.get(&key),
            );
            if !matches!(resolution, FieldOwnerResolution::Missing) {
                return resolution;
            }
        }

        if let Some(key) = type_offset_key(query.field_type_id, query.offset) {
            let resolution = self.resolve_key(
                FieldOwnerResolutionKind::TypeOffsetKey,
                self.owners_by_type_offset_key.get(&key),
            );
            if !matches!(resolution, FieldOwnerResolution::Missing) {
                return resolution;
            }
        }

        FieldOwnerResolution::Missing
    }

    #[must_use]
    pub fn fields_for_owner(&self, owner_type_id: Uuid) -> &[FieldOwnerEvidence] {
        self.owner_fields_by_type_id
            .get(&owner_type_id)
            .map_or(&[], Vec::as_slice)
    }

    fn insert_class_fields(&mut self, model: &SerializeContextModel, class: &ReflectedClass) {
        for member in &class.members {
            let evidence = field_owner_evidence(model, class, member);
            self.owner_fields_by_type_id
                .entry(evidence.owner_type_id)
                .or_default()
                .push(evidence.clone());
            if let Some(key) = exact_key(
                Some(&evidence.field_name),
                Some(evidence.field_type_id),
                evidence.offset,
            ) {
                self.owners_by_exact_key
                    .entry(key)
                    .or_default()
                    .push(evidence.clone());
            }
            if let Some(key) =
                name_type_key(Some(&evidence.field_name), Some(evidence.field_type_id))
            {
                self.owners_by_name_type_key
                    .entry(key)
                    .or_default()
                    .push(evidence.clone());
            }
            if let Some(key) = type_offset_key(Some(evidence.field_type_id), evidence.offset) {
                self.owners_by_type_offset_key
                    .entry(key)
                    .or_default()
                    .push(evidence);
            }
        }
    }

    fn resolve_key(
        &self,
        kind: FieldOwnerResolutionKind,
        owners: Option<&Vec<FieldOwnerEvidence>>,
    ) -> FieldOwnerResolution {
        let Some(owners) = owners else {
            return FieldOwnerResolution::Missing;
        };
        if let Some(owner) = unique_owner(owners.iter()) {
            FieldOwnerResolution::Found { kind, owner }
        } else {
            FieldOwnerResolution::Ambiguous {
                kind,
                candidate_count: distinct_owner_count(owners.iter()),
            }
        }
    }

    fn build_summary(&self) -> FieldOwnerEvidenceSummary {
        FieldOwnerEvidenceSummary {
            owner_type_count: self.owner_fields_by_type_id.len(),
            field_count: self
                .owner_fields_by_type_id
                .values()
                .map(Vec::len)
                .sum::<usize>(),
            exact_key_count: self.owners_by_exact_key.len(),
            name_type_key_count: self.owners_by_name_type_key.len(),
            type_offset_key_count: self.owners_by_type_offset_key.len(),
            ambiguous_exact_key_count: ambiguous_key_count(&self.owners_by_exact_key),
            ambiguous_name_type_key_count: ambiguous_key_count(&self.owners_by_name_type_key),
            ambiguous_type_offset_key_count: ambiguous_key_count(&self.owners_by_type_offset_key),
        }
    }
}

impl FieldOwnerEvidence {
    fn matches(&self, query: &FieldOwnerQuery) -> bool {
        let mut constrained = false;
        if let Some(field_name) = normalized_field_name(query.field_name.as_deref()) {
            constrained = true;
            if normalized_field_name(Some(&self.field_name)).as_deref() != Some(&field_name) {
                return false;
            }
        }
        if let Some(field_type_id) = query.field_type_id {
            constrained = true;
            if self.field_type_id != field_type_id {
                return false;
            }
        }
        if let Some(offset) = query.offset {
            constrained = true;
            if self.offset != Some(offset) {
                return false;
            }
        }
        constrained
    }
}

fn field_owner_evidence(
    model: &SerializeContextModel,
    class: &ReflectedClass,
    member: &ReflectedMember,
) -> FieldOwnerEvidence {
    FieldOwnerEvidence {
        owner_type_id: class.type_id,
        owner_name: class.name.clone(),
        field_name: member.name.clone(),
        field_type_id: member.type_id,
        field_type_name: member
            .az_rtti
            .as_ref()
            .and_then(|rtti| rtti.type_name.clone())
            .or_else(|| model.type_name(member.type_id).map(str::to_owned)),
        offset: member.offset,
        is_base_class: member.is_base_class,
    }
}

fn exact_key(
    field_name: Option<&str>,
    field_type_id: Option<Uuid>,
    offset: Option<u32>,
) -> Option<FieldOwnerExactKey> {
    Some(FieldOwnerExactKey {
        field_name: normalized_field_name(field_name)?,
        field_type_id: field_type_id?,
        offset: offset?,
    })
}

fn name_type_key(
    field_name: Option<&str>,
    field_type_id: Option<Uuid>,
) -> Option<FieldOwnerNameTypeKey> {
    Some(FieldOwnerNameTypeKey {
        field_name: normalized_field_name(field_name)?,
        field_type_id: field_type_id?,
    })
}

fn type_offset_key(
    field_type_id: Option<Uuid>,
    offset: Option<u32>,
) -> Option<FieldOwnerTypeOffsetKey> {
    Some(FieldOwnerTypeOffsetKey {
        field_type_id: field_type_id?,
        offset: offset?,
    })
}

fn normalized_field_name(field_name: Option<&str>) -> Option<String> {
    field_name
        .map(str::trim)
        .filter(|field_name| !field_name.is_empty())
        .map(str::to_owned)
}

fn unique_owner<'a>(
    owners: impl IntoIterator<Item = &'a FieldOwnerEvidence>,
) -> Option<FieldOwnerEvidence> {
    let distinct = owners.into_iter().collect::<BTreeSet<_>>();
    if distinct.len() == 1 {
        distinct.into_iter().next().cloned()
    } else {
        None
    }
}

fn distinct_owner_count<'a>(owners: impl IntoIterator<Item = &'a FieldOwnerEvidence>) -> usize {
    owners.into_iter().collect::<BTreeSet<_>>().len()
}

fn ambiguous_key_count<K: Ord>(index: &BTreeMap<K, Vec<FieldOwnerEvidence>>) -> usize {
    index
        .values()
        .filter(|owners| unique_owner(owners.iter()).is_none())
        .count()
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use uuid::uuid;

    use crate::model::SerializeContextModel;

    use super::*;

    #[test]
    fn resolves_field_owner_from_exact_schema_key() {
        let owner_id = uuid!("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa");
        let field_id = uuid!("bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb");
        let model = SerializeContextModel::from_root(&json!({
            "uuidMap": {
                "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa": {
                    "name": "Example::Owner",
                    "typeId": "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa",
                    "elements": [{
                        "name": "m_value",
                        "typeId": "bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb",
                        "offset": 16,
                        "is_base_class": false
                    }]
                },
                "bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb": {
                    "name": "Example::Value",
                    "typeId": "bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb",
                    "elements": []
                }
            },
            "classNameToUuid": [],
            "uuidGenericMap": [],
            "uuidAnyCreationMap": {},
            "editContext": {"classData": [], "enumData": []},
            "enumTypeIdToUnderlyingTypeIdMap": {}
        }));
        let index = FieldOwnerEvidenceIndex::from_model(&model);

        let resolution = index.resolve(&FieldOwnerQuery {
            owner_type_id: None,
            field_name: Some("m_value".to_owned()),
            field_type_id: Some(field_id),
            offset: Some(16),
        });

        assert_eq!(
            resolution,
            FieldOwnerResolution::Found {
                kind: FieldOwnerResolutionKind::ExactKey,
                owner: FieldOwnerEvidence {
                    owner_type_id: owner_id,
                    owner_name: "Example::Owner".to_owned(),
                    field_name: "m_value".to_owned(),
                    field_type_id: field_id,
                    field_type_name: Some("Example::Value".to_owned()),
                    offset: Some(16),
                    is_base_class: false,
                },
            }
        );
        assert_eq!(index.summary().owner_type_count, 1);
        assert_eq!(index.summary().field_count, 1);
    }

    #[test]
    fn keeps_shared_field_keys_ambiguous() {
        let field_id = uuid!("bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb");
        let model = SerializeContextModel::from_root(&json!({
            "uuidMap": {
                "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa": {
                    "name": "Example::Left",
                    "typeId": "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa",
                    "elements": [{
                        "name": "m_value",
                        "typeId": "bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb",
                        "offset": 16,
                        "is_base_class": false
                    }]
                },
                "cccccccc-cccc-cccc-cccc-cccccccccccc": {
                    "name": "Example::Right",
                    "typeId": "cccccccc-cccc-cccc-cccc-cccccccccccc",
                    "elements": [{
                        "name": "m_value",
                        "typeId": "bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb",
                        "offset": 16,
                        "is_base_class": false
                    }]
                },
                "bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb": {
                    "name": "Example::Value",
                    "typeId": "bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb",
                    "elements": []
                }
            },
            "classNameToUuid": [],
            "uuidGenericMap": [],
            "uuidAnyCreationMap": {},
            "editContext": {"classData": [], "enumData": []},
            "enumTypeIdToUnderlyingTypeIdMap": {}
        }));
        let index = FieldOwnerEvidenceIndex::from_model(&model);

        let resolution = index.resolve(&FieldOwnerQuery {
            owner_type_id: None,
            field_name: Some("m_value".to_owned()),
            field_type_id: Some(field_id),
            offset: Some(16),
        });

        assert_eq!(
            resolution,
            FieldOwnerResolution::Ambiguous {
                kind: FieldOwnerResolutionKind::ExactKey,
                candidate_count: 2,
            }
        );
        assert_eq!(index.summary().ambiguous_exact_key_count, 1);
    }

    #[test]
    fn owner_body_scope_disambiguates_shared_field_keys() {
        let owner_id = uuid!("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa");
        let field_id = uuid!("bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb");
        let model = SerializeContextModel::from_root(&json!({
            "uuidMap": {
                "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa": {
                    "name": "Example::Left",
                    "typeId": "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa",
                    "elements": [{
                        "name": "m_value",
                        "typeId": "bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb",
                        "offset": 16,
                        "is_base_class": false
                    }]
                },
                "cccccccc-cccc-cccc-cccc-cccccccccccc": {
                    "name": "Example::Right",
                    "typeId": "cccccccc-cccc-cccc-cccc-cccccccccccc",
                    "elements": [{
                        "name": "m_value",
                        "typeId": "bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb",
                        "offset": 16,
                        "is_base_class": false
                    }]
                },
                "bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb": {
                    "name": "Example::Value",
                    "typeId": "bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb",
                    "elements": []
                }
            },
            "classNameToUuid": [],
            "uuidGenericMap": [],
            "uuidAnyCreationMap": {},
            "editContext": {"classData": [], "enumData": []},
            "enumTypeIdToUnderlyingTypeIdMap": {}
        }));
        let index = FieldOwnerEvidenceIndex::from_model(&model);

        let resolution = index.resolve(&FieldOwnerQuery {
            owner_type_id: Some(owner_id),
            field_name: Some("m_value".to_owned()),
            field_type_id: Some(field_id),
            offset: Some(16),
        });

        assert!(matches!(
            resolution,
            FieldOwnerResolution::Found {
                kind: FieldOwnerResolutionKind::OwnerBody,
                ..
            }
        ));
    }
}
