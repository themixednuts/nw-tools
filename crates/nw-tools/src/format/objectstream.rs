use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use clap::Args;
use humansize::{DECIMAL, format_size};
use nw_objectstream::ObjectStreamEncoding;
use nw_objectstream::lookup::NameLookup;

use crate::jobs::{JobArgs, RunCtx};
use crate::support::{
    MatchMode, PathSelector, collect_matching, load_lookup, path_ext, write_guarded,
};
use crate::ui::{Cell, Report, Table};

use super::common::{EncodingArg, finish_scan, lowered, path_label, trim_cell};

#[derive(Debug, Args)]
pub struct ObjectStream {
    /// ObjectStream file or directory. Omit to browse files under the current directory.
    path: Option<PathBuf>,

    #[arg(long, conflicts_with = "to")]
    dom: bool,

    #[arg(long, conflicts_with = "to")]
    query: Option<String>,

    /// Exact substring match instead of the default fuzzy ranking (with --query).
    #[arg(long)]
    exact: bool,

    /// Convert ObjectStream files to this encoding.
    #[arg(long, value_enum)]
    to: Option<EncodingArg>,

    /// Conversion output file or directory. Defaults beside each input.
    #[arg(long, value_name = "PATH", requires = "to")]
    out: Option<PathBuf>,

    /// Replace existing conversion outputs.
    #[arg(long, requires = "to")]
    overwrite: bool,

    /// Case-insensitive path substring prefilter.
    #[arg(long)]
    filter: Option<String>,

    /// Path glob prefilter; repeat for multiple patterns.
    #[arg(long)]
    glob: Vec<String>,

    #[arg(long, default_value_t = 40)]
    show: usize,

    #[arg(long, default_value_t = 20)]
    files: usize,

    #[arg(long)]
    no_names: bool,

    /// Optional extension prefilter before content sniffing.
    #[arg(long = "ext")]
    extensions: Vec<String>,

