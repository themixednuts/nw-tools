//! Specialized template identities folded from a generic's own reflection.
//!
//! `AZ_TYPE_INFO_INTERNAL_SPECIALIZED_TEMPLATE_POSTFIX_UUID` is a left fold
//! with `Uuid::CreateData` over `AzTypeInfo<Arg>::Uuid()` per template
//! argument, then the template base; the `azstd_*` helpers in
//! [`nw_objectstream::type_uuid`] are that fold per template. A generic's
//! `templatedTypeIds` record `GenericClassInfo::GetTemplatedTypeId`, which
//! is `SerializeGenericTypeInfo<Arg>::GetClassTypeId()`, and that is not
//! always the identity the fold takes:
//!
//! | argument | fold takes | `templatedTypeIds` records |
//! |---|---|---|
//! | fundamental or reflected class | its id | the same |
//! | enum with `AZ_TYPE_INFO_SPECIALIZE` | that id | the underlying integer's id |
//! | enum without one | the null id (16 zero bytes, still hashed) | the underlying integer's id |
//! | nested template (`AZStd::string`, `shared_ptr<T>`, another map) | its own folded id | the template base |
//!
//! `SerializeGenericTypeInfo` applies `RemoveEnum` to every enum, so the
//! enum's own id reaches the capture another way: `ClassBuilder::Field` adds
//! the `EnumType` attribute (`AZ_CRC("EnumType", 0xb177e1b5)`) to a field
//! whose enum has a non-null id, the container's reflected `value1` /
//! `element` field carries it, and the pair's or wrapper's
//! `typeIdFoldTypeIds` copy it. [`SpecializedTypeIdFolder`] takes a pair's
//! or wrapper's `typeIdFoldTypeIds` when they are recorded (they carry the
//! null id for an enum without a specialization), otherwise reads each
//! argument from the member that stores it: the `EnumType` attribute, then
//! the member's own generic info folded recursively, then the member's type
//! id. A pair's `value1` / `value2` and a tuple's `Value1`…`ValueN` members
//! are read this way. Only when a generic exposes no member does it fall
//! back to `templatedTypeIds`.
//!
//! An argument the capture does not record at all cannot be recovered. A
//! vector declared with an explicit `AZ::AZStdAlloc<Allocator>` records only
//! its element (`templatedArgumentCount` 1), so the fold assumes
//! `AZStd::allocator` and misses; the sweep test names the three such
//! vectors and `nw_objectstream::type_uuid` reproduces them from their
//! allocators.

use std::collections::{BTreeMap, BTreeSet};

use nw_objectstream::type_uuid::{self, type_ids};
use uuid::Uuid;

use crate::model::{ReflectedGenericClass, ReflectedMember, SerializeContextModel};

const MAX_FOLD_DEPTH: usize = 32;

/// Templates whose specializations [`SpecializedTypeIdFolder`] folds.
pub const FOLDED_TEMPLATE_NAMES: &[&str] = &[
    "AZStd::unordered_map",
    "AZStd::unordered_flat_map",
    "AZStd::map",
    "AZStd::unordered_set",
    "AZStd::set",
    "AZStd::vector",
    "AZStd::list",
    "AZStd::forward_list",
    "AZStd::fixed_vector",
    "AZStd::array",
    "BitSet",
    "AZStd::pair",
    "AZStd::basic_string",
    "AZStd::string",
    "AZStd::shared_ptr",
    "AZStd::unique_ptr",
    "AZStd::intrusive_ptr",
    "AZStd::optional",
    "AZStd::ranged_int",
    "AZStd::tuple",
    "Asset",
    "AZ::Data::Asset",
    "MB::ReplicatedField",
    "Internal::RValueToLValueWrapper",
];

