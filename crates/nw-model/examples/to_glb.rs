//! Convert a `.cgf` (+ `.cgfheap`) to `.glb`:
//! `cargo run -p nw-model --example to_glb -- <file.cgf> [out.glb]`.

fn main() {
    let cgf_path = std::env::args().nth(1).expect("usage: to_glb <file.cgf> [out.glb]");
    let out_path = std::env::args()
        .nth(2)
        .unwrap_or_else(|| cgf_path.replace(".cgf", ".glb"));

    let cgf = std::fs::read(&cgf_path).expect("read cgf");
    let heap_path = format!("{cgf_path}heap"); // foo.cgf -> foo.cgfheap
    let heap = std::fs::read(&heap_path).unwrap_or_default();

    let model = nw_model::model_from_bytes(&cgf, &heap).expect("assemble model");
    println!(
        "meshes={} verts={} tris={} (heap {} bytes)",
        model.meshes.len(),
        model.vertex_count(),
        model.triangle_count(),
        heap.len()
    );
    let glb = nw_model::Gltf::new(&model).to_glb();
    std::fs::write(&out_path, &glb).expect("write glb");
    println!("wrote {out_path} ({} bytes)", glb.len());
}
