//! SerializeContext-owned animation configuration types.
//!
//! These are the generated reflected component and sequence-event records.
//! Native `.animevents` parsing remains separate because the captured
//! `AnimationEvent` string members are pointer-shaped in SerializeContext.

pub use nw_reflected_types::types::{
    AnimatedLayer, AnimationDrivenMotion, AnimationEvent, CharacterAnimationManagerComponent,
    CharacterEvent, MaterialEffect, MotionParameterSmoothingSettings, SetAnimationByCondition,
    SetAnimationSpeedBias, SimpleAnimationComponent,
};
