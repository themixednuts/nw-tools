//! CryAnimation character-definition (`.cdf`) ownership.
//!
//! The XML tree is retained losslessly while typed projections expose the
//! model, attachment list, and reflected asset references used by exporters.

use std::collections::BTreeMap;

pub mod reflected;

pub use cry_xml::XmlElement;
use nw_reflected_types::types::{
    SimpleAssetReferenceBase, SimpleAssetReferenceCharacterDefinitionAsset,
    SimpleAssetReferenceMeshAsset, SimpleAssetReferenceSkinAsset,
};

#[derive(Debug, Clone, PartialEq)]
pub struct CharacterDefinition {
    /// Complete source tree, including modifiers and engine-version extensions.
    pub source: XmlElement,
    pub model: CharacterModel,
    pub attachments: Vec<CharacterAttachment>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CharacterModel {
    pub skeleton: String,
    pub params_override: Option<String>,
    pub material: Option<String>,
    pub physics: Option<String>,
    pub rig: Option<String>,
    pub attributes: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CharacterAttachment {
    pub kind: AttachmentKind,
    pub name: Option<String>,
    pub bone_name: Option<String>,
    pub binding: Option<String>,
    pub simulation_binding: Option<String>,
    pub material: Option<String>,
    pub material_lods: BTreeMap<u8, String>,
    pub flags: Option<u32>,
    pub character_rotation: Option<[f32; 4]>,
    pub character_position: Option<[f32; 3]>,
    pub relative_rotation: Option<[f32; 4]>,
    pub relative_position: Option<[f32; 3]>,
    /// Every authored attribute, including pendulum, spring, row, cloth,
    /// proxy, and physics settings not needed by the glTF projection.
    pub attributes: BTreeMap<String, String>,
    pub children: Vec<XmlElement>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttachmentKind {
    Bone,
    Face,
    Skin,
    Proxy,
    PendulumRow,
    VertexCloth,
    Unknown(String),
}

impl AttachmentKind {
    fn from_cry_name(value: &str) -> Self {
        match value {
            "CA_BONE" => Self::Bone,
            "CA_FACE" => Self::Face,
            "CA_SKIN" => Self::Skin,
            "CA_PROX" => Self::Proxy,
            "CA_PROW" => Self::PendulumRow,
            "CA_VCLOTH" => Self::VertexCloth,
            value => Self::Unknown(value.to_owned()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CharacterAssetGraph {
    pub skeleton: SimpleAssetReferenceMeshAsset,
    pub bindings: Vec<CharacterBindingAsset>,
    pub materials: Vec<String>,
    pub physics: Option<String>,
    pub rig: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CharacterBindingAsset {
    CharacterDefinition(SimpleAssetReferenceCharacterDefinitionAsset),
    Skin(SimpleAssetReferenceSkinAsset),
    Mesh(SimpleAssetReferenceMeshAsset),
    Other(String),
}

impl CharacterDefinition {
    /// Parse a `.cdf` document and build its typed attachment projection.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed XML, a non-CDF root, or a missing model.
    pub fn from_xml(xml: &str) -> Result<Self, CharacterDefinitionError> {
        let source = cry_xml::parse(xml)?;
        if source.name != "CharacterDefinition" {
            return Err(CharacterDefinitionError::UnexpectedRoot(source.name));
        }
        let model_node = source
            .children
            .iter()
            .find(|child| child.name == "Model")
            .ok_or(CharacterDefinitionError::MissingModel)?;
        let skeleton = model_node.attribute("File").unwrap_or_default().to_owned();
        let model = CharacterModel {
            skeleton,
            params_override: owned_attribute(model_node, "ParamsOverride"),
            material: owned_attribute(model_node, "Material"),
            physics: owned_attribute(model_node, "Physics"),
            // CharacterTool historically read `Rig File` and wrote `Rig`.
            rig: owned_attribute(model_node, "Rig")
                .or_else(|| owned_attribute(model_node, "Rig File")),
            attributes: model_node.attributes.clone(),
        };
        let attachments = source
            .children
            .iter()
            .filter(|child| child.name == "AttachmentList")
            .flat_map(|list| list.children.iter())
            .filter(|child| child.name == "Attachment")
            .map(CharacterAttachment::from_element)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            source,
            model,
            attachments,
        })
    }

    #[must_use]
    pub fn asset_graph(&self) -> CharacterAssetGraph {
        let skeleton = mesh_reference(&self.model.skeleton);
        let bindings = self
            .attachments
            .iter()
            .flat_map(|attachment| {
                [
                    attachment.binding.as_deref(),
                    attachment.simulation_binding.as_deref(),
                ]
            })
            .flatten()
            .map(binding_reference)
            .collect();
        let materials = self
            .model
            .material
            .iter()
            .chain(
                self.attachments
                    .iter()
                    .filter_map(|attachment| attachment.material.as_ref()),
            )
            .chain(
                self.attachments
                    .iter()
                    .flat_map(|attachment| attachment.material_lods.values()),
            )
            .cloned()
            .collect();
        CharacterAssetGraph {
            skeleton,
            bindings,
            materials,
            physics: self.model.physics.clone(),
            rig: self.model.rig.clone(),
        }
    }
}

impl CharacterAttachment {
    fn from_element(element: &XmlElement) -> Result<Self, CharacterDefinitionError> {
        let material_lods = (0_u8..=5)
            .filter_map(|lod| {
                element
                    .attribute(&format!("MaterialLOD{lod}"))
                    .map(|value| (lod, value.to_owned()))
            })
            .collect();
        Ok(Self {
            kind: AttachmentKind::from_cry_name(element.attribute("Type").unwrap_or_default()),
            name: owned_attribute(element, "AName"),
            bone_name: owned_attribute(element, "BoneName"),
            binding: owned_attribute(element, "Binding"),
            simulation_binding: owned_attribute(element, "SimBinding"),
            material: owned_attribute(element, "Material"),
            material_lods,
            flags: parse_optional(element, "Flags")?,
            character_rotation: parse_array::<4>(element, "Rotation")?,
            character_position: parse_array::<3>(element, "Position")?,
            relative_rotation: parse_array::<4>(element, "RelRotation")?,
            relative_position: parse_array::<3>(element, "RelPosition")?,
            attributes: element.attributes.clone(),
            children: element.children.clone(),
        })
    }
}

fn parse_optional<T>(
    element: &XmlElement,
    attribute: &'static str,
) -> Result<Option<T>, CharacterDefinitionError>
where
    T: std::str::FromStr,
{
    let Some(value) = element.attribute(attribute) else {
        return Ok(None);
    };
    value
        .parse()
        .map(Some)
        .map_err(|_| CharacterDefinitionError::InvalidAttribute {
            attribute,
            value: value.to_owned(),
        })
}

fn parse_array<const N: usize>(
    element: &XmlElement,
    attribute: &'static str,
) -> Result<Option<[f32; N]>, CharacterDefinitionError> {
    let Some(value) = element.attribute(attribute) else {
        return Ok(None);
    };
    let values = value
        .split(|character: char| character == ',' || character.is_ascii_whitespace())
        .filter(|component| !component.is_empty())
        .map(str::parse::<f32>)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| CharacterDefinitionError::InvalidAttribute {
            attribute,
            value: value.to_owned(),
        })?;
    let values: [f32; N] =
        values
            .try_into()
            .map_err(|_| CharacterDefinitionError::InvalidAttribute {
                attribute,
                value: value.to_owned(),
            })?;
    if !values.iter().all(|component| component.is_finite()) {
        return Err(CharacterDefinitionError::InvalidAttribute {
            attribute,
            value: value.to_owned(),
        });
    }
    Ok(Some(values))
}

fn owned_attribute(element: &XmlElement, name: &str) -> Option<String> {
    element
        .attribute(name)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

fn simple_reference(path: &str) -> SimpleAssetReferenceBase {
    SimpleAssetReferenceBase {
        asset_path: path.to_owned(),
    }
}

fn mesh_reference(path: &str) -> SimpleAssetReferenceMeshAsset {
    SimpleAssetReferenceMeshAsset {
        simple_asset_reference_base: simple_reference(path),
    }
}

fn binding_reference(path: &str) -> CharacterBindingAsset {
    match path
        .rsplit_once('.')
        .map(|(_, extension)| extension.to_ascii_lowercase())
    {
        Some(extension) if extension == "cdf" => CharacterBindingAsset::CharacterDefinition(
            SimpleAssetReferenceCharacterDefinitionAsset {
                simple_asset_reference_base: simple_reference(path),
            },
        ),
        Some(extension) if extension == "skin" => {
            CharacterBindingAsset::Skin(SimpleAssetReferenceSkinAsset {
                simple_asset_reference_base: simple_reference(path),
            })
        }
        Some(extension) if matches!(extension.as_str(), "cgf" | "cga" | "chr") => {
            CharacterBindingAsset::Mesh(mesh_reference(path))
        }
        _ => CharacterBindingAsset::Other(path.to_owned()),
    }
}

#[derive(Debug, thiserror::Error)]
pub enum CharacterDefinitionError {
    #[error(transparent)]
    Xml(#[from] cry_xml::XmlError),
    #[error("expected CharacterDefinition root, found `{0}`")]
    UnexpectedRoot(String),
    #[error("CharacterDefinition has no Model element")]
    MissingModel,
    #[error("CharacterDefinition attribute `{attribute}` has invalid value `{value}`")]
    InvalidAttribute {
        attribute: &'static str,
        value: String,
    },
}

/// One exact `<Animation>` directive from a `.chrparams` AnimationList.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CharacterAnimationEntry {
    pub name: String,
    pub path: String,
    pub flags: Option<String>,
    pub kind: CharacterAnimationEntryKind,
    pub source: XmlElement,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CharacterAnimationEntryKind {
    FilePath,
    ParseSubfolders,
    TracksDatabase,
    Include,
    AnimationEventDatabase,
    FaceLibrary,
    WildcardAsset,
    Asset,
    UnknownDirective,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CharacterParameters {
    pub source: XmlElement,
    pub animations: Vec<CharacterAnimationEntry>,
}

impl CharacterParameters {
    /// Project every CryAnimation directive while retaining the complete
    /// `.chrparams` source tree (IK, bounding boxes, and future categories).
    pub fn from_xml(xml: &str) -> Result<Self, CharacterDefinitionError> {
        let source = cry_xml::parse(xml)?;
        let animations = source
            .children_named("AnimationList")
            .flat_map(|list| list.children_named("Animation"))
            .filter_map(|element| {
                let name = element.attribute("name")?.to_owned();
                let path = element.attribute("path")?.to_owned();
                let kind = animation_entry_kind(&name, &path);
                Some(CharacterAnimationEntry {
                    name,
                    path,
                    flags: owned_attribute(element, "flags"),
                    kind,
                    source: element.clone(),
                })
            })
            .collect();
        Ok(Self { source, animations })
    }
}

fn animation_entry_kind(name: &str, path: &str) -> CharacterAnimationEntryKind {
    if name.eq_ignore_ascii_case("#filepath") {
        CharacterAnimationEntryKind::FilePath
    } else if name.eq_ignore_ascii_case("#ParseSubFolders") {
        CharacterAnimationEntryKind::ParseSubfolders
    } else if name.eq_ignore_ascii_case("$TracksDatabase") {
        CharacterAnimationEntryKind::TracksDatabase
    } else if name.eq_ignore_ascii_case("$Include") {
        CharacterAnimationEntryKind::Include
    } else if name.eq_ignore_ascii_case("$AnimEventDatabase") {
        CharacterAnimationEntryKind::AnimationEventDatabase
    } else if name.eq_ignore_ascii_case("$FaceLib") {
        CharacterAnimationEntryKind::FaceLibrary
    } else if name.starts_with(['#', '$']) {
        CharacterAnimationEntryKind::UnknownDirective
    } else if path.contains(['*', '?']) {
        CharacterAnimationEntryKind::WildcardAsset
    } else {
        CharacterAnimationEntryKind::Asset
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retains_complete_attachment_attributes_and_builds_typed_assets() {
        let definition = CharacterDefinition::from_xml(
            r#"<CharacterDefinition custom="keep">
                <Model File="objects/hero.chr" Material="objects/hero.mtl" Rig="hero.rig"/>
                <AttachmentList>
                    <Attachment Type="CA_SKIN" AName="coat" Binding="objects/coat.skin"
                        MaterialLOD0="objects/coat0.mtl" PA_Gravity="9.81" Flags="5"/>
                    <Attachment Type="CA_BONE" AName="weapon" BoneName="r_hand"
                        Binding="objects/sword.cgf"><Extension value="kept"/></Attachment>
                </AttachmentList>
                <Modifiers><Unknown value="preserved"/></Modifiers>
            </CharacterDefinition>"#,
        )
        .unwrap();

        assert_eq!(definition.model.skeleton, "objects/hero.chr");
        assert_eq!(definition.attachments.len(), 2);
        assert_eq!(definition.attachments[0].attributes["PA_Gravity"], "9.81");
        assert_eq!(definition.attachments[1].children[0].name, "Extension");
        assert_eq!(definition.source.children[2].name, "Modifiers");
        let graph = definition.asset_graph();
        assert_eq!(
            graph.skeleton.simple_asset_reference_base.asset_path,
            "objects/hero.chr"
        );
        assert!(matches!(graph.bindings[0], CharacterBindingAsset::Skin(_)));
        assert!(matches!(graph.bindings[1], CharacterBindingAsset::Mesh(_)));
    }

    #[test]
    fn parses_chrparams_animation_directives_without_dropping_other_categories() {
        let parameters = CharacterParameters::from_xml(
            r##"<Params><IK_Definition custom="keep"/><AnimationList>
                <Animation name="#filepath" path="animations/hero"/>
                <Animation name="idle" path="idle.caf" flags="persistent"/>
                <Animation name="$AnimEventDatabase" path="hero.animevents"/>
                <Animation name="locomotion_*" path="navigation/*.caf"/>
            </AnimationList></Params>"##,
        )
        .unwrap();

        assert_eq!(parameters.animations.len(), 4);
        assert_eq!(
            parameters.animations[0].kind,
            CharacterAnimationEntryKind::FilePath
        );
        assert_eq!(
            parameters.animations[1].kind,
            CharacterAnimationEntryKind::Asset
        );
        assert_eq!(
            parameters.animations[2].kind,
            CharacterAnimationEntryKind::AnimationEventDatabase
        );
        assert_eq!(
            parameters.animations[3].kind,
            CharacterAnimationEntryKind::WildcardAsset
        );
        assert_eq!(parameters.source.children[0].name, "IK_Definition");
    }
}
