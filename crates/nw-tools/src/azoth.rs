//! Stable process boundary used by the AZoth Blender extension.
//!
//! Python owns Blender presentation only. Engine-faithful event/audio scheduling
//! stays here beside the legacy readers and is returned as versioned JSON.

use std::collections::BTreeSet;
use std::fs;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use clap::{Args, Subcommand};

use crate::ui::{OutputFormat, Report, print};

#[derive(Debug, Subcommand)]
pub(crate) enum Cmd {
    #[command(
        about = "Describe a structured package without returning its heavy authored payloads"
    )]
    Describe(Describe),
    #[command(about = "Build an engine-faithful Blender animation/audio schedule")]
    Schedule(Schedule),
}

impl Cmd {
    pub(crate) fn run(self) -> Result<()> {
        match self {
            Self::Describe(command) => command.run(),
            Self::Schedule(command) => command.run(),
        }
    }
}

#[derive(Debug, Args)]
pub(crate) struct Describe {
    /// Structured nw-tools `.gltf` or `.glb` manifest.
    manifest: PathBuf,

    /// Root containing catalog-mirrored shared resources.
    #[arg(long, default_value = r"C:\nwt")]
    package_root: PathBuf,
}

impl Describe {
    fn run(self) -> Result<()> {
        let document = read_document(&self.manifest)?;
        let description = describe_document(&document, &self.manifest, &self.package_root);
        print_document("azoth describe", &description)?;
        Ok(())
    }
}

#[derive(Debug, Args)]
pub(crate) struct Schedule {
    /// Structured nw-tools `.gltf` or `.glb` manifest.
    manifest: PathBuf,

    /// Root containing the manifest's catalog-mirrored shared resources.
    #[arg(long, default_value = r"C:\nwt")]
    package_root: PathBuf,
}

impl Schedule {
    fn run(self) -> Result<()> {
        let document = read_document(&self.manifest)?;
        let schedule = crate::audio_export::blend_schedule_document(&document, &self.package_root)?;
        print_document(
            "azoth schedule",
            &serde_json::json!({
                "schemaVersion": 1,
                "schedule": schedule,
            }),
        )?;
        Ok(())
    }
}

fn print_document(title: &str, document: &serde_json::Value) -> Result<()> {
    if print::output_format() == OutputFormat::Json {
        print::print_json(document);
        return Ok(());
    }

    let mut report = Report::new(title);
    for line in serde_json::to_string_pretty(document)?.lines() {
        report.note(line);
    }
    report.print();
    Ok(())
}

fn read_document(path: &Path) -> Result<serde_json::Value> {
    if path
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("glb"))
    {
        return read_glb_document(path);
    }
    serde_json::from_slice(&fs::read(path).with_context(|| format!("read {}", path.display()))?)
        .with_context(|| format!("parse glTF JSON {}", path.display()))
}

fn read_glb_document(path: &Path) -> Result<serde_json::Value> {
    let mut file = fs::File::open(path).with_context(|| format!("open {}", path.display()))?;
    let mut header = [0_u8; 12];
    file.read_exact(&mut header)
        .with_context(|| format!("read GLB header {}", path.display()))?;
    if &header[..4] != b"glTF" || u32::from_le_bytes(header[4..8].try_into()?) != 2 {
        bail!("{} is not a glTF 2 GLB", path.display());
    }
    loop {
        let mut chunk_header = [0_u8; 8];
        file.read_exact(&mut chunk_header)
            .with_context(|| format!("read GLB JSON chunk {}", path.display()))?;
        let length = u32::from_le_bytes(chunk_header[..4].try_into()?) as usize;
        let kind = u32::from_le_bytes(chunk_header[4..].try_into()?);
        if kind == 0x4e4f_534a {
            let mut json = vec![0_u8; length];
            file.read_exact(&mut json)?;
            return serde_json::from_slice(&json).context("parse GLB JSON chunk");
        }
        file.seek(SeekFrom::Current(i64::try_from(length)?))?;
    }
}

