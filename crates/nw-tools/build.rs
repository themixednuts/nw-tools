//! Keep the SQLite cache migrations in `./drizzle` in sync with the schema
//! declared in `src/cache.rs`.
//!
//! On every build, `drizzle-migrations` parses the `#[SQLiteTable]` definitions,
//! diffs them against the latest snapshot in `./drizzle`, and writes a new
//! migration folder when the schema changed. `cache::open` then embeds those
//! files at compile time via `drizzle::include_migrations!`, so the on-disk
//! database is always migrated to match the Rust definition.

use drizzle_migrations::build::{Config, Output, run};
use drizzle_types::Dialect;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cfg = Config::new(Dialect::SQLite)
        .file("src/cache.rs")
        .out("./drizzle");

    // Rerun whenever the schema source changes.
    cfg.watch();

    if let Output::Generated {
        tag,
        path,
        statement_count,
    } = run(&cfg)?
    {
        println!(
            "cargo:warning=drizzle: generated migration {tag} ({statement_count} statements) at {}",
            path.display()
        );
    }

    Ok(())
}
