//! Context-proven alternate character-definition discovery.
//!
//! A candidate CDF must share render identity with the requested CDF and be
//! selected by an entity with the same complete Mannequin controller/ADB family.
//! The walk starts only from explicitly requested roots, so variants cannot grow
//! the candidate set recursively.

use super::mannequin;
use super::sources::model_context_assets;
use super::*;

#[derive(Debug)]
struct CharacterRenderIdentity {
    skeleton: String,
    bindings: Vec<String>,
}

impl CharacterRenderIdentity {
    fn assets(&self) -> impl Iterator<Item = &str> {
        std::iter::once(self.skeleton.as_str()).chain(self.bindings.iter().map(String::as_str))
    }
}

/// Discover sibling character definitions proven to be alternate contexts for
/// `source_path`, without recursively expanding their own context consumers.
pub(crate) fn context_variant_cdfs(
    source: &dyn AssetSource,
    source_path: &str,
    index: &nw_asset_graph::AssetDependencyIndex,
) -> Result<Vec<String>> {
    if source_extension(source_path) != "cdf" {
        return Ok(Vec::new());
    }
    let source_path = normalize_path(source_path);
    let source_identity = character_render_identity(source, &source_path)?;
    let mut candidates = source_identity
        .assets()
        .flat_map(|path| index.consumers_of(path))
        .filter(|edge| source_extension(edge.source()) == "cdf")
        .map(nw_asset_graph::AssetDependencyEdge::source)
        .filter(|path| !path.eq_ignore_ascii_case(&source_path))
        .map(normalize_path)
        .collect::<Vec<_>>();
    candidates.sort_by_key(|path| path.to_ascii_lowercase());
    candidates.dedup_by(|left, right| left.eq_ignore_ascii_case(right));

    let candidate_identities = candidates
        .into_iter()
        .map(|candidate| {
            character_render_identity(source, &candidate).map(|identity| (candidate, identity))
        })
        .collect::<Result<Vec<_>>>()?;
    let mut binding_owners = std::collections::BTreeMap::new();
    for (_, identity) in &candidate_identities {
        for binding in &identity.bindings {
            *binding_owners
                .entry(binding.to_ascii_lowercase())
                .or_insert(0usize) += 1;
        }
    }
    let mut candidates = candidate_identities
        .into_iter()
        .filter(|(_, identity)| {
            identity.bindings.iter().any(|binding| {
                source_identity
                    .bindings
                    .iter()
                    .any(|source| source.eq_ignore_ascii_case(binding))
                    || binding_owners
                        .get(&binding.to_ascii_lowercase())
                        .is_some_and(|owners| *owners > 1)
            })
        })
        .map(|(candidate, _)| candidate)
        .collect::<Vec<_>>();
    candidates.sort_by_key(|path| path.to_ascii_lowercase());
    candidates.dedup_by(|left, right| left.eq_ignore_ascii_case(right));
    if candidates.is_empty() {
        return Ok(Vec::new());
    }

    let mut source_families = mannequin::character_mannequin_contexts(
        source,
        &model_context_assets(source, &source_path, index),
    )?
    .into_iter()
    .filter(|context| context.cdf_path.eq_ignore_ascii_case(&source_path))
    .map(|context| context.family)
    .collect::<Vec<_>>();
    source_families.sort();
    source_families.dedup();
    if source_families.is_empty() {
        return Ok(Vec::new());
    }

    let mut candidate_scenes = candidates
        .iter()
        .flat_map(|path| index.consumers_of(path))
        .map(nw_asset_graph::AssetDependencyEdge::source)
        .filter(|path| is_legacy_scene_asset(path))
        .map(str::to_owned)
        .collect::<Vec<_>>();
    candidate_scenes.sort_by_key(|path| normalize_path(path).to_ascii_lowercase());
    candidate_scenes.dedup_by(|left, right| left.eq_ignore_ascii_case(right));
    let candidate_contexts = mannequin::character_mannequin_contexts(source, &candidate_scenes)?;

    let mut variants = Vec::new();
    for candidate in candidates {
        let mut families = candidate_contexts
            .iter()
            .filter(|context| context.cdf_path.eq_ignore_ascii_case(&candidate))
            .map(|context| context.family.clone())
            .collect::<Vec<_>>();
        families.sort();
        families.dedup();
        if families
            .iter()
            .any(|family| source_families.binary_search(family).is_ok())
        {
            variants.push(candidate);
            continue;
        }
        if let Some((source_family, candidate_family)) = source_families.iter().find_map(|source| {
            families
                .iter()
                .find(|candidate| source.overlaps(candidate))
                .map(|candidate| (source, candidate))
        }) {
            eprintln!(
                "note: context variant {candidate} shares render identity with {source_path} but has a different Mannequin family (source: [{}]; candidate: [{}]); skipped",
                source_family.describe(),
                candidate_family.describe(),
            );
        }
    }
    Ok(variants)
}

fn character_render_identity(
    source: &dyn AssetSource,
    source_path: &str,
) -> Result<CharacterRenderIdentity> {
    let bytes = read_required(source, source_path)?;
    let xml = str::from_utf8(&bytes).with_context(|| format!("decode UTF-8 CDF {source_path}"))?;
    let definition = CharacterDefinition::from_xml(xml)
        .with_context(|| format!("parse character definition {source_path}"))?;
    let mut bindings = definition
        .attachments
        .iter()
        .filter(|attachment| matches!(attachment.kind, AttachmentKind::Skin | AttachmentKind::Face))
        .filter_map(|attachment| attachment.binding.as_deref())
        .map(normalize_path)
        .collect::<Vec<_>>();
    bindings.sort_by_key(|path| path.to_ascii_lowercase());
    bindings.dedup_by(|left, right| left.eq_ignore_ascii_case(right));
    Ok(CharacterRenderIdentity {
        skeleton: normalize_path(&definition.model.skeleton),
        bindings,
    })
}

