//! Alias-aware BSPACE/COMB dependency scoping over admitted animation clips.

use super::*;

struct BlendSpaceRequirements {
    path: String,
    animation_keys: Vec<Vec<String>>,
    children: Vec<String>,
}

pub(in crate::model_asset) fn scope_blend_space_dependencies(
    source: &dyn AssetSource,
    resolved: &mut ResolvedAsset,
) -> Result<()> {
    let paths = normalized_unique_paths(
        &resolved
            .extras
            .dependencies
            .iter()
            .filter(|path| cry_mannequin::BlendSpaceXmlKind::from_source_path(path).is_some())
            .cloned()
            .collect::<Vec<_>>(),
    );
    if paths.is_empty() {
        return Ok(());
    }

    let mut requirements = load_blend_space_requirements(source, &paths)?;
    resolve_blend_space_children(&mut requirements);
    let references = bound_animation_references(source, resolved)?;
    let retained = retained_blend_spaces(&requirements, &references);
    let rejected = requirements
        .into_iter()
        .filter(|requirement| !retained.contains(&requirement.path))
        .map(|requirement| requirement.path)
        .collect::<HashSet<_>>();
    resolved
        .extras
        .dependencies
        .retain(|path| !rejected.contains(&canonical_asset_path(path)));
    resolved
        .extras
        .source_assets
        .retain(|asset| !rejected.contains(&canonical_asset_path(&asset.path)));
    resolved
        .extras
        .resource_payloads
        .retain(|resource| !rejected.contains(&canonical_asset_path(&resource.source_path)));
    resolved
        .extras
        .embedded_resources
        .retain(|resource| !rejected.contains(&canonical_asset_path(&resource.source_path)));
    Ok(())
}

fn load_blend_space_requirements(
    source: &dyn AssetSource,
    paths: &[String],
) -> Result<Vec<BlendSpaceRequirements>> {
    paths
        .iter()
        .map(|path| {
            let bytes = read_required(source, path)?;
            let document = cry_mannequin::BlendSpaceDocumentSource::from_legacy(path, &bytes)
                .with_context(|| format!("parse blend-space dependency {path}"))?;
            let (animations, children) = match document {
                cry_mannequin::BlendSpaceDocumentSource::BlendSpace(source) => (
                    source
                        .blend_space
                        .examples
                        .iter()
                        .map(|example| example.animation.name.clone())
                        .chain(
                            source
                                .blend_space
                                .motion_combinations
                                .iter()
                                .map(|combination| combination.animation.name.clone()),
                        )
                        .filter(|name| !name.trim().is_empty())
                        .collect::<Vec<_>>(),
                    Vec::new(),
                ),
                cry_mannequin::BlendSpaceDocumentSource::CombinedBlendSpace(source) => (
                    source
                        .combined_blend_space
                        .motion_combinations
                        .iter()
                        .map(|combination| combination.animation.name.clone())
                        .filter(|name| !name.trim().is_empty())
                        .collect::<Vec<_>>(),
                    source
                        .combined_blend_space
                        .blend_spaces
                        .iter()
                        .map(|child| canonical_asset_path(&child.path))
                        .collect(),
                ),
            };
            Ok(BlendSpaceRequirements {
                path: canonical_asset_path(path),
                animation_keys: animations
                    .iter()
                    .map(|animation| animation_reference_keys(animation))
                    .collect(),
                children,
            })
        })
        .collect()
}

fn resolve_blend_space_children(requirements: &mut [BlendSpaceRequirements]) {
    let available = requirements
        .iter()
        .map(|requirement| requirement.path.clone())
        .collect::<HashSet<_>>();
    for requirement in requirements {
        let directory = requirement
            .path
            .rsplit_once('/')
            .map(|(directory, _)| directory);
        for child in &mut requirement.children {
            if available.contains(child) || child.starts_with("animations/") {
                continue;
            }
            if let Some(directory) = directory {
                let relative = canonical_asset_path(&format!("{directory}/{child}"));
                if available.contains(&relative) {
                    *child = relative;
                }
            }
        }
    }
}

