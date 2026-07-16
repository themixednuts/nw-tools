//! SerializeContext-owned audio component and authored-event types.
//!
//! ATL XML and Wwise binary records are runtime asset formats; these generated
//! types cover the serialized Lumberyard/New World components that reference
//! those controls.

pub use nw_reflected_types::types::activity::SetAudioSwitchState as ActivitySetAudioSwitchState;
pub use nw_reflected_types::types::sequence_event::SetAudioSwitchState as SequenceSetAudioSwitchState;
pub use nw_reflected_types::types::{
    AIAudioTriggerActivity, AudioAreaEnvironmentComponent, AudioEnvironmentComponent,
    AudioListenerComponent, AudioOverrideComponent, AudioPreload, AudioPreloadComponent,
    AudioProxyData, AudioRtpcComponent, AudioSetTriggerOverrideComponent,
    AudioSetTriggerOverrideComponentClientFacet, AudioSetTriggerOverrideComponentServerFacet,
    AudioShapeComponent, AudioSplineComponent, AudioSwitchComponent, AudioTriggerB73C9B69,
    AudioTriggerCC4062C6, AudioTriggerComponent, EAudioObjectObstructionCalcType,
    ExecuteAudioTrigger, ItemAudioTrigger, TriggerOverridePair,
};
