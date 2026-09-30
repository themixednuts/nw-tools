# Handoff: global `grep` call stack

Companion to [Design the search pipeline call stack](003-search-pipeline-call-stack.md):
the typed entrypoint-to-effect design its resolution phase reviews. Written
per the `call-stack-design` skill. Tickets 001/002 locked the inputs: extension
+magic dispatch (catalog types are opaque UUIDs — no catalog routing), flat
`(pak, entry, format, location, snippet, score)` rows, fuzzy default, repeatable
OR queries over names+content, every human-text-searchable format, streaming
with on-the-fly ranking.

## 1. Scope, constraints, local precedents, open questions

**Behaviors:** `nw-tools grep <query>...` fuzzy-matches entry names and decoded
content across whole paks; `--exact`, `--name-only`/`--content-only`,
`--max-hits-per-entry` (default 5, soft cap + overflow count), `--limit`,
`--pak` selection, `--jobs`, global `--format` JSON. TTY renders a live ranked
table; pipes stream arrival-ordered hits and finish with the ranked set.
Failures never abort: every skip is counted with a reason.

**Constraints:** keystroke/TTY path never parses (N/A here — CLI, but the same
discipline: the *caller thread* never decodes, only workers do); rayon pool via
the jobs API, never bespoke threads; fff/frizbee is the only matcher;
`--jobs 0` runs on the caller thread (`JobArgs` precedent).

**Local precedents reused:** `asset search Path` arg shapes (`query`,
`AssetRootArg`, `--pak`, `--exact` conflicts, `--limit`, flattened `JobArgs`);
`PakSet::collect` + `collect_matching`; `path_label`, `limit_count`,
`ScanIssues`/`finish_scan`; `Report`/`Table`/`Cell` incl. JSON projection;
`find_sheet_cells` (datasheet), `collect_search_matches` (objectstream),
per-crate `is_*_path` predicates; `RunCtx` progress + `CancellationToken`
signal handling; `tui::browse` + `TableView::streaming` + `RowFeed` for the live
view; zero-padded score text (below).

**Open questions:** no `--max-entry-size` flag exists today (only internal
plumbing + the `large/` skip precedent) — new surface, default TBD in the
performance ticket. (Worker-panic semantics were listed here and then
deleted by the locked panic rule in §6: no panics, so nothing to read.)

## 2. Alternatives and decision

**(A) Central dispatcher:** `match` on `classify_entry` family → call decoder.
Fewer parts today, but every format decision re-centralizes; each new format
edits shared code and the match arm accumulates special cases. Also
`classify_entry` *reads entry bytes* — routing 1M entries through it pays
decompress-per-entry just to decide the decoder.

**(B) Registry of adapters behind a port (chosen):** each adapter owns
predicate (name-only, free) + decode + extract. New formats add a file, never
touch shared code; mirrors the existing per-crate `is_*_path` ownership.
Bytes are read once, by the worker, after routing.

B wins on invariant locality (format knowledge lives with its crate's
predicate) and testability (adapter conformance in isolation). It loses if the
adapter count stays ≤3 forever — then it's ceremony over a match. Evidence
that would reverse: the format list freezing at datasheet+objectstream+paths.
It won't: mannequin/animation/audio/lua/catalog are already named.

## 3. Contracts and leak rules

```rust
// crates/nw-tools/src/grep/adapter.rs
pub struct QueryTerm { pub raw: String, pub lowered: String }
pub struct QuerySet { pub terms: Vec<QueryTerm>, pub fuzzy: bool }

pub enum Tier { Name, Content }          // picker convention: names above content

pub struct ContentHit {
    pub location: String,               // format-native: `row 12 · column Damage`
    pub snippet: String,                // trimmed matched decoded text
    pub score: u16,                     // raw fff score; incomparable across tiers
}

pub struct EntryView<'a> {
    pub pak: &'a str,
    pub name: &'a str,
    pub bytes: Vec<u8>,                 // owned: no lifetimes cross worker threads
}

pub trait SearchAdapter: Send + Sync {
    fn name(&self) -> &'static str;                 // "datasheet", shown in rows
    fn tier(&self) -> Tier;
    fn matches_entry(&self, entry_name: &str) -> bool;  // name-only, never reads bytes
    /// At most MAX_HITS hits, best-first, plus overflow count. Decode failure
    /// is reported as skips, never propagated (see leak rules).
    fn search(&self, entry: &EntryView<'_>, queries: &QuerySet, fuzzy: &mut fuzzy::MultiSearch)
        -> (Vec<ContentHit>, usize);
}
```

Seam rules: the adapter owns decode *and* decode-failure translation —
`nw_datasheet::Error`, XML errors, etc. never cross the seam; failures surface
as `(skipped, reason)` counts. Bytes cross owned. Scores cross raw with the
documented incomparability (tiers, not calibration, make order meaningful).
The fuzzy matcher crosses *into* the adapter per worker (built once via
`map_init`, see §5), never per entry. Untrusted input (pak bytes, query text)
is parsed at the adapter boundary and the CLI boundary respectively; adapters
must treat entry bytes as hostile (corrupt entries are the skip-count path).

`MAX_HITS` (default 5, `--max-hits-per-entry`): enforced inside the adapter,
which owns ranking; the merger trusts order.

## 4. Entrypoint-to-effect stacks

Conventions: `->` call, `<-` return; threads, owners, and side effects inline.

**Shared scan core (both renderers):**

