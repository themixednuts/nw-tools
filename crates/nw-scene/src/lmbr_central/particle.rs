use bevy_color::LinearRgba;
use nw_objectstream::{Element, asset_id, value};
use nw_reflected_types::az::asset::AssetId as ReflectedAssetId;
use nw_reflected_types::az::rtti::AzRtti;
use nw_reflected_types::az::uuid::Uuid as ReflectedUuid;
use nw_reflected_types::types::components::particle_component::{
    ParticleComponent, ParticleEmitBoneLayer, ParticleEmitterSettings,
};

use super::read::{
    LmbrCentralObjectStreamError, checked_version, child, ensure_type, read_component_base,
    read_exact_string, read_optional, read_required, required_legacy_child,
};

const COMPONENT_VERSION: u8 = 3;
const SETTINGS_VERSION: u8 = 7;
const BONE_LAYER_VERSION: u8 = 1;
const INVALID_ENTITY_ID: u64 = u32::MAX as u64;

#[derive(Debug, Clone, PartialEq)]
pub struct ParticleComponentSource {
    pub component: ParticleComponent,
    pub component_version: Option<u8>,
    pub settings_version: Option<u8>,
}

pub fn read_particle_component(
    element: &Element,
) -> Result<ParticleComponentSource, LmbrCentralObjectStreamError> {
    ensure_type(
        element,
        *ParticleComponent::TYPE_ID.as_inner(),
        ParticleComponent::NAME,
    )?;
    checked_version(element, ParticleComponent::NAME, COMPONENT_VERSION)?;

    let settings_element = child(element, "Particle").or_else(|| {
        element
            .children()
            .iter()
            .find(|child| child.id() == ParticleEmitterSettings::TYPE_ID.as_inner())
    });
    let (particle, settings_version) = settings_element
        .map(read_particle_settings)
        .transpose()?
        .unwrap_or_else(|| (native_particle_settings(), None));

    let particle_library_asset_id = child(element, "ParticleLibraryAssetId")
        .map(asset_id::read_non_nil_asset_id)
        .transpose()
        .map_err(|source| LmbrCentralObjectStreamError::Field {
            field: "ParticleLibraryAssetId",
            source,
        })?
        .flatten()
        .map(|asset_id| {
            ReflectedAssetId::new(
                ReflectedUuid::from_bytes(*asset_id.guid.as_bytes()),
                asset_id.sub_id,
            )
        })
        .unwrap_or_else(ReflectedAssetId::nil);

    let mesh_particle = child(element, "MeshParticle")
        .map(read_mesh_particle)
        .transpose()?
        .unwrap_or_default();
    let load_emitter_on_activate =
        read_optional(element, "Load Emitter On Activate", value::read_bool)?.unwrap_or(true);

    Ok(ParticleComponentSource {
        component: ParticleComponent {
            az_component: read_component_base(element)?,
            particle,
            particle_library_asset_id,
            mesh_particle,
            load_emitter_on_activate,
        },
        component_version: element.version(),
        settings_version,
    })
}

fn read_particle_settings(
    element: &Element,
) -> Result<(ParticleEmitterSettings, Option<u8>), LmbrCentralObjectStreamError> {
    ensure_type(
        element,
        *ParticleEmitterSettings::TYPE_ID.as_inner(),
        ParticleEmitterSettings::NAME,
    )?;
    let version = checked_version(element, ParticleEmitterSettings::NAME, SETTINGS_VERSION)?;
    if version == 0 {
        return Err(LmbrCentralObjectStreamError::UnsupportedVersion {
            type_name: ParticleEmitterSettings::NAME,
            version,
            newest: SETTINGS_VERSION,
        });
    }

    let mut settings = native_particle_settings();
    read_common_settings(element, &mut settings)?;
    read_versioned_settings(element, version, &mut settings)?;
    Ok((settings, element.version()))
}