fn describe_document(
    document: &serde_json::Value,
    manifest: &Path,
    package_root: &Path,
) -> serde_json::Value {
    let extras = document.get("extras").unwrap_or(&serde_json::Value::Null);
    let mut resources = BTreeSet::<(String, String, String)>::new();
    let mut add = |kind: &str, path: &str, status: &str| {
        if !path.is_empty() {
            resources.insert((kind.to_owned(), path.replace('\\', "/"), status.to_owned()));
        }
    };
    for item in extras
        .get("sourceAssets")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
    {
        add(json_text(item, "kind"), json_text(item, "path"), "ready");
        add_character_attachments(item, &mut add);
    }
    for item in extras
        .get("embeddedResources")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
    {
        add(
            json_text(item, "kind"),
            json_text(item, "sourcePath"),
            "ready",
        );
    }
    for path in extras
        .get("dependencies")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(serde_json::Value::as_str)
    {
        let status = if package_root.join(path.replace('/', "\\")).is_file() {
            "ready"
        } else {
            "indexed"
        };
        add("dependency", path, status);
    }
    for (index, mesh) in json_array(document, "meshes").enumerate() {
        add("glTFMesh", json_name(mesh, "mesh", index).as_str(), "ready");
    }
    for (index, skin) in json_array(document, "skins").enumerate() {
        add("glTFSkin", json_name(skin, "skin", index).as_str(), "ready");
    }
    for (index, material) in json_array(document, "materials").enumerate() {
        add(
            "glTFMaterial",
            json_name(material, "material", index).as_str(),
            "ready",
        );
    }
    for (index, image) in json_array(document, "images").enumerate() {
        let path = image
            .get("uri")
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned)
            .unwrap_or_else(|| json_name(image, "image", index));
        add("glTFTexture", &path, "ready");
    }
    for node in json_array(document, "nodes") {
        let node_extras = node.get("extras").unwrap_or(&serde_json::Value::Null);
        if let Some(particle) = node_extras.get("particleEmitter") {
            let emitter = particle
                .get("selectedEmitter")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default();
            let label = if json_text(node, "name").is_empty() {
                emitter.to_owned()
            } else {
                format!("{} -> {emitter}", json_text(node, "name"))
            };
            add("particleEmitter", &label, "ready");
            add_alternate_paths(particle.get("context"), &mut add);
        }
        if let Some(physics) = node_extras.get("physics") {
            let index = physics
                .get("index")
                .or_else(|| physics.get("shape"))
                .and_then(serde_json::Value::as_u64)
                .unwrap_or_default();
            add(
                json_text(physics, "kind"),
                &format!("{} [index:{index}]", json_text(node, "name")),
                "ready",
            );
        }
        if node_extras.get("role").and_then(serde_json::Value::as_str) == Some("clothSimulation") {
            add("clothSimulation", json_text(node, "name"), "ready");
        }
    }
    add_alternate_paths(extras.get("physics"), &mut add);
    for item in extras
        .get("unboundAnimations")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
    {
        add("unboundAnimation", json_text(item, "sourcePath"), "unbound");
    }
    for item in extras
        .get("unboundParticleEmitters")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
    {
        add(
            "unboundParticleEmitter",
            item.get("context")
                .map(|context| json_text(context, "sourcePath"))
                .unwrap_or_default(),
            "unbound",
        );
    }

    let mut diagnostics = Vec::new();
    for (table, path_field) in [("buffers", "uri"), ("images", "uri")] {
        for item in json_array(document, table) {
            let uri = json_text(item, path_field);
            if uri.is_empty() || uri.starts_with("data:") {
                continue;
            }
            let beside = manifest
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .join(uri);
            let catalog = package_root.join(uri.replace('/', "\\"));
            let status = if beside.is_file() || catalog.is_file() {
                "ready"
            } else {
                diagnostics.push(format!("missing external resource: {uri}"));
                "missing"
            };
            add("glTFResource", uri, status);
        }
    }
    if let Some(count) = extras
        .get("unboundAnimations")
        .and_then(serde_json::Value::as_array)
        .map(Vec::len)
        .filter(|count| *count != 0)
    {
        diagnostics.push(format!("{count} unbound animation(s)"));
    }
    if let Some(count) = extras
        .get("unboundParticleEmitters")
        .and_then(serde_json::Value::as_array)
        .map(Vec::len)
        .filter(|count| *count != 0)
    {
        diagnostics.push(format!("{count} unbound particle emitter(s)"));
    }

    let animations = json_array(document, "animations")
        .enumerate()
        .map(|(index, animation)| {
            let animation_extras = animation
                .get("extras")
                .unwrap_or(&serde_json::Value::Null);
            serde_json::json!({
                "name": json_name(animation, "animation", index),
                "sourcePath": json_text(animation_extras, "crySourcePath"),
                "duration": animation_extras.get("cryDuration").and_then(serde_json::Value::as_f64).unwrap_or(0.0),
                "sampleRate": animation_extras.get("crySampleRate").and_then(serde_json::Value::as_f64).unwrap_or(0.0),
                "eventCount": animation_extras.get("cryEvents").and_then(serde_json::Value::as_array).map_or(0, Vec::len),
                "audioCount": animation_extras.get("cryMannequinAudio").and_then(serde_json::Value::as_array).map_or(0, Vec::len),
            })
        })
        .collect::<Vec<_>>();
    let resources = resources
        .into_iter()
        .map(|(kind, path, status)| serde_json::json!({"kind": kind, "path": path, "status": status}))
        .collect::<Vec<_>>();
    serde_json::json!({
        "schemaVersion": 1,
        "sourcePath": json_text(extras, "sourcePath"),
        "resources": resources,
        "animations": animations,
        "diagnostics": diagnostics,
    })
}

