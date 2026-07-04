mod support;

use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use support::{run_equivalence, run_stripped_equivalence};

static NEXT_ID: AtomicUsize = AtomicUsize::new(0);

#[test]
fn runtime_equivalence_idiomatic_declaration_sugar_cases() {
    let cases = [
        (
            "module_function_declarations",
            r#"
local M = {}
function M.add(a, b)
    return a + b
end
function M.mul(a, b)
    return a * b
end
print(M.add(2, 3), M.mul(2, 3))
return M
"#,
            assert_module_function_declarations as fn(&str),
        ),
        (
            "method_declaration_self",
            r#"
local M = {}
M.n = 0
function M:inc()
    self.n = self.n + 1
    return self.n
end
print(M:inc(), M:inc())
return M
"#,
            assert_method_declaration_self,
        ),
        (
            "recursive_local_function",
            r#"
local function fact(n)
    if n <= 1 then
        return 1
    end
    return n * fact(n - 1)
end
print(fact(5))
"#,
            assert_recursive_local_function,
        ),
        (
            "guard_clause",
            r#"
local function f(x)
    if x then
        return 1
    end
end
print(f(true), f(false))
"#,
            assert_guard_clause,
        ),
        (
            "elseif_chain",
            r#"
local function f(x)
    local value
    if x == 1 then
        value = "one"
    else
        if x == 2 then
            value = "two"
        else
            value = "other"
        end
    end
    return value
end
print(f(1), f(2), f(3))
"#,
            assert_elseif_chain,
        ),
    ];

    for (name, source, assert_shape) in cases {
        let Some(decompiled) = run_equivalence(name, source) else {
            return;
        };
        assert_shape(&decompiled);
    }
}

fn assert_module_function_declarations(source: &str) {
    assert!(source.contains("function M.add("), "{source}");
    assert!(source.contains("function M.mul("), "{source}");
    assert!(!source.contains("M.add = function"), "{source}");
}

fn assert_method_declaration_self(source: &str) {
    assert!(source.contains("function M:inc("), "{source}");
    assert!(source.contains("self.n = self.n + 1"), "{source}");
}

fn assert_recursive_local_function(source: &str) {
    assert!(source.contains("local function fact("), "{source}");
}

fn assert_guard_clause(source: &str) {
    assert!(source.contains("if not x then"), "{source}");
    assert!(source.contains("return\n\tend\n\treturn 1"), "{source}");
}

fn assert_elseif_chain(source: &str) {
    assert!(source.contains("elseif x == 2 then"), "{source}");
}

#[test]
fn stripped_variants_style_synthetic_module_and_receiver_names() {
    let Some(module_source) = run_stripped_equivalence(
        "stripped_module_table",
        r#"
local M = {}
function M.add(a, b)
    return a + b
end
print(M.add(2, 3))
return M
"#,
    ) else {
        return;
    };
    assert!(
        module_source.contains("function v0.add("),
        "{module_source}"
    );
    assert!(
        !module_source.contains("function v0:add("),
        "{module_source}"
    );

    let Some(method_source) = run_stripped_equivalence(
        "stripped_method_table",
        r#"
local M = {}
M.n = 0
function M:inc()
    self.n = self.n + 1
    return self.n
end
print(M:inc(), M:inc())
return M
"#,
    ) else {
        return;
    };
    assert!(
        method_source.contains("function v0:inc("),
        "{method_source}"
    );
    assert!(
        method_source.contains("self.n = self.n + 1"),
        "{method_source}"
    );

    let Some(source_preserved) = run_without_debug_locals(
        "shopcommon",
        r#"
local M = {}
M.n = 0
function M.add(a, b)
    return a + b
end
function M:inc()
    self.n = self.n + 1
    return self.n
end
print(M.add(2, 3), M:inc(), M:inc())
return M
"#,
    ) else {
        return;
    };
    assert!(
        source_preserved.contains("local Shopcommon ="),
        "{source_preserved}"
    );
    assert!(
        source_preserved.contains("function Shopcommon.add("),
        "{source_preserved}"
    );
    assert!(
        source_preserved.contains("function Shopcommon:inc("),
        "{source_preserved}"
    );
    assert!(
        !source_preserved.contains("local v0 ="),
        "{source_preserved}"
    );
}