fn read_common_settings(
    element: &Element,
    settings: &mut ParticleEmitterSettings,
) -> Result<(), LmbrCentralObjectStreamError> {
    assign_optional(element, "Visible", value::read_bool, &mut settings.visible)?;
    assign_optional(element, "Enable", value::read_bool, &mut settings.enable)?;
    assign_optional(
        element,
        "AttachToMesh",
        value::read_bool,
        &mut settings.attach_to_mesh,
    )?;
    assign_optional(
        element,
        "AttachToDissolvingEdge",
        value::read_bool,
        &mut settings.attach_to_dissolving_edge,
    )?;
    if let Some(value) = read_exact_string(element, "SelectedEmitter")? {
        settings.selected_emitter = value;
    }
    assign_optional(
        element,
        "Alpha Scale",
        value::read_f32,
        &mut settings.alpha_scale,
    )?;
    assign_optional(
        element,
        "Particle Count Scale",
        value::read_f32,
        &mut settings.particle_count_scale,
    )?;
    assign_optional(
        element,
        "Time Scale",
        value::read_f32,
        &mut settings.time_scale,
    )?;
    assign_optional(
        element,
        "Pulse Period",
        value::read_f32,
        &mut settings.pulse_period,
    )?;
    assign_optional(
        element,
        "ParticleSizeZ",
        value::read_f32,
        &mut settings.particle_size_z,
    )?;
    assign_optional(element, "Strength", value::read_f32, &mut settings.strength)?;
    assign_optional(
        element,
        "Ignore Rotation",
        value::read_bool,
        &mut settings.ignore_rotation,
    )?;
    assign_optional(
        element,
        "Not Attached",
        value::read_bool,
        &mut settings.not_attached,
    )?;
    assign_optional(
        element,
        "Register by Bounding Box",
        value::read_bool,
        &mut settings.register_by_bounding_box,
    )?;
    assign_optional(element, "Use LOD", value::read_bool, &mut settings.use_lod)?;
    assign_optional(
        element,
        "Target Entity",
        value::read_entity_id,
        &mut settings.target_entity,
    )?;
    assign_optional(
        element,
        "GPU Edge Dissolve Target Entity",
        value::read_entity_id,
        &mut settings.gpu_edge_dissolve_target_entity,
    )?;
    assign_optional(
        element,
        "Enable Audio",
        value::read_bool,
        &mut settings.enable_audio,
    )?;
    if let Some(value) = read_exact_string(element, "Audio RTPC")? {
        settings.audio_rtpc = value;
    }
    assign_optional(
        element,
        "View Distance Multiplier",
        value::read_f32,
        &mut settings.view_distance_multiplier,
    )?;
    assign_optional(
        element,
        "Use VisArea",
        value::read_bool,
        &mut settings.use_vis_area,
    )?;
    assign_optional(
        element,
        "Accept Decals",
        value::read_bool,
        &mut settings.accept_decals,
    )?;
    assign_optional(
        element,
        "Accept Snow",
        value::read_bool,
        &mut settings.accept_snow,
    )?;
    assign_optional(
        element,
        "Accept Silhouette",
        value::read_bool,
        &mut settings.accept_silhouette,
    )?;
    assign_optional(
        element,
        "Render Always",
        value::read_bool,
        &mut settings.render_always,
    )?;
    assign_optional(
        element,
        "Kill On Deactivate",
        value::read_bool,
        &mut settings.kill_on_deactivate,
    )?;
    assign_optional(
        element,
        "Force Highest Contextual Priority",
        value::read_bool,
        &mut settings.force_highest_contextual_priority,
    )?;
    Ok(())
}

fn read_versioned_settings(
    element: &Element,
    version: u8,
    settings: &mut ParticleEmitterSettings,
) -> Result<(), LmbrCentralObjectStreamError> {
    if version == 1 {
        required_legacy_child(
            element,
            ParticleEmitterSettings::NAME,
            version,
            "Emitter Object Type",
        )?;
        settings.speed_scale = read_required(
            element,
            ParticleEmitterSettings::NAME,
            version,
            "Emission Speed",
            value::read_f32,
        )?;
    } else {
        assign_optional(
            element,
            "Speed Scale",
            value::read_f32,
            &mut settings.speed_scale,
        )?;
    }

    if version <= 2 {
        settings.pre_roll = read_required(
            element,
            ParticleEmitterSettings::NAME,
            version,
            "Prime",
            value::read_bool,
        )?;
        settings.global_size_scale = read_required(
            element,
            ParticleEmitterSettings::NAME,
            version,
            "Particle Size Scale",
            value::read_f32,
        )?;
        settings.particle_size_x = read_required(
            element,
            ParticleEmitterSettings::NAME,
            version,
            "Size X",
            value::read_f32,
        )?;
        settings.particle_size_y = read_required(
            element,
            ParticleEmitterSettings::NAME,
            version,
            "Size Y",
            value::read_f32,
        )?;
        settings.particle_size_random = read_required(
            element,
            ParticleEmitterSettings::NAME,
            version,
            "Size Random X",
            value::read_f32,
        )?;
        require_removed_v2_fields(element, version)?;
    } else {
        assign_optional(
            element,
            "Pre-roll",
            value::read_bool,
            &mut settings.pre_roll,
        )?;
        assign_optional(
            element,
            "GlobalSizeScale",
            value::read_f32,
            &mut settings.global_size_scale,
        )?;
        assign_optional(
            element,
            "ParticleSizeX",
            value::read_f32,
            &mut settings.particle_size_x,
        )?;
        assign_optional(
            element,
            "ParticleSizeY",
            value::read_f32,
            &mut settings.particle_size_y,
        )?;
        assign_optional(
            element,
            "ParticleSizeRandom",
            value::read_f32,
            &mut settings.particle_size_random,
        )?;
    }

    settings.color = if version <= 4 {
        let color = read_required(
            element,
            ParticleEmitterSettings::NAME,
            version,
            "Color",
            value::read_vec3,
        )?;
        LinearRgba::new(color[0], color[1], color[2], 1.0)
    } else {
        read_optional(element, "Color", value::read_color)?
            .map(|color| LinearRgba::new(color[0], color[1], color[2], color[3]))
            .unwrap_or(settings.color)
    };
    Ok(())
}

