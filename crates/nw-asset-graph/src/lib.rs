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
use std::sync::LazyLock;

use anyhow::{Context, Result, bail};
use nw_asset::{
    AssetDependencies, AssetDependency, AssetDependencyTarget, AssetId, AssetReference, AssetType,
    normalize_virtual_path,
};

static OBJECTSTREAM_LOOKUP: LazyLock<nw_objectstream::lookup::NameLookup> = LazyLock::new(|| {
    nw_objectstream::lookup::NameLookup::from_serialize_json(nw_resources::SERIALIZE_JSON)
        .expect("bundled serialize.json must build the ObjectStream lookup")
});

/// Read-only asset access required by dependency resolution.
pub trait AssetSource: Sync {
    fn read(&self, path: &str) -> Option<Vec<u8>>;

    fn contains(&self, path: &str) -> bool {
        self.read(path).is_some()
    }

    fn matching_paths(&self, pattern: &str) -> Result<Vec<String>>;

    /// Enumerate authored products with one of the supplied extensions.
    ///
    /// Install-backed sources should override this with their catalog/TOC
    /// index. The wildcard fallback keeps filesystem and test sources small
    /// while preserving the same dependency-index API.
    fn paths_with_extensions(&self, extensions: &[&str]) -> Result<Vec<String>> {
        let mut paths = BTreeSet::new();
        for extension in extensions {
            for path in
                self.matching_paths(&format!("**/*.{}", extension.trim_start_matches('.')))?
            {
                paths.insert(normalize_virtual_path(path));
            }
        }
        Ok(paths.into_iter().collect())
    }

    fn path_by_id(&self, _asset_id: AssetId) -> Option<String> {
        None
    }

    /// Resolve the GUID-like legacy name stored by Cry's `MtlName` chunk.
    fn legacy_material_path(&self, _name: &str) -> Option<String> {
        None
    }
}

