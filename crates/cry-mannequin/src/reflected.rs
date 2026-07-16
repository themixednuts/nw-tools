//! SerializeContext-owned Mannequin and New World action-grid types.
//!
//! The source parsers in this crate own CryAction XML/authoring semantics;
//! these generated records own serialized component and gameplay metadata.

pub use nw_reflected_types::types::{
    ActionConditionIfMannequinMarker, ActionConditionIfMannequinTag,
    ActionConditionIfMannequinTagInItemSlot, CharacterActionGrid, CharacterActionGridAliasRef,
    CharacterActionGridCell, CharacterActionGridCellScopeBehavior, CharacterActionGridCellValue,
    CharacterActionGridListCache, CharacterActionList, ClearMannequinTagGroup, MannequinComponent,
    MannequinScopeComponent, MannequinTagGroupOptions, MannequinTagOptions, SetMannequinTag,
    SetMannequinTagData, SetMannequinTagFromSource,
    SimpleAssetReferenceMannequinAnimationDatabaseAsset,
    SimpleAssetReferenceMannequinControllerDefinitionAsset,
};
