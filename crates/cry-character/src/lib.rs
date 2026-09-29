//! CryAnimation character-definition (`.cdf`) ownership.
//!
//! The XML tree is retained losslessly while typed projections expose the
//! model, attachment list, and reflected asset references used by exporters.

use std::collections::{BTreeMap, BTreeSet, HashSet};

pub mod reflected;

pub use cry_xml::XmlElement;
use nw_asset::{AssetDependencies, AssetDependency, AssetDependencyTarget, normalize_virtual_path};
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
            .collect();
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

impl AssetDependencies for CharacterDefinition {
    fn asset_dependencies(&self) -> Vec<AssetDependency> {
        let mut dependencies = Vec::new();
        push_required_path(
            &mut dependencies,
            "character.skeleton",
            &self.model.skeleton,
        );
        if let Some(path) = &self.model.params_override {
            push_required_path(&mut dependencies, "character.parameters", path);
        }
        if let Some(path) = &self.model.material {
            push_required_material(&mut dependencies, "character.material", path);
        }
        if let Some(path) = &self.model.physics {
            push_required_path(&mut dependencies, "character.physics", path);
        }
        if let Some(path) = &self.model.rig {
            push_required_path(&mut dependencies, "character.rig", path);
        }
        for attachment in &self.attachments {
            if let Some(path) = &attachment.binding {
                push_required_path(&mut dependencies, "character.attachment", path);
            }
            if let Some(path) = &attachment.simulation_binding {
                push_required_path(&mut dependencies, "character.simulation_binding", path);
            }
            if let Some(path) = &attachment.material {
                push_required_material(&mut dependencies, "character.attachment_material", path);
            }
            for path in attachment.material_lods.values() {
                push_required_material(
                    &mut dependencies,
                    "character.attachment_material_lod",
                    path,
                );
            }
        }
        dependencies
    }
}

impl CharacterAttachment {
    fn from_element(element: &XmlElement) -> Self {
        let material_lods = (0_u8..=5)
            .filter_map(|lod| {
                element
                    .attribute(&format!("MaterialLOD{lod}"))
                    .map(|value| (lod, value.to_owned()))
            })
            .collect();
        Self {
            kind: AttachmentKind::from_cry_name(element.attribute("Type").unwrap_or_default()),
            name: owned_attribute(element, "AName"),
            bone_name: owned_attribute(element, "BoneName"),
            binding: owned_attribute(element, "Binding"),
            simulation_binding: owned_attribute(element, "SimBinding"),
            material: owned_attribute(element, "Material"),
            material_lods,
            flags: parse_optional(element, "Flags"),
            character_rotation: parse_array::<4>(element, "Rotation"),
            character_position: parse_array::<3>(element, "Position"),
            relative_rotation: parse_array::<4>(element, "RelRotation"),
            relative_position: parse_array::<3>(element, "RelPosition"),
            attributes: element.attributes.clone(),
            children: element.children.clone(),
        }
    }
}

