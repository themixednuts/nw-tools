//! Animation asset parsing and assembly: CAF/DBA clip collection, skeleton
//! binding, and CHRPARAMS-driven animation-list resolution.
//!
//! Split out of `model_asset` as a pure move; shared helpers stay in the parent.

use super::*;

mod blendspaces;
pub(super) use blendspaces::scope_blend_space_dependencies;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum AnimationBindingPolicy {
    AutomaticCompatible,
    ExplicitPermissive,
}

impl AnimationBindingPolicy {
    const fn for_required_mapping(required: bool) -> Self {
        if required {
            Self::ExplicitPermissive
        } else {
            Self::AutomaticCompatible
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) struct AnimationAssetEvaluation {
    skeleton: usize,
    path: String,
    policy: AnimationBindingPolicy,
}

impl AnimationAssetEvaluation {
    pub(super) fn new(skeleton: usize, path: &str, policy: AnimationBindingPolicy) -> Self {
        Self {
            skeleton,
            path: canonical_asset_path(path),
            policy,
        }
    }

    pub(super) const fn with_skeleton_offset(mut self, offset: usize) -> Self {
        self.skeleton += offset;
        self
    }
}

#[derive(Clone)]
struct DecodedAnimationAsset {
    path: String,
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
    let requested_paths = normalized_unique_paths(paths);
    let policy = AnimationBindingPolicy::for_required_mapping(require_mapping);
    let paths = unevaluated_animation_paths(&requested_paths, skeleton, policy, resolved);
    let decoded = runner.try_map(&paths, |path| {
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
        Ok::<_, anyhow::Error>(DecodedAnimationAsset {
            path: path.clone(),
            clips,
        })
    })?;
    for asset in decoded {
        evaluate_animation_asset(asset, skeleton, policy, resolved)?;
    }
    scope_animation_dependencies(&requested_paths, resolved);
    if require_mapping {
        ensure_required_animation_mappings(&requested_paths, skeleton, resolved)?;
    }
    Ok(())
}

fn normalized_unique_paths(paths: &[String]) -> Vec<String> {
    let mut paths = paths
        .iter()
        .map(|path| normalize_path(path))
        .collect::<Vec<_>>();
    paths.sort_by_cached_key(|path| path.to_ascii_lowercase());
    paths.dedup_by(|left, right| left.eq_ignore_ascii_case(right));
    paths
}

fn unevaluated_animation_paths(
    paths: &[String],
    skeleton: usize,
    policy: AnimationBindingPolicy,
    resolved: &ResolvedAsset,
) -> Vec<String> {
    paths
        .iter()
        .filter(|path| {
            !resolved
                .animation_asset_evaluations
                .contains_key(&AnimationAssetEvaluation::new(skeleton, path, policy))
        })
        .cloned()
        .collect()
}

fn evaluate_animation_asset(
    asset: DecodedAnimationAsset,
    skeleton: usize,
    policy: AnimationBindingPolicy,
    resolved: &mut ResolvedAsset,
) -> Result<()> {
    let target = resolved
        .model
        .skeletons
        .get(skeleton)
        .with_context(|| format!("animation targets missing skeleton {skeleton}"))?;
    let mut bound = false;
    for clip in asset.clips {
        if resolved.animations.iter().any(|existing| {
            existing.skeleton == skeleton
                && asset_paths_equal(&existing.clip.source_path, &clip.source_path)
        }) {
            clear_unbound_animation(&mut resolved.extras, &clip.source_path, skeleton);
            bound = true;
            continue;
        }
        let targets_skeleton = match policy {
            AnimationBindingPolicy::AutomaticCompatible => {
                clip_is_compatible_with_skeleton(&clip, target)
            }
            AnimationBindingPolicy::ExplicitPermissive => clip_targets_skeleton(&clip, target),
        };
        if !targets_skeleton {
            clear_unbound_animation(&mut resolved.extras, &clip.source_path, skeleton);
            record_unbound_animation(&mut resolved.extras, &clip.source_path, skeleton);
            continue;
        }
        clear_unbound_animation(&mut resolved.extras, &clip.source_path, skeleton);
        bound = true;
        resolved
            .animations
            .push(nw_model::ModelAnimation { skeleton, clip });
    }
    let evaluation = AnimationAssetEvaluation::new(skeleton, &asset.path, policy);
    resolved
        .animation_asset_evaluations
        .entry(evaluation)
        .and_modify(|previous| *previous |= bound)
        .or_insert(bound);
    Ok(())
}

fn clear_unbound_animation(
    extras: &mut nw_model::CryAssetExtras,
    source_path: &str,
    skeleton: usize,
) {
    extras.unbound_animations.retain(|animation| {
        animation.skeleton != skeleton || !asset_paths_equal(&animation.source_path, source_path)
    });
}

fn scope_animation_dependencies(paths: &[String], resolved: &mut ResolvedAsset) {
    for path in paths {
        let bound = (0..resolved.model.skeletons.len()).any(|skeleton| {
            [
                AnimationBindingPolicy::AutomaticCompatible,
                AnimationBindingPolicy::ExplicitPermissive,
            ]
            .into_iter()
            .any(|policy| {
                resolved
                    .animation_asset_evaluations
                    .get(&AnimationAssetEvaluation::new(skeleton, path, policy))
                    .copied()
                    .unwrap_or(false)
            })
        });
        if bound {
            add_dependency(&mut resolved.extras, path);
        } else {
            resolved
                .extras
                .dependencies
                .retain(|dependency| !asset_paths_equal(dependency, path));
        }
    }
}

fn ensure_required_animation_mappings(
    paths: &[String],
    skeleton: usize,
    resolved: &ResolvedAsset,
) -> Result<()> {
    for path in paths {
        let evaluation = AnimationAssetEvaluation::new(
            skeleton,
            path,
            AnimationBindingPolicy::ExplicitPermissive,
        );
        if resolved
            .animation_asset_evaluations
            .get(&evaluation)
            .is_some_and(|bound| !bound)
        {
            if source_extension(path) == "dba" {
                bail!(
                    "tracks database {path} has no controllers targeting model skeleton {skeleton}"
                );
            }
            bail!("CAF {path} has no controllers targeting model skeleton {skeleton}");
        }
    }
    Ok(())
}

pub(super) fn clip_targets_skeleton(
    clip: &cry_animation::AnimationClip,
    skeleton: &nw_model::Skeleton,
) -> bool {
    matching_controller_count(clip, skeleton) > 0
}

fn clip_is_compatible_with_skeleton(
    clip: &cry_animation::AnimationClip,
    skeleton: &nw_model::Skeleton,
) -> bool {
    let total = clip.caf.controllers.len();
    if total == 0 {
        return false;
    }
    let matching = matching_controller_count(clip, skeleton);

    // Animation-list discovery carries no skeleton identity. Three-quarters
    // controller coverage is the smallest corpus-supported ambiguity boundary:
    // it retains shared-rig clips (including the Bow set) while rejecting the
    // observed fish/Mindangel and Isabella phase-family outliers. This remains a
    // compatibility heuristic, not proof that two assets author the same rig.
    matching * 4 >= total * 3
}

fn matching_controller_count(
    clip: &cry_animation::AnimationClip,
    skeleton: &nw_model::Skeleton,
) -> usize {
    clip.caf
        .controllers
        .iter()
        .filter(|controller| {
            skeleton
                .bones
                .iter()
                .any(|bone| bone.controller_id == controller.controller_id)
        })
        .count()
}

fn canonical_asset_path(path: &str) -> String {
    normalize_path(path)
        .trim_start_matches("./")
        .to_ascii_lowercase()
}

fn asset_paths_equal(left: &str, right: &str) -> bool {
    canonical_asset_path(left) == canonical_asset_path(right)
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

    fn skeleton(controller_ids: &[u32]) -> nw_model::Skeleton {
        nw_model::Skeleton {
            bones: controller_ids
                .iter()
                .map(|controller_id| nw_model::Bone {
                    name: format!("bone_{controller_id}"),
                    controller_id: *controller_id,
                    parent: None,
                    local: Mat4::IDENTITY,
                    inverse_bind: Mat4::IDENTITY,
                })
                .collect(),
            placement: None,
        }
    }

    fn clip(path: &str, controller_ids: &[u32]) -> cry_animation::AnimationClip {
        let file = path.rsplit(['/', '\\']).next().unwrap_or(path);
        let name = file
            .strip_suffix(".i_caf")
            .or_else(|| file.strip_suffix(".caf"))
            .unwrap_or(file)
            .to_owned();
        cry_animation::AnimationClip {
            source_path: path.to_owned(),
            name,
            caf: cry_animation::CafAnimation {
                header: cry_animation::CafAnimationHeader {
                    flags: 0,
                    start_sec: 0.0,
                    end_sec: 0.0,
                    total_duration: 0.0,
                    controller_count: controller_ids.len() as u32,
                    source: cry_chunk::CafAnimationHeaderSource::Timing,
                    file_path: None,
                },
                sample_rate: 30.0,
                controllers: controller_ids
                    .iter()
                    .map(|controller_id| cry_animation::CafController {
                        controller_id: *controller_id,
                        flags: 0,
                        rotations: Vec::new(),
                        positions: Vec::new(),
                        scales: Vec::new(),
                    })
                    .collect(),
                root_motion: None,
            },
            events: Vec::new(),
        }
    }

    #[test]
    fn automatic_controller_coverage_accepts_the_three_quarter_boundary() {
        let skeleton = skeleton(&[1, 2, 3]);

        assert!(clip_is_compatible_with_skeleton(
            &clip("animations/three_of_four.caf", &[1, 2, 3, 4]),
            &skeleton,
        ));
        assert!(!clip_is_compatible_with_skeleton(
            &clip("animations/two_of_three.caf", &[1, 2, 4]),
            &skeleton,
        ));
    }

    #[test]
    fn automatic_controller_coverage_accepts_bow_style_partial_clips() {
        let skeleton = skeleton(&[1, 2, 3, 4, 5, 6, 7, 8, 9]);

        assert!(clip_is_compatible_with_skeleton(
            &clip(
                "animations/gameplay/character/player/bow/shared.caf",
                &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10],
            ),
            &skeleton,
        ));
    }

    #[test]
    fn automatic_controller_coverage_rejects_fish_style_low_overlap_and_empty_clips() {
        let skeleton = skeleton(&[1, 2]);

        assert!(!clip_is_compatible_with_skeleton(
            &clip("animations/fish/foreign.caf", &[1, 3, 4, 5]),
            &skeleton,
        ));
        assert!(!clip_is_compatible_with_skeleton(
            &clip("animations/empty.caf", &[]),
            &skeleton,
        ));
    }

    #[test]
    fn explicit_controller_mapping_remains_any_overlap() {
        let skeleton = skeleton(&[1, 2]);

        assert!(clip_targets_skeleton(
            &clip("animations/explicit-partial.caf", &[1, 3, 4, 5]),
            &skeleton,
        ));
        assert!(!clip_targets_skeleton(
            &clip("animations/explicit-foreign.caf", &[3, 4]),
            &skeleton,
        ));
    }

    fn resolved_asset(controller_ids: &[u32]) -> ResolvedAsset {
        ResolvedAsset {
            model: nw_model::Model {
                meshes: Vec::new(),
                skeletons: vec![skeleton(controller_ids)],
                auxiliary_nodes: Vec::new(),
            },
            materials: None,
            animations: Vec::new(),
            extras: nw_model::CryAssetExtras::default(),
            physics: nw_model::PhysicsScene::default(),
            animation_asset_evaluations: HashMap::new(),
        }
    }

    fn decoded(path: &str, clips: Vec<cry_animation::AnimationClip>) -> DecodedAnimationAsset {
        DecodedAnimationAsset {
            path: path.to_owned(),
            clips,
        }
    }

    #[test]
    fn explicit_retry_binds_automatic_rejection_and_clears_unbound_marker() {
        let automatic_path = "Animations\\Foreign.CAF";
        let explicit_path = "animations/foreign.caf";
        let mut resolved = resolved_asset(&[1]);

        evaluate_animation_asset(
            decoded(automatic_path, vec![clip(automatic_path, &[1, 2])]),
            0,
            AnimationBindingPolicy::AutomaticCompatible,
            &mut resolved,
        )
        .unwrap();
        scope_animation_dependencies(&[automatic_path.to_owned()], &mut resolved);
        assert!(resolved.animations.is_empty());
        assert!(resolved.extras.dependencies.is_empty());
        assert_eq!(resolved.extras.unbound_animations.len(), 1);
        assert_eq!(
            unevaluated_animation_paths(
                &[explicit_path.to_owned()],
                0,
                AnimationBindingPolicy::ExplicitPermissive,
                &resolved,
            ),
            [explicit_path]
        );

        evaluate_animation_asset(
            decoded(explicit_path, vec![clip(explicit_path, &[1, 2])]),
            0,
            AnimationBindingPolicy::ExplicitPermissive,
            &mut resolved,
        )
        .unwrap();
        scope_animation_dependencies(&[explicit_path.to_owned()], &mut resolved);
        ensure_required_animation_mappings(&[explicit_path.to_owned()], 0, &resolved).unwrap();

        assert_eq!(resolved.animations.len(), 1);
        assert!(resolved.extras.unbound_animations.is_empty());
        assert_eq!(resolved.extras.dependencies, [explicit_path]);
    }

    #[test]
    fn cached_explicit_failure_preserves_required_error() {
        let path = "animations/foreign.caf";
        let mut resolved = resolved_asset(&[1]);
        evaluate_animation_asset(
            decoded(path, vec![clip(path, &[2])]),
            0,
            AnimationBindingPolicy::ExplicitPermissive,
            &mut resolved,
        )
        .unwrap();

        let error = ensure_required_animation_mappings(&[path.to_owned()], 0, &resolved)
            .unwrap_err()
            .to_string();
        assert!(error.contains("CAF animations/foreign.caf has no controllers"));
    }

    #[test]
    fn animation_path_identity_is_case_and_separator_insensitive() {
        let paths = normalized_unique_paths(&[
            "Animations\\Walk.CAF".to_owned(),
            "animations/walk.caf".to_owned(),
        ]);
        assert_eq!(paths.len(), 1);
        let mut resolved = resolved_asset(&[1]);
        resolved.animation_asset_evaluations.insert(
            AnimationAssetEvaluation::new(
                0,
                "Animations\\Walk.CAF",
                AnimationBindingPolicy::AutomaticCompatible,
            ),
            true,
        );

        assert!(
            unevaluated_animation_paths(
                &["animations/walk.caf".to_owned()],
                0,
                AnimationBindingPolicy::AutomaticCompatible,
                &resolved,
            )
            .is_empty()
        );
    }

    #[test]
    fn dba_dependency_requires_at_least_one_bound_clip() {
        let path = "animations/database.dba";
        let mut rejected = resolved_asset(&[1]);
        rejected.extras.dependencies.push(path.to_owned());
        evaluate_animation_asset(
            decoded(path, vec![clip("animations/foreign.caf", &[2])]),
            0,
            AnimationBindingPolicy::AutomaticCompatible,
            &mut rejected,
        )
        .unwrap();
        scope_animation_dependencies(&[path.to_owned()], &mut rejected);
        assert!(rejected.extras.dependencies.is_empty());

        let mut mixed = resolved_asset(&[1]);
        evaluate_animation_asset(
            decoded(
                path,
                vec![
                    clip("animations/bound.caf", &[1]),
                    clip("animations/foreign.caf", &[2]),
                ],
            ),
            0,
            AnimationBindingPolicy::AutomaticCompatible,
            &mut mixed,
        )
        .unwrap();
        scope_animation_dependencies(&[path.to_owned()], &mut mixed);
        assert_eq!(mixed.animations.len(), 1);
        assert_eq!(mixed.extras.unbound_animations.len(), 1);
        assert_eq!(mixed.extras.dependencies, [path]);
    }

    #[test]
    fn animation_dependency_remains_when_any_skeleton_binds_the_asset() {
        let path = "animations/shared.caf";
        let mut resolved = resolved_asset(&[1]);
        resolved.model.skeletons.push(skeleton(&[2]));

        evaluate_animation_asset(
            decoded(path, vec![clip(path, &[1])]),
            0,
            AnimationBindingPolicy::AutomaticCompatible,
            &mut resolved,
        )
        .unwrap();
        scope_animation_dependencies(&[path.to_owned()], &mut resolved);
        evaluate_animation_asset(
            decoded(path, vec![clip(path, &[3])]),
            1,
            AnimationBindingPolicy::AutomaticCompatible,
            &mut resolved,
        )
        .unwrap();
        scope_animation_dependencies(&[path.to_owned()], &mut resolved);

        assert_eq!(resolved.animations.len(), 1);
        assert_eq!(resolved.extras.dependencies, [path]);
    }

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
            animation_asset_evaluations: HashMap::new(),
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
