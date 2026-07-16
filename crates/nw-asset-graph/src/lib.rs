//! Typed authored-asset dependency discovery.
//!
//! The optimized catalog provides identity and path lookup, not the product
//! dependency map exposed by Lumberyard's runtime `AssetRegistry`. This crate
//! therefore derives closures from the actual owning formats. Each parser emits
//! [`nw_asset::AssetDependency`] values; this layer resolves paths, wildcards,
//! reflected AssetIds, and animation aliases against an [`AssetSource`].

use std::collections::{BTreeSet, HashMap, VecDeque};
use std::path::Path;
use std::str;

use anyhow::{Context, Result, bail};
use nw_asset::{
    AssetDependencies, AssetDependency, AssetDependencyTarget, AssetId, AssetReference,
    normalize_virtual_path,
};

/// Read-only asset access required by dependency resolution.
pub trait AssetSource: Sync {
    fn read(&self, path: &str) -> Option<Vec<u8>>;

    fn contains(&self, path: &str) -> bool {
        self.read(path).is_some()
    }

    fn matching_paths(&self, pattern: &str) -> Result<Vec<String>>;

    fn path_by_id(&self, _asset_id: AssetId) -> Option<String> {
        None
    }

    /// Resolve the GUID-like legacy name stored by Cry's `MtlName` chunk.
    fn legacy_material_path(&self, _name: &str) -> Option<String> {
        None
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetDependencyEdge {
    source: String,
    target: String,
    relation: String,
    required: bool,
}

impl AssetDependencyEdge {
    #[must_use]
    pub fn source(&self) -> &str {
        &self.source
    }

    #[must_use]
    pub fn target(&self) -> &str {
        &self.target
    }

    #[must_use]
    pub fn relation(&self) -> &str {
        &self.relation
    }

    #[must_use]
    pub const fn is_required(&self) -> bool {
        self.required
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnresolvedReason {
    MissingAsset,
    PatternMatchedNothing,
    UnknownSymbol,
    AmbiguousSymbol,
    AmbiguousPath,
    MissingIdentity,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnresolvedDependency {
    source: String,
    target: AssetDependencyTarget,
    relation: String,
    required: bool,
    reason: UnresolvedReason,
}

impl UnresolvedDependency {
    #[must_use]
    pub fn source(&self) -> &str {
        &self.source
    }

    #[must_use]
    pub const fn target(&self) -> &AssetDependencyTarget {
        &self.target
    }

    #[must_use]
    pub fn relation(&self) -> &str {
        &self.relation
    }

    #[must_use]
    pub const fn is_required(&self) -> bool {
        self.required
    }

    #[must_use]
    pub const fn reason(&self) -> UnresolvedReason {
        self.reason
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AssetDependencyGraph {
    roots: Vec<String>,
    assets: Vec<String>,
    edges: Vec<AssetDependencyEdge>,
    unresolved: Vec<UnresolvedDependency>,
}

impl AssetDependencyGraph {
    #[must_use]
    pub fn roots(&self) -> &[String] {
        &self.roots
    }

    #[must_use]
    pub fn assets(&self) -> &[String] {
        &self.assets
    }

    #[must_use]
    pub fn edges(&self) -> &[AssetDependencyEdge] {
        &self.edges
    }

    #[must_use]
    pub fn unresolved(&self) -> &[UnresolvedDependency] {
        &self.unresolved
    }

    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.unresolved
            .iter()
            .all(|dependency| !dependency.is_required())
    }

    pub fn direct_dependencies<'a>(
        &'a self,
        source: &'a str,
    ) -> impl Iterator<Item = &'a str> + 'a {
        self.edges
            .iter()
            .filter(move |edge| edge.source.eq_ignore_ascii_case(source))
            .map(|edge| edge.target.as_str())
    }
}

#[derive(Debug, Clone, Default)]
pub struct ResolveOptions {
    /// Extra roots are resolved in the same closure (for explicit mannequin,
    /// event-database, or audio inputs supplied by a caller).
    pub additional_roots: Vec<String>,
}

#[derive(Debug, Clone)]
struct PendingSymbol {
    source: String,
    dependency: AssetDependency,
    required: bool,
}

/// Resolve a complete authored dependency closure.
pub fn resolve(
    source: &dyn AssetSource,
    root: &str,
    options: &ResolveOptions,
) -> Result<AssetDependencyGraph> {
    resolve_with_runner(source, root, options, &nw_jobs::JobRunner::automatic())
}

/// Resolve a complete authored dependency closure using the caller's worker
/// policy for independent asset reads.
pub fn resolve_with_runner(
    source: &dyn AssetSource,
    root: &str,
    options: &ResolveOptions,
    runner: &nw_jobs::JobRunner,
) -> Result<AssetDependencyGraph> {
    let mut resolver = Resolver::new(source, runner);
    resolver.add_root(root)?;
    for root in &options.additional_roots {
        resolver.add_root(root)?;
    }
    resolver.finish()
}

struct Resolver<'a> {
    source: &'a dyn AssetSource,
    runner: &'a nw_jobs::JobRunner,
    roots: Vec<String>,
    assets: BTreeSet<String>,
    asset_required: HashMap<String, bool>,
    queue: VecDeque<String>,
    edges: Vec<AssetDependencyEdge>,
    edge_keys: BTreeSet<(String, String, String)>,
    unresolved: Vec<UnresolvedDependency>,
    unresolved_keys: BTreeSet<(String, String, String)>,
    animation_aliases: HashMap<String, BTreeSet<String>>,
    pending_symbols: Vec<PendingSymbol>,
    model_material_overrides: BTreeSet<String>,
    model_default_material_required: BTreeSet<String>,
}

impl<'a> Resolver<'a> {
    fn new(source: &'a dyn AssetSource, runner: &'a nw_jobs::JobRunner) -> Self {
        Self {
            source,
            runner,
            roots: Vec::new(),
            assets: BTreeSet::new(),
            asset_required: HashMap::new(),
            queue: VecDeque::new(),
            edges: Vec::new(),
            edge_keys: BTreeSet::new(),
            unresolved: Vec::new(),
            unresolved_keys: BTreeSet::new(),
            animation_aliases: HashMap::new(),
            pending_symbols: Vec::new(),
            model_material_overrides: BTreeSet::new(),
            model_default_material_required: BTreeSet::new(),
        }
    }

    fn add_root(&mut self, root: &str) -> Result<()> {
        let root = normalize_virtual_path(root);
        if !self.source.contains(&root) {
            bail!("root asset not found: {root}");
        }
        if !self.roots.iter().any(|existing| existing == &root) {
            self.roots.push(root.clone());
        }
        if matches!(AssetFormat::from_path(&root), AssetFormat::CryModel) {
            self.require_default_model_material(&root);
        }
        self.enqueue(root, true);
        Ok(())
    }

    fn finish(mut self) -> Result<AssetDependencyGraph> {
        loop {
            while !self.queue.is_empty() {
                let frontier = self.queue.drain(..).collect::<Vec<_>>();
                let loaded = self.runner.try_map(&frontier, |path| {
                    self.source
                        .read(path)
                        .with_context(|| format!("read dependency asset {path}"))
                        .map(|bytes| (path.clone(), bytes))
                })?;
                for (path, bytes) in loaded {
                    let parent_required = self.asset_required.get(&path).copied().unwrap_or(false);
                    let dependencies = self
                        .extract(&path, &bytes, parent_required)
                        .with_context(|| format!("extract dependencies from {path}"))?;
                    for dependency in dependencies {
                        self.resolve_dependency(&path, dependency, parent_required)?;
                    }
                }
            }

            // Symbols may precede their CHRPARAMS alias table. Resolve every
            // newly-known alias, inspect any concrete assets that adds, and
            // repeat until no remaining symbol can extend the graph.
            let pending = std::mem::take(&mut self.pending_symbols);
            for pending in pending {
                let AssetDependencyTarget::Symbol(symbol) = pending.dependency.target() else {
                    continue;
                };
                match self.animation_alias_paths(symbol) {
                    Some(paths) if paths.len() == 1 => {
                        let path = paths.first().expect("one alias path").clone();
                        self.resolve_path_target(
                            &pending.source,
                            &pending.dependency,
                            &path,
                            pending.required,
                        )?;
                    }
                    Some(_) => self.push_unresolved(
                        &pending.source,
                        &pending.dependency,
                        UnresolvedReason::AmbiguousSymbol,
                        pending.required,
                    ),
                    None => self.pending_symbols.push(pending),
                }
            }

            if self.queue.is_empty() {
                break;
            }
        }

        for pending in std::mem::take(&mut self.pending_symbols) {
            self.push_unresolved(
                &pending.source,
                &pending.dependency,
                UnresolvedReason::UnknownSymbol,
                pending.required,
            );
        }

        self.edges.sort_by(|left, right| {
            left.source
                .cmp(&right.source)
                .then_with(|| left.target.cmp(&right.target))
                .then_with(|| left.relation.cmp(&right.relation))
        });
        self.unresolved.sort_by(|left, right| {
            left.source
                .cmp(&right.source)
                .then_with(|| left.target.to_string().cmp(&right.target.to_string()))
                .then_with(|| left.relation.cmp(&right.relation))
        });
        Ok(AssetDependencyGraph {
            roots: self.roots,
            assets: self.assets.into_iter().collect(),
            edges: self.edges,
            unresolved: self.unresolved,
        })
    }

    fn extract(
        &mut self,
        path: &str,
        bytes: &[u8],
        parent_required: bool,
    ) -> Result<Vec<AssetDependency>> {
        if nw_objectstream::looks_like_objectstream(bytes) {
            let stream = nw_objectstream::ObjectStream::from_bytes(bytes, None)?;
            return Ok(
                nw_objectstream::asset_reference::collect_asset_references(&stream)?
                    .into_iter()
                    .map(|reference| {
                        AssetDependency::required(
                            "objectstream.asset_reference",
                            AssetDependencyTarget::Asset(reference),
                        )
                    })
                    .collect(),
            );
        }

        match AssetFormat::from_path(path) {
            AssetFormat::CharacterDefinition => {
                let xml = str::from_utf8(bytes)?;
                let definition = cry_character::CharacterDefinition::from_xml(xml)?;
                self.register_character_material_usage(&definition, parent_required);
                let mut dependencies = definition.asset_dependencies();
                if definition.model.params_override.is_none() {
                    let params = replace_extension(&definition.model.skeleton, "chrparams");
                    if self.source.contains(&params) {
                        dependencies.push(AssetDependency::optional_path(
                            "character.default_parameters",
                            params,
                        ));
                    }
                }
                Ok(dependencies)
            }
            AssetFormat::CharacterParameters => {
                let xml = str::from_utf8(bytes)?;
                let parameters = cry_character::CharacterParameters::from_xml(xml)?;
                self.register_animation_aliases(&parameters)?;
                Ok(parameters.asset_dependencies())
            }
            AssetFormat::Material => {
                let material = str::from_utf8(bytes)?.parse::<nw_model::MaterialSet>()?;
                Ok(material.asset_dependencies())
            }
            AssetFormat::AnimationEvents => {
                let events =
                    cry_animation::AnimationEventDatabase::from_xml(str::from_utf8(bytes)?)?;
                Ok(events.asset_dependencies())
            }
            AssetFormat::Mannequin(kind) => match kind {
                cry_mannequin::MannequinXmlKind::AnimationDatabase => Ok(
                    cry_mannequin::MannequinAnimationDatabaseSource::from_legacy(path, bytes)?
                        .asset_dependencies(),
                ),
                cry_mannequin::MannequinXmlKind::Actions
                | cry_mannequin::MannequinXmlKind::Tags => Ok(
                    cry_mannequin::MannequinTagDefinitionSource::from_legacy(path, bytes)?
                        .asset_dependencies(),
                ),
                cry_mannequin::MannequinXmlKind::ControllerDefinition => Ok(
                    cry_mannequin::MannequinControllerDefinitionSource::from_legacy(path, bytes)?
                        .asset_dependencies(),
                ),
            },
            AssetFormat::BlendSpace => Ok(cry_mannequin::BlendSpaceDocumentSource::from_legacy(
                path, bytes,
            )?
            .asset_dependencies()),
            AssetFormat::AudioControls => {
                let xml = str::from_utf8(bytes)?;
                match cry_audio::AudioControlsSource::from_xml(path, xml) {
                    Ok(controls) => Ok(controls.asset_dependencies()),
                    Err(_) => Ok(Vec::new()),
                }
            }
            AssetFormat::CryModel => self.cry_model_dependencies(path, bytes),
            AssetFormat::Texture => self.texture_dependencies(path),
            AssetFormat::Leaf => Ok(Vec::new()),
        }
    }

    fn register_animation_aliases(
        &mut self,
        parameters: &cry_character::CharacterParameters,
    ) -> Result<()> {
        use cry_character::CharacterAnimationEntryKind;

        let mut directory = String::new();
        for entry in &parameters.animations {
            let path = normalize_virtual_path(entry.path.trim_start_matches(['/', '\\']));
            match entry.kind {
                CharacterAnimationEntryKind::FilePath => directory = path,
                CharacterAnimationEntryKind::Asset if !entry.name.starts_with(['#', '$']) => {
                    let path = if directory.is_empty() || path.contains('/') {
                        path
                    } else {
                        format!("{directory}/{path}")
                    };
                    self.register_animation_alias(&entry.name, path);
                }
                CharacterAnimationEntryKind::WildcardAsset => {
                    let pattern = if directory.is_empty() || path.contains('/') {
                        path
                    } else {
                        format!("{directory}/{path}")
                    };
                    for path in self.source.matching_paths(&pattern)? {
                        if let Some(alias) = wildcard_animation_alias(&entry.name, &path) {
                            self.register_animation_alias(&alias, normalize_virtual_path(path));
                        }
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn register_animation_alias(&mut self, alias: &str, path: String) {
        self.animation_aliases
            .entry(alias.to_ascii_lowercase())
            .or_default()
            .insert(path);
    }

    fn animation_alias_paths(&self, symbol: &str) -> Option<BTreeSet<String>> {
        self.animation_aliases
            .get(&symbol.to_ascii_lowercase())
            .cloned()
    }

    fn register_character_material_usage(
        &mut self,
        definition: &cry_character::CharacterDefinition,
        required: bool,
    ) {
        let skeleton = normalize_virtual_path(&definition.model.skeleton);
        if definition.model.material.is_some() {
            self.model_material_overrides.insert(skeleton);
        } else if required {
            self.require_default_model_material(&skeleton);
        }

        for attachment in &definition.attachments {
            let Some(binding) = attachment.binding.as_deref() else {
                continue;
            };
            let binding = normalize_virtual_path(binding);
            if attachment.material.is_some() {
                self.model_material_overrides.insert(binding);
            } else if required {
                self.require_default_model_material(&binding);
            }
        }
    }

    fn require_default_model_material(&mut self, path: &str) {
        let path = normalize_virtual_path(path);
        if !self.model_default_material_required.insert(path.clone()) {
            return;
        }
        for edge in &mut self.edges {
            if edge.source.eq_ignore_ascii_case(&path) && edge.relation == "cry_model.material" {
                edge.required = true;
            }
        }
        for unresolved in &mut self.unresolved {
            if unresolved.source.eq_ignore_ascii_case(&path)
                && unresolved.relation == "cry_model.material"
            {
                unresolved.required = true;
            }
        }
    }

    fn cry_model_dependencies(&self, path: &str, bytes: &[u8]) -> Result<Vec<AssetDependency>> {
        let file = cry_chunk::CgfFile::parse(bytes)?;
        let mut dependencies = Vec::new();
        let path = normalize_virtual_path(path);
        let material_is_required = self.model_default_material_required.contains(&path)
            || !self.model_material_overrides.contains(&path);
        for material in file.materials().values() {
            let authored = material.name.as_str().trim();
            if authored.is_empty() {
                continue;
            }
            if let Some(resolved) = self.source.legacy_material_path(authored) {
                dependencies.push(model_material_dependency(resolved, material_is_required));
                continue;
            }
            let mut material_path = normalize_virtual_path(authored);
            if extension(&material_path).is_empty() {
                material_path.push_str(".mtl");
            }
            // Bare MtlName values are engine search names, not unique asset
            // identities. Include an unambiguous match when one exists, but a
            // missing/ambiguous search name falls back to Cry's default material.
            let required = material_is_required && authored.contains(['/', '\\']);
            dependencies.push(model_material_dependency(material_path, required));
        }
        let heap = format!("{path}heap");
        if self.source.contains(&heap) {
            dependencies.push(AssetDependency::required_path("cry_model.heap", heap));
        }
        Ok(dependencies)
    }

    fn texture_dependencies(&self, path: &str) -> Result<Vec<AssetDependency>> {
        let pattern = format!("{}.*", normalize_virtual_path(path));
        let sidecars = self.source.matching_paths(&pattern)?;
        Ok(sidecars
            .into_iter()
            .filter(|sidecar| is_texture_sidecar(path, sidecar))
            .map(|sidecar| AssetDependency::required_path("texture.streaming_part", sidecar))
            .collect())
    }

    fn resolve_dependency(
        &mut self,
        source: &str,
        dependency: AssetDependency,
        parent_required: bool,
    ) -> Result<()> {
        let required = parent_required && dependency.is_required();
        match dependency.target() {
            AssetDependencyTarget::Asset(reference) => {
                self.resolve_asset_reference(source, &dependency, reference, required)
            }
            AssetDependencyTarget::PathPattern(pattern) => {
                let paths = self.source.matching_paths(pattern)?;
                if paths.is_empty() {
                    self.push_unresolved(
                        source,
                        &dependency,
                        UnresolvedReason::PatternMatchedNothing,
                        required,
                    );
                } else {
                    for path in paths {
                        self.add_resolved(
                            source,
                            &dependency,
                            normalize_virtual_path(path),
                            required,
                        );
                    }
                }
                Ok(())
            }
            AssetDependencyTarget::Symbol(symbol) => match self.animation_alias_paths(symbol) {
                Some(paths) if paths.len() == 1 => {
                    let path = paths.first().expect("one alias path").clone();
                    self.resolve_path_target(source, &dependency, &path, required)
                }
                Some(_) => {
                    self.push_unresolved(
                        source,
                        &dependency,
                        UnresolvedReason::AmbiguousSymbol,
                        required,
                    );
                    Ok(())
                }
                None => {
                    self.pending_symbols.push(PendingSymbol {
                        source: source.to_owned(),
                        dependency,
                        required,
                    });
                    Ok(())
                }
            },
        }
    }

    fn resolve_asset_reference(
        &mut self,
        source: &str,
        dependency: &AssetDependency,
        reference: &AssetReference,
        required: bool,
    ) -> Result<()> {
        if !reference.asset_id.is_nil() {
            if let Some(path) = self.source.path_by_id(reference.asset_id) {
                self.add_resolved(source, dependency, normalize_virtual_path(path), required);
                return Ok(());
            }
            if reference.hint().is_none() {
                self.push_unresolved(
                    source,
                    dependency,
                    UnresolvedReason::MissingIdentity,
                    required,
                );
                return Ok(());
            }
        }
        let Some(path) = reference.hint() else {
            self.push_unresolved(
                source,
                dependency,
                UnresolvedReason::MissingIdentity,
                required,
            );
            return Ok(());
        };
        self.resolve_path_target(source, dependency, path, required)
    }

    fn resolve_path_target(
        &mut self,
        source_path: &str,
        dependency: &AssetDependency,
        authored_path: &str,
        required: bool,
    ) -> Result<()> {
        let authored_path = normalize_virtual_path(authored_path);
        let candidates = path_candidates(source_path, &authored_path);
        for candidate in &candidates {
            if self.source.contains(candidate) {
                self.add_resolved(source_path, dependency, candidate.clone(), required);
                return Ok(());
            }
        }

        if extension(&authored_path).eq_ignore_ascii_case("tif") {
            let dds = replace_extension(&authored_path, "dds");
            if self.source.contains(&dds) {
                self.add_resolved(source_path, dependency, dds, required);
                return Ok(());
            }
        }

        if !authored_path.contains('/') {
            let matches = self.source.matching_paths(&format!("**/{authored_path}"))?;
            match matches.as_slice() {
                [path] => {
                    self.add_resolved(
                        source_path,
                        dependency,
                        normalize_virtual_path(path),
                        required,
                    );
                    return Ok(());
                }
                [] => {}
                _ => {
                    self.push_unresolved(
                        source_path,
                        dependency,
                        UnresolvedReason::AmbiguousPath,
                        required,
                    );
                    return Ok(());
                }
            }
        }

        self.push_unresolved(
            source_path,
            dependency,
            UnresolvedReason::MissingAsset,
            required,
        );
        Ok(())
    }

    fn add_resolved(
        &mut self,
        source: &str,
        dependency: &AssetDependency,
        target: String,
        required: bool,
    ) {
        if matches!(AssetFormat::from_path(&target), AssetFormat::CryModel)
            && !dependency.relation().starts_with("character.")
            && required
        {
            self.require_default_model_material(&target);
        }
        let key = (
            source.to_ascii_lowercase(),
            target.to_ascii_lowercase(),
            dependency.relation().to_owned(),
        );
        if self.edge_keys.insert(key) {
            self.edges.push(AssetDependencyEdge {
                source: source.to_owned(),
                target: target.clone(),
                relation: dependency.relation().to_owned(),
                required,
            });
        } else if required
            && let Some(edge) = self.edges.iter_mut().find(|edge| {
                edge.source.eq_ignore_ascii_case(source)
                    && edge.target.eq_ignore_ascii_case(&target)
                    && edge.relation == dependency.relation()
            })
        {
            edge.required = true;
        }
        self.enqueue(target, required);
    }

    fn enqueue(&mut self, path: String, required: bool) {
        if self.assets.insert(path.clone()) {
            self.asset_required.insert(path.clone(), required);
            self.queue.push_back(path);
        } else if required && !self.asset_required.get(&path).copied().unwrap_or(false) {
            self.asset_required.insert(path.clone(), true);
            self.queue.push_back(path);
        }
    }

    fn push_unresolved(
        &mut self,
        source: &str,
        dependency: &AssetDependency,
        reason: UnresolvedReason,
        required: bool,
    ) {
        let key = (
            source.to_ascii_lowercase(),
            dependency.target().to_string().to_ascii_lowercase(),
            dependency.relation().to_owned(),
        );
        if self.unresolved_keys.insert(key) {
            self.unresolved.push(UnresolvedDependency {
                source: source.to_owned(),
                target: dependency.target().clone(),
                relation: dependency.relation().to_owned(),
                required,
                reason,
            });
        } else if required
            && let Some(unresolved) = self.unresolved.iter_mut().find(|unresolved| {
                unresolved.source.eq_ignore_ascii_case(source)
                    && unresolved
                        .target
                        .to_string()
                        .eq_ignore_ascii_case(&dependency.target().to_string())
                    && unresolved.relation == dependency.relation()
            })
        {
            unresolved.required = true;
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AssetFormat {
    CharacterDefinition,
    CharacterParameters,
    Material,
    AnimationEvents,
    Mannequin(cry_mannequin::MannequinXmlKind),
    BlendSpace,
    AudioControls,
    CryModel,
    Texture,
    Leaf,
}

impl AssetFormat {
    fn from_path(path: &str) -> Self {
        if let Some(kind) = cry_mannequin::MannequinXmlKind::from_source_path(path) {
            return Self::Mannequin(kind);
        }
        if cry_mannequin::BlendSpaceXmlKind::from_source_path(path).is_some() {
            return Self::BlendSpace;
        }
        match extension(path).as_str() {
            "cdf" => Self::CharacterDefinition,
            "chrparams" => Self::CharacterParameters,
            "mtl" => Self::Material,
            "animevents" => Self::AnimationEvents,
            "xml" => Self::AudioControls,
            "cgf" | "cga" | "chr" | "skin" => Self::CryModel,
            "dds" => Self::Texture,
            _ => Self::Leaf,
        }
    }
}

fn path_candidates(source_path: &str, authored_path: &str) -> Vec<String> {
    let mut candidates = vec![authored_path.to_owned()];
    if let Some((directory, _)) = source_path.rsplit_once('/') {
        let relative = normalize_virtual_path(format!("{directory}/{authored_path}"));
        if relative != authored_path {
            candidates.push(relative);
        }
    }
    candidates
}

fn is_texture_sidecar(texture: &str, candidate: &str) -> bool {
    let Some(suffix) = candidate.strip_prefix(texture) else {
        return false;
    };
    suffix == ".a"
        || suffix
            .strip_prefix('.')
            .is_some_and(|value| value.parse::<u32>().is_ok())
        || suffix
            .strip_prefix(".a.")
            .is_some_and(|value| value.parse::<u32>().is_ok())
}

fn extension(path: &str) -> String {
    Path::new(path)
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
}

fn replace_extension(path: &str, extension: &str) -> String {
    let Some((stem, _)) = path.rsplit_once('.') else {
        return format!("{path}.{extension}");
    };
    format!("{stem}.{extension}")
}

fn model_material_dependency(path: String, required: bool) -> AssetDependency {
    if required {
        AssetDependency::required_path("cry_model.material", path)
    } else {
        AssetDependency::optional_path("cry_model.material", path)
    }
}

fn wildcard_animation_alias(template: &str, path: &str) -> Option<String> {
    let stem = Path::new(path).file_stem()?.to_str()?;
    Some(if let Some((prefix, suffix)) = template.split_once('*') {
        format!("{prefix}{stem}{suffix}")
    } else {
        format!("{template}{stem}")
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[derive(Default)]
    struct MemorySource {
        assets: BTreeMap<String, Vec<u8>>,
        ids: HashMap<AssetId, String>,
    }

    impl MemorySource {
        fn with(mut self, path: &str, bytes: impl Into<Vec<u8>>) -> Self {
            self.assets
                .insert(normalize_virtual_path(path), bytes.into());
            self
        }
    }

    impl AssetSource for MemorySource {
        fn read(&self, path: &str) -> Option<Vec<u8>> {
            self.assets.get(&normalize_virtual_path(path)).cloned()
        }

        fn matching_paths(&self, pattern: &str) -> Result<Vec<String>> {
            if let Some(suffix) = pattern.strip_prefix("**/") {
                return Ok(self
                    .assets
                    .keys()
                    .filter(|path| path.rsplit('/').next() == Some(suffix))
                    .cloned()
                    .collect());
            }
            if let Some(prefix) = pattern.strip_suffix(".*") {
                return Ok(self
                    .assets
                    .keys()
                    .filter(|path| path.starts_with(prefix))
                    .cloned()
                    .collect());
            }
            if let Some(wildcard) = pattern.find(['*', '?']) {
                let prefix = &pattern[..wildcard];
                let suffix = pattern[wildcard..]
                    .rfind('.')
                    .map_or("", |extension| &pattern[wildcard + extension..]);
                return Ok(self
                    .assets
                    .keys()
                    .filter(|path| path.starts_with(prefix) && path.ends_with(suffix))
                    .cloned()
                    .collect());
            }
            Ok(Vec::new())
        }

        fn path_by_id(&self, asset_id: AssetId) -> Option<String> {
            self.ids.get(&asset_id).cloned()
        }
    }

    #[test]
    fn resolves_character_material_texture_and_streaming_parts() {
        let source = MemorySource::default()
            .with(
                "objects/hero.cdf",
                br#"<CharacterDefinition><Model File="objects/hero.chr" Material="objects/hero.mtl"/></CharacterDefinition>"#,
            )
            .with("objects/hero.chr", minimal_cgf())
            .with(
                "objects/hero.mtl",
                br#"<Material><Textures><Texture Map="Diffuse" File="textures/hero.tif"/></Textures></Material>"#,
            )
            .with("textures/hero.dds", b"dds".to_vec())
            .with("textures/hero.dds.1", b"mip".to_vec());

        let graph = resolve(&source, "objects/hero.cdf", &ResolveOptions::default()).unwrap();

        assert!(graph.is_complete());
        assert!(
            graph
                .assets()
                .iter()
                .any(|path| path == "textures/hero.dds")
        );
        assert!(
            graph
                .assets()
                .iter()
                .any(|path| path == "textures/hero.dds.1")
        );
    }

    #[test]
    fn required_missing_assets_make_the_closure_incomplete() {
        let source = MemorySource::default().with(
            "objects/hero.cdf",
            br#"<CharacterDefinition><Model File="objects/missing.chr"/></CharacterDefinition>"#,
        );

        let graph = resolve(&source, "objects/hero.cdf", &ResolveOptions::default()).unwrap();

        assert!(!graph.is_complete());
        assert_eq!(
            graph.unresolved()[0].reason(),
            UnresolvedReason::MissingAsset
        );
    }

    #[test]
    fn wildcard_animation_masks_register_file_stem_aliases() {
        let source = MemorySource::default()
            .with(
                "objects/hero.chrparams",
                br##"<Params><AnimationList>
                    <Animation name="#filepath" path="animations/hero"/>
                    <Animation name="*" path="*/*.caf"/>
                    <Animation name="*" path="*/*.bspace"/>
                </AnimationList></Params>"##,
            )
            .with("animations/hero/moves/walk.caf", b"caf".to_vec())
            .with(
                "animations/hero/moves/locomotion.bspace",
                br#"<ParaGroup>
                    <Dimensions><Param name="MoveSpeed" min="0" max="1" cells="2"/></Dimensions>
                    <ExampleList><Example AName="walk" SetPara0="0"/></ExampleList>
                </ParaGroup>"#,
            );

        let graph = resolve(
            &source,
            "objects/hero.chrparams",
            &ResolveOptions::default(),
        )
        .unwrap();

        assert!(graph.is_complete(), "{:?}", graph.unresolved());
        assert!(graph.edges().iter().any(|edge| {
            edge.source() == "animations/hero/moves/locomotion.bspace"
                && edge.target() == "animations/hero/moves/walk.caf"
                && !edge.is_required()
        }));
    }

    #[test]
    fn optional_wildcard_reachability_propagates_but_explicit_roots_fail_closed() {
        let blend_space = br#"<ParaGroup>
            <Dimensions><Param name="MoveSpeed" min="0" max="1" cells="2"/></Dimensions>
            <ExampleList><Example AName="missing" SetPara0="0"/></ExampleList>
        </ParaGroup>"#;
        let source = MemorySource::default()
            .with(
                "objects/hero.chrparams",
                br##"<Params><AnimationList>
                    <Animation name="#filepath" path="animations/hero"/>
                    <Animation name="*" path="*/*.bspace"/>
                </AnimationList></Params>"##,
            )
            .with("animations/hero/moves/locomotion.bspace", blend_space);

        let character = resolve(
            &source,
            "objects/hero.chrparams",
            &ResolveOptions::default(),
        )
        .unwrap();
        assert!(character.is_complete());
        assert!(
            character
                .unresolved()
                .iter()
                .all(|dependency| !dependency.is_required())
        );

        let explicit = resolve(
            &source,
            "animations/hero/moves/locomotion.bspace",
            &ResolveOptions::default(),
        )
        .unwrap();
        assert!(!explicit.is_complete());
        assert!(explicit.unresolved()[0].is_required());
    }

    #[test]
    fn wildcard_alias_masks_preserve_prefix_and_suffix() {
        assert_eq!(
            wildcard_animation_alias("pre_*_post", "animations/move/walk.caf").as_deref(),
            Some("pre_walk_post")
        );
    }

    fn minimal_cgf() -> Vec<u8> {
        let mut bytes = b"CrCh".to_vec();
        bytes.extend_from_slice(&0x746_u32.to_le_bytes());
        bytes.extend_from_slice(&0_u32.to_le_bytes());
        bytes.extend_from_slice(&16_u32.to_le_bytes());
        bytes
    }
}