fn retained_blend_spaces(
    requirements: &[BlendSpaceRequirements],
    references: &HashMap<usize, HashSet<String>>,
) -> HashSet<String> {
    let mut retained = references
        .keys()
        .map(|skeleton| (*skeleton, HashSet::<String>::new()))
        .collect::<HashMap<_, _>>();
    loop {
        let before = retained.values().map(HashSet::len).sum::<usize>();
        for (skeleton, references) in references {
            let additions = {
                let current = retained.get(skeleton).expect("initialized skeleton scope");
                requirements
                    .iter()
                    .filter(|requirement| {
                        requirement
                            .animation_keys
                            .iter()
                            .all(|keys| animation_reference_is_bound(references, keys))
                            && requirement
                                .children
                                .iter()
                                .all(|child| current.contains(child))
                    })
                    .map(|requirement| requirement.path.clone())
                    .collect::<Vec<_>>()
            };
            retained
                .get_mut(skeleton)
                .expect("initialized skeleton scope")
                .extend(additions);
        }
        if retained.values().map(HashSet::len).sum::<usize>() == before {
            break;
        }
    }
    retained.into_values().flatten().collect()
}

fn bound_animation_references(
    source: &dyn AssetSource,
    resolved: &ResolvedAsset,
) -> Result<HashMap<usize, HashSet<String>>> {
    let mut references = (0..resolved.model.skeletons.len())
        .map(|skeleton| (skeleton, HashSet::new()))
        .collect::<HashMap<_, _>>();
    let mut paths = HashMap::<usize, HashSet<String>>::new();
    for animation in &resolved.animations {
        let set = references.entry(animation.skeleton).or_default();
        add_animation_reference_keys(set, &animation.clip.name);
        add_animation_reference_keys(set, &animation.clip.source_path);
        paths
            .entry(animation.skeleton)
            .or_default()
            .insert(canonical_asset_path(&animation.clip.source_path));
    }

    for (alias, path) in character_animation_aliases(source, resolved)? {
        for (skeleton, bound_paths) in &paths {
            if bound_paths.contains(&path) {
                references
                    .entry(*skeleton)
                    .or_default()
                    .insert(alias.clone());
            }
        }
    }
    Ok(references)
}

fn character_animation_aliases(
    source: &dyn AssetSource,
    resolved: &ResolvedAsset,
) -> Result<Vec<(String, String)>> {
    let mut documents = HashMap::new();
    for asset in resolved.extras.source_assets.iter().filter(|asset| {
        matches!(
            asset.kind,
            nw_model::CrySourceAssetKind::CharacterParameters
        )
    }) {
        let path = canonical_asset_path(&asset.path);
        if documents.contains_key(&path) {
            continue;
        }
        let bytes = read_required(source, &asset.path)?;
        let xml = str::from_utf8(&bytes)
            .with_context(|| format!("decode UTF-8 character parameters {}", asset.path))?;
        let parameters = CharacterParameters::from_xml(xml)
            .with_context(|| format!("parse character parameters {}", asset.path))?;
        documents.insert(path, parameters);
    }

    let included = documents
        .values()
        .flat_map(|parameters| &parameters.animations)
        .filter(|entry| entry.kind == CharacterAnimationEntryKind::Include)
        .map(|entry| canonical_asset_path(&entry.path))
        .collect::<HashSet<_>>();
    let mut roots = documents
        .keys()
        .filter(|path| !included.contains(*path))
        .cloned()
        .collect::<Vec<_>>();
    if roots.is_empty() {
        roots.extend(documents.keys().cloned());
    }

    let mut aliases = Vec::new();
    for path in roots {
        collect_character_animation_aliases(
            &documents,
            &path,
            &mut CharacterAnimationPathResolver::default(),
            &mut HashSet::new(),
            &mut aliases,
        )?;
    }
    Ok(aliases)
}

