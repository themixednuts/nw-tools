//! Walk paks, extract adapter facts, persist them.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

use drizzle::core::expr::{eq, in_array, max};
use nw_jobs::JobRunner;
use nw_pak::PakMmapReader;

use crate::cache::{
    Cache, CacheFutureExt, InsertIdxEntry, InsertIdxFact, InsertIdxPak, InsertMeta, Schema,
};
use crate::grep::adapter::FactDraft;
use crate::resources;
use crate::support::PakSet;

use super::intern::Interner;
use super::{EntryFormat, IndexStatus};

const FACT_CHUNK: usize = 256;
const ENTRY_CHUNK: usize = 4_000;
const EXTRACT_CHUNK: usize = 256;
const VERSION_KEY: &str = "content_index_version";
const ROOT_KEY: &str = "content_index_root";
// Extraction and stamp semantics must invalidate caches produced by older code.
const VERSION: &str = "2";

type PakStamp = (String, PathBuf, Option<(i64, i64)>);

struct PakRecord {
    size: i64,
    mtime: i64,
    complete: bool,
}

impl PakRecord {
    fn matches(&self, stamp: Option<(i64, i64)>) -> bool {
        self.complete && stamp == Some((self.size, self.mtime))
    }
}

pub fn build(cache: &mut Cache, root: &Path, runner: &JobRunner) -> anyhow::Result<IndexStatus> {
    let paks = PakSet::collect(root.to_path_buf(), Vec::new())?;
    let stamps: Vec<PakStamp> = paks
        .paths()
        .iter()
        .map(|path| (paks.relative(path), path.clone(), pak_stamp(path)))
        .collect();
    prepare(cache)?;
    let root_identity = root.canonicalize()?.to_string_lossy().into_owned();
    let records = pak_records(cache)?;
    let same_root = metadata_matches(cache, ROOT_KEY, &root_identity)?;
    for name in records
        .keys()
        .filter(|name| name.as_str() != resources::PAK_NAME)
    {
        if !same_root || !stamps.iter().any(|(current, _, _)| current == name) {
            drop_pak(cache, name)?;
        }
    }
    set_metadata(cache, ROOT_KEY, &root_identity)?;
    let mut interner = Interner::load(cache)?;
    let mut indexed = 0usize;
    for (name, path, stamp) in &stamps {
        if same_root && records.get(name).is_some_and(|row| row.matches(*stamp)) {
            indexed += 1;
            continue;
        }
        drop_pak(cache, name)?;
        mark_pak(cache, &mut interner, name, *stamp, false)?;
        let complete = index_pak(cache, &mut interner, runner, name, path)?;
        mark_pak(cache, &mut interner, name, *stamp, complete)?;
        if complete && stamp.is_some() {
            indexed += 1;
        }
    }
    Ok(if indexed == stamps.len() {
        IndexStatus::Complete { paks: indexed }
    } else {
        IndexStatus::Partial {
            indexed_paks: indexed,
        }
    })
}

pub fn ingest_entry(
    cache: &mut Cache,
    interner: &mut Interner,
    pak_id: i64,
    name_id: i64,
    format: EntryFormat,
    facts: &[FactDraft],
) -> anyhow::Result<()> {
    flush_pak(
        cache,
        interner,
        pak_id,
        vec![(name_id, format, facts.to_vec())],
    )
}

pub fn is_complete(cache: &Cache, root: &Path) -> bool {
    let Ok(root_identity) = root.canonicalize() else {
        return false;
    };
    if !metadata_matches(cache, VERSION_KEY, VERSION).unwrap_or(false)
        || !metadata_matches(cache, ROOT_KEY, &root_identity.to_string_lossy()).unwrap_or(false)
    {
        return false;
    }
    let Ok(paks) = PakSet::collect(root.to_path_buf(), Vec::new()) else {
        return false;
    };
    let Ok(records) = pak_records(cache) else {
        return false;
    };
    records
        .keys()
        .filter(|name| name.as_str() != resources::PAK_NAME)
        .count()
        == paks.paths().len()
        && paks.paths().iter().all(|path| {
            records
                .get(&paks.relative(path))
                .is_some_and(|row| row.matches(pak_stamp(path)))
        })
}

pub fn has_embedded(cache: &Cache) -> bool {
    metadata_matches(cache, VERSION_KEY, VERSION).unwrap_or(false)
        && pak_records(cache).is_ok_and(|records| {
            records
                .get(resources::PAK_NAME)
                .is_some_and(|row| row.matches(Some(resources::ResourceView::embedded_stamp())))
        })
}

pub fn index_resources(cache: &mut Cache, interner: &mut Interner) -> anyhow::Result<()> {
    if has_embedded(cache) {
        return Ok(());
    }
    drop_pak(cache, resources::PAK_NAME)?;
    let pak_id = interner.intern(resources::PAK_NAME);
    for row in &resources::SessionOverlay::embedded().rows {
        let started = Instant::now();
        tracing::info!(
            entry = row.entry,
            facts = row.facts.len(),
            "indexing resource"
        );
        let name_id = interner.intern(&row.entry);
        flush_pak(
            cache,
            interner,
            pak_id,
            vec![(name_id, row.format, row.facts.clone())],
        )?;
        tracing::info!(
            entry = row.entry,
            elapsed_ms = started.elapsed().as_millis(),
            "indexed resource"
        );
    }
    mark_pak(
        cache,
        interner,
        resources::PAK_NAME,
        Some(resources::ResourceView::embedded_stamp()),
        true,
    )
}