#[test]
fn nw_corpus_robustness_report() {
    let Some(tools) = lua_tools() else {
        return;
    };
    let files = corpus_files(50);
    if files.is_empty() {
        eprintln!("P9b NW corpus robustness: skipped; corpus paths not found");
        return;
    }

    let mut reparse_ok = 0;
    let mut recompile_ok = 0;
    let mut failures = Vec::new();
    for source in &files {
        let paths = TempPaths::new("phase9b_corpus");
        let result = compile_lua(tools.luac, source, &paths.bytecode)
            .and_then(|_| fs::read(&paths.bytecode).map_err(|err| err.to_string()))
            .and_then(|bytes| nw_lua::decompile(&bytes).map_err(|err| err.to_string()))
            .and_then(|decompiled| {
                full_moon::parse(&decompiled).map_err(|errors| format!("{errors:#?}"))?;
                reparse_ok += 1;
                fs::write(&paths.decompiled, decompiled).map_err(|err| err.to_string())?;
                compile_lua(tools.luac, &paths.decompiled, &paths.recompiled)?;
                recompile_ok += 1;
                Ok(())
            });
        if let Err(error) = result
            && failures.len() < 5
        {
            failures.push(format!("{}: {error}", source.display()));
        }
        paths.cleanup();
    }

    eprintln!(
        "P9b NW corpus robustness: {}/{} reparse, {}/{} recompile",
        reparse_ok,
        files.len(),
        recompile_ok,
        files.len()
    );
    if !failures.is_empty() {
        eprintln!("P9b NW corpus first failures:\n{}", failures.join("\n"));
    }
}

struct LuaTools {
    lua: &'static str,
    luac: &'static str,
}

struct TempPaths {
    bytecode: PathBuf,
    decompiled: PathBuf,
    recompiled: PathBuf,
}

impl TempPaths {
    fn new(name: &str) -> Self {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let millis = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time before UNIX_EPOCH")
            .as_millis();
        let stem = format!(
            "nw_lua_{name}_{}_{}",
            std::process::id(),
            millis + id as u128
        );
        let dir = std::env::temp_dir();
        Self {
            bytecode: dir.join(format!("{stem}.luac")),
            decompiled: dir.join(format!("{stem}_decompiled.lua")),
            recompiled: dir.join(format!("{stem}_recompiled.luac")),
        }
    }

    fn cleanup(&self) {
        let _ = fs::remove_file(&self.bytecode);
        let _ = fs::remove_file(&self.decompiled);
        let _ = fs::remove_file(&self.recompiled);
    }
}

struct SourceCasePaths {
    dir: PathBuf,
    source: PathBuf,
    bytecode: PathBuf,
    decompiled: PathBuf,
}

impl SourceCasePaths {
    fn new(stem: &str) -> Self {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let millis = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time before UNIX_EPOCH")
            .as_millis();
        let dir = std::env::temp_dir().join(format!(
            "nw_lua_phase9b_{}_{}",
            std::process::id(),
            millis + id as u128
        ));
        Self {
            source: dir.join(format!("{stem}.lua")),
            bytecode: dir.join(format!("{stem}.luac")),
            decompiled: dir.join(format!("{stem}_decompiled.lua")),
            dir,
        }
    }

    fn cleanup(&self) {
        let _ = fs::remove_file(&self.source);
        let _ = fs::remove_file(&self.bytecode);
        let _ = fs::remove_file(&self.decompiled);
        let _ = fs::remove_dir(&self.dir);
    }
}

