use std::{
    alloc::{GlobalAlloc, Layout, System},
    hint::black_box,
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant},
};

use nw_lua::{DecompOptions, bytecode::OpcodeTable, chunk::Chunk, ir};

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

static ALLOCS: AtomicU64 = AtomicU64::new(0);
static ALLOC_BYTES: AtomicU64 = AtomicU64::new(0);
static REALLOCS: AtomicU64 = AtomicU64::new(0);
static REALLOC_BYTES: AtomicU64 = AtomicU64::new(0);

struct CountingAllocator;

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCS.fetch_add(1, Ordering::Relaxed);
        ALLOC_BYTES.fetch_add(layout.size() as u64, Ordering::Relaxed);
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) };
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        REALLOCS.fetch_add(1, Ordering::Relaxed);
        REALLOC_BYTES.fetch_add(new_size as u64, Ordering::Relaxed);
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[derive(Debug, Clone, Copy)]
struct Sample {
    name: &'static str,
    bytes: &'static [u8],
}

const SAMPLES: &[Sample] = &[
    Sample {
        name: "shopcommon",
        bytes: include_bytes!("../tests/fixtures/shopcommon.luac"),
    },
    Sample {
        name: "const_arith",
        bytes: include_bytes!("../tests/fixtures/linear/const_arith.luac"),
    },
    Sample {
        name: "local_add",
        bytes: include_bytes!("../tests/fixtures/linear/local_add.luac"),
    },
    Sample {
        name: "method_string",
        bytes: include_bytes!("../tests/fixtures/linear/method_string.luac"),
    },
    Sample {
        name: "square_local",
        bytes: include_bytes!("../tests/fixtures/linear/square_local.luac"),
    },
    Sample {
        name: "table_field",
        bytes: include_bytes!("../tests/fixtures/linear/table_field.luac"),
    },
    Sample {
        name: "generic_for",
        bytes: include_bytes!("../tests/fixtures/control_flow/generic_for.luac"),
    },
    Sample {
        name: "if_else_phi",
        bytes: include_bytes!("../tests/fixtures/control_flow/if_else_phi.luac"),
    },
    Sample {
        name: "if_elseif_else",
        bytes: include_bytes!("../tests/fixtures/control_flow/if_elseif_else.luac"),
    },
    Sample {
        name: "nested_for_if",
        bytes: include_bytes!("../tests/fixtures/control_flow/nested_for_if.luac"),
    },
    Sample {
        name: "numeric_for",
        bytes: include_bytes!("../tests/fixtures/control_flow/numeric_for.luac"),
    },
    Sample {
        name: "repeat",
        bytes: include_bytes!("../tests/fixtures/control_flow/repeat.luac"),
    },
    Sample {
        name: "while",
        bytes: include_bytes!("../tests/fixtures/control_flow/while.luac"),
    },
];

#[derive(Debug, Clone, Copy)]
struct AllocSnapshot {
    allocs: u64,
    alloc_bytes: u64,
    reallocs: u64,
    realloc_bytes: u64,
}

#[derive(Debug)]
struct BenchResult {
    name: &'static str,
    iterations: usize,
    files: usize,
    elapsed: Duration,
    allocs: u64,
    alloc_bytes: u64,
    reallocs: u64,
    realloc_bytes: u64,
}

fn main() {
    let iterations = std::env::var("NW_LUA_BENCH_ITERS")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(20);

    let parsed = parsed_samples();
    let ssa = ssa_samples(&parsed);
    let blocks = ast_samples(&parsed, &ssa);

    let results = [
        measure("full_decompile", iterations, SAMPLES.len(), || {
            for sample in SAMPLES {
                let source = nw_lua::decompile(black_box(sample.bytes))
                    .unwrap_or_else(|err| panic!("{}: {err}", sample.name));
                black_box(source);
            }
        }),
        measure("parse_chunk", iterations, SAMPLES.len(), || {
            for sample in SAMPLES {
                let chunk = nw_lua::parse_chunk(black_box(sample.bytes))
                    .unwrap_or_else(|err| panic!("{}: {err}", sample.name));
                black_box(chunk);
            }
        }),
        measure("build_ssa", iterations, parsed.len(), || {
            for parsed in &parsed {
                let ssa = ir::build_ssa(black_box(&parsed.chunk.root), black_box(&parsed.table));
                black_box(ssa);
            }
        }),
        measure("decompile_ast_core", iterations, ssa.len(), || {
            for item in &ssa {
                let block = nw_lua::decompile::decompile_proto_with_options(
                    black_box(&item.parsed.chunk.root),
                    black_box(&item.ssa),
                    black_box(&item.parsed.table),
                    DecompOptions::core(),
                )
                .unwrap_or_else(|err| panic!("{}: {err}", item.parsed.name));
                black_box(block);
            }
        }),
        measure("decompile_ast_idiomatic", iterations, ssa.len(), || {
            for item in &ssa {
                let block = nw_lua::decompile::decompile_proto_with_options(
                    black_box(&item.parsed.chunk.root),
                    black_box(&item.ssa),
                    black_box(&item.parsed.table),
                    DecompOptions::idiomatic(),
                )
                .unwrap_or_else(|err| panic!("{}: {err}", item.parsed.name));
                black_box(block);
            }
        }),
        measure("emit_to_source", iterations, blocks.len(), || {
            for item in &blocks {
                let source = nw_lua::emit::to_source(black_box(&item.block))
                    .unwrap_or_else(|err| panic!("{}: {err}", item.name));
                black_box(source);
            }
        }),
    ];

    println!("nw-lua decompile hot path benchmark");
    println!("samples={} iterations={iterations}", SAMPLES.len());
    println!(
        "{:<24} {:>10} {:>12} {:>14} {:>14} {:>14} {:>14}",
        "stage", "ms", "us/file", "allocs/file", "bytes/file", "reallocs/file", "rbytes/file"
    );
    for result in results {
        print_result(&result);
    }
}