/// The identity the capture records for a specialization: `specializedTypeId`,
/// else the generic's `typeId`, else the `uuidGenericMap` key.
///
/// `Asset<T>` reports the `GenericClassGenericAsset` identity as both its
/// generic and its specialized id; its specialization is the id it is
/// registered under.
#[must_use]
pub fn recorded_type_id(generic: &ReflectedGenericClass) -> Option<Uuid> {
    let specialized = generic.specialized_type_id.or(generic.type_id);
    if specialized.is_some() && specialized == generic.generic_type_id {
        return generic
            .map_key_type_id
            .or_else(|| generic.registered_type_ids.first().copied())
            .or(specialized);
    }
    specialized.or(generic.map_key_type_id)
}

/// Folds a generic's specialized identity from the identities its arguments
/// contribute.
#[derive(Debug, Clone, Copy)]
pub struct SpecializedTypeIdFolder<'a> {
    model: &'a SerializeContextModel,
}

impl<'a> SpecializedTypeIdFolder<'a> {
    #[must_use]
    pub const fn new(model: &'a SerializeContextModel) -> Self {
        Self { model }
    }

    /// The identity the fold reproduces for `generic`, or `None` when its
    /// template is not in [`FOLDED_TEMPLATE_NAMES`] or it exposes fewer
    /// arguments than the template takes.
    #[must_use]
    pub fn fold(&self, generic: &ReflectedGenericClass) -> Option<Uuid> {
        self.fold_at(generic, 0)
    }

    /// The identity each argument contributes to the fold, in template order.
    #[must_use]
    pub fn argument_identities(&self, generic: &ReflectedGenericClass) -> Vec<Uuid> {
        self.argument_identities_at(generic, 0)
    }

    /// The fold over `templatedTypeIds` alone: what the specialization would
    /// be if every argument were the identity the capture records for it.
    #[must_use]
    pub fn fold_from_templated_type_ids(&self, generic: &ReflectedGenericClass) -> Option<Uuid> {
        fold_template(generic, &generic.templated_type_ids)
    }

    /// Folds every registered specialization of the named templates and
    /// compares each with the identity the capture records.
    #[must_use]
    pub fn sweep(&self, template_names: &[&str]) -> SpecializedTypeIdSweep {
        let mut seen = BTreeSet::new();
        let mut entries = Vec::new();
        for generic in self.model.generic_classes.values() {
            let Some(class_name) = generic.class_name.as_deref() else {
                continue;
            };
            if !template_names.contains(&class_name) {
                continue;
            }
            let Some(recorded) = recorded_type_id(generic) else {
                continue;
            };
            if !seen.insert(recorded) {
                continue;
            }
            let argument_identities = self.argument_identities(generic);
            entries.push(SpecializedTypeIdFoldEntry {
                recorded_type_id: recorded,
                class_name: class_name.to_owned(),
                templated_type_ids: generic.templated_type_ids.clone(),
                folded_type_id: fold_template(generic, &argument_identities),
                templated_fold_type_id: self.fold_from_templated_type_ids(generic),
                argument_identities,
            });
        }
        entries.sort_by(|left, right| {
            left.class_name
                .cmp(&right.class_name)
                .then(left.recorded_type_id.cmp(&right.recorded_type_id))
        });
        SpecializedTypeIdSweep { entries }
    }

    fn fold_at(&self, generic: &ReflectedGenericClass, depth: usize) -> Option<Uuid> {
        if depth >= MAX_FOLD_DEPTH {
            return None;
        }
        let arguments = self.argument_identities_at(generic, depth);
        fold_template(generic, &arguments)
    }

    fn argument_identities_at(&self, generic: &ReflectedGenericClass, depth: usize) -> Vec<Uuid> {
        // `MB::ReplicatedField` writes its own template base into
        // `typeIdFoldTypeIds`; its `value` member carries the argument.
        let fold_type_ids_carry_arguments =
            generic.class_name.as_deref() != Some("MB::ReplicatedField");
        let leading = if generic.type_id_fold_type_ids.is_empty() || !fold_type_ids_carry_arguments
        {
            self.member_argument_identities(generic, depth)
        } else {
            generic.type_id_fold_type_ids.clone()
        };
        if leading.is_empty() {
            return generic.templated_type_ids.clone();
        }
        let mut arguments = leading;
        if let Some(trailing) = generic.templated_type_ids.get(arguments.len()..) {
            arguments.extend_from_slice(trailing);
        }
        arguments
    }

