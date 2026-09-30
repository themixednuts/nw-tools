# Inventory catalog-to-format dispatch

Status: resolved
Kind: `wayfinder:research`
Blocked by: (none)

## Question

What maps an asset or catalog entry to its format decoder today, and where
are the gaps? Survey: extension globs (`is_datasheet_path` and siblings),
catalog type ids / `AssetCatalog` lookups, `AssetStore` routing, and how each
`format` subcommand and `asset objectstream search` discovers its inputs.
For each content-bearing format (datasheet, objectstream, catalog, audio,
lua, dds/model, mannequin/adb, animation, …) record: decoder crate, entry
predicate, and whether a content-search hook already exists or must be added.
Note anything the global find cannot reuse and why.

## Resolution

Dispatch today is **extension globs + magic bytes, not catalog types** —
the map's assumption was wrong, and that reshapes the call-stack ticket.

**Per-format entry predicates (all extension-based, one per crate):**

- datasheet: `nw_datasheet::is_datasheet_path` (`crates/nw-datasheet/src/lib.rs`)
- dds: `nw_dds::is_dds_path` / `is_dds_name`
- lua: `is_luac_path` (`crates/nw-tools/src/format/lua.rs`)
- catalog: `nw_asset::is_asset_catalog_path`
- mannequin: `cry_mannequin::is_animation_database_path` (`.adb`)
  plus `inspect_animation_database_path`
- animation: `cry_animation::is_animation_path` (`.caf`/`.i_caf`)
- model meshes: `is_mesh_file` (`crates/nw-tools/src/model.rs`:
  cgf/cga/chr/skin/caf/anm/fbx)
- objectstream: caller-supplied extension list *plus* magic
  (`ObjectPayload::from_wrapped`, azcs-envelope detection)

Every `format` subcommand globs its own inputs via `collect_matching`
(`crates/nw-tools/src/support.rs`) — siloed, no shared dispatcher.

**Closest thing to central dispatch:** `classify_entry`
(`crates/nw-tools/src/asset.rs`) reads wrapped bytes and reports a shape
(Oodle/compression, DDS parse, objectstream envelope, catalog/datasheet
paths, else `nw_pak::shape::path_family` extension families:
shader/terrain/texture/model/audio/script/data/root). But it classifies for
*reporting* (`asset summary`), never routes to decoders.

**Catalog types are opaque.** `RascEntry` carries `asset_type: AssetType`
(`crates/nw-asset/src/catalog.rs`), but `AssetType` is an uninterpreted UUID
wrapper (`crates/nw-asset/src/id.rs`) — nothing in the tree maps it to a
decoder. Catalog-driven dispatch would first require decoding what those
type UUIDs mean; extension/magic dispatch works today with zero new research.

**Content-search hooks already built:** `format datasheet --find` (cell
values + column names), `asset objectstream search` (decoded payloads via
`collect_search_matches`), `asset search` (entry paths only), `format
catalog find` (path text). **Gaps:** mannequin fragment/tag search, animation
content (caf event names etc.), audio bank names, lua source text, dds/model
metadata — none have a find path; each needs an adapter, not a decoder
(the parsers exist).

**Consequence for the pipeline design:** dispatch on extension + magic
(`classify_entry`/`path_family` lineage), not catalog types. Catalog stays
in its current role: path/GUID loading (`AssetStore::read_path`,
`entry_by_path`), not format routing.