fn parse_optional<T>(element: &XmlElement, attribute: &'static str) -> Option<T>
where
    T: std::str::FromStr,
{
    // Cry XML `getAttr` leaves the destination at its default when conversion fails.
    element.attribute(attribute)?.parse().ok()
}

fn parse_array<const N: usize>(element: &XmlElement, attribute: &'static str) -> Option<[f32; N]> {
    let value = element.attribute(attribute)?;
    // Cry's Vec/Quat overloads use sscanf: they require the requested prefix but
    // ignore extra components, and report failure rather than rejecting the CDF.
    let mut components = value
        .split(|character: char| character == ',' || character.is_ascii_whitespace())
        .filter(|component| !component.is_empty());
    let mut values = [0.0; N];
    for output in &mut values {
        let value = components.next()?.parse::<f32>().ok()?;
        if !value.is_finite() {
            return None;
        }
        *output = value;
    }
    Some(values)
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

/// Sequential path state for a `.chrparams` `<AnimationList>`.
///
/// CryAnimation treats `#Filepath` as the directory for every following
/// concrete or wildcard animation entry, including entries whose relative
/// path contains directory separators. Other directives keep their authored
/// paths and are only normalized to forward slashes.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CharacterAnimationPathResolver {
    directory: String,
}

impl CharacterAnimationPathResolver {
    /// Advance the list's `#Filepath` state and return the engine path for an
    /// animation entry.
    #[must_use]
    pub fn resolve_entry(&mut self, entry: &CharacterAnimationEntry) -> String {
        let path = normalize_authored_path(&entry.path);
        match entry.kind {
            CharacterAnimationEntryKind::FilePath => {
                self.directory = path.trim_end_matches('/').to_owned();
                self.directory.clone()
            }
            CharacterAnimationEntryKind::WildcardAsset | CharacterAnimationEntryKind::Asset
                if !self.directory.is_empty() =>
            {
                format!("{}/{path}", self.directory)
            }
            _ => path,
        }
    }
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

impl AssetDependencies for CharacterParameters {
    fn asset_dependencies(&self) -> Vec<AssetDependency> {
        let mut dependencies = Vec::new();
        let mut paths = CharacterAnimationPathResolver::default();
        for entry in &self.animations {
            let path = paths.resolve_entry(entry);
            match entry.kind {
                CharacterAnimationEntryKind::FilePath => {}
                CharacterAnimationEntryKind::ParseSubfolders => {}
                CharacterAnimationEntryKind::TracksDatabase => {
                    dependencies.push(path_dependency("animation.tracks_database", path, true))
                }
                CharacterAnimationEntryKind::Include => {
                    push_required_path(&mut dependencies, "animation.parameters_include", &path);
                }
                CharacterAnimationEntryKind::AnimationEventDatabase => {
                    push_required_path(&mut dependencies, "animation.events", &path);
                }
                CharacterAnimationEntryKind::FaceLibrary => {
                    push_required_path(&mut dependencies, "animation.face_library", &path);
                }
                CharacterAnimationEntryKind::WildcardAsset => {
                    dependencies.push(path_dependency("animation.clip_pattern", path, false));
                }
                CharacterAnimationEntryKind::Asset => {
                    push_required_path(&mut dependencies, "animation.asset", &path);
                }
                CharacterAnimationEntryKind::UnknownDirective => {
                    dependencies.push(AssetDependency::required(
                        "animation.unknown_directive",
                        AssetDependencyTarget::symbol(format!("{}={}", entry.name, entry.path)),
                    ));
                }
            }
        }
        dependencies
    }
}

/// Exact CDF/CHRPARAMS ownership of CAF authoring sources.
///
/// This follows each character definition's parameter root, preserves the
/// sequential `#Filepath` state through includes, expands wildcard entries
/// through the caller's asset index, and records every owning skeleton. It does
/// not infer ownership from controller overlap.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CharacterAnimationOwnershipIndex {
    skeletons_by_clip: BTreeMap<String, BTreeSet<String>>,
}

/// Exact animation-set aliases recovered from CDF/CHRPARAMS roots.
///
/// Cry resolves Mannequin and blend-space animation names through the owning
/// character's `AnimationList`. This index preserves that authored lookup,
/// including shared includes, sequential `#filepath` state, and wildcard alias
/// expansion. Aliases that resolve to different assets across characters are
/// intentionally omitted from [`Self::path_for_alias`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CharacterAnimationAliasIndex {
    paths_by_alias: BTreeMap<String, CharacterAnimationAliasPath>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum CharacterAnimationAliasPath {
    Unique(String),
    Ambiguous,
}

impl CharacterAnimationAliasIndex {
    /// Build exact alias bindings from character definitions and their
    /// parameter graphs.
    pub fn build<Definitions, ReadParameters, MatchingPaths>(
        definitions: Definitions,
        mut read_parameters: ReadParameters,
        mut matching_paths: MatchingPaths,
    ) -> Result<Self, CharacterAnimationAliasError>
    where
        Definitions: IntoIterator<Item = (String, CharacterDefinition)>,
        ReadParameters: FnMut(&str) -> Result<Option<CharacterParameters>, String>,
        MatchingPaths: FnMut(&str) -> Result<Vec<String>, String>,
    {
        let mut aliases = Self::default();
        for (definition_path, definition) in definitions {
            let skeleton = normalize_virtual_path(&definition.model.skeleton);
            if skeleton.is_empty() {
                return Err(CharacterAnimationOwnershipError::MissingSkeleton {
                    definition: definition_path,
                });
            }
            let (parameters_path, required) =
                if let Some(path) = definition.model.params_override.as_deref() {
                    (normalize_virtual_path(path), true)
                } else {
                    (replace_virtual_extension(&skeleton, "chrparams"), false)
                };
            let Some(parameters) = read_parameters(&parameters_path).map_err(|message| {
                CharacterAnimationOwnershipError::ReadParameters {
                    path: parameters_path.clone(),
                    message,
                }
            })?
            else {
                if required {
                    return Err(
                        CharacterAnimationOwnershipError::MissingRequiredParameters {
                            definition: definition_path,
                            path: parameters_path,
                        },
                    );
                }
                continue;
            };
            let mut state = CharacterAnimationPathResolver::default();
            let mut visited = HashSet::new();
            visit_character_animation_parameters(
                &parameters_path,
                &parameters,
                &mut state,
                &mut visited,
                &mut read_parameters,
                &mut matching_paths,
                &mut |alias, source_path| aliases.add_alias(alias, source_path),
            )?;
        }
        Ok(aliases)
    }

    /// Return a path only when every character that defines the alias agrees
    /// on the same animation asset.
    #[must_use]
    pub fn path_for_alias(&self, alias: &str) -> Option<&str> {
        match self.paths_by_alias.get(&normalize_animation_alias(alias)) {
            Some(CharacterAnimationAliasPath::Unique(path)) => Some(path),
            Some(CharacterAnimationAliasPath::Ambiguous) | None => None,
        }
    }

    pub fn aliases(&self) -> impl Iterator<Item = (&str, &str)> {
        self.paths_by_alias
            .iter()
            .filter_map(|(alias, path)| match path {
                CharacterAnimationAliasPath::Unique(path) => Some((alias.as_str(), path.as_str())),
                CharacterAnimationAliasPath::Ambiguous => None,
            })
    }

    fn add_alias(&mut self, alias: &str, source_path: &str) {
        if !is_character_animation_asset_path(source_path) {
            return;
        }
        let alias = normalize_animation_alias(alias);
        if alias.is_empty() {
            return;
        }
        let source_path = normalize_virtual_path(source_path).to_ascii_lowercase();
        self.paths_by_alias
            .entry(alias)
            .and_modify(|entry| {
                if !matches!(entry, CharacterAnimationAliasPath::Unique(existing) if existing == &source_path)
                {
                    *entry = CharacterAnimationAliasPath::Ambiguous;
                }
            })
            .or_insert(CharacterAnimationAliasPath::Unique(source_path));
    }
}

/// Alias-index traversal has the same source-graph failure surface as ownership
/// traversal; keep one durable public error vocabulary for both indices.
pub type CharacterAnimationAliasError = CharacterAnimationOwnershipError;

impl CharacterAnimationOwnershipIndex {
    /// Build exact clip ownership from parsed character definitions.
    ///
    /// `read_parameters` must return a parsed CHRPARAMS document for a virtual
    /// path. `matching_paths` must expand Cry wildcards against the same asset
    /// source used to read the definitions.
    pub fn build<Definitions, ReadParameters, MatchingPaths>(
        definitions: Definitions,
        mut read_parameters: ReadParameters,
        mut matching_paths: MatchingPaths,
    ) -> Result<Self, CharacterAnimationOwnershipError>
    where
        Definitions: IntoIterator<Item = (String, CharacterDefinition)>,
        ReadParameters: FnMut(&str) -> Result<Option<CharacterParameters>, String>,
        MatchingPaths: FnMut(&str) -> Result<Vec<String>, String>,
    {
        let mut ownership = Self::default();
        for (definition_path, definition) in definitions {
            let skeleton = normalize_virtual_path(&definition.model.skeleton);
            if skeleton.is_empty() {
                return Err(CharacterAnimationOwnershipError::MissingSkeleton {
                    definition: definition_path,
                });
            }
            let (parameters_path, required) =
                if let Some(path) = definition.model.params_override.as_deref() {
                    (normalize_virtual_path(path), true)
                } else {
                    (replace_virtual_extension(&skeleton, "chrparams"), false)
                };
            let Some(parameters) = read_parameters(&parameters_path).map_err(|message| {
                CharacterAnimationOwnershipError::ReadParameters {
                    path: parameters_path.clone(),
                    message,
                }
            })?
            else {
                if required {
                    return Err(
                        CharacterAnimationOwnershipError::MissingRequiredParameters {
                            definition: definition_path,
                            path: parameters_path,
                        },
                    );
                }
                continue;
            };
            let mut state = CharacterAnimationPathResolver::default();
            let mut visited = HashSet::new();
            visit_character_animation_parameters(
                &parameters_path,
                &parameters,
                &mut state,
                &mut visited,
                &mut read_parameters,
                &mut matching_paths,
                &mut |_, source_path| ownership.add_clip_owner(source_path, &skeleton),
            )?;
        }
        Ok(ownership)
    }

    #[must_use]
    pub fn skeletons_for_clip(&self, source_path: &str) -> Option<&BTreeSet<String>> {
        self.skeletons_by_clip
            .get(&canonical_animation_clip_path(source_path))
    }

    pub fn clips(&self) -> impl Iterator<Item = (&str, &BTreeSet<String>)> {
        self.skeletons_by_clip
            .iter()
            .map(|(clip, skeletons)| (clip.as_str(), skeletons))
    }

    fn add_clip_owner(&mut self, source_path: &str, skeleton: &str) {
        if !is_animation_clip_path(source_path) {
            return;
        }
        self.skeletons_by_clip
            .entry(canonical_animation_clip_path(source_path))
            .or_default()
            .insert(normalize_virtual_path(skeleton));
    }
}

#[allow(clippy::too_many_arguments)]
fn visit_character_animation_parameters<ReadParameters, MatchingPaths, VisitAsset>(
    source_path: &str,
    parameters: &CharacterParameters,
    state: &mut CharacterAnimationPathResolver,
    visited: &mut HashSet<String>,
    read_parameters: &mut ReadParameters,
    matching_paths: &mut MatchingPaths,
    visit_asset: &mut VisitAsset,
) -> Result<(), CharacterAnimationOwnershipError>
where
    ReadParameters: FnMut(&str) -> Result<Option<CharacterParameters>, String>,
    MatchingPaths: FnMut(&str) -> Result<Vec<String>, String>,
    VisitAsset: FnMut(&str, &str),
{
    if !visited.insert(normalize_virtual_path(source_path).to_ascii_lowercase()) {
        return Ok(());
    }
    for entry in &parameters.animations {
        let path = state.resolve_entry(entry);
        match entry.kind {
            CharacterAnimationEntryKind::FilePath
            | CharacterAnimationEntryKind::ParseSubfolders
            | CharacterAnimationEntryKind::TracksDatabase
            | CharacterAnimationEntryKind::AnimationEventDatabase
            | CharacterAnimationEntryKind::FaceLibrary => {}
            CharacterAnimationEntryKind::Include => {
                if path.trim().is_empty() {
                    continue;
                }
                let included = read_parameters(&path).map_err(|message| {
                    CharacterAnimationOwnershipError::ReadParameters {
                        path: path.clone(),
                        message,
                    }
                })?;
                let Some(included) = included else {
                    return Err(
                        CharacterAnimationOwnershipError::MissingIncludedParameters {
                            source_path: source_path.to_string(),
                            path,
                        },
                    );
                };
                visit_character_animation_parameters(
                    &path,
                    &included,
                    state,
                    visited,
                    read_parameters,
                    matching_paths,
                    visit_asset,
                )?;
            }
            CharacterAnimationEntryKind::WildcardAsset => {
                let matches = matching_paths(&path).map_err(|message| {
                    CharacterAnimationOwnershipError::MatchPattern {
                        source_path: source_path.to_string(),
                        pattern: path.clone(),
                        message,
                    }
                })?;
                for matched in matches {
                    if let Some(alias) = expanded_animation_alias(&entry.name, &matched) {
                        visit_asset(&alias, &matched);
                    }
                }
            }
            CharacterAnimationEntryKind::Asset => visit_asset(&entry.name, &path),
            CharacterAnimationEntryKind::UnknownDirective => {
                return Err(CharacterAnimationOwnershipError::UnsupportedDirective {
                    source_path: source_path.to_string(),
                    name: entry.name.clone(),
                    path: entry.path.clone(),
                });
            }
        }
    }
    Ok(())
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum CharacterAnimationOwnershipError {
    #[error("character definition {definition} has no skeleton")]
    MissingSkeleton { definition: String },
    #[error("character definition {definition} requires missing parameters {path}")]
    MissingRequiredParameters { definition: String, path: String },
    #[error("included character parameters {path} from {source_path} are missing")]
    MissingIncludedParameters { source_path: String, path: String },
    #[error("read character parameters {path}: {message}")]
    ReadParameters { path: String, message: String },
    #[error("expand animation pattern {pattern} from {source_path}: {message}")]
    MatchPattern {
        source_path: String,
        pattern: String,
        message: String,
    },
    #[error("unsupported CHRPARAMS directive `{name}` with path `{path}` in {source_path}")]
    UnsupportedDirective {
        source_path: String,
        name: String,
        path: String,
    },
}

#[must_use]
pub fn canonical_animation_clip_path(path: &str) -> String {
    let normalized = normalize_virtual_path(path).to_ascii_lowercase();
    normalized
        .strip_suffix(".i_caf")
        .map_or(normalized.clone(), |stem| format!("{stem}.caf"))
}

fn is_animation_clip_path(path: &str) -> bool {
    let path = normalize_virtual_path(path).to_ascii_lowercase();
    path.ends_with(".caf") || path.ends_with(".i_caf")
}

fn is_character_animation_asset_path(path: &str) -> bool {
    let path = normalize_virtual_path(path).to_ascii_lowercase();
    [".caf", ".i_caf", ".bspace", ".comb"]
        .iter()
        .any(|extension| path.ends_with(extension))
}

fn normalize_animation_alias(alias: &str) -> String {
    alias.trim().to_ascii_lowercase()
}

fn expanded_animation_alias(pattern: &str, source_path: &str) -> Option<String> {
    let path = normalize_virtual_path(source_path);
    if !is_character_animation_asset_path(&path) {
        return None;
    }
    let file = path.rsplit('/').next()?;
    let stem = file.rsplit_once('.').map_or(file, |(stem, _)| stem);
    Some(pattern.split_once('*').map_or_else(
        || pattern.to_owned(),
        |(prefix, suffix)| format!("{prefix}{stem}{suffix}"),
    ))
}

fn replace_virtual_extension(path: &str, extension: &str) -> String {
    let path = normalize_virtual_path(path);
    path.rsplit_once('.').map_or_else(
        || format!("{path}.{extension}"),
        |(stem, _)| format!("{stem}.{extension}"),
    )
}

fn path_dependency(relation: &str, path: String, required: bool) -> AssetDependency {
    let target = if path.contains(['*', '?', '[']) {
        AssetDependencyTarget::pattern(path)
    } else {
        AssetDependencyTarget::path(path)
    };
    if required {
        AssetDependency::required(relation, target)
    } else {
        AssetDependency::optional(relation, target)
    }
}

fn push_required_path(dependencies: &mut Vec<AssetDependency>, relation: &str, path: &str) {
    if !path.trim().is_empty() {
        dependencies.push(AssetDependency::required_path(relation, path));
    }
}

fn push_required_material(dependencies: &mut Vec<AssetDependency>, relation: &str, path: &str) {
    if path.trim().is_empty() {
        return;
    }
    let path = if path
        .rsplit_once('.')
        .is_some_and(|(_, extension)| extension.eq_ignore_ascii_case("mtl"))
    {
        path.to_owned()
    } else {
        format!("{path}.mtl")
    };
    dependencies.push(AssetDependency::required_path(relation, path));
}

fn normalize_authored_path(path: &str) -> String {
    path.trim_start_matches(['/', '\\']).replace('\\', "/")
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
    fn mirrors_cry_get_attr_for_malformed_attachment_transforms() {
        let definition = CharacterDefinition::from_xml(
            r#"<CharacterDefinition><Model File="objects/weapon.chr"/><AttachmentList>
                <Attachment Type="CA_BONE" Flags="invalid"
                    RelPosition="-8.7422777e-08,0,0,0.99999994"
                    RelRotation="-8.5965385e-10,0.054916643,0.088664211"/>
            </AttachmentList></CharacterDefinition>"#,
        )
        .unwrap();

        let attachment = &definition.attachments[0];
        assert_eq!(attachment.flags, None);
        assert_eq!(
            attachment.relative_position,
            Some([-8.742_278e-8, 0.0, 0.0])
        );
        assert_eq!(attachment.relative_rotation, None);
        assert_eq!(
            attachment.attributes["RelRotation"],
            "-8.5965385e-10,0.054916643,0.088664211"
        );
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

    #[test]
    fn filepath_rebases_nested_animation_paths_and_wildcards() {
        let parameters = CharacterParameters::from_xml(
            r##"<Params><AnimationList>
                <Animation name="#filepath" path="animations\Gameplay\Alligator\"/>
                <Animation name="walk" path="navigation\walk.caf"/>
                <Animation name="*" path="*\*.caf"/>
                <Animation name="$AnimEventDatabase" path="animations/alligator.animevents"/>
            </AnimationList></Params>"##,
        )
        .unwrap();

        let dependencies = parameters.asset_dependencies();

        assert!(dependencies.iter().any(|dependency| {
            matches!(
                dependency.target(),
                AssetDependencyTarget::Asset(reference)
                    if reference.hint()
                        == Some("animations/Gameplay/Alligator/navigation/walk.caf")
            )
        }));
        assert!(dependencies.iter().any(|dependency| {
            dependency.target()
                == &AssetDependencyTarget::pattern("animations/Gameplay/Alligator/*/*.caf")
        }));
        assert!(dependencies.iter().any(|dependency| {
            matches!(
                dependency.target(),
                AssetDependencyTarget::Asset(reference)
                    if reference.hint() == Some("animations/alligator.animevents")
            )
        }));
    }

    #[test]
    fn animation_ownership_uses_cdf_parameters_and_shared_include_path_state() {
        let definition = CharacterDefinition::from_xml(
            r#"<CharacterDefinition><Model File="objects/hero.chr"/></CharacterDefinition>"#,
        )
        .unwrap();
        let root = CharacterParameters::from_xml(
            r##"<Params><AnimationList>
                <Animation name="#filepath" path="animations/shared"/>
                <Animation name="$Include" path="objects/shared.chrparams"/>
                <Animation name="$Include" path=""/>
            </AnimationList></Params>"##,
        )
        .unwrap();
        let shared = CharacterParameters::from_xml(
            r#"<Params><AnimationList>
                <Animation name="idle" path="idle.caf"/>
                <Animation name="moves" path="move_*.caf"/>
            </AnimationList></Params>"#,
        )
        .unwrap();
        let parameters = BTreeMap::from([
            ("objects/hero.chrparams".to_string(), root),
            ("objects/shared.chrparams".to_string(), shared),
        ]);

        let ownership = CharacterAnimationOwnershipIndex::build(
            [("objects/hero.cdf".to_string(), definition)],
            |path| Ok(parameters.get(path).cloned()),
            |pattern| {
                assert_eq!(pattern, "animations/shared/move_*.caf");
                Ok(vec!["animations/shared/move_forward.i_caf".to_string()])
            },
        )
        .unwrap();

        assert_eq!(
            ownership.skeletons_for_clip("animations/shared/idle.i_caf"),
            Some(&BTreeSet::from(["objects/hero.chr".to_string()]))
        );
        assert_eq!(
            ownership.skeletons_for_clip("animations/shared/move_forward.caf"),
            Some(&BTreeSet::from(["objects/hero.chr".to_string()]))
        );
    }

    #[test]
    fn animation_aliases_follow_includes_filepath_and_native_wildcard_names() {
        let definition = CharacterDefinition::from_xml(
            r#"<CharacterDefinition><Model File="objects/hero.chr"/></CharacterDefinition>"#,
        )
        .unwrap();
        let root = CharacterParameters::from_xml(
            r##"<Params><AnimationList>
                <Animation name="#filepath" path="animations/shared"/>
                <Animation name="$Include" path="objects/shared.chrparams"/>
            </AnimationList></Params>"##,
        )
        .unwrap();
        let shared = CharacterParameters::from_xml(
            r#"<Params><AnimationList>
                <Animation name="idle" path="idle.caf"/>
                <Animation name="move_*_loop" path="move_*.caf"/>
            </AnimationList></Params>"#,
        )
        .unwrap();
        let parameters = BTreeMap::from([
            ("objects/hero.chrparams".to_string(), root),
            ("objects/shared.chrparams".to_string(), shared),
        ]);

        let aliases = CharacterAnimationAliasIndex::build(
            [("objects/hero.cdf".to_string(), definition)],
            |path| Ok(parameters.get(path).cloned()),
            |pattern| {
                assert_eq!(pattern, "animations/shared/move_*.caf");
                Ok(vec!["animations/shared/move_forward.i_caf".to_string()])
            },
        )
        .unwrap();

        assert_eq!(
            aliases.path_for_alias("IDLE"),
            Some("animations/shared/idle.caf")
        );
        assert_eq!(
            aliases.path_for_alias("move_move_forward_loop"),
            Some("animations/shared/move_forward.i_caf")
        );
    }

    #[test]
    fn animation_aliases_omit_cross_character_disagreements() {
        let first = CharacterDefinition::from_xml(
            r#"<CharacterDefinition><Model File="objects/first.chr"/></CharacterDefinition>"#,
        )
        .unwrap();
        let second = CharacterDefinition::from_xml(
            r#"<CharacterDefinition><Model File="objects/second.chr"/></CharacterDefinition>"#,
        )
        .unwrap();
        let first_parameters = CharacterParameters::from_xml(
            r#"<Params><AnimationList><Animation name="idle" path="animations/first_idle.caf"/></AnimationList></Params>"#,
        )
        .unwrap();
        let second_parameters = CharacterParameters::from_xml(
            r#"<Params><AnimationList><Animation name="idle" path="animations/second_idle.caf"/></AnimationList></Params>"#,
        )
        .unwrap();
        let parameters = BTreeMap::from([
            ("objects/first.chrparams".to_string(), first_parameters),
            ("objects/second.chrparams".to_string(), second_parameters),
        ]);

        let aliases = CharacterAnimationAliasIndex::build(
            [
                ("objects/first.cdf".to_string(), first),
                ("objects/second.cdf".to_string(), second),
            ],
            |path| Ok(parameters.get(path).cloned()),
            |_| Ok(Vec::new()),
        )
        .unwrap();

        assert_eq!(aliases.path_for_alias("idle"), None);
        assert_eq!(aliases.aliases().count(), 0);
    }
}
