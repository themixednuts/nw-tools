use std::collections::BTreeMap;
use std::str;

use super::*;

struct LoadedParticleLibrary {
    path: String,
    emitter_indices: Vec<usize>,
    bytes: Vec<u8>,
    document: cry_xml::XmlElement,
    resources: Vec<cry_particles::ParticleResourceReference>,
}

pub(super) fn resolve_particle_libraries(
    runner: &nw_jobs::JobRunner,
    source: &dyn AssetSource,
    extras: &mut nw_model::CryAssetExtras,
    emitters: &mut [SceneParticleEmitter],
    dependency_graph: &nw_asset_graph::AssetDependencyGraph,
) -> Result<()> {
    let mut libraries = BTreeMap::<String, (String, Vec<usize>)>::new();
    for (index, emitter) in emitters.iter().enumerate() {
        let Some(asset_id) = emitter.particle_library_asset_id else {
            continue;
        };
        let path = source.path_by_id(asset_id).with_context(|| {
            format!(
                "resolve particle library {asset_id} for emitter {} in {}",
                emitter.emitter.selected_emitter, emitter.emitter.context.source_path
            )
        })?;
        let path = normalize_path(&path);
        libraries
            .entry(path.to_ascii_lowercase())
            .or_insert_with(|| (path, Vec::new()))
            .1
            .push(index);
    }
    let libraries = libraries.into_values().collect::<Vec<_>>();
    let loaded = runner.try_map(&libraries, |(path, emitter_indices)| {
        load_particle_library(source, path, emitter_indices, emitters)
    })?;

    let mut resources = BTreeMap::<String, (String, nw_model::CryEmbeddedResourceKind)>::new();
    for library in &loaded {
        for reference in &library.resources {
            if reference.kind.is_audio() {
                continue;
            }
            let path = resolve_particle_resource_path(source, reference).with_context(|| {
                format!(
                    "resolve {} for particle effect {} in {}",
                    reference.kind.relation(),
                    reference.effect_path,
                    library.path
                )
            })?;
            let kind = embedded_resource_kind(reference.kind);
            let key = path.to_ascii_lowercase();
            match resources.entry(key) {
                std::collections::btree_map::Entry::Vacant(entry) => {
                    entry.insert((path, kind));
                }
                std::collections::btree_map::Entry::Occupied(entry) => {
                    if entry.get().1 != kind {
                        bail!(
                            "particle resource {} is authored as both {:?} and {:?}",
                            entry.get().0,
                            entry.get().1,
                            kind
                        );
                    }
                }
            }
        }
    }
    let texture_paths = resources
        .values()
        .filter(|(_, kind)| *kind == nw_model::CryEmbeddedResourceKind::ParticleTexture)
        .map(|(path, _)| path.clone())
        .collect::<Vec<_>>();
    for texture in texture_paths {
        for sidecar in nw_asset_graph::texture_streaming_sidecars(source, &texture)? {
            resources.entry(sidecar.to_ascii_lowercase()).or_insert((
                sidecar,
                nw_model::CryEmbeddedResourceKind::ParticleTextureSidecar,
            ));
        }
    }
    let resource_roots = resources
        .values()
        .map(|(path, _)| path.clone())
        .collect::<Vec<_>>();
    let transitive_resources = dependency_graph
        .transitive_dependencies_where(resource_roots.iter().map(String::as_str), |_| true);
    for dependency in transitive_resources {
        let Some(kind) = transitive_resource_kind(&dependency) else {
            continue;
        };
        resources
            .entry(dependency.to_ascii_lowercase())
            .or_insert((dependency, kind));
    }
    let resources = resources.into_values().collect::<Vec<_>>();
    let loaded_resources = runner.try_map(&resources, |(path, kind)| {
        let bytes = read_required(source, path)
            .with_context(|| format!("read selected particle resource {path}"))?;
        Ok::<_, anyhow::Error>((path.clone(), *kind, bytes))
    })?;

    for library in loaded {
        add_xml_source(
            extras,
            &library.path,
            nw_model::CrySourceAssetKind::ParticleLibrary,
            &library.document,
        );
        add_resource(
            extras,
            &library.path,
            nw_model::CryEmbeddedResourceKind::ParticleLibrary,
            library.bytes,
        );
        for index in library.emitter_indices {
            emitters[index].emitter.particle_library_path = Some(library.path.clone());
        }
    }
    for (path, kind, bytes) in loaded_resources {
        add_dependency(extras, &path);
        add_resource(extras, &path, kind, bytes);
    }
    Ok(())
}