struct ParsedSample {
    name: &'static str,
    chunk: Chunk,
    table: OpcodeTable,
}

struct SsaSample<'a> {
    parsed: &'a ParsedSample,
    ssa: ir::SsaFunction,
}

struct AstSample {
    name: &'static str,
    block: nw_lua::decompile::ast::Block,
}

fn parsed_samples() -> Vec<ParsedSample> {
    SAMPLES
        .iter()
        .map(|sample| {
            let chunk = nw_lua::parse_chunk(sample.bytes)
                .unwrap_or_else(|err| panic!("{} parse: {err}", sample.name));
            let table = OpcodeTable::builtin(chunk.header.version)
                .unwrap_or_else(|err| panic!("{} opcode table: {err}", sample.name));
            ParsedSample {
                name: sample.name,
                chunk,
                table,
            }
        })
        .collect()
}

fn ssa_samples(parsed: &[ParsedSample]) -> Vec<SsaSample<'_>> {
    parsed
        .iter()
        .map(|parsed| SsaSample {
            parsed,
            ssa: ir::build_ssa(&parsed.chunk.root, &parsed.table),
        })
        .collect()
}

fn ast_samples(parsed: &[ParsedSample], ssa: &[SsaSample<'_>]) -> Vec<AstSample> {
    parsed
        .iter()
        .zip(ssa)
        .map(|(parsed, ssa)| {
            let block = nw_lua::decompile::decompile_proto_with_options(
                &parsed.chunk.root,
                &ssa.ssa,
                &parsed.table,
                DecompOptions::idiomatic(),
            )
            .unwrap_or_else(|err| panic!("{} decompile ast: {err}", parsed.name));
            AstSample {
                name: parsed.name,
                block,
            }
        })
        .collect()
}

fn measure(
    name: &'static str,
    iterations: usize,
    files: usize,
    mut f: impl FnMut(),
) -> BenchResult {
    f();
    reset_allocs();
    let start_allocs = snapshot_allocs();
    let start = Instant::now();
    for _ in 0..iterations {
        f();
    }
    let elapsed = start.elapsed();
    let end_allocs = snapshot_allocs();

    BenchResult {
        name,
        iterations,
        files,
        elapsed,
        allocs: end_allocs.allocs - start_allocs.allocs,
        alloc_bytes: end_allocs.alloc_bytes - start_allocs.alloc_bytes,
        reallocs: end_allocs.reallocs - start_allocs.reallocs,
        realloc_bytes: end_allocs.realloc_bytes - start_allocs.realloc_bytes,
    }
}

fn reset_allocs() {
    ALLOCS.store(0, Ordering::Relaxed);
    ALLOC_BYTES.store(0, Ordering::Relaxed);
    REALLOCS.store(0, Ordering::Relaxed);
    REALLOC_BYTES.store(0, Ordering::Relaxed);
}

fn snapshot_allocs() -> AllocSnapshot {
    AllocSnapshot {
        allocs: ALLOCS.load(Ordering::Relaxed),
        alloc_bytes: ALLOC_BYTES.load(Ordering::Relaxed),
        reallocs: REALLOCS.load(Ordering::Relaxed),
        realloc_bytes: REALLOC_BYTES.load(Ordering::Relaxed),
    }
}

fn print_result(result: &BenchResult) {
    let files = (result.iterations * result.files) as u64;
    println!(
        "{:<24} {:>10.2} {:>12.2} {:>14.2} {:>14.0} {:>14.2} {:>14.0}",
        result.name,
        result.elapsed.as_secs_f64() * 1_000.0,
        per_file(result.elapsed.as_secs_f64() * 1_000_000.0, files),
        per_file(result.allocs as f64, files),
        per_file(result.alloc_bytes as f64, files),
        per_file(result.reallocs as f64, files),
        per_file(result.realloc_bytes as f64, files),
    );
}

fn per_file(value: f64, files: u64) -> f64 {
    if files == 0 {
        0.0
    } else {
        value / files as f64
    }
}