    #[command(flatten)]
    jobs: JobArgs,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ObjectMode {
    Stats,
    Dom { limit: usize },
}

#[derive(Debug, Clone)]
enum ObjectScan {
    Stats(ObjectStatsScan),
    Dom(ObjectDomScan),
    Search {
        source: String,
        hits: Vec<ObjectHit>,
    },
}

#[derive(Debug, Clone)]
struct ObjectStatsScan {
    source: String,
    stats: nw_objectstream::stats::Stats,
    names_loaded: bool,
}

#[derive(Debug, Clone)]
struct ObjectDomScan {
    source: String,
    version: u32,
    top_level_elements: usize,
    total_elements: usize,
    rows: Vec<ObjectDomRow>,
}

#[derive(Debug, Clone)]
struct ObjectDomRow {
    index: String,
    flags: String,
    id: String,
    type_name: String,
    field: String,
}

#[derive(Debug, Clone)]
struct ObjectHit {
    kind: String,
    count: u64,
    score: u32,
    value: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ObjectConvertRow {
    source: String,
    output: String,
    encoding: String,
    bytes: String,
}

impl ObjectStream {
    pub(super) fn run(self) -> Result<()> {
        let ctx = self.jobs.ctx()?;
        let lookup = load_lookup(self.no_names)?;
        let selector = PathSelector::new(self.filter, self.glob);
        let root = self.path.clone().unwrap_or_else(|| PathBuf::from("."));
        let paths = objectstream_paths(&root, &self.extensions, &selector)?;
        if let Some(encoding) = self.to {
            return convert_objectstreams(
                &ctx,
                &root,
                &paths,
                self.out.as_deref(),
                encoding.into(),
                self.overwrite,
                lookup.as_ref(),
            );
        }

        if self.dom && self.query.is_none() && crate::tui::interactive() {
            return browse_objectstreams(&paths, lookup.as_ref());
        }

        let mode = if self.dom {
            ObjectMode::Dom { limit: self.show }
        } else {
            ObjectMode::Stats
        };
        let query = self.query.clone();
        let match_mode = MatchMode::from_flags(false, false, self.exact);
        let batch = ctx.map_results_compact(
            "objectstream",
            &paths,
            |path| path_label(path),
            |path, progress| {
                progress.step(|| {
                    scan_objectstream(
                        path,
                        mode,
                        query.as_deref(),
                        match_mode,
                        self.show,
                        lookup.as_ref(),
                    )
                })
            },
        );
        let skipped = batch.skipped();
        let cancelled = batch.was_cancelled();
        let mut scans = Vec::new();
        let mut errors = Vec::new();

        for result in batch.into_completed() {
            match result {
                Ok(Some(scan)) => scans.push(scan),
                Ok(None) => {}
                Err(error) => errors.push(error),
            }
        }
        scans.sort_by(|left, right| object_source(left).cmp(object_source(right)));

        let shown = scans.len().min(self.files);
        let mut report = Report::new("objectstream")
            .stat("files", scans.len())
            .stat("shown", shown)
            .stat("names", if lookup.is_some() { "loaded" } else { "off" });
        for scan in scans.into_iter().take(self.files) {
            match scan {
                ObjectScan::Stats(scan) => push_object_stats(&mut report, &scan),
                ObjectScan::Dom(scan) => push_object_dom(&mut report, &scan),
                ObjectScan::Search { source, hits } => {
                    push_object_search(&mut report, &source, hits)
                }
            }
        }
        report.print();

        finish_scan(cancelled, skipped, &errors, "objectstream")
    }
}

fn convert_objectstreams(
    ctx: &RunCtx,
    root: &Path,
    paths: &[PathBuf],
    out: Option<&Path>,
    encoding: ObjectStreamEncoding,
    overwrite: bool,
    lookup: Option<&NameLookup>,
) -> Result<()> {
    let batch = ctx.map_results_compact(
        "objectstream conversion",
        paths,
        |path| path_label(path),
        |path, progress| {
            progress.step(|| convert_objectstream(root, path, out, encoding, overwrite, lookup))
        },
    );
    let skipped = batch.skipped();
    let cancelled = batch.was_cancelled();
    let mut converted = Vec::new();
    let mut errors = Vec::new();

    for result in batch.into_completed() {
        match result {
            Ok(Some(row)) => converted.push(row),
            Ok(None) => {}
            Err(error) => errors.push(error),
        }
    }
    converted.sort_by(|left, right| left.source.cmp(&right.source));

    let mut report = Report::new("objectstream conversion")
        .stat("objectstreams", paths.len())
        .stat("converted", converted.len())
        .stat("encoding", encoding);
    let mut table = Table::new(["Source", "Output", "Encoding", "Bytes"]).right([3]);
    for row in converted {
        table.push([
            Cell::path(row.source),
            Cell::path(row.output),
            Cell::text(row.encoding),
            Cell::size(row.bytes),
        ]);
    }
    report.table_or(table, "no ObjectStreams converted");
    report.print();

    finish_scan(cancelled, skipped, &errors, "objectstream conversion")
}

fn convert_objectstream(
    root: &Path,
    path: &Path,
    out: Option<&Path>,
    encoding: ObjectStreamEncoding,
    overwrite: bool,
    lookup: Option<&NameLookup>,
) -> Result<Option<ObjectConvertRow>> {
    let bytes = std::fs::read(path).with_context(|| format!("read {}", path.display()))?;
    let Some(payload) = objectstream_payload(&bytes)
        .with_context(|| format!("decode wrapper for {}", path.display()))?
    else {
        return Ok(None);
    };
    let converted = nw_objectstream::ObjectStream::transcode_bytes(&payload, encoding, lookup)
        .with_context(|| format!("convert {}", path.display()))?;
    let output = objectstream_output_path(root, path, out, encoding);
    write_guarded(&output, &converted, overwrite.into())?;

    Ok(Some(ObjectConvertRow {
        source: path_label(path),
        output: output.display().to_string(),
        encoding: encoding.to_string(),
        bytes: format_size(converted.len(), DECIMAL),
    }))
}

fn scan_objectstream(
    path: &Path,
    mode: ObjectMode,
    query: Option<&str>,
    match_mode: MatchMode,
    limit: usize,
    lookup: Option<&NameLookup>,
) -> Result<Option<ObjectScan>> {
    let bytes = std::fs::read(path).with_context(|| format!("read {}", path.display()))?;
    let Some(bytes) = objectstream_payload(&bytes)
        .with_context(|| format!("decode wrapper for {}", path.display()))?
    else {
        return Ok(None);
    };
    let source = path.display().to_string();
    if let Some(query) = query {
        let needle = query.to_ascii_lowercase();
        let mut search = match_mode.is_fuzzy().then(|| crate::fuzzy::Search::new(query));
        let hits =
            nw_objectstream::query::collect_search_matches(
                &bytes,
                lookup,
                |value| match &mut search {
                    Some(search) => search.score(value).map(u32::from),
                    None => value.to_ascii_lowercase().contains(&needle).then_some(1),
                },
            )
            .with_context(|| format!("search {}", path.display()))?;
        let mut hits = hits
            .into_iter()
            .map(|(hit, stats)| ObjectHit {
                kind: hit.kind.label().to_string(),
                count: stats.count,
                score: stats.score,
                value: trim_cell(hit.value),
            })
            .collect::<Vec<_>>();
        hits.sort_by(|left, right| {
            right
                .score
                .cmp(&left.score)
                .then(right.count.cmp(&left.count))
                .then(left.kind.cmp(&right.kind))
                .then(left.value.cmp(&right.value))
        });
        hits.truncate(limit);
        return Ok(Some(ObjectScan::Search { source, hits }));
    }

    match mode {
        ObjectMode::Stats => {
            let stats = nw_objectstream::stats::Stats::from_bytes(&bytes, lookup)
                .with_context(|| format!("inspect {}", path.display()))?;
            Ok(Some(ObjectScan::Stats(ObjectStatsScan {
                source,
                stats,
                names_loaded: lookup.is_some(),
            })))
        }
        ObjectMode::Dom { limit } => {
            let stream = nw_objectstream::ObjectStream::from_bytes(&bytes, lookup)
                .with_context(|| format!("parse {}", path.display()))?;
            Ok(Some(ObjectScan::Dom(object_dom_scan(
                source, &stream, limit,
            ))))
        }
    }
}

fn browse_objectstreams(paths: &[PathBuf], lookup: Option<&NameLookup>) -> Result<()> {
    if paths.is_empty() {
        Report::new("objectstream").stat("files", 0usize).print();
        return Ok(());
    }
    if paths.len() == 1 {
        return open_objectstream_tree(&paths[0], lookup);
    }

    loop {
        let mut table = Table::new(["ObjectStream", "Bytes"]).right([1]);
        for path in paths {
            let size = std::fs::metadata(path).map(|meta| meta.len()).unwrap_or(0);
            table.push([
                Cell::path(path_label(path)),
                Cell::size(format_size(size, DECIMAL)),
            ]);
        }
        let stats = vec![("objectstreams".to_string(), paths.len().to_string())];
        match crate::tui::pick("objectstreams", stats, table, 0)? {
            Some(selection) if !selection.is_empty() => {
                open_objectstream_tree(Path::new(&selection), lookup)?;
            }
            _ => return Ok(()),
        }
    }
}

fn open_objectstream_tree(path: &Path, lookup: Option<&NameLookup>) -> Result<()> {
    let bytes = std::fs::read(path).with_context(|| format!("read {}", path.display()))?;
    let Some(bytes) = objectstream_payload(&bytes)
        .with_context(|| format!("decode wrapper for {}", path.display()))?
    else {
        bail!("{} is not an ObjectStream payload", path.display());
    };
    let stream = nw_objectstream::ObjectStream::from_bytes(&bytes, lookup)
        .with_context(|| format!("parse {}", path.display()))?;
    let mut nodes = Vec::new();
    collect_tree_nodes(stream.elements(), 0, &mut nodes);
    crate::tui::tree(path.display().to_string(), nodes)?;
    Ok(())
}

fn collect_tree_nodes(
    elements: &[nw_objectstream::Element],
    depth: usize,
    nodes: &mut Vec<crate::tui::TreeNode>,
) {
    for element in elements {
        let unresolved_type = element.name().is_empty();
        let type_name = if unresolved_type {
            "<unknown-type>".to_string()
        } else {
            element.name().to_string()
        };
        let (field, unresolved_field) = match element.field() {
            Some(name) => (name.to_string(), false),
            None => match element.name_crc() {
                Some(crc) => (format!("#{crc:08x}"), true),
                None => (String::new(), false),
            },
        };
        let meta = format!("id {} flags {:#04x}", element.id(), element.flags);
        let children = element.children();
        nodes.push(crate::tui::TreeNode {
            depth,
            children: children.len(),
            type_name,
            field,
            meta,
            unresolved_type,
            unresolved_field,
        });
        collect_tree_nodes(children, depth + 1, nodes);
    }
}

fn object_dom_scan(
    source: String,
    stream: &nw_objectstream::ObjectStream,
    limit: usize,
) -> ObjectDomScan {
    let mut rows = Vec::new();
    let mut total_elements = 0usize;
    for (index, element) in stream.iter_recursive().enumerate() {
        total_elements += 1;
        if rows.len() < limit {
            let type_name = if element.name().is_empty() {
                "<unknown-type>".to_string()
            } else {
                element.name().to_string()
            };
            rows.push(ObjectDomRow {
                index: index.to_string(),
                flags: format!("{:#04x}", element.flags),
                id: element.id().to_string(),
                type_name,
                field: element
                    .field()
                    .map_or_else(String::new, ToString::to_string),
            });
        }
    }

    ObjectDomScan {
        source,
        version: stream.version(),
        top_level_elements: stream.elements().len(),
        total_elements,
        rows,
    }
}

fn push_object_stats(report: &mut Report, scan: &ObjectStatsScan) {
    let stats = scan.stats;
    report.section(format!("{} ({})", scan.source, stats.mode_label()));
    report.kv("version", stats.version.to_string());
    report.kv("elements", stats.elements.to_string());
    report.kv("max depth", stats.max_depth.to_string());
    report.kv("bytes", stats.bytes.to_string());
    if scan.names_loaded {
        report.kv(
            "resolved",
            format!(
                "{} elements had a known type, {} fields had a known name",
                stats.resolved_types, stats.resolved_fields
            ),
        );
    } else {
        report.kv("resolved", "(no serialize.json - names unresolved)");
    }
}

fn push_object_dom(report: &mut Report, scan: &ObjectDomScan) {
    report.section(format!("{} (DOM)", scan.source));
    report.kv("version", scan.version.to_string());
    report.kv("top-level elements", scan.top_level_elements.to_string());
    if !scan.rows.is_empty() {
        let mut table = Table::new(["Index", "Flags", "Id", "Type", "Field"]).right([0]);
        for row in &scan.rows {
            table.push([
                Cell::text(row.index.clone()),
                Cell::dim(row.flags.clone()),
                Cell::text(row.id.clone()),
                Cell::text(row.type_name.clone()),
                Cell::text(row.field.clone()),
            ]);
        }
        report.table(table);
    }
    let remaining = scan.total_elements.saturating_sub(scan.rows.len());
    if remaining > 0 {
        report.more(remaining, "element(s)");
    }
}

fn push_object_search(report: &mut Report, source: &str, hits: Vec<ObjectHit>) {
    report.section(format!("{source}: {} hit group(s)", hits.len()));
    let mut table = Table::new(["Kind", "Count", "Score", "Value"]).right([1, 2]);
    for hit in hits {
        table.push([
            Cell::text(hit.kind),
            Cell::text(hit.count.to_string()),
            Cell::text(hit.score.to_string()),
            Cell::text(hit.value),
        ]);
    }
    report.table_or(table, "no ObjectStream matches");
}

fn objectstream_paths(
    root: &Path,
    extensions: &[String],
    selector: &PathSelector,
) -> Result<Vec<PathBuf>> {
    if root.is_file() {
        return Ok(path_selected(root, root, selector)
            .then(|| root.to_path_buf())
            .into_iter()
            .collect());
    }

    let extensions = lowered(extensions.to_vec());
    collect_matching(root, |path| {
        (extensions.is_empty()
            || path_ext(path).is_some_and(|extension| extensions.contains(&extension)))
            && path_selected(root, path, selector)
    })
}

fn objectstream_output_path(
    root: &Path,
    source: &Path,
    out: Option<&Path>,
    encoding: ObjectStreamEncoding,
) -> PathBuf {
    let Some(out) = out else {
        return objectstream_encoded_path(source.to_path_buf(), encoding);
    };
    if root.is_file() {
        return out.to_path_buf();
    }

    let relative = source.strip_prefix(root).unwrap_or(source);
    objectstream_encoded_path(out.join(relative), encoding)
}

fn objectstream_encoded_path(path: PathBuf, encoding: ObjectStreamEncoding) -> PathBuf {
    let stripped = strip_objectstream_text_extension(&path);
    match encoding.extension() {
        "" => stripped,
        extension => {
            let mut output = stripped.clone();
            if let Some(file_name) = stripped.file_name().and_then(|name| name.to_str()) {
                output.set_file_name(format!("{file_name}.{extension}"));
            }
            output
        }
    }
}

fn strip_objectstream_text_extension(path: &Path) -> PathBuf {
    let Some(extension) = path.extension().and_then(|extension| extension.to_str()) else {
        return path.to_path_buf();
    };
    if !matches!(extension.to_ascii_lowercase().as_str(), "json" | "xml") {
        return path.to_path_buf();
    }

    let Some(stem) = path.file_stem() else {
        return path.to_path_buf();
    };
    let mut stripped = path.to_path_buf();
    stripped.set_file_name(stem);
    stripped
}

fn path_selected(root: &Path, path: &Path, selector: &PathSelector) -> bool {
    let relative = nw_filesystem::display_relative(root, path);
    if !relative.is_empty() && selector.matches(&relative) {
        return true;
    }
    if path
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| selector.matches(name))
    {
        return true;
    }
    selector.matches(&path.display().to_string())
}

fn objectstream_payload(bytes: &[u8]) -> Result<Option<Vec<u8>>> {
    if nw_objectstream::looks_like_objectstream(bytes) {
        return Ok(Some(bytes.to_vec()));
    }

    if !nw_pak::azcs::is_azcs(bytes) {
        return Ok(None);
    }

    let mut cursor = Cursor::new(bytes);
    let mut reader = nw_pak::azcs::decompress(&mut cursor)?;
    let mut decoded = Vec::new();
    reader.read_to_end(&mut decoded)?;
    Ok(nw_objectstream::looks_like_objectstream(&decoded).then_some(decoded))
}

fn object_source(scan: &ObjectScan) -> &str {
    match scan {
        ObjectScan::Stats(scan) => &scan.source,
        ObjectScan::Dom(scan) => &scan.source,
        ObjectScan::Search { source, .. } => source,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn objectstream_directory_conversion_paths_use_target_encoding() {
        let root = Path::new("in");
        let source = Path::new("in/slices/player.slice.json");
        let out = Path::new("out");

        assert_eq!(
            objectstream_output_path(root, source, Some(out), ObjectStreamEncoding::Xml),
            PathBuf::from("out/slices/player.slice.xml")
        );
        assert_eq!(
            objectstream_output_path(root, source, Some(out), ObjectStreamEncoding::Binary),
            PathBuf::from("out/slices/player.slice")
        );
    }

    #[test]
    fn objectstream_conversion_defaults_next_to_input() {
        let source = Path::new("in/slices/player.slice.json");

        assert_eq!(
            objectstream_output_path(Path::new("in"), source, None, ObjectStreamEncoding::Binary),
            PathBuf::from("in/slices/player.slice")
        );
        assert_eq!(
            objectstream_output_path(
                Path::new("in"),
                Path::new("in/slices/player.slice"),
                None,
                ObjectStreamEncoding::Xml
            ),
            PathBuf::from("in/slices/player.slice.xml")
        );
    }
}
