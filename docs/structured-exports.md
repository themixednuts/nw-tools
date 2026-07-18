# Structured exports

`nw-tools format model` supports the two standard glTF container forms, selected
with `--container`:

- `--container glb` writes one self-contained binary. It is convenient to move, but
  every GLB owns another copy of all of its buffers and images.
- `--container gltf` writes a JSON manifest with external standard glTF buffers and
  images, laid out to mirror the game's own asset-catalog directory tree.

The glTF specification technically allows a GLB JSON chunk to reference external
resources too. `nw-tools` deliberately reserves `--container glb` for the useful
self-contained behavior and uses `.gltf` manifests for shared packages. A hybrid
GLB would lose the single-file advantage while providing no additional sharing
or compatibility benefit over the inspectable `.gltf` layout.

Pass `--geometry-only` to skip material and texture resolution entirely and export
just the meshes and skeletons.

## Batch exports from the install

With no path argument, `format model` converts straight out of the install paks.
Narrow the run with `--filter <substring>` (case-insensitive path match). The flag
is repeatable: each `--filter` adds to a union, so

```text
nw-tools format model --container gltf --filter alligator.cdf --filter barghestwolf.cdf --out ./out
```

exports every mesh whose path contains *either* substring in a single run. Batching
several characters this way builds the shared authored-asset dependency index once
for the whole run rather than once per character. Omit `--filter` entirely to
convert the whole install.

All exports share one output root, so identical resources (skeletons, animations,
textures, geometry) are written once and deduplicated across models *and* across
successive runs into the same root — see the path-normalization rules below.

The output root mirrors the asset catalog:

```text
<output>/
  objects/characters/npc/natural/alligator/
    alligator.gltf        manifest, at the source asset's catalog path
    alligator.bin         derived geometry buffer, next to its manifest
    alligator.cdf         raw dependency, at its exact pak path
  animations/.../attack.caf        glTF channel buffer, at the CAF's exact path
  textures/.../alligator_diff.png  decoded image, at the texture's .dds path
```

The guiding principle is **content at the catalog path**: every file keeps the
authentic pak path and extension of the asset it represents, and its *content*
is the glTF-consumable form of that asset. An animation's `.caf` file therefore
holds the sampled glTF channel buffer the manifest references — not the raw
CryAnimation payload. Raw animation sources (`.caf`/`.i_caf`/`.dba`) are not
part of the package; glTF represents them natively as channels, so shipping the
compiled bytes would only duplicate what the manifest already encodes. Control
files that glTF *cannot* represent (CDF, CHRPARAMS, ADB, bspace, audio, …) still
ship as their raw bytes at their exact pak paths, since those bytes are the
needed format. Provenance for each clip survives on its glTF animation
(`extras.crySourcePath`) and in the manifest's `extras.dependencies`.

Placement rules:

- **Raw control dependencies** (CDF/CHRPARAMS/ADB/bspace/audio/… retained in
  extras) keep their exact pak source paths. Animation-event audio triggers are
  resolved end-to-end through the authored catalogs — never by filename, stem,
  or parameter-prefix matching. Each `cryEvents[].parameter` runs the chain:
  1. `footstep`-kind events resolve their parameter through the MaterialEffects
     FX library at `libs/materialeffects/fxlibs/<parameter>.xml` (shipped at its
     catalog path) to the real ATL trigger(s) and the surfaces they cover;
     `sound`/`audio` events use the parameter as the ATL trigger directly.
  2. The ATL controls (`libs/gameaudio/wwise/atl_controls.xml`) map the trigger
     to its Wwise event name(s).
  3. The event's owning banks come from the Wwise trigger-bank map
     (`triggerbankmapatlbin.bin`, keyed by `AZ::Crc32` of the event name); where
     the map does not cover an event, the ATL preload catalog
     (`preloaddata.xml`) supplies the bank group whose HIRC defines the event.
  4. A typed HIRC walk (Event → Action → Sound / random-sequence / switch /
     layer container) collects only the media a play action can actually reach
     from that event — a switch container's per-surface branches and a
     random container's variations all count — each tagged with the shipped bank
     whose `DIDX` owns it (or a loose `sounds/wwise/<mediaId>.wem` when no
     shipped bank embeds it).

  The manifest-level `extras.audioTriggers` table is keyed by the authored
  parameter so each keyframed event resolves in one hop; parameters that resolve
  to no catalog entry are dropped with a single summary note rather than shipped
  half-filled. For structured glTF, the reachable media is decoded to PCM WAV at
  `sounds/wwise/decoded/<mediaId>.wav` (via `vgmstream-cli`) and one playable
  Blender `.blend` is written next to *each* exported manifest
  (`<manifest dir>/<stem>.blend`) — single-file, tree, and install/batch runs
  alike, so a batch of N characters yields N blends beside their manifests and a
  closing `N blend(s) written` summary. Each blend has one scene per clip (the scene
  dropdown is the clip browser), each with that clip active on the shared
  armature and its audio events as VSE sound strips at the keyframed frames, WAVs
  packed so the file survives moves. Footstep strips follow the engine's
  weighted-random selection (deterministic seed, one continuing shuffle across
  clips, mirroring the engine's persistent container state). All placement
  decisions are computed in `nw-tools`; Blender only executes the plan. Pass
  `--no-decode-audio` / `--no-blend` to skip either step.

  Blender 5.x viewing notes: the top-bar Scene dropdown selects which clip
  plays (audio included); the Video Sequencer's own header dropdown is a
  per-workspace *display pin* that does not follow the active scene — match it
  manually to inspect a clip's strips. Viewport audio requires Preferences ▸
  System ▸ Audio Device ≠ "None".
- **Animation channel buffers** take the source CAF's exact catalog path (e.g.
  `.../attack.caf`); the glTF-format bytes replace the raw payload, so no
  `.caf.bin` sibling exists. A clip sourced from a `.dba` lands at its own
  per-clip CAF path.