    /// The leading arguments read from the members that store them.
    fn member_argument_identities(
        &self,
        generic: &ReflectedGenericClass,
        depth: usize,
    ) -> Vec<Uuid> {
        match generic.class_name.as_deref() {
            Some(
                "AZStd::map"
                | "AZStd::unordered_map"
                | "AZStd::unordered_flat_map"
                | "AZStd::unordered_multimap",
            ) => member_named(generic, "element")
                .and_then(|element| self.nested_generic(element))
                .filter(|pair| pair.class_name.as_deref() == Some("AZStd::pair"))
                .map(|pair| self.argument_identities_at(pair, depth + 1))
                .unwrap_or_default(),
            Some("AZStd::pair") => {
                match (
                    member_named(generic, "value1"),
                    member_named(generic, "value2"),
                ) {
                    (Some(first), Some(second)) => vec![
                        self.member_identity(first, depth),
                        self.member_identity(second, depth),
                    ],
                    _ => Vec::new(),
                }
            }
            Some("AZStd::tuple") => generic
                .members
                .iter()
                .map(|member| self.member_identity(member, depth))
                .collect(),
            Some("BitSet" | "Amazon::Pervasives::UID") => Vec::new(),
            _ => member_named(generic, "element")
                .or_else(|| member_named(generic, "value"))
                .or_else(|| generic.members.first())
                .map(|member| vec![self.member_identity(member, depth)])
                .unwrap_or_default(),
        }
    }

    /// The identity a member's type contributes: the enum named by its
    /// `EnumType` attribute, else its own generic info folded, else its type
    /// id.
    fn member_identity(&self, member: &ReflectedMember, depth: usize) -> Uuid {
        if let Some(enum_type_id) = member.enum_type_id() {
            return enum_type_id;
        }
        if let Some(nested) = self.nested_generic(member)
            && let Some(folded) = self.fold_at(nested, depth + 1)
        {
            return folded;
        }
        member.type_id
    }

    fn nested_generic<'m>(
        &'m self,
        member: &'m ReflectedMember,
    ) -> Option<&'m ReflectedGenericClass> {
        member
            .generic_class
            .as_deref()
            .or_else(|| self.model.generic_class(member.type_id))
    }
}

fn member_named<'m>(generic: &'m ReflectedGenericClass, name: &str) -> Option<&'m ReflectedMember> {
    generic.members.iter().find(|member| member.name == name)
}