fn drop_pak(cache: &mut Cache, name: &str) -> anyhow::Result<()> {
    let Schema {
        idx_entry,
        idx_fact,
        idx_pak,
        idx_token,
        ..
    } = Schema::new();
    let pak_ids: Vec<(i64,)> = cache
        .db()
        .select((idx_token.id,))
        .from(idx_token)
        .r#where(eq(idx_token.text, name.to_owned()))
        .all()
        .wait()?;
    let Some(&(pak_id,)) = pak_ids.first() else {
        return Ok(());
    };
    let entries: Vec<(i64,)> = cache
        .db()
        .select((idx_entry.id,))
        .from(idx_entry)
        .r#where(eq(idx_entry.pak_id, pak_id))
        .all()
        .wait()?;
    cache
        .db_mut()
        .transaction(
            drizzle::sqlite::connection::SQLiteTransactionType::Deferred,
            async |tx| {
                for chunk in entries.chunks(ENTRY_CHUNK) {
                    tx.delete(idx_fact)
                        .r#where(in_array(idx_fact.entry_id, chunk.iter().map(|(id,)| *id)))
                        .execute()
                        .await?;
                    tx.delete(idx_entry)
                        .r#where(in_array(idx_entry.id, chunk.iter().map(|(id,)| *id)))
                        .execute()
                        .await?;
                }
                tx.delete(idx_pak)
                    .r#where(eq(idx_pak.name_id, pak_id))
                    .execute()
                    .await?;
                Ok(())
            },
        )
        .wait()?;
    Ok(())
}

fn index_pak(
    cache: &mut Cache,
    interner: &mut Interner,
    runner: &JobRunner,
    pak_name: &str,
    path: &Path,
) -> anyhow::Result<bool> {
    let reader = match PakMmapReader::open(path) {
        Ok(reader) => reader,
        Err(error) => {
            tracing::warn!(pak = pak_name, %error, "pak left incomplete");
            return Ok(false);
        }
    };
    let pak_id = interner.intern(pak_name);
    let items: Vec<(usize, String)> = reader
        .entries()
        .map(|entry| (entry.index(), entry.name().to_owned()))
        .collect();
    let started = Instant::now();
    let mut skipped = 0usize;
    tracing::info!(pak = pak_name, entries = items.len(), "indexing");
    for chunk in items.chunks(EXTRACT_CHUNK) {
        let extracted = runner.map(chunk, |(index, name)| {
            (
                name.clone(),
                crate::extract::entry(name, || reader.read_by_index(*index).ok()),
            )
        });
        let extracted = extracted
            .into_iter()
            .map(|(name, extracted)| {
                if let Some(reason) = extracted.skip {
                    skipped += 1;
                    tracing::warn!(
                        pak = pak_name,
                        entry = name,
                        ?reason,
                        "entry left incomplete"
                    );
                }
                (interner.intern(&name), extracted.format, extracted.facts)
            })
            .collect();
        flush_pak(cache, interner, pak_id, extracted)?;
    }
    tracing::info!(
        pak = pak_name,
        entries = items.len(),
        skipped,
        elapsed_ms = started.elapsed().as_millis(),
        "indexed"
    );
    Ok(skipped == 0)
}

fn flush_pak(
    cache: &mut Cache,
    interner: &mut Interner,
    pak_id: i64,
    extracted: Vec<(i64, EntryFormat, Vec<FactDraft>)>,
) -> anyhow::Result<()> {
    let mut entries = Vec::new();
    let mut facts = Vec::new();
    for (entry_id, (name_id, format, drafts)) in (next_entry_id(cache)?..).zip(extracted) {
        entries.push((entry_id, pak_id, name_id, format as i64));
        for draft in drafts {
            if draft.value.is_empty() {
                continue;
            }
            facts.push((
                entry_id,
                interner.intern(&draft.field),
                interner.intern(&draft.value),
                draft.kind as i64,
                i64::from(draft.loc_a),
                i64::from(draft.loc_b),
            ));
        }
    }
    interner.flush(cache)?;
    let Schema {
        idx_entry,
        idx_fact,
        ..
    } = Schema::new();
    cache
        .db_mut()
        .transaction(
            drizzle::sqlite::connection::SQLiteTransactionType::Deferred,
            async |tx| {
                for chunk in entries.chunks(ENTRY_CHUNK) {
                    let rows: Vec<_> = chunk
                        .iter()
                        .map(|&(id, pak_id, name_id, format)| {
                            InsertIdxEntry::new(pak_id, name_id, format, 0).with_id(id)
                        })
                        .collect();
                    tx.insert(idx_entry).values(rows).execute().await?;
                }
                for chunk in facts.chunks(FACT_CHUNK) {
                    let rows: Vec<_> = chunk
                        .iter()
                        .map(|&(entry_id, field_id, value_id, kind, loc_a, loc_b)| {
                            InsertIdxFact::new(entry_id, field_id, value_id, kind, loc_a, loc_b)
                        })
                        .collect();
                    tx.insert(idx_fact).values(rows).execute().await?;
                }
                Ok(())
            },
        )
        .wait()?;
    Ok(())
}