- **Derived payloads** sit next to what they derive from: the model's geometry
  buffer is `<manifest path>.bin`, and decoded images take the shipped texture's
  catalog path with a `.png` extension (glTF registers only png/jpeg for image
  URIs). A CryEngine `ddna` normal is the one derived-name exception: its blue
  channel is rebuilt into an RGB normal at the texture's `.png` catalog path,
  while the gloss packed in its alpha splits into a metallic-roughness sibling
  named `<ddna stem>.rough.png` (that channel has no catalog identity of its
  own). Anonymous resources with no catalog identity are named after their
  manifest (`<manifest>.<index>.<ext>`).

Paths are normalized to forward-slash ASCII-lowercase, matching catalog
convention, so one authored asset arriving under different casings resolves to
one file. Exporting identical bytes to the same path — within one asset, across
assets, or in a later run — writes one file. A within-run claim on an
already-claimed path with *different* bytes (for example one CAF retargeted
onto two different skeletons in one batch) is disambiguated with a short
content-hash infix inserted before the final extension: `attack.caf` →
`attack.<12hex>.caf`. A leftover file from a previous run whose bytes differ is
atomically replaced.

Each manifest uses relative forward-slash URIs, so the complete output tree can
be moved as a unit. Meshes, inverse-bind matrices, individual animation clips,
and images remain independent resources to maximize useful sharing.

Resources are published atomically before their manifest. Manifest serialization
is compact, buffered, and streamed to a temporary file, which avoids allocating a
second copy of very large animation-heavy JSON documents. Publication is also
safe when `nw-jobs` sends multiple assets or resources to concurrent workers.

## What standard glTF can share

A `.gltf` document can reference external buffer and image files. That is the
portable mechanism used here and is understood by standard glTF consumers.

Core glTF does not provide a way for one document to import a mesh, skin,
animation, material, or node definition from another glTF document. Those JSON
definitions therefore remain in each manifest even when their binary payloads
share one file on disk. A custom extension could add cross-document object
references, but ordinary Blender and glTF tools would not understand it.

A structured export reduces on-disk payload duplication; it does not require an
importer such as Blender to lazy-load a huge character. Importing a manifest with
thousands of animation objects can still need substantial memory. GLB has the
same logical content and typically has the highest temporary-memory cost because
all payloads are also one monolithic binary.

## Reusable package API

`nw-artifact::PackageWriter` is format-independent. Other exporters can use the
same package root for geometry, textures, audio, generated tables, or any other
immutable payload:

1. Call `store_at(path, bytes)` from `nw-jobs` workers, passing the resource's
   catalog-relative path.
2. Call `uri_from(artifact_path, stored_blob)` for a portable manifest URI —
   the returned [`StoredBlob`] reports the final path actually used, plain or
   disambiguated.
3. Publish the manifest with `write_stream` after all referenced resources
   succeed.

The abstraction rejects absolute paths and parent traversal. Concurrent
identical stores converge on one write inside a process, and path-keyed
identity provides reuse across runs and exporters.