fn collect_character_animation_aliases(
    documents: &HashMap<String, CharacterParameters>,
    source_path: &str,
    resolver: &mut CharacterAnimationPathResolver,
    visited: &mut HashSet<String>,
    aliases: &mut Vec<(String, String)>,
) -> Result<()> {
    let source_path = canonical_asset_path(source_path);
    if !visited.insert(source_path.clone()) {
        return Ok(());
    }
    let parameters = documents
        .get(&source_path)
        .with_context(|| format!("included character parameters were not loaded: {source_path}"))?;
    for entry in &parameters.animations {
        let path = resolver.resolve_entry(entry);
        match entry.kind {
            CharacterAnimationEntryKind::Include => {
                collect_character_animation_aliases(documents, &path, resolver, visited, aliases)?
            }
            CharacterAnimationEntryKind::Asset
                if matches!(source_extension(&path).as_str(), "caf" | "i_caf")
                    && !entry.name.trim().is_empty()
                    && entry.name != "*" =>
            {
                aliases.push((
                    normalize_path(&entry.name).to_ascii_lowercase(),
                    canonical_asset_path(&path),
                ));
            }
            _ => {}
        }
    }
    Ok(())
}

fn animation_reference_is_bound(references: &HashSet<String>, keys: &[String]) -> bool {
    keys.iter().any(|key| references.contains(key))
}

fn add_animation_reference_keys(references: &mut HashSet<String>, reference: &str) {
    references.extend(animation_reference_keys(reference));
}

