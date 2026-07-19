# AZoth Blender extension

AZoth turns the structured glTF packages produced by `nw-tools` into Blender
workspaces without copying Cry/Lumberyard parsers into Python.

From Blender 5.2, install the packaged extension and open the **AZoth** tab in
the 3D View sidebar. It scans `C:\nwt`, then **Open AZoth Workspace** performs the
whole workflow: import, deterministic categorization, particle/bone attachment,
shared-library creation, editable rig override, and per-character workspace save.
Opening an unchanged package again loads that existing workspace immediately.
Engine helpers stay categorized and one click away in the Outliner, but start
hidden so the initial viewport is a centered material-preview character view.
**Clean Character View** restores those presentation defaults at any time without
disabling armature deformation or removing authoring data.

Heavy data lives in `C:\nwt\.azoth\libraries`; the small files in
`C:\nwt\.azoth\workspaces` link it. Images and decoded WAV previews remain shared
external resources. Animation selection reuses one rig and swaps Actions; it does
not duplicate a character into one scene per clip.

The extension can also ask `nw-tools` to export a catalog filter. `nw-tools`
continues to own dependency discovery, legacy parsing, texture/audio conversion,
Mannequin event evaluation, and `nw-jobs` parallelism. Set `NW_TOOLS` only if the
binary is neither on `PATH`, in `C:\nwt`, nor in the development checkout.

Build and validate the distributable with Blender:

```powershell
& 'C:\Program Files\Blender Foundation\Blender 5.2\blender.exe' --command extension build --source-dir integrations\blender\azoth --output-dir C:\nwt\.azoth
& 'C:\Program Files\Blender Foundation\Blender 5.2\blender.exe' --command extension validate C:\nwt\.azoth\azoth-0.1.0.zip
```