/// The postfix (or, for the few prefix templates, prefix) fold of `arguments`
/// under `generic`'s template base.
fn fold_template(generic: &ReflectedGenericClass, arguments: &[Uuid]) -> Option<Uuid> {
    let class_name = generic.class_name.as_deref()?;
    Some(match (class_name, arguments) {
        ("AZStd::unordered_map", [key, value, ..]) => type_uuid::azstd_unordered_map(*key, *value),
        ("AZStd::unordered_flat_map", [key, value, ..]) => {
            type_uuid::azstd_unordered_flat_map(*key, *value)
        }
        ("AZStd::map", [key, value, ..]) => type_uuid::azstd_map(*key, *value),
        ("AZStd::unordered_set", [key, ..]) => type_uuid::azstd_unordered_set(*key),
        ("AZStd::set", [key, ..]) => type_uuid::azstd_set(*key),
        ("AZStd::vector", [element]) => type_uuid::azstd_vector(*element),
        ("AZStd::vector", [element, allocator, ..]) => {
            type_uuid::azstd_vector_with_allocator(*element, *allocator)
        }
        ("AZStd::list", [element, ..]) => type_uuid::azstd_list(*element),
        ("AZStd::forward_list", [element, ..]) => type_uuid::azstd_forward_list(*element),
        ("AZStd::fixed_vector", [element, ..]) => {
            type_uuid::azstd_fixed_vector(*element, generic.non_type_capacity()?)
        }
        ("AZStd::array", [element, ..]) => {
            type_uuid::azstd_array(*element, generic.non_type_capacity()?)
        }
        ("BitSet", _) => type_uuid::azstd_bitset(generic.non_type_capacity()?),
        ("AZStd::pair", [first, second, ..]) => type_uuid::azstd_pair(*first, *second),
        ("AZStd::basic_string" | "AZStd::string", [char_type]) if *char_type == type_ids::CHAR => {
            type_uuid::azstd_string()
        }
        ("AZStd::basic_string" | "AZStd::string", [char_type, traits, allocator, ..]) => {
            type_uuid::azstd_basic_string(*char_type, *traits, *allocator)
        }
        ("AZStd::shared_ptr", [element, ..]) => type_uuid::azstd_shared_ptr(*element),
        ("AZStd::unique_ptr", [element, ..]) => type_uuid::azstd_unique_ptr(*element),
        ("AZStd::intrusive_ptr", [element, ..]) => type_uuid::azstd_intrusive_ptr(*element),
        ("AZStd::optional", [element, ..]) => type_uuid::azstd_optional(*element),
        ("AZStd::ranged_int", [value, ..]) => {
            let (min, max) = generic.non_type_integer_bounds()?;
            type_uuid::azstd_ranged_int(
                *value,
                usize::try_from(min).ok()?,
                usize::try_from(max).ok()?,
            )
        }
        ("AZStd::tuple", arguments) => type_uuid::azstd_tuple(arguments)?,
        ("Asset" | "AZ::Data::Asset", [asset_type, ..]) => type_uuid::az_data_asset(*asset_type),
        ("MB::ReplicatedField", [value, ..]) => type_uuid::mb_replicated_field(*value),
        ("Internal::RValueToLValueWrapper", [value, ..]) => {
            type_uuid::az_internal_rvalue_to_lvalue_wrapper(*value)
        }
        _ => return None,
    })
}

/// One specialization compared with the identity the capture records for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpecializedTypeIdFoldEntry {
    pub recorded_type_id: Uuid,
    pub class_name: String,
    pub templated_type_ids: Vec<Uuid>,
    /// The identities the fold takes, per the rule in the module docs.
    pub argument_identities: Vec<Uuid>,
    /// The fold over `argument_identities`.
    pub folded_type_id: Option<Uuid>,
    /// The fold over `templated_type_ids` alone.
    pub templated_fold_type_id: Option<Uuid>,
}

impl SpecializedTypeIdFoldEntry {
    #[must_use]
    pub fn reproduces(&self) -> bool {
        self.folded_type_id == Some(self.recorded_type_id)
    }

    #[must_use]
    pub fn reproduces_from_templated_type_ids(&self) -> bool {
        self.templated_fold_type_id == Some(self.recorded_type_id)
    }
}

/// Per-template counts of a [`SpecializedTypeIdSweep`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SpecializedTypeIdSweepCount {
    pub recorded: usize,
    pub reproduced: usize,
    pub reproduced_from_templated_type_ids: usize,
}

/// Every registered specialization of the swept templates, folded.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SpecializedTypeIdSweep {
    pub entries: Vec<SpecializedTypeIdFoldEntry>,
}

impl SpecializedTypeIdSweep {
    #[must_use]
    pub fn reproduced(&self) -> usize {
        self.entries
            .iter()
            .filter(|entry| entry.reproduces())
            .count()
    }

    #[must_use]
    pub fn reproduced_from_templated_type_ids(&self) -> usize {
        self.entries
            .iter()
            .filter(|entry| entry.reproduces_from_templated_type_ids())
            .count()
    }

