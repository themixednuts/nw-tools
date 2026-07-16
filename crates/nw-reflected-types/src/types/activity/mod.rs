use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

pub mod execution_activity;

pub use self::execution_activity::{
    AIAddRotationToPose, AIAssignOrder, AIAudioTriggerActivity, AITargetRelativeFacing,
    ActivateBlock, ActivateCamera, ActivateGrit, ActivatePassiveAbility, AddLoadedAmmo,
    AimDirectionMode, AnimationDrivenMotion, ApplyEquipLoadMoveSpeedMultiplier,
    ApplyMoveSpeedMultiplier, AttachItemMesh, AttemptInteraction, BehaviorTreeTaskActivity,
    BlockEmotes, BlockEnteringStore, Build, CameraRelativeFacing, CameraRelativeMotion,
    CancelLookingThroughLoadout, CancelPlacingStructure, CastSpell, CastSpellRaycast,
    CastSpellTargetArc, CastSpellTargeting, ChangeActionFragment, ChangeStatMultiplier,
    ChannelSpell, CharacterRelativeMovement, ClearAbilityHits, ClearActiveAbilityMoveInput,
    ClearBlackboardFact, ClearDamageDealt, ClearDamaged, ClearDynamicTags, ClearInput,
    ClearInteractionResult, ClearMannequinTagGroup, ClearMotion, ClearStimuli, CombatStatus,
    CommittedInteraction, CompleteMountSummon, ConsumeAmmo, ConsumeCharge, ConsumeLoadedAmmo,
    Craft, Death, DestroySelf, DisableCameraControls, DisableCameraHits, DisableCameraLock,
    DisableCameraLockTargetChange, DisableCollision, DisableGravity, DisableInteractions,
    DisableInteractor, DisableLoadoutSwapping, DisableMesh, DisableStaminaRegen, DisableTimewarp,
    DisableUICanvas, DisableUIVisibility, DropContentsOfInventory, EnablePostEffectGroup,
    ExecuteAudioTrigger, ExecuteEquipItem, ExecuteInteraction, ExecuteSwapWeapon,
    ExecutionActivity, ExecutionActivityGroup, FakeFlyer, FastTravelChanneling, FlipBackStabAngle,
    ForceCameraLock, Gather, GatherMomentOfImpact, GiveCharge, GiveUpGhost, HTNPlanDoneActivity,
    HandleMusicalPerformance, HidePlayerNameTag, HomingOverrideTargetClear, IfThenElse,
    IgnoreAITarget, InputActionMapToggle, InputFilterToggle, InputRelativeFacing,
    InteractionAlignment, ItemAudioTrigger, JavelinSampleActivity, LookThroughLoadout,
    ManageThrowableItem, MaterialOverride, ModifyGravity, ModifyStaminaRegen, ModifyStatusEffects,
    MotionRelativeFacing, Mount, MountSetDashing, MountSetWalking, PayManaCost,
    PayMountStaminaCost, PayStaminaCost, PerformActiveAbility, PlaceStructure,
    PlayerEmoteController, ProcessPredictedEffects, ReactionMotion, ReadLoreItem, Reload,
    RemoveExcessAbilityInstances, ResetCamera, ResetGrit, ResetReactionCount, ResetStamina,
    ResizeCharacterController, Respawn, ReviveFromDeathsDoor, RotateStructure, RunFxScript,
    SendMessageToOwnedSlices, SendUIEvent, SetActionStateFlag, SetActiveWeapon,
    SetAnimationSpeedBias, SetAttachmentVisibility, SetAudioSwitchState, SetBlockBroken,
    SetCameraLockTargetTargetable, SetCooldownTimer, SetDesiredFacingBlendParam, SetFishingAction,
    SetGatherFragment, SetHaltMotionOnCollision, SetHomingTargetTargetable, SetInUsePaperdollItem,
    SetIncapacitatedState, SetInteractionFragment, SetIsValidHomingOverrrideTarget, SetLoadedAmmo,
    SetMannequinTag, SetMannequinTagFromSource, SetReactionFragment, SetReticle,
    SetStaminaRegenDelay, SetTargetFriendliesFallbackForReEnteringCameraLock, SetTimelineFragment,
    SetToggleInput, SetWeaponAccuracyBonus, SheatheWeapon, ShowUINotification, SlopeRelativeMotion,
    SocialAlignment, SpawnParticleEffect, SpectatorModeActivity, StopFxScript, SummonDismissMount,
    SuppressVerticalCameraMovement, ToggleLimbIK, ToggleTerrainAlignment, TrackTimeInAction,
    TraversalFixUp, TriggerEntity, TriggerRemoteEntity, TurretAimActivity,
    UpdateRemoteMountedState, UsePaperdollItem, WeaponEffects,
};

#[derive(
    Debug,
    Default,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    serde::Serialize,
    serde::Deserialize,
    bevy_reflect::Reflect,
)]
#[reflect(Serialize, Deserialize)]
pub struct Activity;

impl AzRtti for Activity {
    const NAME: &'static str = "Activity";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0xA6BD80A7_D0D3_445D_BA68_F4EC586B224A);
}
