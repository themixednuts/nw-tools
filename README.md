# nw-tools

New World asset inspection and pak tooling.

## Install

```powershell
cargo install --git https://github.com/themixednuts/nw-tools --locked nw-tools
```

## Usage

```powershell
nw-tools --help
```

Use `--help` on any subcommand for details.

## Cry model and character export

`format model` converts standalone or PAK-mounted `.cgf`, `.skin`, `.chr`,
`.cdf`, `.caf`, and `.dba` assets to GLB/glTF. CDF export resolves the complete
character graph: independent nested skeletons, skin/bone/cloth attachments,
CHRPARAMS animation lists and DBA tracks, animation events, Mannequin sources,
materials, split DDS textures (including attached alpha), and optional ATL/Wwise
metadata.

```powershell
# Resolve one shipped CDF and every referenced asset from the located install.
nw-tools --plain format model --filter objects/characters/example.cdf --out models

# Add explicit animation, Mannequin, and audio control sources.
nw-tools format model character.cdf --animation locomotion.caf `
  --mannequin controllerdefs.xml --audio atl_controls.xml --out character.glb
```

Non-render physics proxies and runtime-defined material textures such as
`nearest_cubemap` remain typed in Cry extras instead of being fabricated as glTF
geometry or file textures. Missing explicit dependencies fail the conversion.

Choose `--format glb` for one self-contained binary file. Choose `--format gltf`
for automatic structured output: manifests keep mesh, skeleton, animation, and
texture payloads in `_shared/sha256`, and every export under the same output root
reuses byte-identical resources. No manual asset linking is required.

```text
models/
  _shared/sha256/<content-hash>.bin
  _shared/sha256/<content-hash>.png
  objects/characters/example.gltf
  objects/characters/another.gltf
```

See [docs/structured-exports.md](docs/structured-exports.md) for the container
tradeoffs, package guarantees, and reusable exporter API.

## ATL and Wwise inspection

`format audio` validates ATL XML, Wwise BNK/WEM containers, and New World's
`triggerbankmapatlbin.bin`. BNK DIDX/DATA media can be extracted and every WEM is
validated before it is written.

```powershell
nw-tools format audio sounds/example.bnk --out example.json `
  --extract-embedded embedded-wem
```

## SerializeContext type generation

Use a checked-in selection manifest to generate the reflected type closure used
by runtime crates instead of maintaining hand-written mirrors:

```powershell
nw-serialize-codegen generate --language rust --rust-layout vendored `
  --selection explicit --selection-file selection.json --out crates/generated-types
```

## GameData SDK generation

See [docs/gamedata-codegen.md](docs/gamedata-codegen.md) for the self-contained
Rust, TypeScript, and Go architecture and public API contract.
