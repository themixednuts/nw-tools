use std::path::PathBuf;

use clap::{Args, ValueEnum};
use nw_datasheet::game_system::Crc32;
use nw_tools::grep::adapter::FactKind;
use nw_tools::grep::find::{self, Source};
use nw_tools::index::{ContentIndex, CrcCase, IndexedHit, Lookup, LookupTarget};
use nw_tools::ui::{Cell, OutputFormat, Report, Table, print};
use serde::Serialize;

#[derive(Debug, Args)]
pub struct Cmd {
    /// Query terms, OR-combined; fuzzy unless --exact is set.
    #[arg(required_unless_present = "crc", conflicts_with = "crc")]
    queries: Vec<String>,

    /// Look up an unsigned CRC32 in decimal or hexadecimal with a 0x prefix.
    #[arg(long, value_name = "CRC", value_parser = parse_crc, allow_hyphen_values = true)]
    crc: Option<u32>,

    /// Token role for CRC lookup.
    #[arg(long, value_enum, default_value = "value", requires = "crc")]
    target: Target,

    /// CRC bytes: original or ASCII lowercase. Defaults to original for fields, lower otherwise.
    #[arg(long = "case", value_enum, requires = "crc")]
    case: Option<Case>,

    /// Match literal substrings instead of fuzzy text.
    #[arg(long, conflicts_with = "crc")]
    exact: bool,

    /// Match filename facts only.
    #[arg(long, conflicts_with_all = ["content_only", "crc"])]
    name_only: bool,

    /// Match content facts only.
    #[arg(long, conflicts_with_all = ["name_only", "crc"])]
    content_only: bool,

    /// Extract live even when a complete index is available.
    #[arg(long)]
    live: bool,

    /// Existing index database; defaults to the local cache. Never builds an index.
    #[arg(long, value_name = "DB")]
    index: Option<PathBuf>,

    #[command(flatten)]
    root: nw_tools::support::AssetRootArg,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum Target {
    Value,
    Field,
    Name,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum Case {
    Original,
    Lower,
}

fn parse_crc(value: &str) -> Result<u32, String> {
    let parsed = match value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
    {
        Some(hex) => u32::from_str_radix(hex, 16),
        None => value.parse::<u32>(),
    };
    parsed.map_err(|_| {
        "CRC must be an unsigned 32-bit decimal integer or 0x-prefixed hexadecimal integer".into()
    })
}

#[derive(Serialize)]
struct Hit {
    pak: String,
    entry: String,
    format: String,
    location: String,
    field: String,
    value: String,
    snippet: String,
    score: u16,
}

impl Cmd {
    pub fn run(self) -> anyhow::Result<()> {
        let root = self.root.resolve()?;
        let index = match &self.index {
            Some(path) if !path.is_file() => {
                anyhow::bail!("index does not exist: {}", path.display())
            }
            Some(path) if !self.live => Some(ContentIndex::open(path)?),
            Some(_) => None,
            None if !self.live => nw_tools::index::open_existing(),
            None => None,
        };
        let indexed = index.as_ref().filter(|index| index.is_complete(&root));
        let source = match indexed {
            Some(index) => Source::Indexed(index),
            None => Source::Live(&root),
        };
        let session = nw_tools::resources::session();
        let facts = match self.crc {
            Some(crc) => find::lookup(
                source,
                session.overlay(),
                Lookup {
                    target: match self.target {
                        Target::Value => LookupTarget::Value,
                        Target::Field => LookupTarget::Field,
                        Target::Name => LookupTarget::Name,
                    },
                    case: match self.case {
                        Some(Case::Original) => CrcCase::Original,
                        Some(Case::Lower) => CrcCase::Lower,
                        None if matches!(self.target, Target::Field) => CrcCase::Original,
                        None => CrcCase::Lower,
                    },
                    crc: Crc32::new(crc),
                },
            )?,
            None => find::search(source, session.overlay(), &self.queries, self.exact)?,
        };
        let queries: Vec<_> = self
            .queries
            .iter()
            .map(|query| query.to_ascii_lowercase())
            .collect();
        let mut matcher = nw_tools::fuzzy::MultiSearch::new(&queries);
        let mut hits: Vec<(bool, Hit)> = facts
            .into_iter()
            .filter(|hit| {
                (!self.name_only || hit.kind == FactKind::FileName)
                    && (!self.content_only || hit.kind != FactKind::FileName)
            })
            .map(|hit: IndexedHit| {
                let name = hit.kind == FactKind::FileName;
                let score = if self.crc.is_some() || self.exact {
                    0
                } else {
                    matcher
                        .score_any(
                            [
                                hit.field.to_ascii_lowercase(),
                                hit.value.to_ascii_lowercase(),
                            ]
                            .iter()
                            .map(String::as_str),
                        )
                        .unwrap_or(0)
                };
                (
                    name,
                    Hit {
                        pak: hit.pak,
                        entry: hit.entry,
                        format: format!("{:?}", hit.format).to_ascii_lowercase(),
                        location: hit.location,
                        field: hit.field,
                        snippet: hit.value.trim().to_owned(),
                        value: hit.value,
                        score,
                    },
                )
            })
            .collect();
        hits.sort_by(|(left_name, left), (right_name, right)| {
            right_name
                .cmp(left_name)
                .then_with(|| right.score.cmp(&left.score))
                .then_with(|| {
                    (
                        &left.pak,
                        &left.entry,
                        &left.location,
                        &left.field,
                        &left.value,
                    )
                        .cmp(&(
                            &right.pak,
                            &right.entry,
                            &right.location,
                            &right.field,
                            &right.value,
                        ))
                })
        });
        if self.crc.is_none() {
            let mut names = std::collections::HashSet::new();
            hits.retain(|(name, hit)| !name || names.insert((hit.pak.clone(), hit.entry.clone())));
        }
        let hits: Vec<_> = hits.into_iter().map(|(_, hit)| hit).collect();
        let source = if indexed.is_some() { "index" } else { "live" };
        if print::output_format() == OutputFormat::Json {
            print::print_json(&serde_json::json!({
                "schema": "nw-tools.grep.v1", "source": source, "crc": self.crc, "hits": hits,
            }));
        } else {
            let mut report = Report::new("grep")
                .stat("source", source)
                .stat("hits", hits.len());
            let mut table = Table::new([
                "pak", "entry", "format", "location", "field", "value", "score",
            ]);
            for hit in hits {
                table.push([
                    Cell::path(hit.pak),
                    Cell::path(hit.entry),
                    Cell::text(hit.format),
                    Cell::text(hit.location),
                    Cell::text(hit.field),
                    Cell::text(hit.value),
                    Cell::text(hit.score.to_string()),
                ]);
            }
            report.table_or(table, "no matches").print();
        }
        Ok(())
    }
}
