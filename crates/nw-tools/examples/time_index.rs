//! Cold + resume timing for [`nw_tools::index::ContentIndex`] against the
//! local install. Uses a dedicated sqlite file so the catalog cache is left
//! alone.

use std::time::Instant;

use humansize::{DECIMAL, format_size};
use nw_jobs::JobRunner;
use nw_tools::index::{ContentIndex, Lookup};

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::builder()
                .with_default_directive(tracing_subscriber::filter::LevelFilter::INFO.into())
                .from_env_lossy(),
        )
        .with_target(false)
        .init();

    let install = nw_locator::Install::locate()?;
    let root = install.assets().to_path_buf();
    let cache = cache_path();
    if cache.exists() {
        std::fs::remove_file(&cache)?;
    }

    let jobs = std::env::var("JOBS")
        .ok()
        .and_then(|value| value.parse().ok());
    let runner = JobRunner::from_jobs(jobs)?;

    println!("source     {}", install.source());
    println!("root       {}", install.root().display());
    println!("assets     {}", root.display());
    println!("cache      {}", cache.display());
    println!("runner     {}", runner.policy());
    println!("threads    {}", runner.parallelism());

    let mut index = ContentIndex::open(&cache)?;
    let started = Instant::now();
    index.ingest_embedded()?;
    let status = index.build_with_runner(&root, &runner)?;
    let cold = started.elapsed();
    println!("cold       {status:?} in {cold:.2?}");

    let started = Instant::now();
    let cells = index.lookup(Lookup::value("ironsword"))?;
    let lookup = started.elapsed();
    println!(
        "lookup     {} hit(s) for ironsword in {lookup:.2?}",
        cells.len()
    );

    let started = Instant::now();
    let fragments = index.search(&["kickout".to_owned()], false)?;
    let search = started.elapsed();
    println!(
        "search     {} hit(s) for kickout in {search:.2?}",
        fragments.len()
    );

    drop(index);
    let bytes = std::fs::metadata(&cache)
        .map(|meta| meta.len())
        .unwrap_or(0);
    println!("db         {}", format_size(bytes, DECIMAL));

    let mut index = ContentIndex::open(&cache)?;
    let started = Instant::now();
    let status = index.build_with_runner(&root, &runner)?;
    let resume = started.elapsed();
    println!("resume     {status:?} in {resume:.2?}");
    Ok(())
}

fn cache_path() -> std::path::PathBuf {
    if let Ok(path) = std::env::var("NW_INDEX_CACHE") {
        return std::path::PathBuf::from(path);
    }
    let mut path = nw_tools::cache::default_path();
    path.set_file_name("index-bench.sqlite");
    path
}