    pub fn misses(&self) -> impl Iterator<Item = &SpecializedTypeIdFoldEntry> {
        self.entries.iter().filter(|entry| !entry.reproduces())
    }

    #[must_use]
    pub fn by_template(&self) -> BTreeMap<&str, SpecializedTypeIdSweepCount> {
        let mut counts = BTreeMap::<&str, SpecializedTypeIdSweepCount>::new();
        for entry in &self.entries {
            let count = counts.entry(entry.class_name.as_str()).or_default();
            count.recorded += 1;
            count.reproduced += usize::from(entry.reproduces());
            count.reproduced_from_templated_type_ids +=
                usize::from(entry.reproduces_from_templated_type_ids());
        }
        counts
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use serde_json::json;
    use uuid::uuid;

    use super::*;
    use crate::document::SerializeContextDocument;

    fn shipped_model() -> SerializeContextModel {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("resources")
            .join("serialize.json");
        let document = SerializeContextDocument::from_path(path)
            .expect("project serialize.json should match generated schema");
        SerializeContextModel::from_document(&document)
    }

    fn registered_generic(model: &SerializeContextModel, type_id: Uuid) -> &ReflectedGenericClass {
        model
            .generic_class(type_id)
            .unwrap_or_else(|| panic!("serialize.json should register {type_id}"))
    }

    /// The three enum-keyed members and the three other enum-argument maps:
    /// `AttributeComponent::m_preReloadAttributes`
    /// (`unordered_map<CharacterAttributeType, int>`, key getter
    /// `NewWorld+0x27a0cb0`), `SBItemClass::m_ItemClasses`
    /// (`unordered_set<ItemClasses>`, key getter `NewWorld+0x2e8a550`),
    /// `PaperdollComponent::m_paperdollVisualSlotMapping`
    /// (`map<PaperdollSlotTypes, AZStd::string>`, key getter
    /// `NewWorld+0x4620da0`), and the maps whose pair `typeIdFoldTypeIds`
    /// name `ContributionType`, `SettlementProgressionCategory` and
    /// `PaperdollSlotTypes` where `templatedTypeIds` record `int`.
    #[test]
    fn folds_enum_arguments_under_their_own_identity() {
        let model = shipped_model();
        let folder = SpecializedTypeIdFolder::new(&model);
        let character_attribute_type = uuid!("F4197081-D1D9-4A95-8FA0-81534BE2C33B");
        let item_classes = uuid!("A3755086-8B0A-4D14-B073-FC3E1433C3F6");
        let paperdoll_slot_types = uuid!("5D42C439-A859-4133-9032-88DE31048F2C");

        for (recorded, class_name, arguments) in [
            (
                uuid!("F87FA543-7DF5-5FC8-8767-9F1AFCBD1D0D"),
                "AZStd::unordered_map",
                vec![character_attribute_type, type_ids::INT],
            ),
            (
                uuid!("104B16EC-793B-5BB1-B613-1F4343F3C94F"),
                "AZStd::unordered_set",
                vec![item_classes],
            ),
            (
                uuid!("5D30068C-1D6A-51F6-94A1-FA512EF61ED6"),
                "AZStd::map",
                vec![paperdoll_slot_types, type_ids::AZSTD_STRING],
            ),
            (
                uuid!("74CCF29C-5848-5404-80AD-EC6284EA6E12"),
                "AZStd::unordered_map",
                vec![
                    uuid!("EA27A445-C5F3-42AB-8BA0-8F617A19DC38"),
                    type_ids::FLOAT,
                ],
            ),
            (
                uuid!("07EDC5F7-55F7-5CB3-BFB6-5C783891CDE4"),
                "AZStd::unordered_map",
                vec![
                    type_ids::ENTITY_ID,
                    uuid!("99FFBB9B-34A3-44A1-A576-1D13D732B0AA"),
                ],
            ),
            (
                uuid!("A1A7BDBF-18A6-579D-9261-37DC8F309DBA"),
                "AZStd::unordered_map",
                vec![
                    paperdoll_slot_types,
                    uuid!("1BE36174-FD4F-4A1C-8E52-7C28D50EEC5A"),
                ],
            ),
        ] {
            let generic = registered_generic(&model, recorded);
            assert_eq!(generic.class_name.as_deref(), Some(class_name));
            assert_eq!(
                folder.argument_identities(generic),
                arguments,
                "{recorded} arguments"
            );
            assert_eq!(folder.fold(generic), Some(recorded), "{recorded} fold");
            assert_ne!(
                folder.fold_from_templated_type_ids(generic),
                Some(recorded),
                "{recorded} records the enum as its underlying integer"
            );
        }
    }

    /// `NewWorld+0x6966860` folds an `unordered_map<enum, VitalsStatData>`
    /// whose key enum has no `AZ_TYPE_INFO_SPECIALIZE`, with
    /// `AZ::Uuid::GetNull()` as the key; the pair's `typeIdFoldTypeIds`
    /// record that null id where `templatedTypeIds` record `u8`.
    #[test]
    fn folds_an_unspecialized_enum_argument_as_the_null_identity() {
        let model = shipped_model();
        let folder = SpecializedTypeIdFolder::new(&model);
        let recorded = uuid!("585E4F2A-A289-53DE-ACF6-766B76FB7147");
        let generic = registered_generic(&model, recorded);

        assert_eq!(
            folder.argument_identities(generic),
            [Uuid::nil(), uuid!("050982C9-1218-4C39-9B5A-E4192297825D")]
        );
        assert_eq!(folder.fold(generic), Some(recorded));
        assert_ne!(folder.fold_from_templated_type_ids(generic), Some(recorded));
    }

    /// `unordered_map<AZ::Uuid, shared_ptr<D34A1B4F>>` records the
    /// `shared_ptr` base as its value argument; the value member's own
    /// generic info folds the pointee into the specialization the fold takes.
    #[test]
    fn folds_a_nested_specialization_argument() {
        let model = shipped_model();
        let folder = SpecializedTypeIdFolder::new(&model);
        let recorded = uuid!("FB2ABB26-BEA1-5A5E-885F-5A0038082549");
        let generic = registered_generic(&model, recorded);
        let pointee = uuid!("D34A1B4F-C0F3-4E73-A88A-FBC48FF7800E");

        assert_eq!(
            generic.templated_type_ids,
            [type_ids::AZ_UUID, type_ids::AZSTD_SHARED_PTR]
        );
        assert_eq!(
            folder.argument_identities(generic),
            [type_ids::AZ_UUID, type_uuid::azstd_shared_ptr(pointee)]
        );
        assert_eq!(folder.fold(generic), Some(recorded));
        assert_ne!(folder.fold_from_templated_type_ids(generic), Some(recorded));
    }

    /// A tuple's members carry the identities its arguments contribute the
    /// way a pair's do: `PlayerAttributeData`'s
    /// `Paperdoll Slot Unlocks By Tradeskill Rank` is
    /// `vector<tuple<PaperdollSlotTypes, AZStd::string, int>>`, whose tuple
    /// records `[int, basic_string, int]` while `Value1` carries the
    /// `EnumType` attribute and `Value2` its own `AZStd::string` generic info.
    #[test]
    fn folds_a_tuple_argument_from_its_members() {
        let model = shipped_model();
        let folder = SpecializedTypeIdFolder::new(&model);
        let tuple = uuid!("DE1CB64D-EBC4-583E-AF31-EB257B8AC677");
        let vector = uuid!("363ED6CD-26B0-5FB8-BF5C-A0320F380286");
        let paperdoll_slot_types = uuid!("5D42C439-A859-4133-9032-88DE31048F2C");

        let vector_generic = registered_generic(&model, vector);
        let tuple_generic = model
            .generic_class(tuple)
            .or_else(|| {
                vector_generic
                    .members
                    .first()
                    .and_then(|member| member.generic_class.as_deref())
            })
            .expect("the tuple should be registered or nested under the vector's element");

        assert_eq!(tuple_generic.class_name.as_deref(), Some("AZStd::tuple"));
        assert_eq!(
            tuple_generic.templated_type_ids,
            [type_ids::INT, type_ids::AZSTD_BASIC_STRING, type_ids::INT]
        );
        assert_eq!(
            folder.argument_identities(tuple_generic),
            [paperdoll_slot_types, type_ids::AZSTD_STRING, type_ids::INT]
        );
        assert_eq!(folder.fold(tuple_generic), Some(tuple));
        assert_ne!(
            folder.fold_from_templated_type_ids(tuple_generic),
            Some(tuple)
        );

        assert_eq!(folder.argument_identities(vector_generic), [tuple]);
        assert_eq!(folder.fold(vector_generic), Some(vector));
        assert_ne!(
            folder.fold_from_templated_type_ids(vector_generic),
            Some(vector)
        );
    }

    /// Every registered `unordered_map`, `unordered_set`, `map`, `vector`,
    /// `pair` and `unordered_flat_map` specialization folds to the identity
    /// the capture records, except the vectors declared with an explicit
    /// `AZ::AZStdAlloc<Allocator>`: the capture records only their element,
    /// so the allocator the fold needs is not in the resource.
    /// `nw_objectstream::type_uuid`'s `folds_vectors_with_explicit_allocators`
    /// reproduces each of those from its allocator.
    #[test]
    fn folds_every_recorded_container_specialization_whose_arguments_are_recorded() {
        let model = shipped_model();
        let folder = SpecializedTypeIdFolder::new(&model);
        let sweep = folder.sweep(&[
            "AZStd::unordered_map",
            "AZStd::unordered_set",
            "AZStd::map",
            "AZStd::vector",
            "AZStd::pair",
            "AZStd::unordered_flat_map",
        ]);

        for (template, count) in sweep.by_template() {
            eprintln!(
                "{template}: {} recorded, {} reproduced from templatedTypeIds, {} reproduced from the argument identities",
                count.recorded, count.reproduced_from_templated_type_ids, count.reproduced
            );
        }
        // ComponentApplication::Descriptor::modules over AZStdAlloc<OSAllocator>;
        // Composite::Children and ConditionGroup::Conditions over
        // AZStdAlloc<SystemAllocator>.
        let unrecorded_allocator_vectors = BTreeSet::from([
            uuid!("8E779F80-AEAA-565B-ABB1-DE10B18CF995"),
            uuid!("15B4F50E-8C6E-5262-8555-E181A9B6FFAC"),
            uuid!("A3BE97B0-BE01-51C4-9717-7CDD03C6C10E"),
        ]);
        let misses = sweep
            .misses()
            .map(|entry| {
                format!(
                    "{} {} templatedTypeIds {:?} arguments {:?} folded {:?}",
                    entry.class_name,
                    entry.recorded_type_id,
                    entry.templated_type_ids,
                    entry.argument_identities,
                    entry.folded_type_id
                )
            })
            .collect::<Vec<_>>();
        assert!(
            sweep.reproduced() > sweep.reproduced_from_templated_type_ids(),
            "the argument identities should fold more specializations than templatedTypeIds"
        );
        assert_eq!(
            sweep
                .misses()
                .map(|entry| entry.recorded_type_id)
                .collect::<BTreeSet<_>>(),
            unrecorded_allocator_vectors,
            "{} of {} recorded specializations do not fold from their argument identities:\n{}",
            misses.len(),
            sweep.entries.len(),
            misses.join("\n")
        );
        assert!(
            sweep
                .misses()
                .all(|entry| entry.class_name == "AZStd::vector"
                    && entry.templated_type_ids.len() == 1),
            "every miss should be a vector whose capture records only the element"
        );
    }

    #[test]
    fn folds_supported_templates_from_fixture_members() {
        let enum_id = uuid!("11111111-1111-4111-8111-111111111111");
        let pair_id = type_uuid::azstd_pair(enum_id, type_ids::INT);
        let map_id = type_uuid::azstd_unordered_map(enum_id, type_ids::INT);
        let model = SerializeContextModel::from_root(&json!({
            "uuidMap": {},
            "uuidGenericMap": [[
                map_id.hyphenated().to_string(),
                {
                    "$id": 1,
                    "typeId": map_id.hyphenated().to_string(),
                    "registeredTypeIds": [map_id.hyphenated().to_string()],
                    "templatedArgumentCount": 2,
                    "templatedTypeIds": [
                        type_ids::INT.hyphenated().to_string(),
                        type_ids::INT.hyphenated().to_string()
                    ],
                    "typeIdFoldTypeIds": null,
                    "specializedTypeId": map_id.hyphenated().to_string(),
                    "genericTypeId": "18456A80-63CC-40C5-BF16-6AF94F9A9ECC",
                    "legacySpecializedTypeId": null,
                    "nonTypeTemplateArguments": null,
                    "classData": {
                        "$id": 2,
                        "name": "AZStd::unordered_map",
                        "typeId": map_id.hyphenated().to_string(),
                        "elements": [{
                            "$id": 3,
                            "name": "element",
                            "nameCrc": 1,
                            "typeId": pair_id.hyphenated().to_string(),
                            "genericClassInfo": {
                                "$id": 4,
                                "typeId": pair_id.hyphenated().to_string(),
                                "registeredTypeIds": [pair_id.hyphenated().to_string()],
                                "templatedArgumentCount": 2,
                                "templatedTypeIds": [
                                    type_ids::INT.hyphenated().to_string(),
                                    type_ids::INT.hyphenated().to_string()
                                ],
                                "typeIdFoldTypeIds": null,
                                "specializedTypeId": pair_id.hyphenated().to_string(),
                                "genericTypeId": null,
                                "legacySpecializedTypeId": null,
                                "nonTypeTemplateArguments": null,
                                "classData": {
                                    "$id": 5,
                                    "name": "AZStd::pair",
                                    "typeId": pair_id.hyphenated().to_string(),
                                    "elements": [
                                        {
                                            "$id": 6,
                                            "name": "value1",
                                            "nameCrc": 2,
                                            "typeId": type_ids::INT.hyphenated().to_string(),
                                            "attributes": [[
                                                2977423797_u32,
                                                {
                                                    "$id": 7,
                                                    "attributeId": 2977423797_u32,
                                                    "attributeName": "EnumType",
                                                    "value": {
                                                        "kind": "Uuid",
                                                        "value": enum_id.hyphenated().to_string()
                                                    }
                                                }
                                            ]]
                                        },
                                        {
                                            "$id": 8,
                                            "name": "value2",
                                            "nameCrc": 3,
                                            "typeId": type_ids::INT.hyphenated().to_string()
                                        }
                                    ]
                                }
                            }
                        }]
                    }
                }
            ]],
            "enumTypeIdToUnderlyingTypeIdMap": {}
        }));
        let folder = SpecializedTypeIdFolder::new(&model);

        let map = registered_generic(&model, map_id);
        assert_eq!(
            folder.argument_identities(map),
            [enum_id, type_ids::INT],
            "the pair's value1 EnumType attribute names the key"
        );
        assert_eq!(folder.fold(map), Some(map_id));
        assert_eq!(
            folder.fold_from_templated_type_ids(map),
            Some(type_uuid::azstd_unordered_map(type_ids::INT, type_ids::INT))
        );
        let pair = registered_generic(&model, pair_id);
        assert_eq!(folder.fold(pair), Some(pair_id));

        let sweep = folder.sweep(FOLDED_TEMPLATE_NAMES);
        assert_eq!(sweep.entries.len(), 2);
        assert_eq!(sweep.reproduced(), 2);
        assert_eq!(sweep.reproduced_from_templated_type_ids(), 0);
    }
}