/// Authored formats which may carry references to other products.
///
/// This intentionally excludes opaque leaf payloads such as CAF/DBA/WEM. It
/// includes model/material formats because their material, heap, and texture
/// relations are useful to exporters other than the model command too.
pub const DEPENDENCY_INDEX_EXTENSIONS: &[&str] = &[
    "cdf",
    "chrparams",
    "mtl",
    "animevents",
    "adb",
    "bspace",
    "comb",
    "xml",
    "cgf",
    "cga",
    "chr",
    "skin",
    "dds",
    "rnr",
    "cloth",
    "clothmaterial",
    "vshapec",
    "collisionfilters",
    "physicsmaterialset",
    "heightmap",
    "surfacemap",
    "waterqt",
    "regionmat",
    "worldmat",
    "tractmap.tif",
    "terrain.json",
    "tracts.json",
    "terrain",
    "distribution",
    "vegetation",
    "vegimage",
    "slice",
    "dynamicslice",
    "entity",
    "entities",
    "entities_xml",
    "prefab",
    "slicedata",
    "metadata",
    "chunks",
    "meta",
    "csv",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetDependencyEdge {
    source: String,
    target: String,
    relation: String,
    required: bool,
    association: Option<String>,
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

    /// Path selected by the same authored record, when this edge is an
    /// associated sidecar rather than an independent dependency.
    #[must_use]
    pub fn association(&self) -> Option<&str> {
        self.association.as_deref()
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
    association: Option<String>,
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
    pub fn association(&self) -> Option<&str> {
        self.association.as_deref()
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

/// Reusable direct authored-dependency index.
///
/// Unlike [`AssetDependencyGraph`], this is not tied to one root closure. It is
/// built once from independent source assets, then serves both forward and
/// reverse queries to model, world, audio, terrain, and other exporters.
#[derive(Debug, Clone, Default)]
pub struct AssetDependencyIndex {
    edges: Vec<AssetDependencyEdge>,
    unresolved: Vec<UnresolvedDependency>,
    forward: HashMap<String, Vec<usize>>,
    reverse: HashMap<String, Vec<usize>>,
}

impl AssetDependencyIndex {
    /// Build an index over every dependency-bearing format exposed by `source`.
    pub fn discover(source: &dyn AssetSource) -> Result<Self> {
        Self::discover_with_runner(source, &nw_jobs::JobRunner::automatic())
    }

    /// Build an index using the caller's worker policy.
    pub fn discover_with_runner(
        source: &dyn AssetSource,
        runner: &nw_jobs::JobRunner,
    ) -> Result<Self> {
        let paths = source.paths_with_extensions(DEPENDENCY_INDEX_EXTENSIONS)?;
        Self::build_with_runner(source, &paths, runner)
    }

    /// Build an index over an explicit set of authored source paths.
    pub fn build_with_runner(
        source: &dyn AssetSource,
        paths: &[String],
        runner: &nw_jobs::JobRunner,
    ) -> Result<Self> {
        let paths = paths
            .iter()
            .map(normalize_virtual_path)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let graphs = runner.try_map(&paths, |path| {
            inspect_direct(source, path)
                .with_context(|| format!("inspect direct dependencies from {path}"))
        })?;

        let mut edge_keys = BTreeSet::new();
        let mut unresolved_keys = BTreeSet::new();
        let mut edges = Vec::new();
        let mut unresolved = Vec::new();
        for graph in graphs {
            for edge in graph.edges {
                let key = (
                    edge.source.to_ascii_lowercase(),
                    edge.target.to_ascii_lowercase(),
                    edge.relation.clone(),
                    edge.association.clone(),
                );
                if edge_keys.insert(key) {
                    edges.push(edge);
                }
            }
            for dependency in graph.unresolved {
                let key = (
                    dependency.source.to_ascii_lowercase(),
                    dependency.target.to_string().to_ascii_lowercase(),
                    dependency.relation.clone(),
                    dependency.association.clone(),
                );
                if unresolved_keys.insert(key) {
                    unresolved.push(dependency);
                }
            }
        }
        edges.sort_by(|left, right| {
            left.source
                .cmp(&right.source)
                .then_with(|| left.target.cmp(&right.target))
                .then_with(|| left.relation.cmp(&right.relation))
                .then_with(|| left.association.cmp(&right.association))
        });
        unresolved.sort_by(|left, right| {
            left.source
                .cmp(&right.source)
                .then_with(|| left.target.to_string().cmp(&right.target.to_string()))
                .then_with(|| left.relation.cmp(&right.relation))
                .then_with(|| left.association.cmp(&right.association))
        });

        let mut index = Self {
            edges,
            unresolved,
            forward: HashMap::new(),
            reverse: HashMap::new(),
        };
        for (edge_index, edge) in index.edges.iter().enumerate() {
            index
                .forward
                .entry(edge.source.to_ascii_lowercase())
                .or_default()
                .push(edge_index);
            index
                .reverse
                .entry(edge.target.to_ascii_lowercase())
                .or_default()
                .push(edge_index);
        }
        Ok(index)
    }

    #[must_use]
    pub fn edges(&self) -> &[AssetDependencyEdge] {
        &self.edges
    }

    #[must_use]
    pub fn unresolved(&self) -> &[UnresolvedDependency] {
        &self.unresolved
    }

    pub fn dependencies_of<'a>(
        &'a self,
        source: &str,
    ) -> impl Iterator<Item = &'a AssetDependencyEdge> + 'a {
        self.forward
            .get(&normalize_virtual_path(source).to_ascii_lowercase())
            .into_iter()
            .flatten()
            .map(|index| &self.edges[*index])
    }

    pub fn consumers_of<'a>(
        &'a self,
        target: &str,
    ) -> impl Iterator<Item = &'a AssetDependencyEdge> + 'a {
        self.reverse
            .get(&normalize_virtual_path(target).to_ascii_lowercase())
            .into_iter()
            .flatten()
            .map(|index| &self.edges[*index])
    }

    /// Return every transitive authored consumer of `target`, nearest-first.
    #[must_use]
    pub fn transitive_consumers_of(&self, target: &str) -> Vec<String> {
        let target = normalize_virtual_path(target);
        let mut seen = BTreeSet::from([target.to_ascii_lowercase()]);
        let mut queue = VecDeque::from([target]);
        let mut consumers = Vec::new();
        while let Some(current) = queue.pop_front() {
            for edge in self.consumers_of(&current) {
                let key = edge.source.to_ascii_lowercase();
                if seen.insert(key) {
                    consumers.push(edge.source.clone());
                    queue.push_back(edge.source.clone());
                }
            }
        }
        consumers
    }

    /// Return the transitive reverse projection from `target`, following only
    /// consumer edges accepted by `include`.
    ///
    /// This is the reverse counterpart to [`Self::transitive_dependencies_where`].
    /// It lets exporters cross explicit ownership wrappers without accidentally
    /// walking from one asset instance into every world, region, or encounter
    /// that happens to consume it.
    #[must_use]
    pub fn transitive_consumers_where<F>(&self, target: &str, mut include: F) -> Vec<String>
    where
        F: FnMut(&AssetDependencyEdge) -> bool,
    {
        let target = normalize_virtual_path(target);
        let mut seen = BTreeSet::from([target.to_ascii_lowercase()]);
        let mut queue = VecDeque::from([target]);
        let mut consumers = Vec::new();
        while let Some(current) = queue.pop_front() {
            for edge in self.consumers_of(&current) {
                if !include(edge) {
                    continue;
                }
                let key = edge.source.to_ascii_lowercase();
                if seen.insert(key) {
                    consumers.push(edge.source.clone());
                    queue.push_back(edge.source.clone());
                }
            }
        }
        consumers
    }

    /// Return associated dependencies authored by direct consumers of
    /// `target`.
    ///
    /// This models formats where one record selects both a primary asset and
    /// associated metadata. For example, vegetation and slice spawners point to
    /// a dynamic slice and its exact variant sidecar as correlated edges.
    /// Exporters can recover only sidecars associated with `target`, even when
    /// one consumer contains many independent records.
    #[must_use]
    pub fn associated_dependencies_from_consumers_where<F>(
        &self,
        target: &str,
        mut include_dependency: F,
    ) -> Vec<String>
    where
        F: FnMut(&AssetDependencyEdge) -> bool,
    {
        let target = normalize_virtual_path(target);
        let mut seen = BTreeSet::new();
        let mut dependencies = Vec::new();
        for consumer in self.consumers_of(&target) {
            for dependency in self.dependencies_of(consumer.source()).filter(|edge| {
                edge.association()
                    .is_some_and(|association| association.eq_ignore_ascii_case(&target))
                    && include_dependency(edge)
            }) {
                let key = dependency.target.to_ascii_lowercase();
                if seen.insert(key) {
                    dependencies.push(dependency.target.clone());
                }
            }
        }
        dependencies
    }

    /// Return the transitive forward projection from `roots`, following only
    /// edges accepted by `include`.
    ///
    /// Exporters use this to project one concern (for example render/physics
    /// context) from the shared authored graph without treating every sibling
    /// dependency of a consumer as another required root.
    #[must_use]
    pub fn transitive_dependencies_where<'a, I, F>(
        &'a self,
        roots: I,
        mut include: F,
    ) -> Vec<String>
    where
        I: IntoIterator<Item = &'a str>,
        F: FnMut(&AssetDependencyEdge) -> bool,
    {
        let mut seen = BTreeSet::new();
        let mut queue = VecDeque::new();
        for root in roots {
            let root = normalize_virtual_path(root);
            if seen.insert(root.to_ascii_lowercase()) {
                queue.push_back(root);
            }
        }

        let mut dependencies = Vec::new();
        while let Some(current) = queue.pop_front() {
            for edge in self.dependencies_of(&current) {
                if !include(edge) {
                    continue;
                }
                let key = edge.target.to_ascii_lowercase();
                if seen.insert(key) {
                    dependencies.push(edge.target.clone());
                    queue.push_back(edge.target.clone());
                }
            }
        }
        dependencies
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

/// Inspect and resolve only the references authored directly by `root`.
///
/// The target assets are not parsed. This makes independent roots safe to
/// process concurrently when constructing [`AssetDependencyIndex`].
pub fn inspect_direct(source: &dyn AssetSource, root: &str) -> Result<AssetDependencyGraph> {
    let runner = nw_jobs::JobRunner::inline();
    let mut resolver = Resolver::new(source, &runner);
    resolver.add_root(root)?;
    resolver.finish_direct()
}

struct Resolver<'a> {
    source: &'a dyn AssetSource,
    runner: &'a nw_jobs::JobRunner,
    roots: Vec<String>,
    assets: BTreeSet<String>,
    asset_required: HashMap<String, bool>,
    queue: VecDeque<String>,
    edges: Vec<AssetDependencyEdge>,
    edge_keys: BTreeSet<(String, String, String, Option<String>)>,
    unresolved: Vec<UnresolvedDependency>,
    unresolved_keys: BTreeSet<(String, String, String, Option<String>)>,
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
                .then_with(|| left.association.cmp(&right.association))
        });
        self.unresolved.sort_by(|left, right| {
            left.source
                .cmp(&right.source)
                .then_with(|| left.target.to_string().cmp(&right.target.to_string()))
                .then_with(|| left.relation.cmp(&right.relation))
                .then_with(|| left.association.cmp(&right.association))
        });
        Ok(AssetDependencyGraph {
            roots: self.roots,
            assets: self.assets.into_iter().collect(),
            edges: self.edges,
            unresolved: self.unresolved,
        })
    }

    fn finish_direct(mut self) -> Result<AssetDependencyGraph> {
        let root = self
            .queue
            .pop_front()
            .context("direct dependency resolver root queue is empty")?;
        let bytes = self
            .source
            .read(&root)
            .with_context(|| format!("read dependency asset {root}"))?;
        let dependencies = self
            .extract(&root, &bytes, true)
            .with_context(|| format!("extract dependencies from {root}"))?;
        for dependency in dependencies {
            self.resolve_dependency(&root, dependency, true)?;
        }
        self.queue.clear();
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
                .then_with(|| left.association.cmp(&right.association))
        });
        self.unresolved.sort_by(|left, right| {
            left.source
                .cmp(&right.source)
                .then_with(|| left.target.to_string().cmp(&right.target.to_string()))
                .then_with(|| left.relation.cmp(&right.relation))
                .then_with(|| left.association.cmp(&right.association))
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
        let format = AssetFormat::from_path(path);

        // Known ObjectStream-backed products must go through their owning
        // format parser before the generic Asset-reference visitor. The latter
        // deliberately understands only reference envelopes and therefore is
        // not sufficient validation for RNR/collision/physics products.
        match format {
            AssetFormat::RockNRollShape => {
                nw_rnr::legacy::parse_shape_asset(bytes)?;
                return Ok(Vec::new());
            }
            AssetFormat::ClothFabric => {
                let fabric = nv_cloth_assets::parse_cloth_fabric(bytes)?;
                let mut dependencies = vec![AssetDependency::required_path(
                    "cloth.render_model",
                    fabric.render_model,
                )];
                if let Some(material) = fabric.material {
                    dependencies.push(AssetDependency::required_path("cloth.material", material));
                }
                return Ok(dependencies);
            }
            AssetFormat::ClothMaterial => {
                nv_cloth_assets::parse_cloth_material(bytes)?;
                return Ok(Vec::new());
            }
            AssetFormat::VertexShape => {
                lmbr_central_vshape::parse_vertex_shape_asset(bytes)?;
                return Ok(Vec::new());
            }
            AssetFormat::CollisionFilters => {
                az_physics_assets::CollisionFiltersAsset::parse(bytes)?;
                return Ok(Vec::new());
            }
            AssetFormat::PhysicsMaterialSet => {
                az_physics_assets::MaterialSetAsset::parse(bytes)?;
                return Ok(Vec::new());
            }
            AssetFormat::TerrainHeightmap => {
                nw_terrain::RegionHeightmap::parse_tiff(bytes)?;
                return Ok(Vec::new());
            }
            AssetFormat::TerrainSurfaceMap => {
                nw_terrain::SurfaceMap::parse(bytes)?;
                return Ok(Vec::new());
            }
            AssetFormat::TerrainMapSettings => {
                nw_terrain::MapSettings::parse_json(bytes)?;
                return Ok(Vec::new());
            }
            AssetFormat::TerrainWaterQuadtree => {
                nw_terrain::parse_water_quadtree(bytes)?;
                return Ok(Vec::new());
            }
            AssetFormat::TerrainTractMap => {
                nw_terrain::TractMap::parse_tiff(bytes)?;
                return Ok(Vec::new());
            }
            AssetFormat::TerrainRegionMaterial => {
                let material = nw_terrain::parse_region_material_data_asset(bytes)?;
                return Ok(region_material_dependencies(&material));
            }
            AssetFormat::TerrainWorldMaterial => {
                let material = nw_terrain::parse_world_material_data_asset(bytes)?;
                return Ok(world_material_dependencies(&material));
            }
            AssetFormat::TerrainSettings => {
                let settings = nw_terrain::TerrainSettings::parse_bytes(bytes)?;
                return Ok(settings
                    .world_material_asset_path
                    .filter(|path| !path.trim().is_empty())
                    .map(|path| {
                        vec![AssetDependency::required_path(
                            "terrain.settings.world_material",
                            path.into_owned(),
                        )]
                    })
                    .unwrap_or_default());
            }
            AssetFormat::TerrainTracts => {
                let document = nw_terrain::TractsDocument::parse_bytes(bytes)?;
                return Ok(terrain_tract_dependencies(self.source, &document));
            }
            AssetFormat::VegetationDistribution => {
                let distribution = nw_vegetation::distribution::Distribution::parse(bytes)?;
                let mut dependencies = Vec::new();
                for entry in distribution.entries().iter() {
                    if let Some(slice) = entry.dynamic_slice_source_path() {
                        let slice = slice.into_owned();
                        dependencies.push(AssetDependency::required_path(
                            "vegetation.distribution.slice",
                            slice.clone(),
                        ));
                        if let Some(metadata) = entry.variant_metadata_source_path() {
                            dependencies.push(
                                AssetDependency::optional_path(
                                    "vegetation.distribution.variant",
                                    metadata,
                                )
                                .associated_with_path(&slice),
                            );
                        }
                    }
                }
                return Ok(dependencies);
            }
            AssetFormat::VegetationRegion => {
                let image = nw_vegetation::VegetationImage::parse(bytes)?;
                let mut dependencies = Vec::new();
                for entry in image.asset_table()?.entries() {
                    let entry = entry?;
                    if !entry.path.trim().is_empty() {
                        dependencies.push(AssetDependency::optional_path(
                            "vegetation.region.asset",
                            entry.path.to_owned(),
                        ));
                    }
                }
                return Ok(dependencies);
            }
            AssetFormat::VegetationImage => {
                nw_vegetation::VegetationImageAsset::parse(bytes)?;
                return Ok(Vec::new());
            }
            AssetFormat::SliceMetadata => {
                let stream = nw_objectstream::ObjectStream::from_bytes(bytes, None)?;
                let metadata = nw_objectstream::slice_meta::read_slice_meta_data(&stream)?;
                return Ok(slice_metadata_dependencies(&metadata));
            }
            AssetFormat::RegionSliceData => {
                let stream = nw_objectstream::ObjectStream::from_bytes(bytes, None)?;
                let lookup = nw_objectstream::region_slice_data::read_region_slice_data(&stream)?;
                return Ok(region_slice_data_dependencies(&lookup));
            }
            AssetFormat::RegionMetadata => {
                nw_scene::parse_region_metadata(bytes, Some(&OBJECTSTREAM_LOOKUP))?;
                return Ok(Vec::new());
            }
            AssetFormat::RegionChunks => {
                let chunks = nw_scene::parse_region_chunks(bytes, Some(&OBJECTSTREAM_LOOKUP))?;
                let asset_ids = chunks
                    .chunks
                    .iter()
                    .filter(|chunk| !chunk.asset_id.is_nil())
                    .map(|chunk| {
                        AssetId::new(*chunk.asset_id.guid.as_inner(), chunk.asset_id.sub_id)
                    })
                    .collect::<BTreeSet<_>>();
                return Ok(asset_ids
                    .into_iter()
                    .map(|asset_id| {
                        asset_id_dependency("region_manifest.chunk", asset_id, None, true)
                    })
                    .collect());
            }
            AssetFormat::AudioMapping => {
                cry_audio::parse_audio_mapping(path, bytes)?;
                return Ok(Vec::new());
            }
            _ => {}
        }

        if matches!(format, AssetFormat::GenericObjectStream) {
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

        match format {
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
            AssetFormat::RockNRollShape
            | AssetFormat::ClothFabric
            | AssetFormat::ClothMaterial
            | AssetFormat::VertexShape
            | AssetFormat::CollisionFilters
            | AssetFormat::PhysicsMaterialSet
            | AssetFormat::TerrainHeightmap
            | AssetFormat::TerrainSurfaceMap
            | AssetFormat::TerrainMapSettings
            | AssetFormat::TerrainWaterQuadtree
            | AssetFormat::TerrainTractMap
            | AssetFormat::TerrainRegionMaterial
            | AssetFormat::TerrainWorldMaterial
            | AssetFormat::TerrainSettings
            | AssetFormat::TerrainTracts
            | AssetFormat::VegetationDistribution
            | AssetFormat::VegetationRegion
            | AssetFormat::VegetationImage
            | AssetFormat::SliceMetadata
            | AssetFormat::RegionSliceData
            | AssetFormat::RegionMetadata
            | AssetFormat::RegionChunks
            | AssetFormat::AudioMapping
            | AssetFormat::GenericObjectStream => {
                unreachable!("known binary products return before generic ObjectStream handling")
            }
            AssetFormat::Leaf => Ok(Vec::new()),
        }
    }

    fn register_animation_aliases(
        &mut self,
        parameters: &cry_character::CharacterParameters,
    ) -> Result<()> {
        use cry_character::{CharacterAnimationEntryKind, CharacterAnimationPathResolver};

        let mut paths = CharacterAnimationPathResolver::default();
        for entry in &parameters.animations {
            let path = normalize_virtual_path(paths.resolve_entry(entry));
            match entry.kind {
                CharacterAnimationEntryKind::FilePath => {}
                CharacterAnimationEntryKind::Asset if !entry.name.starts_with(['#', '$']) => {
                    self.register_animation_alias(&entry.name, path);
                }
                CharacterAnimationEntryKind::WildcardAsset => {
                    for path in self.source.matching_paths(&path)? {
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
            dependency.association().map(str::to_owned),
        );
        if self.edge_keys.insert(key) {
            self.edges.push(AssetDependencyEdge {
                source: source.to_owned(),
                target: target.clone(),
                relation: dependency.relation().to_owned(),
                required,
                association: dependency.association().map(str::to_owned),
            });
        } else if required
            && let Some(edge) = self.edges.iter_mut().find(|edge| {
                edge.source.eq_ignore_ascii_case(source)
                    && edge.target.eq_ignore_ascii_case(&target)
                    && edge.relation == dependency.relation()
                    && edge.association.as_deref() == dependency.association()
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
            dependency.association().map(str::to_owned),
        );
        if self.unresolved_keys.insert(key) {
            self.unresolved.push(UnresolvedDependency {
                source: source.to_owned(),
                target: dependency.target().clone(),
                relation: dependency.relation().to_owned(),
                required,
                association: dependency.association().map(str::to_owned),
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
                    && unresolved.association.as_deref() == dependency.association()
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
    RockNRollShape,
    ClothFabric,
    ClothMaterial,
    VertexShape,
    CollisionFilters,
    PhysicsMaterialSet,
    TerrainHeightmap,
    TerrainSurfaceMap,
    TerrainMapSettings,
    TerrainWaterQuadtree,
    TerrainTractMap,
    TerrainRegionMaterial,
    TerrainWorldMaterial,
    TerrainSettings,
    TerrainTracts,
    VegetationDistribution,
    VegetationRegion,
    VegetationImage,
    SliceMetadata,
    RegionSliceData,
    RegionMetadata,
    RegionChunks,
    AudioMapping,
    GenericObjectStream,
    Leaf,
}

impl AssetFormat {
    fn from_path(path: &str) -> Self {
        let lowercase = path.to_ascii_lowercase();
        let is_region_path = lowercase.starts_with("regions/") || lowercase.contains("/regions/");
        if lowercase.ends_with("mapsettings.json") {
            return Self::TerrainMapSettings;
        }
        if lowercase.ends_with("tractmap.tif") {
            return Self::TerrainTractMap;
        }
        if lowercase.ends_with("terrain.json") {
            return Self::TerrainSettings;
        }
        if lowercase.ends_with("tracts.json") {
            return Self::TerrainTracts;
        }
        if lowercase.ends_with(".slice.meta") {
            return Self::SliceMetadata;
        }
        if is_region_path && lowercase.ends_with("/region.metadata") {
            return Self::RegionMetadata;
        }
        if is_region_path && lowercase.ends_with(".chunks") {
            return Self::RegionChunks;
        }
        if cry_audio::is_audio_mapping_source(path) {
            return Self::AudioMapping;
        }
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
            "rnr" => Self::RockNRollShape,
            "cloth" => Self::ClothFabric,
            "clothmaterial" => Self::ClothMaterial,
            "vshapec" => Self::VertexShape,
            "collisionfilters" => Self::CollisionFilters,
            "physicsmaterialset" => Self::PhysicsMaterialSet,
            "heightmap" => Self::TerrainHeightmap,
            "surfacemap" => Self::TerrainSurfaceMap,
            "waterqt" => Self::TerrainWaterQuadtree,
            "regionmat" => Self::TerrainRegionMaterial,
            "worldmat" => Self::TerrainWorldMaterial,
            "terrain" => Self::TerrainSettings,
            "distribution" => Self::VegetationDistribution,
            "vegetation" => Self::VegetationRegion,
            "vegimage" => Self::VegetationImage,
            "slicedata" => Self::RegionSliceData,
            "slice" | "dynamicslice" | "entity" | "entities" | "entities_xml" | "prefab"
            | "meta" => Self::GenericObjectStream,
            _ => Self::Leaf,
        }
    }
}

fn region_material_dependencies(
    material: &nw_reflected_types::types::RegionMaterialDataAsset,
) -> Vec<AssetDependency> {
    let mut dependencies = Vec::new();
    for (relation, asset) in [
        (
            "terrain.region_material.default",
            &material.default_material,
        ),
        (
            "terrain.region_material.macro_color",
            &material.macro_color_map,
        ),
        (
            "terrain.region_material.macro_gloss",
            &material.macro_gloss_map,
        ),
        (
            "terrain.region_material.macro_normal",
            &material.macro_normal_map,
        ),
    ] {
        if let Some(dependency) = reflected_asset_dependency(relation, asset, false) {
            dependencies.push(dependency);
        }
    }
    for layer in &material.layers {
        if let Some(dependency) = reflected_asset_dependency(
            "terrain.region_material.layer.material",
            &layer.material,
            false,
        ) {
            dependencies.push(dependency);
        }
        if let Some(dependency) = reflected_asset_dependency(
            "terrain.region_material.layer.splat_map",
            &layer.splat_map,
            false,
        ) {
            dependencies.push(dependency);
        }
    }
    dependencies
}

fn world_material_dependencies(
    material: &nw_reflected_types::types::WorldMaterialDataAsset,
) -> Vec<AssetDependency> {
    material
        .regions
        .iter()
        .filter_map(|region| {
            reflected_asset_dependency("terrain.world_material.region", &region.layers, true)
        })
        .collect()
}

fn reflected_asset_dependency(
    relation: &'static str,
    asset: &nw_reflected_types::az::asset::Asset,
    required: bool,
) -> Option<AssetDependency> {
    if asset.is_empty() {
        return None;
    }
    let target = AssetDependencyTarget::Asset(AssetReference::new(
        AssetId::new(*asset.asset_id.guid.as_inner(), asset.asset_id.sub_id),
        AssetType::new(*asset.asset_type.as_inner()),
        asset.hint().map(str::to_owned),
    ));
    Some(if required {
        AssetDependency::required(relation, target)
    } else {
        AssetDependency::optional(relation, target)
    })
}

fn terrain_tract_dependencies(
    source: &dyn AssetSource,
    document: &nw_terrain::TractsDocument<'_>,
) -> Vec<AssetDependency> {
    let mut dependencies = Vec::new();
    if let Some(path) = document
        .territory_master_slice_path
        .as_deref()
        .and_then(nw_vegetation::distribution::dynamic_slice_source_path)
    {
        dependencies.push(AssetDependency::required_path(
            "terrain.tracts.territory_master_slice",
            path.into_owned(),
        ));
    }

    for tract in document
        .tracts
        .iter()
        .chain(document.regions.iter().flat_map(|region| &region.tracts))
    {
        for capital in &tract.capitals {
            push_tract_slice_dependency(
                source,
                &mut dependencies,
                "terrain.tracts.capital",
                capital.asset_id.as_deref(),
                capital.variant_id.as_deref(),
            );
        }
        for outpost in &tract.outposts {
            push_tract_slice_dependency(
                source,
                &mut dependencies,
                "terrain.tracts.outpost",
                outpost.asset_id.as_deref(),
                outpost.variant_id.as_deref(),
            );
        }
    }
    dependencies
}

fn push_tract_slice_dependency(
    source: &dyn AssetSource,
    dependencies: &mut Vec<AssetDependency>,
    relation: &'static str,
    asset: Option<&str>,
    variant: Option<&str>,
) {
    let Some(asset) = asset.map(str::trim).filter(|asset| !asset.is_empty()) else {
        return;
    };
    let (dependency, resolved_path) = if let Ok(asset_id) = asset.parse::<AssetId>() {
        (
            asset_id_dependency(relation, asset_id, None, true),
            source.path_by_id(asset_id),
        )
    } else if let Some(path) = nw_vegetation::distribution::dynamic_slice_source_path(asset) {
        let path = path.into_owned();
        (
            AssetDependency::required_path(relation, path.clone()),
            Some(path),
        )
    } else {
        return;
    };
    dependencies.push(dependency);

    if let (Some(path), Some(variant)) = (resolved_path, variant)
        && let Some(metadata) =
            nw_vegetation::distribution::slice_metadata_source_path(&path, variant)
    {
        dependencies.push(
            AssetDependency::optional_path("terrain.tracts.slice_variant", metadata)
                .associated_with_path(&path),
        );
    }
}

fn slice_metadata_dependencies(
    metadata: &nw_objectstream::slice_meta::SliceMetaData<'_>,
) -> Vec<AssetDependency> {
    let mut dependencies = Vec::new();
    for mesh in &metadata.meshes {
        dependencies.push(asset_id_dependency(
            "slice_metadata.mesh",
            mesh.mesh_asset_id,
            None,
            true,
        ));
        if !mesh.material_override_asset_id.is_nil() {
            dependencies.push(asset_id_dependency(
                "slice_metadata.material",
                mesh.material_override_asset_id,
                None,
                false,
            ));
        }
    }
    for spawner in &metadata.spawners {
        let slice_path = nw_vegetation::distribution::dynamic_slice_source_path(spawner.slice_name)
            .map(|path| path.into_owned());
        dependencies.push(asset_id_dependency(
            "slice_metadata.spawner",
            spawner.slice_asset_id,
            slice_path.as_deref(),
            true,
        ));
        if let Some(slice_path) = slice_path
            && let Some(variant) = nw_vegetation::distribution::slice_metadata_source_path(
                &slice_path,
                spawner.variation_name,
            )
        {
            dependencies.push(
                AssetDependency::optional_path("slice_metadata.spawner_variant", variant)
                    .associated_with_path(&slice_path),
            );
        }
    }
    dependencies.extend(
        metadata
            .child_spawn_slice_ids
            .iter()
            .copied()
            .map(|asset_id| {
                asset_id_dependency("slice_metadata.child_spawn", asset_id, None, true)
            }),
    );
    dependencies
}

fn region_slice_data_dependencies(
    lookup: &nw_objectstream::region_slice_data::RegionSliceDataLookup<'_>,
) -> Vec<AssetDependency> {
    let mut dependencies = Vec::new();
    for entry in &lookup.entries {
        if let Some(slice_path) =
            nw_vegetation::distribution::dynamic_slice_source_path(entry.key.slice_name)
        {
            let slice_path = slice_path.into_owned();
            dependencies.push(AssetDependency::required_path(
                "region_slice.entry",
                slice_path.clone(),
            ));
            if let Some(variant) = nw_vegetation::distribution::slice_metadata_source_path(
                &slice_path,
                entry.key.variant_name,
            ) {
                dependencies.push(
                    AssetDependency::optional_path("region_slice.entry_variant", variant)
                        .associated_with_path(&slice_path),
                );
            }
        }
        dependencies.extend(slice_metadata_dependencies(&entry.metadata));
    }
    dependencies
}

fn asset_id_dependency(
    relation: &'static str,
    asset_id: AssetId,
    hint: Option<&str>,
    required: bool,
) -> AssetDependency {
    let target = AssetDependencyTarget::Asset(AssetReference::new(
        asset_id,
        AssetType::nil(),
        hint.map(str::to_owned),
    ));
    if required {
        AssetDependency::required(relation, target)
    } else {
        AssetDependency::optional(relation, target)
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
    fn direct_index_reuses_forward_edges_for_transitive_reverse_discovery() {
        let source = MemorySource::default()
            .with(
                "objects/hero.cdf",
                br#"<CharacterDefinition><Model File="objects/hero.chr"/></CharacterDefinition>"#,
            )
            .with("objects/hero.chr", minimal_cgf())
            .with(
                "slices/hero.slice",
                br#"<ObjectStream version="3"><Class name="Asset" type="{77A19D40-8731-4D3C-9041-1B43047366A4}" value="id={7A1472D1-DF54-5362-BC71-9974D5F25572}:0,type={78802ABF-9595-463A-8D2B-D022F906F9B1},hint={objects/hero.cdf}"/></ObjectStream>"#,
            );
        let paths = vec![
            "objects/hero.cdf".to_owned(),
            "slices/hero.slice".to_owned(),
        ];
        let index =
            AssetDependencyIndex::build_with_runner(&source, &paths, &nw_jobs::JobRunner::inline())
                .unwrap();

        assert_eq!(
            index
                .dependencies_of("objects/hero.cdf")
                .map(AssetDependencyEdge::target)
                .collect::<Vec<_>>(),
            vec!["objects/hero.chr"]
        );
        assert_eq!(
            index.transitive_consumers_of("objects/hero.chr"),
            vec!["objects/hero.cdf", "slices/hero.slice"]
        );
        assert_eq!(
            index.transitive_consumers_where("objects/hero.chr", |edge| edge
                .source()
                .ends_with(".cdf")),
            vec!["objects/hero.cdf"]
        );
        assert_eq!(
            index.transitive_dependencies_where(["slices/hero.slice"], |edge| !edge
                .target()
                .ends_with(".slice")),
            vec!["objects/hero.cdf", "objects/hero.chr"]
        );
    }

    #[test]
    fn vegetation_distribution_resolves_exact_slice_variants() {
        let mut distribution = Vec::new();
        distribution.extend_from_slice(&1u16.to_le_bytes());
        push_distribution_string(&mut distribution, "gatherables/master_tree");
        push_distribution_string(&mut distribution, "Tree/A");
        distribution.extend_from_slice(&0u32.to_le_bytes());
        distribution.extend_from_slice(&0u32.to_le_bytes());
        distribution.extend_from_slice(&0u32.to_le_bytes());

        let empty_stream = nw_objectstream::ObjectStream::new(3).to_bytes();
        let empty_metadata = nw_objectstream::ObjectStream {
            elements: vec![nw_objectstream::Element::new(
                nw_objectstream::slice_meta::SLICE_META_DATA_TYPE_ID,
            )],
            ..nw_objectstream::ObjectStream::new(3)
        }
        .to_encoding_bytes(nw_objectstream::ObjectStreamEncoding::Xml)
        .unwrap();
        let source = MemorySource::default()
            .with("regions/r1/region.distribution", distribution)
            .with(
                "slices/gatherables/master_tree.dynamicslice",
                empty_stream.clone(),
            )
            .with(
                "slices/gatherables/master_tree_tree_a.slice.meta",
                empty_metadata,
            );
        let graph = resolve(
            &source,
            "regions/r1/region.distribution",
            &ResolveOptions::default(),
        )
        .unwrap();

        assert!(graph.is_complete());
        assert!(graph.edges().iter().any(|edge| {
            edge.relation() == "vegetation.distribution.slice"
                && edge.target() == "slices/gatherables/master_tree.dynamicslice"
        }));
        assert!(graph.edges().iter().any(|edge| {
            edge.relation() == "vegetation.distribution.variant"
                && edge.target() == "slices/gatherables/master_tree_tree_a.slice.meta"
                && edge.association() == Some("slices/gatherables/master_tree.dynamicslice")
        }));
        let index = AssetDependencyIndex::build_with_runner(
            &source,
            &["regions/r1/region.distribution".to_owned()],
            &nw_jobs::JobRunner::inline(),
        )
        .unwrap();
        assert_eq!(
            index.associated_dependencies_from_consumers_where(
                "slices/gatherables/master_tree.dynamicslice",
                |edge| edge.relation().ends_with("variant")
            ),
            vec!["slices/gatherables/master_tree_tree_a.slice.meta"]
        );
    }

    #[test]
    fn terrain_settings_and_tracts_resolve_world_material_and_exact_variants() {
        let source = MemorySource::default()
            .with(
                "sharedassets/coatlicue/world/terrain.json",
                br#"{"worldMaterialAssetPath":"materials/terrain/world/world.worldmat"}"#,
            )
            .with("materials/terrain/world/world.worldmat", b"present".to_vec())
            .with(
                "sharedassets/coatlicue/world/tracts.json",
                br#"{
                    "territoryMasterSlicePath":"slices/territories/master.dynamicslice",
                    "tracts":[{"capitals":[{"assetId":"slices/capital.slice","variantId":"Fall/A"}]}]
                }"#,
            )
            .with("slices/territories/master.dynamicslice", b"present".to_vec())
            .with("slices/capital.slice", b"present".to_vec())
            .with("slices/capital_fall_a.slice.meta", b"present".to_vec());

        let terrain = inspect_direct(&source, "sharedassets/coatlicue/world/terrain.json").unwrap();
        assert!(terrain.edges().iter().any(|edge| {
            edge.relation() == "terrain.settings.world_material"
                && edge.target() == "materials/terrain/world/world.worldmat"
        }));

        let tracts = inspect_direct(&source, "sharedassets/coatlicue/world/tracts.json").unwrap();
        assert!(tracts.edges().iter().any(|edge| {
            edge.relation() == "terrain.tracts.territory_master_slice"
                && edge.target() == "slices/territories/master.dynamicslice"
        }));
        assert!(tracts.edges().iter().any(|edge| {
            edge.relation() == "terrain.tracts.slice_variant"
                && edge.target() == "slices/capital_fall_a.slice.meta"
        }));
    }

    #[test]
    fn terrain_region_material_resolves_generated_asset_fields() {
        let material = br#"<ObjectStream version="3">
            <Class name="RegionMaterialDataAsset" version="3" type="{9A623978-DFB6-4CC1-A649-1F172637E52A}">
                <Class name="Asset" field="Default Material" value="id={EF39F51B-AF6A-58FD-8693-611D094138CF}:0,type={F46985B5-F7FF-4FCB-8E8C-DC240D701841},hint={materials/terrain/default.mtl}" version="1" type="{77A19D40-8731-4D3C-9041-1B43047366A4}"/>
            </Class>
        </ObjectStream>"#;
        let source = MemorySource::default()
            .with("materials/terrain/world/region.regionmat", material)
            .with("materials/terrain/default.mtl", b"<Material/>".to_vec());

        let graph = inspect_direct(&source, "materials/terrain/world/region.regionmat").unwrap();
        assert!(graph.edges().iter().any(|edge| {
            edge.relation() == "terrain.region_material.default"
                && edge.target() == "materials/terrain/default.mtl"
        }));
    }

    #[test]
    fn slice_metadata_resolves_mesh_spawner_and_exact_variant() {
        use nw_objectstream::slice_meta::{
            SLICE_META_DATA_MESH_ENTRY_TYPE_ID, SLICE_META_DATA_SPAWNER_ENTRY_TYPE_ID,
            SLICE_META_DATA_TYPE_ID,
        };
        use nw_objectstream::{Element, ObjectStream, types};
        use uuid::uuid;

        let mesh_id = AssetId::new(uuid!("A1BB52F8-7884-5B5E-BD5B-8B608E7601D6"), 0);
        let slice_id = AssetId::new(uuid!("53C24E32-9B91-5C08-90F2-FCF51832F8FA"), 0);
        let stream = ObjectStream {
            elements: vec![
                Element::new(SLICE_META_DATA_TYPE_ID).with_children([
                    Element::new(types::AZSTD_VECTOR)
                        .with_field("meshes")
                        .with_children([Element::new(SLICE_META_DATA_MESH_ENTRY_TYPE_ID)
                            .with_children([objectstream_asset_id("meshAssetId", mesh_id)])]),
                    Element::new(types::AZSTD_VECTOR)
                        .with_field("Spawners")
                        .with_children([Element::new(SLICE_META_DATA_SPAWNER_ENTRY_TYPE_ID)
                            .with_children([
                                objectstream_asset_id("sliceAssetId", slice_id),
                                objectstream_leaf("sliceName", types::AZSTD_STRING, b"Tree"),
                                objectstream_leaf("variationName", types::AZSTD_STRING, b"Fall/A"),
                            ])]),
                ]),
            ],
            ..ObjectStream::new(3)
        };
        let mut source = MemorySource::default()
            .with(
                "slices/owner.slice.meta",
                stream
                    .to_encoding_bytes(nw_objectstream::ObjectStreamEncoding::Xml)
                    .unwrap(),
            )
            .with("objects/tree.cgf", minimal_cgf())
            .with("slices/tree.dynamicslice", ObjectStream::new(3).to_bytes())
            .with("slices/tree_fall_a.slice.meta", b"present".to_vec());
        source.ids.insert(mesh_id, "objects/tree.cgf".to_owned());
        source
            .ids
            .insert(slice_id, "slices/tree.dynamicslice".to_owned());

        let graph = inspect_direct(&source, "slices/owner.slice.meta").unwrap();

        assert!(graph.edges().iter().any(|edge| {
            edge.relation() == "slice_metadata.mesh" && edge.target() == "objects/tree.cgf"
        }));
        assert!(graph.edges().iter().any(|edge| {
            edge.relation() == "slice_metadata.spawner"
                && edge.target() == "slices/tree.dynamicslice"
        }));
        assert!(graph.edges().iter().any(|edge| {
            edge.relation() == "slice_metadata.spawner_variant"
                && edge.target() == "slices/tree_fall_a.slice.meta"
        }));
    }

    fn push_distribution_string(bytes: &mut Vec<u8>, value: &str) {
        bytes.push(u8::try_from(value.len()).unwrap());
        bytes.extend_from_slice(value.as_bytes());
    }

    fn objectstream_leaf(
        field: &str,
        id: uuid::Uuid,
        data: impl Into<Vec<u8>>,
    ) -> nw_objectstream::Element {
        nw_objectstream::Element::new(id)
            .with_field(field)
            .with_data(data)
    }

    fn objectstream_asset_id(field: &str, asset_id: AssetId) -> nw_objectstream::Element {
        nw_objectstream::Element::new(nw_objectstream::types::ASSET)
            .with_field(field)
            .with_children([
                objectstream_leaf(
                    "guid",
                    nw_objectstream::types::AZ_UUID,
                    asset_id.guid.as_bytes(),
                ),
                objectstream_leaf(
                    "subId",
                    nw_objectstream::types::UNSIGNED_INT,
                    asset_id.sub_id.to_be_bytes(),
                ),
            ])
    }

    #[test]
    fn region_chunks_resolve_nested_generated_asset_ids() {
        use nw_objectstream::{Element, ObjectStream, types};
        use nw_reflected_types::{
            az::rtti::AzRtti,
            types::{ChunkEntry, GridGenericAssetAssetData},
        };
        use uuid::uuid;

        let chunk_id = AssetId::new(uuid!("53C24E32-9B91-5C08-90F2-FCF51832F8FA"), 7);
        let stream = ObjectStream {
            elements: vec![
                Element::new(GridGenericAssetAssetData::TYPE_ID.into()).with_children([
                    Element::new(types::AZSTD_VECTOR)
                        .with_field("Chunks")
                        .with_children([Element::new(ChunkEntry::TYPE_ID.into()).with_children([
                            Element::new(types::AZ_U64)
                                .with_field("size")
                                .with_data(64_u64.to_be_bytes()),
                            Element::new(types::ASSET_ID)
                                .with_field("assetId")
                                .with_children([
                                    objectstream_leaf(
                                        "guid",
                                        types::AZ_UUID,
                                        chunk_id.guid.as_bytes(),
                                    ),
                                    objectstream_leaf(
                                        "subId",
                                        types::UNSIGNED_INT,
                                        chunk_id.sub_id.to_be_bytes(),
                                    ),
                                ]),
                        ])]),
                ]),
            ],
            ..ObjectStream::new(3)
        };
        let mut source = MemorySource::default()
            .with(
                "regions/r_+00_+00/region.chunks",
                stream
                    .to_encoding_bytes(nw_objectstream::ObjectStreamEncoding::Xml)
                    .unwrap(),
            )
            .with("regions/chunks/terrain.chunk", b"chunk".to_vec());
        source
            .ids
            .insert(chunk_id, "regions/chunks/terrain.chunk".to_owned());

        let graph = inspect_direct(&source, "regions/r_+00_+00/region.chunks").unwrap();

        assert!(
            graph.edges().iter().any(|edge| {
                edge.relation() == "region_manifest.chunk"
                    && edge.target() == "regions/chunks/terrain.chunk"
            }),
            "{graph:?}"
        );
    }

    #[test]
    fn opaque_model_heap_is_not_sniffed_as_a_generic_objectstream() {
        let mut heap = nw_objectstream::ObjectStream::new(3).to_bytes();
        heap.extend_from_slice(b"opaque geometry heap payload");
        let source = MemorySource::default().with("objects/hero.cgfheap", heap);

        let graph = inspect_direct(&source, "objects/hero.cgfheap").unwrap();

        assert!(graph.edges().is_empty());
        assert!(graph.unresolved().is_empty());
    }

    #[test]
    fn resolves_and_validates_character_rnr_physics() {
        let source = MemorySource::default()
            .with(
                "objects/hero.cdf",
                br#"<CharacterDefinition><Model File="objects/hero.chr" Physics="physics/hero.rnr"/></CharacterDefinition>"#,
            )
            .with("objects/hero.chr", minimal_cgf())
            .with("physics/hero.rnr", minimal_rnr());

        let graph = resolve(&source, "objects/hero.cdf", &ResolveOptions::default()).unwrap();

        assert!(graph.is_complete(), "{:?}", graph.unresolved());
        assert!(graph.edges().iter().any(|edge| {
            edge.source() == "objects/hero.cdf"
                && edge.target() == "physics/hero.rnr"
                && edge.relation() == "character.physics"
                && edge.is_required()
        }));
    }

    #[test]
    fn resolves_rnr_referenced_by_objectstream_asset() {
        let stream = br#"<ObjectStream version="3"><Class name="Asset" type="{77A19D40-8731-4D3C-9041-1B43047366A4}" value="id={7A1472D1-DF54-5362-BC71-9974D5F25572}:0,type={78802ABF-9595-463A-8D2B-D022F906F9B1},hint={physics/hero.rnr}"/></ObjectStream>"#;
        let source = MemorySource::default()
            .with("slices/hero.slice", stream)
            .with("physics/hero.rnr", minimal_rnr());

        let graph = resolve(&source, "slices/hero.slice", &ResolveOptions::default()).unwrap();

        assert!(graph.is_complete(), "{:?}", graph.unresolved());
        assert!(graph.edges().iter().any(|edge| {
            edge.source() == "slices/hero.slice"
                && edge.target() == "physics/hero.rnr"
                && edge.relation() == "objectstream.asset_reference"
        }));
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
            .with("animations/player/moves/walk.caf", b"caf".to_vec())
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
        assert!(
            !graph
                .assets()
                .iter()
                .any(|path| path == "animations/player/moves/walk.caf")
        );
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

    fn minimal_rnr() -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&nw_rnr::legacy::MAGIC.to_le_bytes());
        bytes.extend_from_slice(&7_u32.to_le_bytes());
        bytes.extend_from_slice(&0_u32.to_le_bytes());
        bytes.extend_from_slice(&[0; 16]);
        bytes.push(0);
        bytes.push(0);
        bytes.push(1);
        bytes.extend_from_slice(&(nw_rnr::legacy::ShapeKind::Box as u32).to_le_bytes());
        for value in [1.0_f32, 2.0, 3.0, 0.0] {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        bytes.extend_from_slice(&0_u32.to_le_bytes());
        bytes
    }
}