fn load_particle_library(
    source: &dyn AssetSource,
    path: &str,
    emitter_indices: &[usize],
    emitters: &[SceneParticleEmitter],
) -> Result<LoadedParticleLibrary> {
    let bytes = source
        .read(path)
        .with_context(|| format!("read referenced particle library {path}"))?;
    let xml =
        str::from_utf8(&bytes).with_context(|| format!("decode UTF-8 particle library {path}"))?;
    let library = cry_particles::ParticleLibrarySource::from_xml(xml)
        .with_context(|| format!("parse particle library {path}"))?;
    let mut resources = Vec::new();
    for index in emitter_indices {
        let emitter = &emitters[*index].emitter;
        resources.extend(
            library
                .resources_for_effect(&emitter.selected_emitter)
                .with_context(|| {
                    format!(
                        "resolve selected emitter from {} ({})",
                        emitter.context.source_path,
                        emitter
                            .context
                            .entity_name
                            .as_deref()
                            .unwrap_or("unnamed entity")
                    )
                })?,
        );
    }
    resources.sort_by(|left, right| {
        left.kind
            .cmp(&right.kind)
            .then_with(|| {
                left.value
                    .to_ascii_lowercase()
                    .cmp(&right.value.to_ascii_lowercase())
            })
            .then_with(|| left.effect_path.cmp(&right.effect_path))
    });
    resources.dedup_by(|left, right| {
        left.kind == right.kind && left.value.eq_ignore_ascii_case(&right.value)
    });
    Ok(LoadedParticleLibrary {
        path: path.to_owned(),
        emitter_indices: emitter_indices.to_vec(),
        bytes,
        document: library.source,
        resources,
    })
}

fn resolve_particle_resource_path(
    source: &dyn AssetSource,
    reference: &cry_particles::ParticleResourceReference,
) -> Result<String> {
    let authored = normalize_path(&reference.value);
    let mut candidates = vec![authored.clone()];
    if reference.kind.is_texture() && source_extension(&authored) == "tif" {
        candidates.push(replace_extension(&authored, "dds"));
    } else if reference.kind == cry_particles::ParticleResourceKind::Material
        && source_extension(&authored).is_empty()
    {
        candidates.push(format!("{authored}.mtl"));
    } else if reference.kind == cry_particles::ParticleResourceKind::Geometry
        && source_extension(&authored).is_empty()
    {
        candidates.push(format!("{authored}.cgf"));
    }
    for candidate in candidates {
        if source.contains(&candidate) {
            return Ok(candidate);
        }
    }
    if !authored.contains('/') {
        let mut matches = source.matching_paths(&format!("**/{authored}"))?;
        matches.sort_by_key(|path| path.to_ascii_lowercase());
        matches.dedup_by(|left, right| left.eq_ignore_ascii_case(right));
        match matches.as_slice() {
            [path] => return Ok(normalize_path(path)),
            [] => {}
            _ => bail!("ambiguous basename-only particle resource {authored}"),
        }
    }
    bail!("missing authored particle resource {authored}")
}

fn embedded_resource_kind(
    kind: cry_particles::ParticleResourceKind,
) -> nw_model::CryEmbeddedResourceKind {
    match kind {
        cry_particles::ParticleResourceKind::Texture
        | cry_particles::ParticleResourceKind::NormalMap
        | cry_particles::ParticleResourceKind::GlowMap => {
            nw_model::CryEmbeddedResourceKind::ParticleTexture
        }
        cry_particles::ParticleResourceKind::Material => {
            nw_model::CryEmbeddedResourceKind::ParticleMaterial
        }
        cry_particles::ParticleResourceKind::Geometry => {
            nw_model::CryEmbeddedResourceKind::ParticleGeometry
        }
        cry_particles::ParticleResourceKind::AudioStartTrigger
        | cry_particles::ParticleResourceKind::AudioStopTrigger => {
            unreachable!("audio controls resolve through the audio dependency pipeline")
        }
    }
}

fn transitive_resource_kind(path: &str) -> Option<nw_model::CryEmbeddedResourceKind> {
    let lowercase = path.to_ascii_lowercase();
    if lowercase.ends_with(".cgfheap") || lowercase.ends_with(".cgaheap") {
        return Some(nw_model::CryEmbeddedResourceKind::ParticleGeometryHeap);
    }
    if lowercase.ends_with(".dds.a")
        || lowercase.contains(".dds.a.")
        || lowercase
            .rsplit_once(".dds.")
            .is_some_and(|(_, suffix)| suffix.parse::<u32>().is_ok())
    {
        return Some(nw_model::CryEmbeddedResourceKind::ParticleTextureSidecar);
    }
    match source_extension(path).as_str() {
        "dds" => Some(nw_model::CryEmbeddedResourceKind::ParticleTexture),
        "mtl" => Some(nw_model::CryEmbeddedResourceKind::ParticleMaterial),
        "cgf" | "cga" => Some(nw_model::CryEmbeddedResourceKind::ParticleGeometry),
        _ => None,
    }
}

fn replace_extension(path: &str, extension: &str) -> String {
    path.rsplit_once('.').map_or_else(
        || format!("{path}.{extension}"),
        |(stem, _)| format!("{stem}.{extension}"),
    )
}
