//! Legacy CryEngine particle-library parsing and dependency projection.
//!
//! Resource fields follow `ParticleParams` and `CParticleItem::ResolveAssets`
//! in Lumberyard. The shared XML tree remains intact so unknown particle
//! parameters, LOD data, and future fields are never discarded.

use std::collections::BTreeSet;

use cry_xml::XmlElement;
use nw_asset::{AssetDependencies, AssetDependency, AssetDependencyTarget};
use thiserror::Error;

const PARTICLE_LIBRARY: &str = "ParticleLibrary";
const PARTICLES: &str = "Particles";
const PARAMS: &str = "Params";

/// Lumberyard's legacy particle libraries live under `libs/particles` and use
/// the otherwise-shared `.xml` extension.
#[must_use]
pub fn is_legacy_particle_library_source(path: &str) -> bool {
    let path = nw_asset::normalize_virtual_path(path).to_ascii_lowercase();
    path.ends_with(".xml")
        && (path.starts_with("libs/particles/") || path.contains("/libs/particles/"))
}

/// A loss-preserving Cry particle library with typed identity/version access.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParticleLibrarySource {
    pub name: String,
    pub sandbox_version: Option<String>,
    pub particle_version: Option<u32>,
    pub source: XmlElement,
}

impl ParticleLibrarySource {
    /// Parse and validate one legacy `<ParticleLibrary>` document.
    pub fn from_xml(xml: &str) -> Result<Self, ParticleLibraryError> {
        let source = cry_xml::parse(xml)?;
        if !source.name.eq_ignore_ascii_case(PARTICLE_LIBRARY) {
            return Err(ParticleLibraryError::UnexpectedRoot(source.name.clone()));
        }
        let name = attribute_ignore_case(&source, "Name")
            .filter(|value| !value.trim().is_empty())
            .ok_or(ParticleLibraryError::MissingName)?
            .to_owned();
        let sandbox_version = attribute_ignore_case(&source, "SandboxVersion").map(str::to_owned);
        let particle_version = attribute_ignore_case(&source, "ParticleVersion")
            .map(|value| {
                value
                    .parse::<u32>()
                    .map_err(|_| ParticleLibraryError::InvalidParticleVersion(value.to_owned()))
            })
            .transpose()?;
        Ok(Self {
            name,
            sandbox_version,
            particle_version,
            source,
        })
    }

    /// Every authored file/control resource in the whole library.
    #[must_use]
    pub fn resource_references(&self) -> Vec<ParticleResourceReference> {
        let mut references = Vec::new();
        let mut effect_path = Vec::new();
        collect_all_resources(&self.source, &mut effect_path, &mut references);
        canonicalize_references(&mut references);
        references
    }

    /// Resources used by one selected emitter, including inherited ancestor
    /// parameters and its complete child/LOD subtree.
    pub fn resources_for_effect(
        &self,
        selected_emitter: &str,
    ) -> Result<Vec<ParticleResourceReference>, ParticleEffectLookupError> {
        let selected = selected_emitter.trim();
        let mut effect_path = Vec::new();
        let mut inherited = Vec::new();
        let mut references = Vec::new();
        if !find_effect_resources(
            &self.source,
            &self.name,
            selected,
            &mut effect_path,
            &mut inherited,
            &mut references,
        ) {
            return Err(ParticleEffectLookupError {
                library: self.name.clone(),
                selected_emitter: selected_emitter.to_owned(),
            });
        }
        canonicalize_references(&mut references);
        Ok(references)
    }
}

impl AssetDependencies for ParticleLibrarySource {
    fn asset_dependencies(&self) -> Vec<AssetDependency> {
        self.resource_references()
            .into_iter()
            .map(|reference| reference.dependency())
            .collect()
    }
}

/// One runtime resource field in `ParticleParams`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ParticleResourceKind {
    Texture,
    NormalMap,
    GlowMap,
    Material,
    Geometry,
    AudioStartTrigger,
    AudioStopTrigger,
}

