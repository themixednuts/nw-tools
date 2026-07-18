//! Animation asset parsing and assembly: CAF/DBA clip collection, skeleton
//! binding, and CHRPARAMS-driven animation-list resolution.
//!
//! Split out of `model_asset` as a pure move; shared helpers stay in the parent.

use super::*;

struct ParsedAnimationAsset {
    path: String,
    is_dba: bool,
    clips: Vec<cry_animation::AnimationClip>,
}

pub(super) fn push_animation_assets(
    runner: &nw_jobs::JobRunner,
    source: &dyn AssetSource,
    paths: &[String],
    events: Option<&cry_animation::AnimationEventDatabase>,
    skeleton: usize,
    require_mapping: bool,
    resolved: &mut ResolvedAsset,
) -> Result<()> {
    let mut paths = paths
        .iter()
        .map(|path| normalize_path(path))
        .filter(|path| {
            !resolved
                .parsed_animation_assets
                .contains(&(skeleton, path.clone()))
        })
        .collect::<Vec<_>>();
    paths.sort_unstable();
    paths.dedup_by(|left, right| left.eq_ignore_ascii_case(right));
    if paths.is_empty() {
        return Ok(());
    }
    let parsed = runner.try_map(&paths, |path| {
        let bytes = read_required(source, path)?;
        let is_dba = source_extension(path) == "dba";
        let mut clips = if is_dba {
            cry_animation::AnimationClip::parse_dba(&bytes)
                .with_context(|| format!("decode tracks database {path}"))?
        } else {
            vec![
                cry_animation::AnimationClip::parse(path.clone(), &bytes)
                    .with_context(|| format!("decode animation {path}"))?,
            ]
        };
        if let Some(events) = events {
            for clip in &mut clips {
                clip.events = events.events_for(&clip.source_path).cloned().collect();
            }
        }
        Ok::<_, anyhow::Error>(ParsedAnimationAsset {
            path: path.clone(),
            is_dba,
            clips,
        })
    })?;
    let target = resolved
        .model
        .skeletons
        .get(skeleton)
        .with_context(|| format!("animation targets missing skeleton {skeleton}"))?;
    for asset in parsed {
        // Each clip ships as its glTF channel buffer at its own catalog path
        // (see `gltf::append_animation`); the raw CAF/DBA payload is never
        // embedded. Clip `cry_source_path` and the dependency listing preserve
        // provenance.
        let mut mapped = 0usize;
        let mut duplicate = false;
        for clip in asset.clips {
            if resolved.animations.iter().any(|existing| {
                existing.skeleton == skeleton
                    && existing
                        .clip
                        .source_path
                        .eq_ignore_ascii_case(&clip.source_path)
            }) {
                duplicate = true;
                continue;
            }
            if !clip_targets_skeleton(&clip, target) {
                record_unbound_animation(&mut resolved.extras, &clip.source_path, skeleton);
                continue;
            }
            mapped += 1;
            resolved
                .animations
                .push(nw_model::ModelAnimation { skeleton, clip });
        }
        resolved
            .parsed_animation_assets
            .insert((skeleton, asset.path.clone()));
        add_dependency(&mut resolved.extras, &asset.path);
        if require_mapping && mapped == 0 && (asset.is_dba || !duplicate) {
            if asset.is_dba {
                bail!(
                    "tracks database {} has no controllers targeting model skeleton {skeleton}",
                    asset.path
                );
            }
            bail!(
                "CAF {} has no controllers targeting model skeleton {skeleton}",
                asset.path
            );
        }
    }
    Ok(())
}

pub(super) fn clip_targets_skeleton(
    clip: &cry_animation::AnimationClip,
    skeleton: &nw_model::Skeleton,
) -> bool {
    clip.caf.controllers.iter().any(|controller| {
        skeleton
            .bones
            .iter()
            .any(|bone| bone.controller_id == controller.controller_id)
    })
}

