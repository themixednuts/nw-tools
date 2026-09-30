# nw-tools

Inspect New World paks and the human-text they contain.

## Language

**Pak**:
A game archive under the install assets root. Bundled reflection dumps appear as a virtual pak named `nw-resources`.
_Avoid_: archive, container

**Entry**:
One file inside a pak, identified by pak + path.
_Avoid_: file, asset

**Fact**:
One extracted field, value, and location from an entry.
_Avoid_: cell, record, row

**Token**:
A unique string stored once, with a CRC of the original bytes and a CRC of the lowercased text.

**Hit**:
One matched occurrence: pak, entry, format, location, snippet, and score.
_Avoid_: result, match

**Location**:
A format-specific address inside an entry.
_Avoid_: path, offset

**Snippet**:
The trimmed matched decoded text shown on a hit.

**Score**:
The fff value that orders hits. Zero in exact mode.

**Searchable**:
A format counts if it carries human-readable text. Leftover pak XML counts. Pixel and binary blobs never count.
_Avoid_: indexed

**Complete extract**:
Every human-readable string in an entry is present as a fact field or value. A partial extract is a miss that looks like absence.
_Avoid_: line slice, snippet (a snippet is hit display, not extract)

**Extract**:
Producing the facts for an entry. The first matching content format claims the entry; a decode failure skips that entry's content and does not pass the bytes to another content format.
_Avoid_: decode, search, collect

**Leftover XML**:
An `.xml` entry no earlier content format claimed. Extract walks the document: every element name, attribute name, attribute value, and text or CDATA node is a fact. `field` is the element path (`Root/Item/@id` for an attribute). Comments and processing instructions are not facts. A failed parse still recovers quoted strings and element-ish names and text; skip only when nothing human-readable remains.
_Avoid_: line slice, leftover text

**Grep**:
The find entry. Always works by extracting and matching facts. A complete content index is an optional fast path for the same queries (text or CRC); it does not gate the feature.
_Avoid_: find, search command, ContentIndex lookup

**Content index**:
Persisted tokens, entries, and facts for an install. Accelerator for find — never required for search capability. Holds only persisted facts.
_Avoid_: database, cache

**Session overlay**:
The process-local replacement of one or more bundled reflection dumps. Overlay facts never persist. Only replaced dumps are in the overlay. The overlay table holds facts plus entry identity, not a second token dictionary.
_Avoid_: ResourceView, override set, world clone

**Bundled dumps**:
The serialize, behavior-context, and module JSON compiled into the tool. Always what the content index persists for `nw-resources`.
_Avoid_: embedded resources, nw-resources files
