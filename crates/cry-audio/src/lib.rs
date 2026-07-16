//! Engine-owned legacy audio formats used by Lumberyard and New World.
//!
//! This crate is intentionally runtime-free. It parses ATL control XML,
//! Wwise `.bnk` section graphs, `.wem` RIFF containers, and the ATL
//! trigger-bank map without depending on a renderer, audio backend, or ECS.

mod atl;
pub mod reflected;
mod wwise;

pub use atl::{
    AudioBackendReference, AudioBackendReferenceKind, AudioControlAttribute, AudioControlsError,
    AudioControlsSource, AudioEnvironment, AudioPreload, AudioPreloadConfigGroup, AudioPreloadFile,
    AudioPreloadPlatform, AudioRtpc, AudioSwitch, AudioSwitchState, AudioTrigger,
    AudioTriggerPlaybackInfo,
};
pub use wwise::{
    AudioControlId, WWISE_TRIGGER_BANK_MAP_FILE, WwiseBankHeader, WwiseBankId, WwiseBankSection,
    WwiseHierarchyObject, WwiseHierarchyObjectKind, WwiseMediaChunk, WwiseMediaChunkId,
    WwiseMediaEntry, WwiseMediaId, WwiseMediaInfo, WwiseMediaParseError, WwiseNameId,
    WwiseObjectId, WwiseSectionId, WwiseSoundBank, WwiseSoundBankParseError, WwiseTriggerBankMap,
    WwiseTriggerBankMapEntry, WwiseTriggerBankMapError, WwiseWaveFormat,
};