```text
cli: nw-tools grep <q>... [--exact] [--name-only|--content-only] [--pak ...] [--limit N] [--jobs J]
  -> grep::Cmd::run (crates/nw-tools/src/grep/mod.rs): parse QuerySet, build registry
    -> PakSet::collect(root, paks): Vec<PathBuf>                       // caller thread, headers only
    -> open readers: Vec<Arc<PakMmapReader>>                           // mmap shared across workers (browser precedent)
    -> flatten work list: Vec<WorkItem{pak, reader, entry_index, name}> // caller thread, no entry reads
    -> RunCtx::map_results_compact_init("grep", &items, name, init, f) // NEW thin wrapper over map_init_until_cancelled; init builds fuzzy::MultiSearch once per worker
       worker thread (rayon pool): WorkItem
        -> size guard: entry > --max-entry-size => Skip(Oversized)      // classify_entry large/ precedent
        -> reader.read_by_index => pak IO error => Skip(Unreadable)     // NO abort (locked Q2)
        -> registry.adapters.filter(matches_entry(name))               // name-only routing; bytes untouched
          -> adapter.search(entry_view, queries, fuzzy)                // decode + match + cap
             <- (hits, overflow) | Skip(DecodeFailed)                   // translated at seam
        -> (pak, name, adapter_name, tier, hits, overflow, skips)
       <- JobBatch<Vec<ScanHits>>                                       // stops intake on ctx.cancel (Ctrl-C)
    <- merger (caller thread): tier-split, per-entry cap already applied, ScanIssues counts
```

**TTY renderer (interactive):**

```text
  -> merger posts row batches to RowFeed::extend + mark_done per pak      // Arc<RowFeed>, lock-brief
  -> app::run(TableView::streaming(preset sort: score desc)) on caller thread
     <- tick ingest() pushes + recomputes; zero-padded score text makes text-sort numeric
  <- Enter on capped row expands overflow (needs expand affordance: native to TableView or bespoke — implementer picks, ticket notes the constraint)
  <- q/Esc or scan-done: final exact two-tier order applied before exit summary
```

Live rows interleave tiers mid-run (approximation, documented); the settled
view and all pipe output are exactly names-then-content, score-desc within tiers.
`~` marks content rows (picker language). Rejected alternative: a
`RowFeed::replace` + merger-owned global order — shared-widget churn for one
consumer; the preset-sort + settle-pass achieves it with a 2-line
`TableView::preset_sort` setter instead.

**Pipe renderer (non-TTY):** same scan core; rows stream arrival-ordered to
`Report`/`Table` (existing JSON projection, no custom serializer), then the
exact two-tier table + `ScanIssues` summary. `--limit` truncates display only;
counts stay whole (SearchPath precedent).

## 5. Failure and change paths

- Entry unreadable / oversized / undecodable → skip + counted reason; scan continues.
- Pak open failure → that pak's items all Skip(Unreadable); scan continues (no abort paths, locked).
- Query matches nothing → `table_or(..., "no matches")` precedent, exit 0.
- Ctrl-C → `ctx.cancel` stops intake via `map_until_cancelled`; merged-so-far prints ranked with a "cancelled" summary line; exit code signals interruption (existing convention — verify current code).
- Empty query → rejected by clap (`query: Vec<String>` min 1), no empty-fuzzy-keep-all surprise.
- `--jobs 0` → caller-thread execution through the same stack (no separate code path).
- Worker panic → impossible by construction: adapters contain no panicking
  code on any path (locked rule below), so `JobRunner`'s panic semantics are
  irrelevant to the no-abort guarantee and need not be read.

## 6. Panic rule (locked)

No panics, full stop: adapter code paths — including corrupt, hostile, and
empty inputs — report skips as values and never `panic!`/`unwrap`/`expect`
on untrusted data. A panic in an adapter is a bug, not a failure mode. This
dissolves the worker-panic question instead of answering it: with no possible
panic, the pool's panic semantics cannot violate the no-abort guarantee.

## 7. File and module ownership

```text
crates/nw-tools/src/grep/mod.rs        command, QuerySet, registry build, merger, renderers
crates/nw-tools/src/grep/adapter.rs    port trait, Tier/Hit/EntryView/QuerySet types
crates/nw-tools/src/grep/adapters/
  name.rs        entry-name matching (always registered; Tier::Name)
  datasheet.rs   reuses pub(crate) find_sheet_cells (promote from format/datasheet.rs)
  objectstream.rs reuses nw_objectstream::query::collect_search_matches
  catalog.rs     entry text via existing catalog find logic
  mannequin.rs   NEW thin extract over cry-mannequin (fragment/group names + tags)
  animation.rs   NEW thin extract over cry-animation (names/events)
  audio.rs       NEW thin extract over bank string tables
  lua.rs         decompiled text via nw-lua (skip on decompile failure)
crates/nw-tools/src/jobs.rs            ADD RunCtx::map_results_compact_init (10-line mirror)
crates/nw-tools/src/tui/table.rs       ADD TableView::preset_sort (2-line setter)
crates/nw-tools/src/main.rs            Cmd::Grep wiring
```

Each layer owns one invariant: adapters own format knowledge + failure
translation; the merger owns tiering/caps/counts; renderers own presentation;
`RunCtx` owns pool/cancel/progress. Deleting any adapter removes its format
from results and nothing else.

## 8. Vertical tests and proof obligations (input to ticket 004)

Through the public CLI seam + adapter seam, no internals: per-adapter
conformance on known-good literals (each format, incl. mannequin fragment/tag
and caf name cases); exact-vs-fuzzy divergence; repeatable-OR queries;
per-entry cap + overflow count; corrupt entry → skip-counted (never aborts);
oversized entry → skipped with reason; cancel token injected mid-scan drains
partial results; tier ordering incl. `~` marking; TTY/pipe renderer selection
as a pure function of TTY-ness; unsupported-`search_contents`-style default —
N/A here (registry always contains the name adapter). Measurable proofs
(corpus, p50/p95, `--jobs` scaling) belong to ticket 005.