fn next_entry_id(cache: &Cache) -> anyhow::Result<i64> {
    let Schema { idx_entry, .. } = Schema::new();
    let highest: (Option<i64>,) = cache
        .db()
        .select((max(idx_entry.id),))
        .from(idx_entry)
        .get()
        .wait()?;
    Ok(highest.0.unwrap_or(0) + 1)
}

fn pak_stamp(path: &Path) -> Option<(i64, i64)> {
    let meta = std::fs::metadata(path).ok()?;
    let size = i64::try_from(meta.len()).ok()?;
    let mtime = i64::try_from(
        meta.modified()
            .ok()?
            .duration_since(std::time::UNIX_EPOCH)
            .ok()?
            .as_nanos(),
    )
    .ok()?;
    Some((size, mtime))
}

fn mark_pak(
    cache: &mut Cache,
    interner: &mut Interner,
    name: &str,
    stamp: Option<(i64, i64)>,
    complete: bool,
) -> anyhow::Result<()> {
    let Some((size, mtime)) = stamp else {
        return Ok(());
    };
    let name_id = interner.intern(name);
    interner.flush(cache)?;
    let Schema { idx_pak, .. } = Schema::new();
    cache
        .db_mut()
        .transaction(
            drizzle::sqlite::connection::SQLiteTransactionType::Deferred,
            async |tx| {
                tx.delete(idx_pak)
                    .r#where(eq(idx_pak.name_id, name_id))
                    .execute()
                    .await?;
                tx.insert(idx_pak)
                    .value(InsertIdxPak::new(name_id, size, mtime, i64::from(complete)))
                    .execute()
                    .await?;
                Ok(())
            },
        )
        .wait()?;
    Ok(())
}

fn wipe(cache: &mut Cache) -> anyhow::Result<()> {
    let Schema {
        idx_fact,
        idx_entry,
        idx_pak,
        idx_token,
        meta,
        ..
    } = Schema::new();
    cache
        .db_mut()
        .transaction(
            drizzle::sqlite::connection::SQLiteTransactionType::Deferred,
            async |tx| {
                tx.delete(idx_fact).execute().await?;
                tx.delete(idx_entry).execute().await?;
                tx.delete(idx_pak).execute().await?;
                tx.delete(idx_token).execute().await?;
                tx.delete(meta)
                    .r#where(eq(meta.key, ROOT_KEY.to_owned()))
                    .execute()
                    .await?;
                Ok(())
            },
        )
        .wait()?;
    Ok(())
}

fn pak_records(cache: &Cache) -> anyhow::Result<HashMap<String, PakRecord>> {
    let Schema {
        idx_pak, idx_token, ..
    } = Schema::new();
    let rows: Vec<(String, i64, i64, i64)> = cache
        .db()
        .select((
            idx_token.text,
            idx_pak.size,
            idx_pak.mtime,
            idx_pak.complete,
        ))
        .from(idx_pak)
        .inner_join((idx_token, eq(idx_pak.name_id, idx_token.id)))
        .all()
        .wait()?;
    Ok(rows
        .into_iter()
        .map(|(name, size, mtime, complete)| {
            (
                name,
                PakRecord {
                    size,
                    mtime,
                    complete: complete == 1,
                },
            )
        })
        .collect())
}

fn metadata_matches(cache: &Cache, key: &str, value: &str) -> anyhow::Result<bool> {
    let Schema { meta, .. } = Schema::new();
    let rows: Vec<(String,)> = cache
        .db()
        .select((meta.key,))
        .from(meta)
        .r#where((
            eq(meta.key, key.to_owned()),
            eq(meta.value, value.to_owned()),
        ))
        .all()
        .wait()?;
    Ok(!rows.is_empty())
}

fn set_metadata(cache: &mut Cache, key: &str, value: &str) -> anyhow::Result<()> {
    let Schema { meta, .. } = Schema::new();
    cache
        .db_mut()
        .transaction(
            drizzle::sqlite::connection::SQLiteTransactionType::Deferred,
            async |tx| {
                tx.delete(meta)
                    .r#where(eq(meta.key, key.to_owned()))
                    .execute()
                    .await?;
                tx.insert(meta)
                    .value(InsertMeta::new(key.to_owned(), value.to_owned()))
                    .execute()
                    .await?;
                Ok(())
            },
        )
        .wait()?;
    Ok(())
}

pub fn prepare(cache: &mut Cache) -> anyhow::Result<()> {
    if !metadata_matches(cache, VERSION_KEY, VERSION)? {
        wipe(cache)?;
        set_metadata(cache, VERSION_KEY, VERSION)?;
    }
    Ok(())
}
