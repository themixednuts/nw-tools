use std::{env, fs, path::Path, process::ExitCode};

use nw_rnr::legacy::{BvhTree, PhysicalShape, ShapeData, parse_shape_asset};

fn main() -> ExitCode {
    let mut failed = false;
    for path in env::args_os().skip(1) {
        if let Err(error) = inspect(Path::new(&path)) {
            eprintln!("{}: {error}", Path::new(&path).display());
            failed = true;
        }
    }

    ExitCode::from(u8::from(failed))
}

fn inspect(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let bytes = fs::read(path)?;
    let asset = parse_shape_asset(&bytes)?;
    println!(
        "{}: version={} objects={} shapes={}",
        path.display(),
        asset.version,
        asset.objects.len(),
        asset.shapes.len()
    );
    for shape in &asset.shapes {
        print_shape(shape, 1);
    }
    Ok(())
}

fn print_shape(shape: &PhysicalShape<'_>, depth: usize) {
    let indent = "  ".repeat(depth);
    print!("{indent}{:?}", shape.kind());
    match &shape.data {
        ShapeData::ConvexHull(value) => print!(
            " vertices={} planes={} convex_radius={} extra={}",
            value.vertices.len(),
            value.planes.len(),
            value.convex_radius,
            value.extra.is_some()
        ),
        ShapeData::Mesh(value) => {
            let bvh_version = match &value.bvh {
                BvhTree::V1(_) => 1,
                BvhTree::V2(_) => 2,
            };
            print!(
                " stream_header={} vertices={} triangles={} adjacency={} bvh={bvh_version}",
                value.stream_header,
                value.vertices.len(),
                value.indices.len() / 3,
                value.adjacent_triangles.is_some()
            );
        }
        ShapeData::Compound(value) => print!(" children={}", value.children.len()),
        ShapeData::Transform(_) => {}
        ShapeData::ScaleConvexPolytope(value) | ShapeData::ScaleMesh(value) => {
            print!(
                " stream_header={} scale={:?}",
                value.stream_header, value.scale
            );
        }
        ShapeData::HeightField(value) => {
            if let Some(data) = value.data {
                print!(
                    " layout={} version={} size={}x{} height_scale={} aabb={:?}..{:?} bytes={}",
                    value.layout,
                    data.version,
                    data.width,
                    data.length,
                    data.height_scale,
                    data.aabb_min,
                    data.aabb_max,
                    data.samples.len()
                );
            } else {
                print!(" layout={}", value.layout);
            }
        }
        _ => {}
    }
    println!(" extra={}", shape.extra.map_or(0, <[u8]>::len));

    match &shape.data {
        ShapeData::Compound(value) => {
            for child in &value.children {
                print_shape(&child.shape, depth + 1);
            }
        }
        ShapeData::Transform(value) => print_shape(&value.shape, depth + 1),
        ShapeData::ScaleConvexPolytope(value) | ShapeData::ScaleMesh(value) => {
            print_shape(&value.shape, depth + 1);
        }
        _ => {}
    }
}