fn require_removed_v2_fields(
    element: &Element,
    version: u8,
) -> Result<(), LmbrCentralObjectStreamError> {
    let attach_type = if version == 1 {
        "Emitter Object Type"
    } else {
        "Attach Type"
    };
    for field in [
        attach_type,
        "Emitter Shape",
        "Geometry",
        "Count Per Unit",
        "Position Offset",
        "Random Offset",
        "Size Random Y",
        "Init Angles",
        "Rotation Rate X",
        "Rotation Rate Y",
        "Rotation Rate Z",
        "Rotation Rate Random X",
        "Rotation Rate Random Y",
        "Rotation Rate Random Z",
        "Rotation Random Angles",
    ] {
        required_legacy_child(element, ParticleEmitterSettings::NAME, version, field)?;
    }
    Ok(())
}

fn read_mesh_particle(
    element: &Element,
) -> Result<Vec<ParticleEmitBoneLayer>, LmbrCentralObjectStreamError> {
    if element.id() == ParticleEmitBoneLayer::TYPE_ID.as_inner() {
        return read_bone_layer(element).map(|layer| vec![layer]);
    }
    element
        .children()
        .iter()
        .filter(|child| child.id() == ParticleEmitBoneLayer::TYPE_ID.as_inner())
        .map(read_bone_layer)
        .collect()
}

fn read_bone_layer(
    element: &Element,
) -> Result<ParticleEmitBoneLayer, LmbrCentralObjectStreamError> {
    ensure_type(
        element,
        *ParticleEmitBoneLayer::TYPE_ID.as_inner(),
        ParticleEmitBoneLayer::NAME,
    )?;
    checked_version(element, ParticleEmitBoneLayer::NAME, BONE_LAYER_VERSION)?;
    let mut layer = ParticleEmitBoneLayer::default();
    if let Some(value) = read_exact_string(element, "Joint name")? {
        layer.joint_name = value;
    }
    assign_optional(
        element,
        "Enable Layer",
        value::read_bool,
        &mut layer.enable_layer,
    )?;
    if let Some(indices) = child(element, "AffectedIndices") {
        layer.affected_indices = indices
            .children()
            .iter()
            .map(value::read_u32)
            .collect::<Result<_, _>>()
            .map_err(|source| LmbrCentralObjectStreamError::Field {
                field: "AffectedIndices",
                source,
            })?;
    }
    Ok(layer)
}

fn assign_optional<T>(
    element: &Element,
    field: &'static str,
    read: impl FnOnce(&Element) -> Result<T, value::ObjectStreamValueError>,
    destination: &mut T,
) -> Result<(), LmbrCentralObjectStreamError> {
    if let Some(value) = read_optional(element, field, read)? {
        *destination = value;
    }
    Ok(())
}

#[must_use]
pub fn native_particle_settings() -> ParticleEmitterSettings {
    ParticleEmitterSettings {
        visible: true,
        enable: true,
        attach_to_mesh: false,
        attach_to_dissolving_edge: false,
        selected_emitter: String::new(),
        color: LinearRgba::WHITE,
        alpha_scale: 1.0,
        pre_roll: false,
        particle_count_scale: 1.0,
        time_scale: 1.0,
        pulse_period: 0.0,
        global_size_scale: 1.0,
        particle_size_x: 1.0,
        particle_size_y: 1.0,
        particle_size_z: 1.0,
        particle_size_random: 0.0,
        speed_scale: 1.0,
        strength: -1.0,
        ignore_rotation: false,
        not_attached: false,
        register_by_bounding_box: false,
        use_lod: true,
        target_entity: INVALID_ENTITY_ID,
        gpu_edge_dissolve_target_entity: INVALID_ENTITY_ID,
        enable_audio: false,
        audio_rtpc: String::new(),
        view_distance_multiplier: 1.0,
        use_vis_area: true,
        accept_decals: true,
        accept_snow: true,
        accept_silhouette: true,
        render_always: false,
        kill_on_deactivate: false,
        force_highest_contextual_priority: false,
    }
}