fn add_character_attachments(item: &serde_json::Value, add: &mut impl FnMut(&str, &str, &str)) {
    if json_text(item, "kind") != "characterDefinition" {
        return;
    }
    let Some(children) = item
        .get("document")
        .and_then(|document| document.get("children"))
        .and_then(serde_json::Value::as_array)
    else {
        return;
    };
    let Some(list) = children
        .iter()
        .find(|child| json_text(child, "name") == "AttachmentList")
    else {
        return;
    };
    for attachment in list
        .get("children")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
    {
        let attributes = attachment
            .get("attributes")
            .unwrap_or(&serde_json::Value::Null);
        let kind = json_text(attributes, "Type");
        let path = ["Binding", "AName", "BoneName"]
            .into_iter()
            .map(|field| json_text(attributes, field))
            .find(|value| !value.is_empty())
            .unwrap_or("attachment");
        add(&format!("characterAttachment:{kind}"), path, "ready");
    }
}

fn add_alternate_paths(value: Option<&serde_json::Value>, add: &mut impl FnMut(&str, &str, &str)) {
    let Some(value) = value else { return };
    if let Some(paths) = value
        .get("alternateSourcePaths")
        .and_then(serde_json::Value::as_array)
    {
        for path in paths.iter().filter_map(serde_json::Value::as_str) {
            add("variant", path, "ready");
        }
    }
    if let Some(object) = value.as_object() {
        for child in object
            .values()
            .filter(|child| child.is_object() || child.is_array())
        {
            add_alternate_paths(Some(child), add);
        }
    } else if let Some(array) = value.as_array() {
        for child in array {
            add_alternate_paths(Some(child), add);
        }
    }
}

fn json_array<'a>(
    value: &'a serde_json::Value,
    field: &str,
) -> impl Iterator<Item = &'a serde_json::Value> {
    value
        .get(field)
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
}

fn json_name(value: &serde_json::Value, fallback: &str, index: usize) -> String {
    value
        .get("name")
        .and_then(serde_json::Value::as_str)
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| format!("{fallback}_{index}"))
}

fn json_text<'a>(value: &'a serde_json::Value, field: &str) -> &'a str {
    value
        .get(field)
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn description_preserves_repeated_hit_volumes_and_particle_variants() {
        let document = serde_json::json!({
            "extras": {},
            "nodes": [
                {
                    "name": "pelvis_hit",
                    "extras": {"physics": {"kind": "hitVolume", "index": 0}}
                },
                {
                    "name": "pelvis_hit",
                    "extras": {"physics": {"kind": "hitVolume", "index": 1}}
                },
                {
                    "name": "hand_fx",
                    "extras": {
                        "particleEmitter": {
                            "selectedEmitter": "magic.cast",
                            "context": {
                                "alternateSourcePaths": ["characters/mage.variant"]
                            }
                        }
                    }
                }
            ]
        });
        let description =
            describe_document(&document, Path::new("character.gltf"), Path::new(r"C:\nwt"));
        let resources = description["resources"].as_array().unwrap();
        let hit_volumes = resources
            .iter()
            .filter(|resource| resource["kind"] == "hitVolume")
            .collect::<Vec<_>>();
        assert_eq!(hit_volumes.len(), 2);
        assert_ne!(hit_volumes[0]["path"], hit_volumes[1]["path"]);
        assert!(resources.iter().any(|resource| {
            resource["kind"] == "particleEmitter" && resource["path"] == "hand_fx -> magic.cast"
        }));
        assert!(resources.iter().any(|resource| {
            resource["kind"] == "variant" && resource["path"] == "characters/mage.variant"
        }));
    }

    #[test]
    fn reads_binary_glb_for_the_shared_description_and_schedule_path() {
        let document = serde_json::json!({
            "asset": {"version": "2.0"},
            "extras": {},
            "animations": []
        });
        let mut json = serde_json::to_vec(&document).unwrap();
        json.resize(json.len().next_multiple_of(4), b' ');
        let total_length = 20 + json.len();
        let mut glb = Vec::with_capacity(total_length);
        glb.extend_from_slice(b"glTF");
        glb.extend_from_slice(&2_u32.to_le_bytes());
        glb.extend_from_slice(&(total_length as u32).to_le_bytes());
        glb.extend_from_slice(&(json.len() as u32).to_le_bytes());
        glb.extend_from_slice(&0x4e4f_534a_u32.to_le_bytes());
        glb.extend_from_slice(&json);
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("character.glb");
        fs::write(&path, glb).unwrap();

        let decoded = read_document(&path).unwrap();
        assert_eq!(decoded, document);
        let schedule =
            crate::audio_export::blend_schedule_document(&decoded, directory.path()).unwrap();
        let encoded = serde_json::to_value(schedule).unwrap();
        assert!(encoded["clips"].as_array().unwrap().is_empty());
    }
}