fn lua_tools() -> Option<LuaTools> {
    let tools = LuaTools {
        lua: r"E:\Projects\lua-5.1.5\src\lua.exe",
        luac: r"E:\Projects\lua-5.1.5\src\luac.exe",
    };
    (Path::new(tools.lua).exists() && Path::new(tools.luac).exists()).then_some(tools)
}

fn run_without_debug_locals(name: &str, source: &str) -> Option<String> {
    let tools = lua_tools()?;
    let paths = SourceCasePaths::new(name);
    fs::create_dir_all(&paths.dir).expect("create temp source directory");
    fs::write(&paths.source, source).expect("write source-preserved synthetic case");
    compile_lua(tools.luac, &paths.source, &paths.bytecode).expect("compile Lua source");

    let original_stdout = run_lua(tools.lua, &paths.source, "original Lua source");
    let bytecode = fs::read(&paths.bytecode).expect("read bytecode");
    let mut chunk = nw_lua::parse_chunk(&bytecode).expect("parse bytecode");
    clear_debug_names(&mut chunk.root);
    let table =
        nw_lua::bytecode::OpcodeTable::builtin(chunk.header.version).expect("Lua opcode table");
    let ssa = nw_lua::ir::build_ssa(&chunk.root, &table);
    let block = nw_lua::decompile::decompile_proto(&chunk.root, &ssa, &table)
        .expect("decompile with synthetic names");
    let decompiled = nw_lua::to_source(&block).expect("emit decompiled source");

    fs::write(&paths.decompiled, &decompiled).expect("write decompiled source");
    let decompiled_stdout = run_lua(tools.lua, &paths.decompiled, "decompiled Lua source");
    assert_eq!(
        original_stdout, decompiled_stdout,
        "{name} stdout differed\nsource:\n{decompiled}"
    );
    paths.cleanup();
    Some(decompiled)
}

fn clear_debug_names(proto: &mut nw_lua::chunk::Proto) {
    proto.loc_vars.clear();
    for upvalue in &mut proto.upvalues {
        upvalue.name.clear();
    }
    for child in &mut proto.protos {
        clear_debug_names(child);
    }
}

fn run_lua(lua: &str, source: &Path, context: &str) -> Vec<u8> {
    let output = Command::new(lua)
        .arg(source)
        .output()
        .unwrap_or_else(|err| panic!("failed to run {context}: {err}"));
    assert_success(output, context)
}

fn corpus_files(limit: usize) -> Vec<PathBuf> {
    let roots = [
        Path::new(r"E:\Projects\az-rs\resources\fixtures\lua\good-lua"),
        Path::new(r"E:\Projects\DEMOJSON"),
    ];
    let mut files = Vec::new();
    for root in roots {
        collect_lua_files(root, limit, &mut files);
        if files.len() >= limit {
            break;
        }
    }
    files.sort();
    files.truncate(limit);
    files
}

fn collect_lua_files(root: &Path, limit: usize, files: &mut Vec<PathBuf>) {
    if !root.exists() {
        return;
    }
    let mut pending = vec![root.to_path_buf()];
    while let Some(path) = pending.pop() {
        if files.len() >= limit {
            return;
        }
        let Ok(entries) = fs::read_dir(path) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                pending.push(path);
            } else if path.extension().is_some_and(|ext| ext == "lua") {
                files.push(path);
                if files.len() >= limit {
                    return;
                }
            }
        }
    }
}

fn compile_lua(luac: &str, source: &Path, bytecode: &Path) -> Result<(), String> {
    let output = Command::new(luac)
        .arg("-o")
        .arg(bytecode)
        .arg(source)
        .output()
        .map_err(|err| err.to_string())?;
    if output.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).into_owned())
    }
}

fn assert_success(output: Output, context: &str) -> Vec<u8> {
    assert!(
        output.status.success(),
        "{context} failed with status {}\nstderr:\n{}\nstdout:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout)
    );
    output.stdout
}