impl ParticleResourceKind {
    #[must_use]
    pub const fn relation(self) -> &'static str {
        match self {
            Self::Texture => "particle.texture",
            Self::NormalMap => "particle.normal_map",
            Self::GlowMap => "particle.glow_map",
            Self::Material => "particle.material",
            Self::Geometry => "particle.geometry",
            Self::AudioStartTrigger => "particle.audio.start_trigger",
            Self::AudioStopTrigger => "particle.audio.stop_trigger",
        }
    }

    #[must_use]
    pub const fn is_audio(self) -> bool {
        matches!(self, Self::AudioStartTrigger | Self::AudioStopTrigger)
    }

    #[must_use]
    pub const fn is_texture(self) -> bool {
        matches!(self, Self::Texture | Self::NormalMap | Self::GlowMap)
    }
}

/// A resource plus the effect path that authored it.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ParticleResourceReference {
    pub kind: ParticleResourceKind,
    pub value: String,
    pub effect_path: String,
}

impl ParticleResourceReference {
    #[must_use]
    pub fn dependency(&self) -> AssetDependency {
        if self.kind.is_audio() {
            AssetDependency::optional(
                self.kind.relation(),
                AssetDependencyTarget::symbol(&self.value),
            )
        } else {
            AssetDependency::required_path(self.kind.relation(), &self.value)
        }
    }
}

fn collect_all_resources(
    element: &XmlElement,
    effect_path: &mut Vec<String>,
    references: &mut Vec<ParticleResourceReference>,
) {
    let is_effect = element.name.eq_ignore_ascii_case(PARTICLES);
    if is_effect {
        effect_path.push(
            attribute_ignore_case(element, "Name")
                .unwrap_or_default()
                .to_owned(),
        );
    }
    if element.name.eq_ignore_ascii_case(PARAMS) {
        collect_params_resources(element, effect_path, references);
    }
    for child in &element.children {
        collect_all_resources(child, effect_path, references);
    }
    if is_effect {
        effect_path.pop();
    }
}

fn find_effect_resources(
    element: &XmlElement,
    library_name: &str,
    selected: &str,
    effect_path: &mut Vec<String>,
    inherited: &mut Vec<ParticleResourceReference>,
    references: &mut Vec<ParticleResourceReference>,
) -> bool {
    let is_effect = element.name.eq_ignore_ascii_case(PARTICLES);
    let inherited_len = inherited.len();
    if is_effect {
        effect_path.push(
            attribute_ignore_case(element, "Name")
                .unwrap_or_default()
                .to_owned(),
        );
        for child in &element.children {
            if child.name.eq_ignore_ascii_case(PARAMS) {
                collect_params_resources(child, effect_path, inherited);
            }
        }
        if effect_matches(library_name, effect_path, selected) {
            references.extend(inherited.iter().cloned());
            collect_subtree_resources(element, effect_path, references);
            effect_path.pop();
            inherited.truncate(inherited_len);
            return true;
        }
    }

    for child in &element.children {
        if find_effect_resources(
            child,
            library_name,
            selected,
            effect_path,
            inherited,
            references,
        ) {
            if is_effect {
                effect_path.pop();
                inherited.truncate(inherited_len);
            }
            return true;
        }
    }
    if is_effect {
        effect_path.pop();
        inherited.truncate(inherited_len);
    }
    false
}

fn collect_subtree_resources(
    element: &XmlElement,
    effect_path: &mut Vec<String>,
    references: &mut Vec<ParticleResourceReference>,
) {
    for child in &element.children {
        if child.name.eq_ignore_ascii_case(PARAMS) {
            collect_params_resources(child, effect_path, references);
            continue;
        }
        let is_effect = child.name.eq_ignore_ascii_case(PARTICLES);
        if is_effect {
            effect_path.push(
                attribute_ignore_case(child, "Name")
                    .unwrap_or_default()
                    .to_owned(),
            );
        }
        collect_subtree_resources(child, effect_path, references);
        if is_effect {
            effect_path.pop();
        }
    }
}

fn collect_params_resources(
    params: &XmlElement,
    effect_path: &[String],
    references: &mut Vec<ParticleResourceReference>,
) {
    for (name, kind) in [
        ("Texture", ParticleResourceKind::Texture),
        ("NormalMap", ParticleResourceKind::NormalMap),
        ("GlowMap", ParticleResourceKind::GlowMap),
        ("Material", ParticleResourceKind::Material),
        ("Geometry", ParticleResourceKind::Geometry),
        ("StartTrigger", ParticleResourceKind::AudioStartTrigger),
        ("StopTrigger", ParticleResourceKind::AudioStopTrigger),
    ] {
        let Some(value) = attribute_ignore_case(params, name)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        else {
            continue;
        };
        references.push(ParticleResourceReference {
            kind,
            value: if kind.is_audio() {
                value.to_owned()
            } else {
                nw_asset::normalize_virtual_path(value)
            },
            effect_path: effect_path.join("."),
        });
    }
}

