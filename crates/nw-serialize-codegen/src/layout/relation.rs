use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayoutBaseEdge {
    pub type_id: Uuid,
    pub source_name: String,
    pub matches_reflected_type: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayoutSlotAnchor {
    pub owner_type_id: Uuid,
    pub owner_source_name: String,
    pub owner_field_name: String,
    pub slot_type_id: Uuid,
    pub slot_source_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayoutSlotOwnerEdge {
    pub owner_type_id: Uuid,
    pub owner_source_name: String,
    pub field_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayoutFieldOwnerEdge {
    pub owner_type_id: Uuid,
    pub owner_source_name: String,
    pub field_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayoutConcreteSlotBinding {
    pub owner_type_id: Uuid,
    pub owner_source_name: String,
    pub slot_owner_type_id: Uuid,
    pub slot_owner_source_name: String,
    pub owner_field_name: String,
    pub slot_type_id: Uuid,
    pub slot_source_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayoutConcreteSlotCandidate {
    pub owner_type_id: Uuid,
    pub owner_source_name: String,
    pub slot_owner_type_id: Uuid,
    pub slot_owner_source_name: String,
    pub owner_field_name: String,
    pub slot_type_id: Uuid,
    pub slot_source_name: String,
    pub match_kind: LayoutConcreteSlotMatchKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LayoutConcreteSlotMatchKind {
    ExactOwnerName,
    OwnerTrailingSemanticWord,
}