pub(super) fn load_character_parameters(
    runner: &nw_jobs::JobRunner,
    source: &dyn AssetSource,
    path: &str,
    required: bool,
    skeleton: usize,
    resolved: &mut ResolvedAsset,
) -> Result<()> {
    let path = normalize_path(path);
    if has_source_asset(&resolved.extras, &path) {
        return Ok(());
    }
    let Some(bytes) = source.read(&path) else {
        if required {
            bail!("CDF ParamsOverride asset not found: {path}");
        }
        return Ok(());
    };
    let xml = str::from_utf8(&bytes).with_context(|| format!("decode UTF-8 {path}"))?;
    let parameters = CharacterParameters::from_xml(xml)
        .with_context(|| format!("parse character parameters {path}"))?;
    add_xml_source(
        &mut resolved.extras,
        &path,
        nw_model::CrySourceAssetKind::CharacterParameters,
        &parameters.source,
    );
    add_resource(
        &mut resolved.extras,
        &path,
        nw_model::CryEmbeddedResourceKind::CharacterParameters,
        bytes,
    );

    let mut animation_paths = CharacterAnimationPathResolver::default();
    let mut event_database = None;
    let mut visited = HashSet::new();
    load_animation_list(
        runner,
        source,
        &path,
        &parameters,
        &mut animation_paths,
        &mut event_database,
        &mut visited,
        skeleton,
        resolved,
    )?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn load_animation_list(
    runner: &nw_jobs::JobRunner,
    source: &dyn AssetSource,
    source_path: &str,
    parameters: &CharacterParameters,
    animation_paths: &mut CharacterAnimationPathResolver,
    event_database: &mut Option<LoadedEventDatabase>,
    visited: &mut HashSet<String>,
    skeleton: usize,
    resolved: &mut ResolvedAsset,
) -> Result<()> {
    if !visited.insert(normalize_path(source_path).to_ascii_lowercase()) {
        return Ok(());
    }
    for entry in &parameters.animations {
        let value = animation_paths.resolve_entry(entry);
        match entry.kind {
            CharacterAnimationEntryKind::FilePath => {}
            CharacterAnimationEntryKind::ParseSubfolders => {}
            CharacterAnimationEntryKind::AnimationEventDatabase => {
                if event_database.is_none() {
                    let database = load_event_database(source, &value)?;
                    add_xml_source(
                        &mut resolved.extras,
                        &value,
                        nw_model::CrySourceAssetKind::AnimationEvents,
                        &database.parsed.source,
                    );
                    add_resource(
                        &mut resolved.extras,
                        &value,
                        nw_model::CryEmbeddedResourceKind::AnimationEvents,
                        database.bytes.clone(),
                    );
                    for animation in resolved
                        .animations
                        .iter_mut()
                        .filter(|animation| animation.skeleton == skeleton)
                    {
                        animation.clip.events = database
                            .parsed
                            .events_for(&animation.clip.source_path)
                            .cloned()
                            .collect();
                    }
                    *event_database = Some(database);
                }
            }
            CharacterAnimationEntryKind::Include => {
                let bytes = read_required(source, &value)?;
                let xml = str::from_utf8(&bytes)
                    .with_context(|| format!("decode included CHRPARAMS {value}"))?;
                let included = CharacterParameters::from_xml(xml)
                    .with_context(|| format!("parse included CHRPARAMS {value}"))?;
                add_xml_source(
                    &mut resolved.extras,
                    &value,
                    nw_model::CrySourceAssetKind::CharacterParameters,
                    &included.source,
                );
                add_resource(
                    &mut resolved.extras,
                    &value,
                    nw_model::CryEmbeddedResourceKind::CharacterParameters,
                    bytes,
                );
                load_animation_list(
                    runner,
                    source,
                    &value,
                    &included,
                    animation_paths,
                    event_database,
                    visited,
                    skeleton,
                    resolved,
                )?;
            }
            CharacterAnimationEntryKind::TracksDatabase => {
                let wildcard = value.contains(['*', '?', '[']);
                let paths = if wildcard {
                    source.matching_paths(&value)?
                } else {
                    vec![value.clone()]
                };
                push_animation_assets(
                    runner,
                    source,
                    &paths,
                    event_database.as_ref().map(|database| &database.parsed),
                    skeleton,
                    false,
                    resolved,
                )?;
            }
            CharacterAnimationEntryKind::FaceLibrary => {
                let bytes = read_required(source, &value)?;
                add_resource(
                    &mut resolved.extras,
                    &value,
                    nw_model::CryEmbeddedResourceKind::FaceLibrary,
                    bytes,
                );
                add_dependency(&mut resolved.extras, &value);
            }
            CharacterAnimationEntryKind::WildcardAsset => {
                let paths = source.matching_paths(&value)?;
                let (animations, mannequin): (Vec<_>, Vec<_>) =
                    paths.into_iter().partition(|path| {
                        matches!(source_extension(path).as_str(), "caf" | "i_caf" | "dba")
                    });
                push_animation_assets(
                    runner,
                    source,
                    &animations,
                    event_database.as_ref().map(|database| &database.parsed),
                    skeleton,
                    false,
                    resolved,
                )?;
                for path in mannequin {
                    if matches!(source_extension(&path).as_str(), "bspace" | "comb") {
                        add_mannequin_source(source, &path, &mut resolved.extras)?;
                    }
                }
            }
            CharacterAnimationEntryKind::Asset => match source_extension(&value).as_str() {
                "caf" | "i_caf" | "dba" => {
                    push_animation_assets(
                        runner,
                        source,
                        std::slice::from_ref(&value),
                        event_database.as_ref().map(|database| &database.parsed),
                        skeleton,
                        false,
                        resolved,
                    )?;
                }
                "bspace" | "comb" => {
                    add_mannequin_source(source, &value, &mut resolved.extras)?;
                }
                _ => add_dependency(&mut resolved.extras, &value),
            },
            CharacterAnimationEntryKind::UnknownDirective => {
                bail!(
                    "unsupported CHRPARAMS directive `{}` in {source_path}",
                    entry.name
                );
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model_asset::tests::EmptySource;

    #[test]
    fn empty_animation_work_does_not_require_a_skeleton() {
        let mut resolved = ResolvedAsset {
            model: nw_model::Model {
                meshes: Vec::new(),
                skeletons: Vec::new(),
                auxiliary_nodes: Vec::new(),
            },
            materials: None,
            animations: Vec::new(),
            extras: nw_model::CryAssetExtras::default(),
            physics: nw_model::PhysicsScene::default(),
            parsed_animation_assets: HashSet::new(),
        };

        push_animation_assets(
            &nw_jobs::JobRunner::inline(),
            &EmptySource,
            &[],
            None,
            0,
            false,
            &mut resolved,
        )
        .unwrap();
    }
}
