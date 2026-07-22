use std::{
    env, fs,
    path::{Path, PathBuf},
    process::{self, Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use nw_lua::{
    bytecode::{OpcodeTable, SemanticOp, opinfo},
    chunk::Proto,
};

const CHILD_OK: &str = "NW_LUA_CORPUS_CHILD_OK";
const CHILD_ERR: &str = "NW_LUA_CORPUS_CHILD_ERR";
const CHILD_STRUCTURAL_REPORT: &str = "NW_LUA_STRUCTURAL_REPORT";
const CHILD_STRUCTURAL_PROTO: &str = "NW_LUA_STRUCTURAL_PROTO";

static NEXT_ID: AtomicUsize = AtomicUsize::new(0);

fn main() {
    let args = env::args_os().collect::<Vec<_>>();
    let result = if args.get(1).is_some_and(|arg| arg == "--idempotent") {
        let Some(luac) = args.get(2) else {
            eprintln!("usage: nw-lua-corpus-child --idempotent <luac.exe> <chunk.luac>");
            process::exit(2);
        };
        let Some(path) = args.get(3) else {
            eprintln!("usage: nw-lua-corpus-child --idempotent <luac.exe> <chunk.luac>");
            process::exit(2);
        };
        decompile_idempotent(Path::new(luac), Path::new(path))
    } else if args.get(1).is_some_and(|arg| arg == "--structural") {
        let Some(luac) = args.get(2) else {
            eprintln!("usage: nw-lua-corpus-child --structural <luac.exe> <chunk.luac>");
            process::exit(2);
        };
        let Some(path) = args.get(3) else {
            eprintln!("usage: nw-lua-corpus-child --structural <luac.exe> <chunk.luac>");
            process::exit(2);
        };
        decompile_structural(Path::new(luac), Path::new(path))
    } else {
        let Some(path) = args.get(1) else {
            eprintln!("usage: nw-lua-corpus-child <chunk.luac>");
            process::exit(2);
        };
        decompile(Path::new(path))
    };

    match result {
        Ok(()) => {
            println!("{CHILD_OK}");
            process::exit(0);
        }
        Err(error) => {
            println!("{CHILD_ERR}\t{}", one_line(&error));
            process::exit(0);
        }
    }
}

fn decompile(path: &Path) -> Result<(), String> {
    let bytes = fs::read(path).map_err(|err| err.to_string())?;
    nw_lua::decompile(&bytes)
        .map(|_| ())
        .map_err(|err| err.to_string())
}

fn decompile_idempotent(luac: &Path, path: &Path) -> Result<(), String> {
    let bytes = fs::read(path).map_err(|err| err.to_string())?;
    let first = nw_lua::decompile(&bytes).map_err(|err| err.to_string())?;
    let paths = TempPaths::new("idempotent");

    let result = (|| {
        fs::write(&paths.source, &first).map_err(|err| err.to_string())?;
        compile_lua(luac, &paths.source, &paths.bytecode)?;
        let second_bytes = fs::read(&paths.bytecode).map_err(|err| err.to_string())?;
        let second = nw_lua::decompile(&second_bytes).map_err(|err| err.to_string())?;
        if first != second {
            return Err(format!(
                "idempotency mismatch: first={} bytes second={} bytes",
                first.len(),
                second.len()
            ));
        }
        Ok(())
    })();

    paths.cleanup();
    result
}

fn decompile_structural(luac: &Path, path: &Path) -> Result<(), String> {
    let bytes = fs::read(path).map_err(|err| err.to_string())?;
    let core_source =
        nw_lua::decompile_core(&bytes).map_err(|err| format!("core decompile: {err}"))?;
    let idiomatic_source =
        nw_lua::decompile(&bytes).map_err(|err| format!("idiomatic decompile: {err}"))?;
    let paths = TempPaths::new("structural");

    let result = (|| {
        fs::write(&paths.source, &core_source).map_err(|err| err.to_string())?;
        compile_lua(luac, &paths.source, &paths.bytecode)
            .map_err(|err| format!("core recompile: {err}"))?;
        let second_bytes = fs::read(&paths.bytecode).map_err(|err| err.to_string())?;
        let original =
            structural_signature(&bytes).map_err(|err| format!("decode original: {err}"))?;
        let recompiled = structural_signature(&second_bytes)
            .map_err(|err| format!("decode recompiled: {err}"))?;
        print_structural_report(&original, &recompiled);

        fs::write(&paths.source, &idiomatic_source).map_err(|err| err.to_string())?;
        compile_lua(luac, &paths.source, &paths.bytecode)
            .map_err(|err| format!("idiomatic recompile: {err}"))?;
        Ok(())
    })();

    paths.cleanup();
    result
}

fn structural_signature(bytes: &[u8]) -> Result<Vec<ProtoSignature>, String> {
    let chunk = nw_lua::parse_chunk(bytes).map_err(|err| err.to_string())?;
    let table = OpcodeTable::builtin(chunk.header.version);
    let mut protos = Vec::new();
    collect_proto_signature(&chunk.root, &table, "root".to_string(), &mut protos);
    Ok(protos)
}

fn collect_proto_signature(
    proto: &Proto,
    table: &OpcodeTable,
    path: String,
    out: &mut Vec<ProtoSignature>,
) {
    let ops = proto
        .code
        .iter()
        .map(|raw| table.decode(*raw).op)
        .filter(|op| opinfo::is_structural_faithfulness_op(*op))
        .collect();
    out.push(ProtoSignature {
        path: path.clone(),
        ops,
    });
    for (index, child) in proto.protos.iter().enumerate() {
        collect_proto_signature(child, table, format!("{path}/{index}"), out);
    }
}

fn print_structural_report(original: &[ProtoSignature], recompiled: &[ProtoSignature]) {
    let report = structural_report(original, recompiled);
    println!(
        "{CHILD_STRUCTURAL_REPORT}\toriginal_protos={}\trecompiled_protos={}\texact_protos={}\ttotal_protos={}\tmatched_ops={}\ttotal_ops={}",
        report.original_protos,
        report.recompiled_protos,
        report.exact_protos,
        report.total_protos,
        report.matched_ops,
        report.total_ops
    );
    for proto in report.protos {
        println!(
            "{CHILD_STRUCTURAL_PROTO}\tpath={}\toriginal_len={}\trecompiled_len={}\tmatched_ops={}\ttotal_ops={}\texact={}",
            proto.path,
            proto.original_len,
            proto.recompiled_len,
            proto.matched_ops,
            proto.total_ops,
            proto.exact
        );
    }
}

fn structural_report(
    original: &[ProtoSignature],
    recompiled: &[ProtoSignature],
) -> StructuralReport {
    let total_protos = original.len().max(recompiled.len());
    let mut report = StructuralReport {
        original_protos: original.len(),
        recompiled_protos: recompiled.len(),
        exact_protos: 0,
        total_protos,
        matched_ops: 0,
        total_ops: 0,
        protos: Vec::with_capacity(total_protos),
    };

    for index in 0..total_protos {
        let left = original.get(index);
        let right = recompiled.get(index);
        let path = left
            .or(right)
            .map_or_else(|| format!("proto{index}"), |proto| proto.path.clone());
        let original_ops = left.map_or(&[][..], |proto| proto.ops.as_slice());
        let recompiled_ops = right.map_or(&[][..], |proto| proto.ops.as_slice());
        let matched_ops = original_ops
            .iter()
            .zip(recompiled_ops)
            .filter(|(original, recompiled)| original == recompiled)
            .count();
        let total_ops = original_ops.len().max(recompiled_ops.len());
        let exact = left.is_some()
            && right.is_some()
            && original_ops.len() == recompiled_ops.len()
            && matched_ops == total_ops;
        if exact {
            report.exact_protos += 1;
        }
        report.matched_ops += matched_ops;
        report.total_ops += total_ops;
        report.protos.push(ProtoStructuralReport {
            path,
            original_len: original_ops.len(),
            recompiled_len: recompiled_ops.len(),
            matched_ops,
            total_ops,
            exact,
        });
    }
    report
}

fn compile_lua(luac: &Path, source: &Path, bytecode: &Path) -> Result<(), String> {
    let output = Command::new(luac)
        .arg("-o")
        .arg(bytecode)
        .arg(source)
        .output()
        .map_err(|err| err.to_string())?;
    if output.status.success() {
        Ok(())
    } else {
        Err(output_summary(&output))
    }
}

fn output_summary(output: &Output) -> String {
    format!(
        "status={} stderr={} stdout={}",
        output.status,
        one_line(&String::from_utf8_lossy(&output.stderr)),
        one_line(&String::from_utf8_lossy(&output.stdout))
    )
}

fn one_line(message: &str) -> String {
    const MAX_LEN: usize = 300;
    let mut line = message.split_whitespace().collect::<Vec<_>>().join(" ");
    if line.len() > MAX_LEN {
        line.truncate(MAX_LEN);
    }
    line
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ProtoSignature {
    path: String,
    ops: Vec<SemanticOp>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct StructuralReport {
    original_protos: usize,
    recompiled_protos: usize,
    exact_protos: usize,
    total_protos: usize,
    matched_ops: usize,
    total_ops: usize,
    protos: Vec<ProtoStructuralReport>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ProtoStructuralReport {
    path: String,
    original_len: usize,
    recompiled_len: usize,
    matched_ops: usize,
    total_ops: usize,
    exact: bool,
}

struct TempPaths {
    source: PathBuf,
    bytecode: PathBuf,
}

impl TempPaths {
    fn new(label: &str) -> Self {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let millis = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time before UNIX_EPOCH")
            .as_millis();
        let stem = format!(
            "nw_lua_child_{label}_{}_{}",
            process::id(),
            millis + id as u128
        );
        let dir = env::temp_dir();
        Self {
            source: dir.join(format!("{stem}.lua")),
            bytecode: dir.join(format!("{stem}.luac")),
        }
    }

    fn cleanup(&self) {
        let _ = fs::remove_file(&self.source);
        let _ = fs::remove_file(&self.bytecode);
    }
}
