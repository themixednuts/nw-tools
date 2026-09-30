# Persist a compact relational content index

Status: resolved
Kind: `wayfinder:call-stack`
Blocked by: [Agree the TDD seams](004-tdd-seams.md)

## Question

How do we persist decoded, relational content so later finds are CRC lookups
over interned strings instead of re-decoding every pak — without approaching
pak size on disk, and without blocking the command that started the build?

## Resolution

Dictionary-encoded star schema in the existing Turso/drizzle cache:

- `idx_token` — each unique string once, with `Crc32` of the original bytes
  and `Crc32::from_str_lower` of the same text. No unique index on `text`
  (that would copy every string into a second B-tree).
- `idx_entry` — pak + path as token FKs, format as a small integer.
- `idx_fact` — `(entry, field_token, value_token, kind, loc_a, loc_b)`.
  Locations are reconstructed; the phrase `row 12 · column Damage` is never
  stored.
- Secondary indexes only on `token.crc`, `token.crc_lower`, `fact.value_id`,
  `fact.field_id`. Those are the read paths.

Build is an explicit `nw-tools index` on the caller thread. Other commands
do not start a build. They may open an existing cache (`open_existing`).
Decode/extract uses `JobRunner::from_jobs(--jobs)` (global Rayon pool, or
inline when `--jobs 0`). Per-pak fingerprints resume across process exits.
All extracted strings are interned and dual-CRC'd, including long fields and
values. Closing the CLI ends the
process. A leftover background index is how a close used to sit on a
cold build for hours.

Queries probe `idx_token` by CRC first (a miss returns immediately).
Fuzzy search ranks the token dictionary in one `frizbee` batch per query, then
joins every matching token in batches. There is no hidden result cap. Facts that
match both their field and value are merged by occurrence ID. Adapters cover every
human-text format from ticket 002: name, datasheet, mannequin (adb +
controller/actions/tags XML), objectstream, catalog, animation
(caf + animevents), audio (mapping CSV + ATL), lua (decompiled
lines), pak `.txt`/`.cfg`/`.ini`, leftover `.xml` as a complete extract
  of names, attributes, and text (ADR-0001), and the bundled
`serialize.json` / `behavior-context` / module dumps as a virtual
`nw-resources` pak through the same adapter dispatch as a filesystem
pak. CLI `--serialize` / `--behavior-context` / `--modules` install a
process-local delta: extract runs once into a session fact table.
`grep::find` takes a [`Source`]: live extract, an open index, or auto
(complete index else live). The index never gates CRC or text find —
live extract is the same pipeline as `grep::run`. Session overlay merges
on top either way. NameLookup reads session serialize. The waiter always
persists the bundled copies. ObjectStream index facts pin bundled
serialize. Stale `nw-resources` drops only that virtual pak. No raw SQL.

## Alternatives

**(A) FTS5 / sqlite-zstd over decoded text.** Fast substring, but it stores
the corpus again (the pak-size failure mode) and cannot be expressed
through drizzle's query API.

**(B) Per-format blob of interned strings, no facts.** Small and enough
for grep, but it drops the relational question ("which entries have field
X = Y") the index exists to answer.

**(C) Chosen — interned tokens + integer facts.** Same dictionary win as
column-store encoding; CRC32 is the inverted-index key; facts are the
posting list. Fuzzy grep scores the unique token table (orders of
magnitude smaller than cells) and joins hits back to facts.

## TDD seams

1. **`ContentIndex` intern/query** — ingest drafts, look up by value /
   field / name CRC, assert original text and reconstructed location.
2. **`ContentIndex::build`** — synthetic pak in, same hits out; skip a
   pak whose fingerprint is already complete.
3. **`start_alongside_at`** — returns before the build finishes.
4. **Adapter `facts`** — stays below (1)/(2); drafts are proven through
   the index, not by reaching into decoder crates.

Numeric CRC values are not asserted (tautological). Membership, casing,
location text, and skip-on-complete are.

## Call stack

```text
cli: nw-tools index
  -> confirm (or --yes)                               // no auto-start on other commands
  -> JobRunner::from_jobs(--jobs)
  -> index::build_install                             // this thread; Ctrl+C kills the process
      -> ContentIndex::open(cache::default_path)
      -> ContentIndex::build_with_runner(assets, runner)
        -> PakSet::collect
        -> per pak: fingerprint miss?
             -> JobRunner::map entries
                -> extract::entry
             -> Interner + drizzle idx_*
           else skip
      <- IndexStatus { complete | partial }
      -> ContentIndex::ingest_embedded

cli: any other command
  -> index::open_existing if a reader needs it        // lazy, no build
  -> command.run()                                    // process exits with the command

query: grep::find::lookup(Source, overlay, Lookup)
  -> Source::Auto { index, root }
       complete? ContentIndex::lookup
                 -> db.query(idx_token).where(eq(crc|crc_lower, crc))
                 -> db.select(idx_fact).where(in_subquery(...))
       else      live extract every entry under root, match drafts by CRC
  -> drop persist/live rows whose nw-resources path the overlay replaced
  -> match already-extracted overlay facts
  <- Vec<IndexedHit>
```

Failures: unreadable/undecodable entries are counted and skipped (grep
precedent), leaving that pak incomplete. A busy cache write waits the existing 5s
timeout and then leaves the pak incomplete for the next process. Incomplete paks
are recorded before extraction and their existing rows are removed before retry.
Extraction runs in batches of 256 entries rather than retaining a whole pak's
facts. Changed size/nanosecond mtime rebuilds only that pak; removed paks lose their
rows. Root identity and extraction version are persisted in `meta`. Completeness
checks read pak metadata without loading the token dictionary. An extraction
version change invalidates the index; queries do not initiate that rebuild.

`nw-tools grep` exposes CRC and text queries through `grep::find`. CRC input is
decimal or `0x` hexadecimal, with `--target value|field|name` and
`--case original|lower`. A pak-only index supplements its bundled facts live until
`ingest_embedded` has completed. Bundled stamps hash the compiled resource bytes,
including the compressed behavior archive, so checking a complete index does not
decompress or parse those dumps.
