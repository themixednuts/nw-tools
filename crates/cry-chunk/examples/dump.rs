//! Dump a Cry chunk file: `cargo run -p cry-chunk --example dump -- <file.cgf>`.

fn main() {
    let path = std::env::args().nth(1).expect("usage: dump <file.cgf>");
    let bytes = std::fs::read(&path).expect("read file");

    let file = cry_chunk::ChunkFile::parse(&bytes).expect("parse chunk file");
    println!("{path}");
    for header in file.chunks() {
        let header = header.expect("chunk header");
        let ty = header.chunk_type();
        let label = ty.map_or_else(
            || format!("0x{:04x}", header.kind()),
            |t| t.source_name().to_string(),
        );
        println!(
            "  #{:<5} {:<32} version=0x{:04x} size={}",
            header.id(),
            label,
            header.version(),
            header.size(),
        );
    }

    let model = cry_chunk::CgfFile::parse(&bytes).expect("build model view");
    println!(
        "model: {} mesh, {} subsets, {} physics, {} streams, {} refs, {} nodes, {} materials",
        model.meshes().len(),
        model.mesh_subsets().len(),
        model.mesh_physics_data().len(),
        model.data_streams().len(),
        model.data_refs().len(),
        model.nodes().len(),
        model.materials().len(),
    );
    for (id, mesh) in model.meshes() {
        println!(
            "  mesh id={id} vertices={} indices={} subsets={} subsets_chunk={} physics={:?}",
            mesh.vertex_count,
            mesh.index_count,
            mesh.subset_count,
            mesh.subsets_chunk_id,
            mesh.physics_data_chunk_ids,
        );
    }
    for (id, physics) in model.mesh_physics_data() {
        println!(
            "  physics id={id} flags={} tetrahedra_chunk={} physical_bytes={} tetrahedra_bytes={}",
            physics.flags,
            physics.tetrahedra_chunk_id,
            physics.physical_data.len(),
            physics.tetrahedra_data.len(),
        );
    }
    for stream in model.data_streams().values() {
        println!(
            "  stream type={} count={} size={}",
            stream.stream_type, stream.element_count, stream.element_size
        );
    }
    for (id, data_ref) in model.data_refs() {
        println!(
            "  data-ref id={id} flags={} index={} offset={} size={} stride={}",
            data_ref.flags, data_ref.index, data_ref.offset, data_ref.size, data_ref.stride,
        );
    }
    for material in model.materials().values() {
        println!(
            "  material name={} sub-materials={:?} physicalize={:?}",
            material.name, material.sub_material_names, material.physicalize_types
        );
    }
    for node in model.nodes().values() {
        println!(
            "  node name={} object={} parent={} material={} properties={:?}",
            node.name, node.object_id, node.parent_id, node.material_chunk_id, node.properties
        );
    }
}
