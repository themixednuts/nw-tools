//! Cry material-set resolution for the primary mesh, basename-only `MtlName`
//! references, and CDF/attachment material tables.
//!
//! Split out of `model_asset` as a pure move; shared helpers stay in the parent.

use super::*;

pub(super) fn resolve_primary_materials(
    source: &dyn AssetSource,
    cgf: &[u8],
    mesh: &MeshRef,
    material_override: Option<&str>,
    no_materials: bool,
    has_geometry: bool,
) -> Result<Option<nw_model::MaterialSet>> {
    if no_materials || !has_geometry {
        return Ok(None);
    }
    if let Some(xml) = material_override {
        return Ok(Some(xml.parse().context("parse --mtl material XML")?));
    }
    if let Some(materials) = source.materials(cgf, mesh) {
        return Ok(Some(materials));
    }
    resolve_material_name(source, cgf)?.map(Some).context(
        "mesh material could not be resolved; use --no-materials for an explicit geometry-only export",
    )
}

/// Resolve Cry's legacy basename-only MtlName references without guessing when
/// multiple shipped paths share that basename. All candidates must project to
/// the same lossless material document.
fn resolve_material_name(
    source: &dyn AssetSource,
    cgf: &[u8],
) -> Result<Option<nw_model::MaterialSet>> {
    let file = cry_chunk::CgfFile::parse(cgf)?;
    let Some(material) = file.materials().values().next() else {
        return Ok(None);
    };
    let mut name = normalize_path(material.name.as_str());
    if name.is_empty() {
        return Ok(None);
    }
    if !name.to_ascii_lowercase().ends_with(".mtl") {
        name.push_str(".mtl");
    }
    let basename = name.rsplit('/').next().unwrap_or(&name);
    let mut paths = if name.contains('/') {
        vec![name.clone()]
    } else {
        source.matching_paths(&format!("**/{basename}"))?
    };
    paths.sort_by_key(|path| path.to_ascii_lowercase());
    paths.dedup_by(|left, right| left.eq_ignore_ascii_case(right));

    let mut resolved: Option<nw_model::MaterialSet> = None;
    for path in paths {
        let Some(bytes) = source.read(&path) else {
            continue;
        };
        let xml =
            str::from_utf8(&bytes).with_context(|| format!("decode UTF-8 material {path}"))?;
        let candidate = xml
            .parse::<nw_model::MaterialSet>()
            .with_context(|| format!("parse material {path}"))?;
        if resolved
            .as_ref()
            .is_some_and(|existing| existing != &candidate)
        {
            bail!("ambiguous basename-only Cry material reference {name}");
        }
        resolved = Some(candidate);
    }
    Ok(resolved)
}

pub(super) fn append_material_table(
    model: &mut nw_model::Model,
    combined: &mut Option<nw_model::MaterialSet>,
    set: nw_model::MaterialSet,
) -> Result<()> {
    let table = combined.get_or_insert_with(nw_model::MaterialSet::default);
    let offset = table.append(set);
    model.rebase_material_ids(offset)?;
    Ok(())
}

pub(super) fn load_material(source: &dyn AssetSource, path: &str) -> Result<nw_model::MaterialSet> {
    let path = if source_extension(path) == "mtl" {
        normalize_path(path)
    } else {
        format!("{}.mtl", normalize_path(path))
    };
    let bytes = read_required(source, &path)?;
    let xml = str::from_utf8(&bytes).with_context(|| format!("decode UTF-8 material {path}"))?;
    xml.parse()
        .with_context(|| format!("parse material {path}"))
}
