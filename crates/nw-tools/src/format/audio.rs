//! `format audio` — validate and inspect legacy CryAudio/Wwise assets.

use std::path::{Path, PathBuf};
use std::str;

use anyhow::{Context, Result, bail};
use clap::Args;

use crate::support::{ensure_parent, guard_existing};
use crate::ui::{Cell, OutputFormat, Report, Table, print};

#[derive(Debug, Args)]
pub struct Audio {
    /// ATL `.xml`, Wwise `.bnk`/`.wem`, or `triggerbankmapatlbin.bin` input.
    path: PathBuf,

    /// Write pretty JSON metadata to this path instead of stdout.
    #[arg(long, value_name = "FILE")]
    out: Option<PathBuf>,

    /// For BNK input, extract and validate every DIDX/DATA embedded WEM here.
    #[arg(long = "out-dir", alias = "extract-embedded", value_name = "DIR")]
    extract_embedded: Option<PathBuf>,

    /// Replace existing JSON or extracted WEM files.
    #[arg(long = "force")]
    overwrite: bool,

    /// Inspect the input and print the write plan without creating output files.
    #[arg(long)]
    dry_run: bool,
}

impl Audio {
    pub fn run(self) -> Result<()> {
        let bytes = std::fs::read(&self.path)
            .with_context(|| format!("read audio asset {}", self.path.display()))?;
        let inspected = inspect(&self.path, &bytes)?;

        if self.dry_run {
            let mut report = Report::new("audio")
                .stat("dry-run", "yes")
                .stat("source", self.path.display());
            if let Some(out) = &self.out {
                report = report.stat("json-output", out.display());
            }
            if let Some(directory) = &self.extract_embedded {
                report = report.stat("embedded-output", directory.display());
            }
            report.print();
            return Ok(());
        }

        if let Some(directory) = &self.extract_embedded {
            extract_embedded(&self.path, &bytes, directory, self.overwrite)?;
        }

        let json = serde_json::to_string_pretty(&inspected)?;
        if let Some(out) = &self.out {
            guard_existing(out, self.overwrite.into())?;
            ensure_parent(out)?;
            std::fs::write(out, format!("{json}\n"))?;
            Report::new("audio")
                .stat("source", self.path.display())
                .stat("output", out.display())
                .print();
        } else {
            print_inspection(&self.path, &inspected);
        }
        Ok(())
    }
}

fn print_inspection(path: &Path, inspected: &serde_json::Value) {
    if print::output_format() == OutputFormat::Json {
        print::print_json(inspected);
        return;
    }

    let kind = inspected
        .get("kind")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("unknown");
    let mut report = Report::new("audio")
        .stat("source", path.display())
        .stat("kind", kind);
    let mut table = Table::new(["Field", "Value"]);
    if let Some(fields) = inspected.as_object() {
        for (name, value) in fields {
            if name != "kind" {
                table.push([
                    Cell::text(name),
                    Cell::text(serde_json::to_string(value).unwrap_or_default()),
                ]);
            }
        }
    }
    report.table_or(table, "no metadata fields");
    report.print();
}

fn inspect(path: &Path, bytes: &[u8]) -> Result<serde_json::Value> {
    let normalized = path
        .to_string_lossy()
        .replace('\\', "/")
        .to_ascii_lowercase();
    if normalized.ends_with(cry_audio::WWISE_TRIGGER_BANK_MAP_FILE)
        || normalized.ends_with("triggerbankmapatlbin.bin")
    {
        let entries = cry_audio::WwiseTriggerBankMap::parse(bytes)?
            .entries()
            .collect::<Vec<_>>();
        return Ok(serde_json::json!({
            "kind": "wwiseTriggerBankMap",
            "recordCount": entries.len(),
            "entries": entries,
        }));
    }

    match extension(path).as_deref() {
        Some("xml") => {
            let xml = str::from_utf8(bytes).context("decode ATL XML as UTF-8")?;
            let source = cry_audio::AudioControlsSource::from_xml(&normalized, xml)?;
            Ok(serde_json::json!({ "kind": "audioControls", "document": source }))
        }
        Some("bnk") => {
            let bank = cry_audio::WwiseSoundBank::parse(bytes)?;
            let embedded_media = bank
                .media
                .iter()
                .copied()
                .map(|entry| {
                    let media = bank.embedded_media(bytes, entry)?;
                    let info = cry_audio::WwiseMediaInfo::parse(media)?;
                    Ok::<_, anyhow::Error>(serde_json::json!({
                        "mediaId": entry.id,
                        "info": info,
                    }))
                })
                .collect::<Result<Vec<_>>>()?;
            Ok(serde_json::json!({
                "kind": "wwiseSoundBank",
                "bank": bank,
                "embeddedMedia": embedded_media,
            }))
        }
        Some("wem") => Ok(serde_json::json!({
            "kind": "wwiseMedia",
            "info": cry_audio::WwiseMediaInfo::parse(bytes)?,
        })),
        _ => bail!("unsupported audio input {}", path.display()),
    }
}

fn extract_embedded(path: &Path, bytes: &[u8], directory: &Path, overwrite: bool) -> Result<()> {
    if extension(path).as_deref() != Some("bnk") {
        bail!("--extract-embedded requires a Wwise .bnk input");
    }
    let bank = cry_audio::WwiseSoundBank::parse(bytes)?;
    std::fs::create_dir_all(directory)?;
    for entry in bank.media.iter().copied() {
        let media = bank.embedded_media(bytes, entry)?;
        cry_audio::WwiseMediaInfo::parse(media)
            .with_context(|| format!("validate embedded WEM {}", entry.id.0))?;
        let out = directory.join(format!("{}.wem", entry.id.0));
        guard_existing(&out, overwrite.into())?;
        std::fs::write(out, media)?;
    }
    Ok(())
}

fn extension(path: &Path) -> Option<String> {
    path.extension()
        .map(|extension| extension.to_string_lossy().to_ascii_lowercase())
}
