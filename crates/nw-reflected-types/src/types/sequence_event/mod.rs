use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

pub mod activate_grit;
pub mod attack_height_retargeting;
pub mod audio_preload;
pub mod audio_trigger;
pub mod audio_trigger_cc4062c6;
pub mod cage_attachment;
pub mod cage_cast_spell;
pub mod cage_damage;
pub mod cage_pay_mana_cost;
pub mod cage_ranged_attack;
pub mod cast_spell_target_arc;
pub mod cast_spell_targeting;
pub mod character_event;
pub mod consume_loaded_ammo;
pub mod crit_window;
pub mod disable_collision;
pub mod disable_stamina_regen;
pub mod e_audio_object_obstruction_calc_type;
pub mod footstep;
pub mod hand_to_weapon_ik;
pub mod hide_attachment;
pub mod hit_stun;
pub mod homing;
pub mod invulnerability;
pub mod material_effect;
pub mod material_override;
pub mod material_override_2b488ec0;
pub mod particle_effect;
pub mod pay_stamina_cost;
pub mod post_effect_group;
pub mod run_fx_script;
pub mod sequence_event_options;
pub mod sequence_marker;
pub mod set_animation_by_condition;
pub mod set_audio_switch_state;
pub mod set_sequence_by_condition;
pub mod shake_camera;
pub mod slayer_script_literal;
pub mod slow_down_prediction;
pub mod spawn_particle_effect;
pub mod spawn_slice;
pub mod test_sequence_event;
pub mod toggle_limb_ik;
pub mod track_time_in_sequence;
pub mod transition;
pub mod use_paperdoll_item;
pub mod weapon_effects;

pub use self::activate_grit::ActivateGrit;
pub use self::attack_height_retargeting::AttackHeightRetargeting;
pub use self::audio_preload::AudioPreload;
pub use self::audio_trigger::AudioTriggerB73C9B69;
pub use self::audio_trigger_cc4062c6::AudioTriggerCC4062C6;
pub use self::cage_attachment::CAGEAttachment;
pub use self::cage_cast_spell::CAGECastSpell;
pub use self::cage_damage::{CAGEDamage, MeleeAttackShapeCastType};
pub use self::cage_pay_mana_cost::CAGEPayManaCost;
pub use self::cage_ranged_attack::CAGERangedAttack;
pub use self::cast_spell_target_arc::CastSpellTargetArc;
pub use self::cast_spell_targeting::CastSpellTargeting;
pub use self::character_event::CharacterEvent;
pub use self::consume_loaded_ammo::ConsumeLoadedAmmo;
pub use self::crit_window::CritWindow;
pub use self::disable_collision::DisableCollision;
pub use self::disable_stamina_regen::DisableStaminaRegen;
pub use self::e_audio_object_obstruction_calc_type::EAudioObjectObstructionCalcType;
pub use self::footstep::Footstep;
pub use self::hand_to_weapon_ik::HandToWeaponIK;
pub use self::hide_attachment::HideAttachment;
pub use self::hit_stun::HitStun;
pub use self::homing::Homing;
pub use self::invulnerability::Invulnerability;
pub use self::material_effect::MaterialEffect;
pub use self::material_override::MaterialOverride199A67B1;
pub use self::material_override_2b488ec0::MaterialOverride2B488EC0;
pub use self::particle_effect::{ParticleEffect, SlayerScriptEditLiteral};
pub use self::pay_stamina_cost::PayStaminaCost;
pub use self::post_effect_group::PostEffectGroup;
pub use self::run_fx_script::RunFxScript;
pub use self::sequence_event_options::SequenceEventOptions;
pub use self::sequence_marker::SequenceMarker;
pub use self::set_animation_by_condition::SetAnimationByCondition;
pub use self::set_audio_switch_state::SetAudioSwitchState;
pub use self::set_sequence_by_condition::SetSequenceByCondition;
pub use self::shake_camera::ShakeCamera;
pub use self::slayer_script_literal::SlayerScriptLiteral;
pub use self::slow_down_prediction::SlowDownPrediction;
pub use self::spawn_particle_effect::SpawnParticleEffect;
pub use self::spawn_slice::SpawnSlice;
pub use self::test_sequence_event::TestSequenceEvent;
pub use self::toggle_limb_ik::ToggleLimbIK;
pub use self::track_time_in_sequence::TrackTimeInSequence;
pub use self::transition::Transition;
pub use self::use_paperdoll_item::UsePaperdollItem;
pub use self::weapon_effects::WeaponEffects;

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
pub struct SequenceEvent {}

impl AzRtti for SequenceEvent {
    const NAME: &'static str = "SequenceEvent";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x9B454E3B_282D_4089_90BE_DF25317205E7);
}
