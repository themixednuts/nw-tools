//! Focused CRC timings without rebuilding the installed game's content index.

use std::time::Instant;

use clap::Parser;
use nw_tools::grep::adapter::{FactDraft, FactKind};
use nw_tools::index::{ContentIndex, EntryFormat, Lookup};

#[derive(Parser)]
struct Args {
    /// Also index the bundled reflection dumps.
    #[arg(long)]
    embedded: bool,
    /// Synthetic occurrences sharing a small token dictionary.
    #[arg(long, default_value_t = 20_000)]
    facts: u32,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();
    let mut index = ContentIndex::open_in_memory()?;
    if args.embedded {
        let started = Instant::now();
        index.ingest_embedded()?;
        println!("bundled ingest: {:?}", started.elapsed());
    }
    let facts: Vec<_> = (0..args.facts)
        .map(|row| FactDraft {
            field: "BenchmarkField".into(),
            value: format!("BenchmarkValue{:02}", row % 64),
            kind: FactKind::Cell,
            loc_a: row,
            loc_b: 0,
        })
        .collect();
    let started = Instant::now();
    index.ingest(
        "fixture.pak",
        "fixture.datasheet",
        EntryFormat::Datasheet,
        &facts,
    )?;
    println!(
        "synthetic ingest: {} facts in {:?}",
        facts.len(),
        started.elapsed()
    );
    for (label, lookup) in [
        ("value hit", Lookup::value("BenchmarkValue01")),
        ("value miss", Lookup::value("AbsentBenchmarkValue")),
        ("field hit", Lookup::field("BenchmarkField")),
    ] {
        let started = Instant::now();
        let hits = index.lookup(lookup)?;
        println!(
            "{label}: {} occurrences in {:?}",
            hits.len(),
            started.elapsed()
        );
    }
    Ok(())
}
