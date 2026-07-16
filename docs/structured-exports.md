# Structured exports

`nw-tools format model` supports the two standard glTF container forms:

- `--format glb` writes one self-contained binary. It is convenient to move, but
  every GLB owns another copy of all of its buffers and images.
- `--format gltf` writes a JSON manifest with external standard glTF buffers and
  images. `nw-tools` automatically stores those resources in a shared,
  content-addressed directory.

The output root has this shape:

```text
<output>/
  _shared/sha256/
    <sha256>.bin
    <sha256>.png
  <asset path>.gltf
```

Each manifest uses relative forward-slash URIs, so the complete output tree can
be moved as a unit. A resource filename is the SHA-256 of its contents. Exporting
the same bytes again—within one asset, across different assets, or in a later
run—reuses the existing file. Meshes, inverse-bind matrices, individual animation
clips, and images remain independent resources to maximize useful sharing.

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
share one content-addressed file. A custom extension could add cross-document
object references, but ordinary Blender and glTF tools would not understand it.

A structured export reduces on-disk payload duplication; it does not require an
importer such as Blender to lazy-load a huge character. Importing a manifest with
thousands of animation objects can still need substantial memory. GLB has the
same logical content and typically has the highest temporary-memory cost because
all payloads are also one monolithic binary.

## Reusable package API

`nw-artifact::PackageWriter` is format-independent. Other exporters can use the
same package root for geometry, textures, audio, generated tables, or any other
immutable payload:

1. Call `store(bytes, extension)` from `nw-jobs` workers.
2. Call `uri_from(artifact_path, stored_blob)` for a portable manifest URI.
3. Publish the manifest with `write_stream` after all referenced blobs succeed.

The abstraction rejects absolute paths, parent traversal, and unsafe extensions.
Concurrent identical stores converge on one write inside a process, while
content-addressed filenames provide reuse across runs and exporters.
