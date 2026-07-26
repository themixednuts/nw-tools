use uuid::Uuid;

use crate::rust::enum_plan::{RustEnumRawConversionPlan, RustVariantPlan};
use crate::rust::identity::RustTypeIdentityPlan;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RustCodegenUnit {
    pub items: Vec<RustItemPlan>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RustItemPlan {
    pub source_type_id: Uuid,
    pub source_name: String,
    pub is_reflected_base: bool,
    pub is_slot_owner: bool,
    pub has_layout_family_descendants: bool,
    pub is_bevy_component: bool,
    pub file_stem_override: Option<String>,
    pub scope_path: Vec<String>,
    pub family_scope_path: Vec<String>,
    pub rust_name: String,
    pub kind: RustItemKind,
    pub identity: RustTypeIdentityPlan,
    pub repr: Option<String>,
    pub raw_conversion: Option<RustEnumRawConversionPlan>,
    /// Canonical Prefab source identity for authorable Bevy components.
    ///
    /// This stays opt-in at the plan boundary so protocol-only and standalone
    /// codegen do not acquire an `az-prefab` dependency.
    pub prefab: Option<RustPrefabPlan>,
    pub derives: Vec<String>,
    pub rtti_bases: Vec<RustRttiBasePlan>,
    pub fields: Vec<RustFieldPlan>,
    pub variants: Vec<RustVariantPlan>,
    /// This type appears in a hashing collection position — a `HashSet`
    /// (`unordered_set`) element or `HashMap` (`unordered_map` /
    /// `unordered_flat_map`) key — so it must reflect **opaque** with
    /// `#[reflect(Hash, PartialEq)]`. Structural reflection of such a type
    /// yields a `DynamicStruct`/`DynamicTupleStruct` whose `reflect_hash()` is
    /// `None`, which panics `bevy_reflect`'s `DynamicSet`/`DynamicMap` hashing.
    /// Set during planning by position; consumed by the source emitter.
    pub reflect_opaque_leaf: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RustPrefabPlan {
    pub tag: String,
    pub source_version: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RustItemKind {
    Struct,
    Enum,
    SumEnum,
    RawEnum,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RustRttiBasePlan {
    pub source_type_id: Uuid,
    pub source_name: String,
    pub rust_type: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RustFieldPlan {
    pub source_name: String,
    pub rust_name: String,
    pub source_type_id: Uuid,
    pub rust_type: String,
    pub reflect_ignore: bool,
    pub unresolved_type: Option<RustUnresolvedTypePlan>,
    pub integer_range: Option<RustIntegerRangePlan>,
    pub data_size: Option<u32>,
    pub offset: Option<u32>,
    pub flags: Option<u32>,
    pub is_base_class: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RustIntegerRangePlan {
    pub rust_type: String,
    pub value_type: String,
    pub start: String,
    pub last: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RustUnresolvedTypePlan {
    pub type_id: Uuid,
    pub reason: String,
}
