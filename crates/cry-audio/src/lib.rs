//! Engine-owned legacy audio formats used by Lumberyard and New World.
//!
//! This crate is intentionally runtime-free. It parses ATL control XML,
//! Wwise `.bnk` section graphs, `.wem` RIFF containers, and the ATL
//! trigger-bank map without depending on a renderer, audio backend, or ECS.

mod atl;
mod fxlib;
mod mapping;
pub mod reflected;
mod wwise;

pub use atl::{
    AudioBackendReference, AudioBackendReferenceKind, AudioControlAttribute, AudioControlsError,
    AudioControlsSource, AudioEnvironment, AudioPreload, AudioPreloadConfigGroup, AudioPreloadFile,
    AudioPreloadPlatform, AudioRtpc, AudioSwitch, AudioSwitchState, AudioTrigger,
    AudioTriggerPlaybackInfo,
};
pub use fxlib::{
    MATERIAL_EFFECTS_FXLIB_DIR, MaterialEffect, MaterialEffectAudio, MaterialEffectSwitch,
    MaterialEffectsError, MaterialEffectsLibrary, footstep_fxlib_path,
};
pub use mapping::{
    AudioEventIdEntry, AudioEventIds, AudioMappingDocument, AudioMappingKind,
    AudioMappingParseError, AudioTagEntry, AudioTagTable, AudioTractSoundBankEntry,
    AudioTractSoundBankMap, audio_mapping_kind, is_audio_mapping_source, parse_audio_mapping,
};
pub use wwise::hirc::{WwiseAction, WwiseSwitchBranch};
pub use wwise::ranseq::{
    WwiseRandomContainer, WwiseRandomMode, WwiseRandomSequenceMode, weighted_sequence_seed,
};
pub use wwise::{
    AudioControlId, WWISE_TRIGGER_BANK_MAP_FILE, WwiseBankHeader, WwiseBankId, WwiseBankSection,
    WwiseHierarchyObject, WwiseHierarchyObjectKind, WwiseMediaChunk, WwiseMediaChunkId,
    WwiseMediaEntry, WwiseMediaId, WwiseMediaInfo, WwiseMediaParseError, WwiseNameId,
    WwiseObjectId, WwiseSectionId, WwiseSoundBank, WwiseSoundBankParseError, WwiseTriggerBankMap,
    WwiseTriggerBankMapEntry, WwiseTriggerBankMapError, WwiseWaveFormat, az_crc32,
};

/// Catalog-relative path for a decoded Wwise media payload (PCM WAV).
///
/// Derived sibling of the bank tree: media has no stable catalog identity of its
/// own once extracted, so we key by media id under `sounds/wwise/decoded/`.
#[must_use]
pub fn decoded_wave_catalog_path(media_id: WwiseMediaId) -> String {
    format!("sounds/wwise/decoded/{}.wav", media_id.0)
}