fn effect_matches(library_name: &str, effect_path: &[String], selected: &str) -> bool {
    let relative = effect_path.join(".");
    let full = if relative.is_empty() {
        library_name.to_owned()
    } else {
        format!("{library_name}.{relative}")
    };
    selected.eq_ignore_ascii_case(&full) || selected.eq_ignore_ascii_case(&relative)
}

fn canonicalize_references(references: &mut Vec<ParticleResourceReference>) {
    references.sort_by(|left, right| {
        left.kind
            .cmp(&right.kind)
            .then_with(|| {
                left.value
                    .to_ascii_lowercase()
                    .cmp(&right.value.to_ascii_lowercase())
            })
            .then_with(|| left.effect_path.cmp(&right.effect_path))
    });
    let mut seen = BTreeSet::new();
    references
        .retain(|reference| seen.insert((reference.kind, reference.value.to_ascii_lowercase())));
}

fn attribute_ignore_case<'a>(element: &'a XmlElement, name: &str) -> Option<&'a str> {
    element
        .attributes
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.as_str())
}

#[derive(Debug, Error)]
pub enum ParticleLibraryError {
    #[error(transparent)]
    Xml(#[from] cry_xml::XmlError),
    #[error("expected ParticleLibrary root, found {0}")]
    UnexpectedRoot(String),
    #[error("particle library is missing its Name")]
    MissingName,
    #[error("invalid ParticleVersion {0:?}")]
    InvalidParticleVersion(String),
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
#[error("particle library {library:?} has no selected emitter {selected_emitter:?}")]
pub struct ParticleEffectLookupError {
    pub library: String,
    pub selected_emitter: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    const LIBRARY: &str = r#"
        <ParticleLibrary Name="cFX_Test" ParticleVersion="33" future="kept">
          <Particles Name="Parent">
            <Params Texture="textures/shared.tif" Material="materials/fx"/>
            <Childs>
              <Particles Name="Child">
                <Params NormalMap="textures/normal.dds" StartTrigger="Play_FX"/>
                <LODs><LevelOfDetail><LodParticle Distance="50"><Params Geometry="objects/lod.cgf"/></LodParticle></LevelOfDetail></LODs>
              </Particles>
              <Particles Name="Sibling"><Params Texture="textures/sibling.dds"/></Particles>
            </Childs>
          </Particles>
        </ParticleLibrary>
    "#;

    #[test]
    fn retains_unknown_source_and_projects_all_native_resource_fields() {
        let library = ParticleLibrarySource::from_xml(LIBRARY).unwrap();
        assert_eq!(library.particle_version, Some(33));
        assert_eq!(library.source.attributes["future"], "kept");
        let resources = library.resource_references();
        assert_eq!(resources.len(), 6);
        assert!(resources.iter().any(|resource| {
            resource.kind == ParticleResourceKind::AudioStartTrigger && resource.value == "Play_FX"
        }));
    }

    #[test]
    fn selected_child_inherits_parent_and_excludes_siblings() {
        let library = ParticleLibrarySource::from_xml(LIBRARY).unwrap();
        let resources = library
            .resources_for_effect("cfx_test.parent.child")
            .unwrap();
        let values = resources
            .iter()
            .map(|resource| resource.value.as_str())
            .collect::<BTreeSet<_>>();
        assert!(values.contains("textures/shared.tif"));
        assert!(values.contains("materials/fx"));
        assert!(values.contains("textures/normal.dds"));
        assert!(values.contains("objects/lod.cgf"));
        assert!(!values.contains("textures/sibling.dds"));
    }

    #[test]
    fn missing_selected_emitter_is_explicit() {
        let library = ParticleLibrarySource::from_xml(LIBRARY).unwrap();
        assert!(library.resources_for_effect("cFX_Test.Missing").is_err());
    }
}
