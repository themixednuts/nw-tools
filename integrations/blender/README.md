# AZoth Blender extension

AZoth turns the structured glTF packages produced by `nw-tools` into Blender
workspaces without copying Cry/Lumberyard parsers into Python.

From Blender 5.2, install the packaged extension and open the **AZoth** tab in
the 3D View sidebar. Set **Package root** in AZoth preferences (or the sidebar
path field) to the directory that holds your structured exports — leave it empty
to use `AZOTH_PACKAGE_ROOT` / `NWT_ROOT`, otherwise `~/nwt`. **Open AZoth
Workspace** then performs the whole workflow: import, deterministic
categorization, particle/bone attachment, shared-library creation, editable rig
override, and per-character workspace save. Opening an unchanged package again
loads that existing workspace immediately. Engine helpers stay categorized and
one click away in the Outliner, but start hidden so the initial viewport is a
centered material-preview character view. **Clean Character View** restores those
presentation defaults at any time without disabling armature deformation or
removing authoring data.

Heavy data lives in `<package-root>/.azoth/libraries`; the small files in
`<package-root>/.azoth/workspaces` link it. Images and decoded WAV previews
remain shared external resources. Animation selection reuses one rig and swaps
Actions; it does not duplicate a character into one scene per clip.

## nw-tools sidecar

Export, schedule, and audio helpers shell out to `nw-tools`. Discovery order:

1. AZoth preference **nw-tools**
2. `NW_TOOLS` environment variable
3. `nw-tools` / `nw-tools.exe` on `PATH`
4. Extension sidecar: `<extension>/nw-tools(.exe)` or `<extension>/bin/nw-tools(.exe)`
5. `<package-root>/nw-tools(.exe)`

Ship a release binary next to the extension (or under `bin/`) when distributing
the zip so recipients do not need a local checkout.

The extension can also ask `nw-tools` to export a catalog filter. `nw-tools`
continues to own dependency discovery, legacy parsing, texture/audio conversion,
Mannequin event evaluation, and `nw-jobs` parallelism.

Build and validate the distributable with Blender:

```powershell
& 'C:\Program Files\Blender Foundation\Blender 5.2\blender.exe' --command extension build --source-dir integrations\blender\azoth --output-dir "$env:USERPROFILE\nwt\.azoth"
& 'C:\Program Files\Blender Foundation\Blender 5.2\blender.exe' --command extension validate "$env:USERPROFILE\nwt\.azoth\azoth-0.1.0.zip"
```
