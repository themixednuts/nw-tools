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
        "model: {} mesh, {} subsets, {} streams, {} refs, {} nodes, {} materials",
        model.meshes().len(),
        model.mesh_subsets().len(),
        model.data_streams().len(),
        model.data_refs().len(),
        model.nodes().len(),
        model.materials().len(),
    );
    for stream in model.data_streams().values() {
        println!(
            "  stream type={} count={} size={}",
            stream.stream_type, stream.element_count, stream.element_size
        );
    }
}
