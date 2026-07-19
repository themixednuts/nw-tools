use nw_objectstream::Element;
use nw_objectstream::value;
use nw_reflected_types::az::rtti::AzRtti;
use nw_reflected_types::types::components::attachment_component::{
    AttachmentComponent, AttachmentConfiguration,
};

use super::read::{
    LmbrCentralObjectStreamError, checked_version, child, ensure_type, read_component_base,
    read_exact_string, read_optional, read_transform,
};

const COMPONENT_VERSION: u8 = 1;
const CONFIGURATION_VERSION: u8 = 1;
const INVALID_ENTITY_ID: u64 = u32::MAX as u64;

#[derive(Debug, Clone, PartialEq)]
pub struct AttachmentComponentSource {
    pub component: AttachmentComponent,
    pub component_version: Option<u8>,
    pub configuration_version: Option<u8>,
}

pub fn read_attachment_component(
    element: &Element,
) -> Result<AttachmentComponentSource, LmbrCentralObjectStreamError> {
    ensure_type(
        element,
        *AttachmentComponent::TYPE_ID.as_inner(),
        AttachmentComponent::NAME,
    )?;
    checked_version(element, AttachmentComponent::NAME, COMPONENT_VERSION)?;

    let (configuration, configuration_version) = child(element, "Configuration")
        .or_else(|| {
            element
                .children()
                .iter()
                .find(|child| child.id() == AttachmentConfiguration::TYPE_ID.as_inner())
        })
        .map(read_attachment_configuration)
        .transpose()?
        .unwrap_or_else(|| (native_attachment_configuration(), None));

    Ok(AttachmentComponentSource {
        component: AttachmentComponent {
            az_component: read_component_base(element)?,
            configuration,
        },
        component_version: element.version(),
        configuration_version,
    })
}

fn read_attachment_configuration(
    element: &Element,
) -> Result<(AttachmentConfiguration, Option<u8>), LmbrCentralObjectStreamError> {
    ensure_type(
        element,
        *AttachmentConfiguration::TYPE_ID.as_inner(),
        AttachmentConfiguration::NAME,
    )?;
    checked_version(
        element,
        AttachmentConfiguration::NAME,
        CONFIGURATION_VERSION,
    )?;
    let mut configuration = native_attachment_configuration();

    if let Some(value) = read_optional(element, "Target ID", value::read_entity_id)? {
        configuration.target_id = value;
    }
    if let Some(value) = read_exact_string(element, "Target Bone Name")? {
        configuration.target_bone_name = value;
    }
    if let Some(value) = read_optional(element, "Target Offset", read_transform)? {
        configuration.target_offset = value;
    }
    if let Some(value) = read_optional(element, "Attached Initially", value::read_bool)? {
        configuration.attached_initially = value;
    }
    if let Some(scale_source) = read_optional(element, "Scale Source", value::read_u8)? {
        if scale_source > 2 {
            return Err(LmbrCentralObjectStreamError::InvalidEnum {
                type_name: AttachmentConfiguration::NAME,
                field: "Scale Source",
                value: scale_source,
            });
        }
        configuration.scale_source = scale_source;
    }
    if let Some(value) = read_optional(element, "Update Tolerance", value::read_f32)? {
        configuration.update_tolerance = value;
    }

    Ok((configuration, element.version()))
}

#[must_use]
pub fn native_attachment_configuration() -> AttachmentConfiguration {
    AttachmentConfiguration {
        target_id: INVALID_ENTITY_ID,
        target_bone_name: String::new(),
        target_offset: Default::default(),
        attached_initially: true,
        scale_source: 0,
        update_tolerance: 0.0,
    }
}