fn animation_reference_keys(reference: &str) -> Vec<String> {
    let normalized = normalize_path(reference).trim().to_ascii_lowercase();
    if normalized.is_empty() {
        return Vec::new();
    }
    let stem = [".anim.glb", ".i_caf", ".caf"]
        .into_iter()
        .find_map(|extension| normalized.strip_suffix(extension))
        .unwrap_or(&normalized)
        .to_owned();
    let mut keys = vec![normalized, stem.clone()];
    if stem.contains('/') && !stem.starts_with("animations/") {
        keys.push(format!("animations/{stem}"));
    }
    keys
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, str};

    use glam::Mat4;

    use super::*;
    use crate::model_asset::tests::ContextSource;

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

    #[test]
    fn dependencies_follow_aliases_and_prune_transitive_foreign_controls() {
        let parameters_path = "objects/test.chrparams";
        let parameters_xml = br##"<Params><AnimationList>
            <Animation name="#filepath" path="animations/shared"/>
            <Animation name="LocomotionAlias" path="bound.caf"/>
        </AnimationList></Params>"##;
        let direct = "animations/shared/direct.bspace";
        let full_path = "animations/shared/full_path.bspace";
        let alias = "animations/shared/alias.bspace";
        let mixed = "animations/shared/mixed.bspace";
        let path_collision = "animations/shared/path_collision.bspace";
        let split = "animations/shared/split.bspace";
        let direct_combined = "animations/shared/direct.comb";
        let rejected_combined = "animations/shared/rejected.comb";
        let source = ContextSource::default()
            .with(parameters_path, parameters_xml)
            .with(
                direct,
                br#"<ParaGroup><Dimensions><Param Name="MoveSpeed"/></Dimensions><ExampleList><Example AName="bound"/></ExampleList></ParaGroup>"#,
            )
            .with(
                full_path,
                br#"<ParaGroup><Dimensions><Param Name="MoveSpeed"/></Dimensions><ExampleList><Example AName="Animations/Shared/Bound.CAF"/></ExampleList></ParaGroup>"#,
            )
            .with(
                alias,
                br#"<ParaGroup><Dimensions><Param Name="MoveSpeed"/></Dimensions><ExampleList><Example AName="locomotionalias"/></ExampleList></ParaGroup>"#,
            )
            .with(
                mixed,
                br#"<ParaGroup><Dimensions><Param Name="MoveSpeed"/></Dimensions><ExampleList><Example AName="bound"/><Example AName="foreign"/></ExampleList></ParaGroup>"#,
            )
            .with(
                path_collision,
                br#"<ParaGroup><Dimensions><Param Name="MoveSpeed"/></Dimensions><ExampleList><Example AName="foreign/bound.caf"/></ExampleList></ParaGroup>"#,
            )
            .with(
                split,
                br#"<ParaGroup><Dimensions><Param Name="MoveSpeed"/></Dimensions><ExampleList><Example AName="bound"/><Example AName="other"/></ExampleList></ParaGroup>"#,
            )
            .with(
                direct_combined,
                br#"<CombinedBlendSpace><Dimensions><Param Name="DesiredFacing"/></Dimensions><BlendSpaces><BlendSpace AName="direct.bspace"/><BlendSpace AName="ALIAS.BSPACE"/></BlendSpaces></CombinedBlendSpace>"#,
            )
            .with(
                rejected_combined,
                br#"<CombinedBlendSpace><Dimensions><Param Name="DesiredFacing"/></Dimensions><BlendSpaces><BlendSpace AName="mixed.bspace"/></BlendSpaces></CombinedBlendSpace>"#,
            );
        let mut resolved = resolved_asset(&[1]);
        resolved.model.skeletons.push(skeleton(&[2]));
        resolved.animations.extend([
            nw_model::ModelAnimation {
                skeleton: 0,
                clip: clip("animations/shared/bound.caf", &[1]),
            },
            nw_model::ModelAnimation {
                skeleton: 1,
                clip: clip("animations/shared/other.caf", &[2]),
            },
        ]);
        let parameters = CharacterParameters::from_xml(str::from_utf8(parameters_xml).unwrap())
            .expect("parse test character parameters");
        add_xml_source(
            &mut resolved.extras,
            parameters_path,
            nw_model::CrySourceAssetKind::CharacterParameters,
            &parameters.source,
        );
        let documents = [
            direct,
            full_path,
            alias,
            mixed,
            path_collision,
            split,
            direct_combined,
            rejected_combined,
        ];
        for path in documents {
            add_mannequin_source(&source, path, &mut resolved.extras).unwrap();
            resolved
                .extras
                .embedded_resources
                .push(nw_model::CryEmbeddedResource {
                    source_path: path.to_owned(),
                    kind: if path.ends_with(".comb") {
                        nw_model::CryEmbeddedResourceKind::CombinedBlendSpace
                    } else {
                        nw_model::CryEmbeddedResourceKind::BlendSpace
                    },
                    buffer_view: 0,
                    mime_type: "application/octet-stream".to_owned(),
                });
        }

        scope_blend_space_dependencies(&source, &mut resolved).unwrap();

        let retained = [direct, full_path, alias, direct_combined];
        assert_eq!(
            resolved
                .extras
                .dependencies
                .iter()
                .filter(|path| {
                    cry_mannequin::BlendSpaceXmlKind::from_source_path(path).is_some()
                })
                .map(String::as_str)
                .collect::<Vec<_>>(),
            retained
        );
        assert_eq!(
            resolved
                .extras
                .source_assets
                .iter()
                .filter(|asset| {
                    matches!(
                        asset.kind,
                        nw_model::CrySourceAssetKind::BlendSpace
                            | nw_model::CrySourceAssetKind::CombinedBlendSpace
                    )
                })
                .map(|asset| asset.path.as_str())
                .collect::<Vec<_>>(),
            retained
        );
        assert_eq!(
            resolved
                .extras
                .resource_payloads
                .iter()
                .map(|resource| resource.source_path.as_str())
                .collect::<Vec<_>>(),
            retained
        );
        assert_eq!(
            resolved
                .extras
                .embedded_resources
                .iter()
                .map(|resource| resource.source_path.as_str())
                .collect::<Vec<_>>(),
            retained
        );
    }
}