fn is_legacy_scene_asset(path: &str) -> bool {
    matches!(
        source_extension(path).as_str(),
        "slice" | "dynamicslice" | "entity" | "entities" | "entities_xml" | "prefab"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model_asset::tests::ContextSource;

    fn character_cdf(skeleton: &str, skin: &str) -> Vec<u8> {
        format!(
            r#"<CharacterDefinition><Model File="{skeleton}"/><AttachmentList><Attachment Type="CA_SKIN" Binding="{skin}"/></AttachmentList></CharacterDefinition>"#
        )
        .into_bytes()
    }

    fn character_scene(cdf: &str, adb: &str) -> Vec<u8> {
        format!(
            r#"<ObjectStream version="3"><Class name="Asset" type="{{77A19D40-8731-4D3C-9041-1B43047366A4}}" value="id={{7A1472D1-DF54-5362-BC71-9974D5F25572}}:0,type={{78802ABF-9595-463A-8D2B022F906F9B1}},hint={{{cdf}}}"/><Class name="AZ::Entity" type="{{75651658-8663-478D-9090-2432DFCAFA44}}"><Class name="Components" field="Components" type="{{0D23B755-6E8F-5C6C-B7C9-A352A55DC1DF}}"><Class name="ActionListComponent" type="{{30ED0ACE-51DD-48B9-BA41-2FA6775CD106}}"><Class name="AZStd::string" field="m_animationDatabase" value="{adb}" type="{{03AAAB3F-5C47-5A66-9EBC-D5FA4DB353C9}}"/></Class><Class name="CharacterComponent" type="{{15407CAA-4970-4D06-8B5C-612FBA11BB45}}"><Class name="AzFramework::SimpleAssetReference&lt;CharacterDefinitionAsset&gt;" field="m_cdfPath" type="{{A1342558-687A-406A-8BE4-784D6546589D}}"><Class name="SimpleAssetReferenceBase" field="BaseClass1" type="{{E16CA6C5-5C78-4AD9-8E9B-F8C1FB4D1DB8}}"><Class name="AZStd::string" field="AssetPath" value="{cdf}" type="{{03AAAB3F-5C47-5A66-9EBC-D5FA4DB353C9}}"/></Class></Class></Class></Class></Class></ObjectStream>"#
        )
        .into_bytes()
    }

    #[test]
    fn context_variant_requires_shared_identity_and_exact_mannequin_family() {
        const BASE_CDF: &str = "objects/base.cdf";
        const MATCHING_CDF: &str = "objects/matching.cdf";
        const DIFFERENT_FAMILY_CDF: &str = "objects/different_family.cdf";
        const SKELETON_ONLY_CDF: &str = "objects/skeleton_only.cdf";
        const SHARED_SKIN: &str = "objects/shared.skin";
        const UNIQUE_SKIN: &str = "objects/unique.skin";
        const BASE_ADB: &str = "animations/base.adb";
        const OTHER_ADB: &str = "animations/other.adb";
        let source = ContextSource::default()
            .with(BASE_CDF, character_cdf("objects/base.chr", SHARED_SKIN))
            .with(
                MATCHING_CDF,
                character_cdf("objects/matching.chr", SHARED_SKIN),
            )
            .with(
                DIFFERENT_FAMILY_CDF,
                character_cdf("objects/different.chr", SHARED_SKIN),
            )
            .with(
                SKELETON_ONLY_CDF,
                character_cdf("objects/base.chr", UNIQUE_SKIN),
            )
            .with("objects/base.chr", b"model")
            .with("objects/matching.chr", b"model")
            .with("objects/different.chr", b"model")
            .with(SHARED_SKIN, b"model")
            .with(UNIQUE_SKIN, b"model")
            .with(BASE_ADB, b"adb")
            .with(OTHER_ADB, b"adb")
            .with(
                "slices/base.dynamicslice",
                character_scene(BASE_CDF, BASE_ADB),
            )
            .with(
                "slices/matching.dynamicslice",
                character_scene(MATCHING_CDF, BASE_ADB),
            )
            .with(
                "slices/different.dynamicslice",
                character_scene(DIFFERENT_FAMILY_CDF, OTHER_ADB),
            )
            .with(
                "slices/skeleton_only.dynamicslice",
                character_scene(SKELETON_ONLY_CDF, BASE_ADB),
            );
        let paths = vec![
            BASE_CDF.to_owned(),
            MATCHING_CDF.to_owned(),
            DIFFERENT_FAMILY_CDF.to_owned(),
            SKELETON_ONLY_CDF.to_owned(),
            "slices/base.dynamicslice".to_owned(),
            "slices/matching.dynamicslice".to_owned(),
            "slices/different.dynamicslice".to_owned(),
            "slices/skeleton_only.dynamicslice".to_owned(),
        ];
        let index = nw_asset_graph::AssetDependencyIndex::build_with_runner(
            &source,
            &paths,
            &nw_jobs::JobRunner::inline(),
        )
        .unwrap();

        assert_eq!(
            context_variant_cdfs(&source, BASE_CDF, &index).unwrap(),
            vec![MATCHING_CDF.to_owned()]
        );
    }
}
