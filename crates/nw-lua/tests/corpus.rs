mod support;

use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use support::run_equivalence;

static NEXT_ID: AtomicUsize = AtomicUsize::new(0);

const LUA: &str = r"E:\Projects\lua-5.1.5\src\lua.exe";
const LUAC: &str = r"E:\Projects\lua-5.1.5\src\luac.exe";
const GOOD_LUA: &str = r"E:\Projects\az-rs\resources\fixtures\lua\good-lua";
const DEMOJSON: &str = r"E:\Projects\DEMOJSON";
const CORPUS_FAST_SAMPLE_LIMIT: usize = 40;
const CORPUS_HEAVY_SAMPLE_LIMIT: usize = 300;
const CHILD_OK: &str = "NW_LUA_CORPUS_CHILD_OK";
const CHILD_ERR: &str = "NW_LUA_CORPUS_CHILD_ERR";
const CHILD_STRUCTURAL_REPORT: &str = "NW_LUA_STRUCTURAL_REPORT";

#[test]
fn runtime_equivalence_table_constructor_lists() {
    let cases = [
        (
            "table_list_simple",
            r#"
local t = {10, 20, 30}
print(t[1], t[2], t[3])
"#,
        ),
        (
            "table_list_mixed",
            r#"
local t = {1, 2, x = 3, [5] = 5}
print(t[1], t[2], t.x, t[5])
"#,
        ),
        ("table_list_long_setlist_batches", &long_table_source()),
        (
            "table_list_trailing_multiret",
            r#"
local t = {select(1, "a", "b", "c")}
print(#t)
"#,
        ),
    ];

    for (name, source) in cases {
        let _ = run_equivalence(name, source);
    }
}

#[test]
fn runtime_equivalence_phase10b_hardening_cases() {
    let cases = [
        (
            "break_while",
            r#"
local i = 0
while i < 5 do
    i = i + 1
    if i == 3 then
        break
    end
    print("w", i)
end
print("done", i)
"#,
        ),
        (
            "break_numeric_for",
            r#"
local total = 0
for i = 1, 5 do
    if i == 4 then
        break
    end
    total = total + i
end
print(total)
"#,
        ),
        (
            "break_generic_for",
            r#"
local total = 0
for _, value in ipairs({1, 2, 3, 4}) do
    if value == 3 then
        break
    end
    total = total + value
end
print(total)
"#,
        ),
        (
            "break_repeat_until",
            r#"
local i = 0
repeat
    i = i + 1
    if i == 3 then
        break
    end
    print("r", i)
until i > 5
print("done", i)
"#,
        ),
        (
            "nested_inner_and_outer_break",
            r#"
local out = {}
for i = 1, 4 do
    for j = 1, 4 do
        if j == 3 then
            break
        end
        out[#out + 1] = i .. ":" .. j
    end
    if i == 2 then
        break
    end
end
print(table.concat(out, ","))
"#,
        ),
        (
            "nested_flagged_outer_break",
            r#"
local out = {}
local stop = false
for i = 1, 4 do
    for j = 1, 4 do
        if i == 3 and j == 2 then
            stop = true
            break
        end
        out[#out + 1] = i .. ":" .. j
    end
    if stop then
        break
    end
end
print(table.concat(out, ","))
"#,
        ),
        (
            "short_circuit_loop_condition",
            r#"
local x = false
local n = 0
while x == false or n < 3 do
    n = n + 1
    x = true
print(n)
end
"#,
        ),
        (
            "and_or_loop_condition",
            r#"
local i = 0
local keep = true
while keep and (i < 3 or i == 5) do
    i = i + 1
    if i == 2 then
        keep = false
    end
    print(i, keep)
end
"#,
        ),
        (
            "returns_inside_loop_branches",
            r#"
local function probe(limit)
    local i = 0
    while i < 5 do
        if i > limit then
            return "over", i
        end
        if i == 2 then
            return "hit", i
        end
        i = i + 1
    end
    return "done", i
end
print(probe(5))
print(probe(1))
"#,
        ),
        (
            "short_circuit_assignment_value",
            r#"
local function f(o)
    o = o or {}
    return o.x
end
print(f({ x = 7 }), f(nil))
"#,
        ),
        (
            "deep_nested_loop_conditionals",
            r#"
local total = 0
for a = 1, 3 do
    for b = 1, 2 do
        local c = 0
        while c < 2 do
            c = c + 1
            total = total + a * b + c
        end
    end
end
print(total)
"#,
        ),
        (
            "repeat_until_mid_body_break",
            r#"
local i = 0
local total = 0
repeat
    i = i + 1
    total = total + i
    if total > 6 then
        break
    end
    total = total + 1
until i > 10
print(i, total)
"#,
        ),
        (
            "mixed_multi_result_local",
            r#"
local function f()
    return 1, 2
end
local function g()
    return 3, 4
end
local a, b, c = f(), g()
print(a, b, c)
"#,
        ),
        (
            "method_multi_result_local",
            r#"
local i, j, minus, int, fraction = string.format("%.0f", -123):find("([-]?)(%d+)([.]?%d*)")
print(i, j, minus, int, fraction)
"#,
        ),
        (
            "open_setlist_constructor",
            r#"
local function f()
    return "b", "c"
end
local t = {"a", f()}
print(t[1], t[2], t[3])
"#,
        ),
        (
            "open_setlist_fallback",
            r#"
local function f()
    return "b", nil, "d"
end
local flag = false
local t = { enabled = flag == true, "a", f() }
print(t.enabled, t[1], t[2] == nil, t[3], t[4])
"#,
        ),
        (
            "inline_order_two_call_args",
            r#"
local function log(x)
    io.write(x)
    return x
end
local a = log("1")
local b = log("2")
print(a, b)
"#,
        ),
        (
            "inline_order_binary_call_operands",
            r#"
local function log(x)
    io.write(x)
    return x
end
local a = log("1")
local b = log("2")
print(a .. b)
"#,
        ),
        (
            "inline_order_mixed_pure_impure",
            r#"
local function log(x)
    io.write(x)
    return x
end
local a = log("A")
local b = 1 + 2
local c = log("B")
print(a, b, c)
"#,
        ),
        (
            "inline_order_intervening_state_read",
            r#"
local state = "0"
local function set(x)
    state = x
    io.write(x)
    return x
end
local a = set("1")
local b = set("2")
print(a, state, b)
"#,
        ),
        (
            "inline_order_table_constructor_fields",
            r#"
local function log(x)
    io.write(x)
    return x
end
local function sink(t)
    print(t.first, t.second)
end
local first = log("1")
local t = { first = first, second = log("2") }
sink(t)
"#,
        ),
    ];

    for (name, source) in cases {
        let _ = run_equivalence(name, source);
    }
}

#[test]
fn nw_named_regressions_decompile_and_reparse() {
    if !tools_available() {
        eprintln!("skipping NW named regression tests; missing Lua 5.1 tools");
        return;
    }

    for source in [
        Path::new(r"E:\Projects\DEMOJSON\lyshineui\_common\difficultycolors.lua"),
        Path::new(r"E:\Projects\DEMOJSON\lyshineui\_common\basescreeninternal.lua"),
    ] {
        if !source.exists() {
            eprintln!("skipping missing NW regression source {}", source.display());
            continue;
        }
        let paths = TempPaths::new("phase9c_named");
        compile_lua(source, &paths.bytecode).expect("compile NW regression source");
        let bytecode = fs::read(&paths.bytecode).expect("read compiled NW regression bytecode");
        let decompiled = nw_lua::decompile(&bytecode)
            .unwrap_or_else(|err| panic!("{} failed to decompile: {err}", source.display()));
        full_moon::parse(&decompiled).unwrap_or_else(|err| {
            panic!(
                "{} decompiled source failed to reparse: {err:#?}",
                source.display()
            )
        });
        paths.cleanup();
    }
}

#[test]
fn nw_corpus_decompiles_and_recompiles_cleanly_with_structural_report() {
    run_nw_corpus_structural_sample(CORPUS_FAST_SAMPLE_LIMIT, "fast");
}

#[test]
#[ignore = "runs the larger NW corpus sweep"]
fn nw_corpus_heavy_decompiles_and_recompiles_cleanly_with_structural_report() {
    run_nw_corpus_structural_sample(CORPUS_HEAVY_SAMPLE_LIMIT, "heavy");
}

fn run_nw_corpus_structural_sample(limit: usize, label: &str) {
    if !tools_available() {
        eprintln!("skipping NW corpus test; missing Lua 5.1 tools");
        return;
    }
    if !Path::new(GOOD_LUA).exists() || !Path::new(DEMOJSON).exists() {
        eprintln!("skipping NW corpus test; corpus roots are missing");
        return;
    }

    let files = corpus_files(limit);
    assert!(!files.is_empty(), "NW corpus roots contained no Lua files");

    let mut ok = 0usize;
    let mut source_compile_err = 0usize;
    let mut decompile_err = 0usize;
    let mut crash = 0usize;
    let mut decompile_ok = 0usize;
    let mut core_recompile_err = 0usize;
    let mut idiomatic_recompile_err = 0usize;
    let mut recompile_ok = 0usize;
    let mut structural_exact_protos = 0usize;
    let mut structural_total_protos = 0usize;
    let mut structural_matched_ops = 0usize;
    let mut structural_total_ops = 0usize;
    let mut buckets = BTreeMap::<String, FailureBucket>::new();
    let worker = Path::new(env!("CARGO_BIN_EXE_nw-lua-corpus-child"));

    for source in &files {
        let paths = TempPaths::new("phase9c_corpus");
        let result = compile_lua(source, &paths.bytecode)
            .map_err(|message| FileResult::Err(format!("luac: {message}")))
            .and_then(|_| run_child_structural(worker, &paths.bytecode));

        match result {
            Ok(report) => {
                ok += 1;
                decompile_ok += 1;
                recompile_ok += 1;
                structural_exact_protos += report.exact_protos;
                structural_total_protos += report.total_protos;
                structural_matched_ops += report.matched_ops;
                structural_total_ops += report.total_ops;
            }
            Err(FileResult::Err(message)) => {
                if message.starts_with("luac:") {
                    source_compile_err += 1;
                } else if message.starts_with("core recompile:") {
                    decompile_ok += 1;
                    core_recompile_err += 1;
                } else if message.starts_with("idiomatic recompile:") {
                    decompile_ok += 1;
                    idiomatic_recompile_err += 1;
                } else {
                    decompile_err += 1;
                }
                record_bucket(&mut buckets, source, message);
            }
            Err(FileResult::Crash(message)) => {
                crash += 1;
                record_bucket(&mut buckets, source, format!("crash: {message}"));
            }
        }
        paths.cleanup();
    }

    eprintln!(
        "Phase 10b NW corpus {label} sample: ok={ok} source_compile_err={source_compile_err} decompile_ok={decompile_ok} decompile_err={decompile_err} core_recompile_err={core_recompile_err} idiomatic_recompile_err={idiomatic_recompile_err} recompile_ok={recompile_ok} crash={crash} total={}",
        files.len()
    );
    eprintln!(
        "Phase 10b structural report: exact_proto_rate={}/{} ({:.2}%) opcode_match_rate={}/{} ({:.2}%)",
        structural_exact_protos,
        structural_total_protos,
        percentage(structural_exact_protos, structural_total_protos),
        structural_matched_ops,
        structural_total_ops,
        percentage(structural_matched_ops, structural_total_ops)
    );
    if !buckets.is_empty() {
        eprintln!("Phase 10b failure buckets:");
        for bucket in buckets.values() {
            eprintln!(
                "- count={} example={} message={}",
                bucket.count,
                bucket.example.display(),
                bucket.message
            );
        }
    }

    assert_eq!(crash, 0, "corpus decompile crashed for {crash} files");
    assert_eq!(
        source_compile_err, 0,
        "corpus source compile failed for {source_compile_err} files"
    );
    assert_eq!(
        decompile_err, 0,
        "corpus core decompile failed for {decompile_err} files"
    );
    assert_eq!(
        decompile_ok,
        files.len(),
        "corpus core decompile OK count must be 100%"
    );
    assert_eq!(
        core_recompile_err, 0,
        "corpus core decompiled output failed to recompile for {core_recompile_err} files"
    );
    assert_eq!(
        idiomatic_recompile_err, 0,
        "corpus idiomatic decompiled output failed to recompile for {idiomatic_recompile_err} files"
    );
    assert_eq!(
        recompile_ok,
        files.len(),
        "corpus recompile-clean count must be 100%"
    );
}

fn long_table_source() -> String {
    let values = (1..=75)
        .map(|value| value.to_string())
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        r#"
local t = {{{values}}}
print(#t, t[1], t[50], t[75])
"#
    )
}

fn run_child_structural(worker: &Path, bytecode: &Path) -> Result<ChildReport, FileResult> {
    let output = Command::new(worker)
        .arg("--structural")
        .arg(LUAC)
        .arg(bytecode)
        .output()
        .map_err(|err| FileResult::Crash(err.to_string()))?;

    if !output.status.success() {
        return Err(FileResult::Crash(output_summary(&output)));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    if stdout.lines().any(|line| line.trim() == CHILD_OK) {
        return Ok(parse_child_report(&stdout));
    }
    if let Some(message) = stdout
        .lines()
        .find_map(|line| line.trim().strip_prefix(CHILD_ERR))
    {
        return Err(FileResult::Err(
            message.trim_start_matches('\t').trim().to_string(),
        ));
    }

    Err(FileResult::Crash(format!(
        "child produced no corpus marker: {}",
        one_line(&stdout)
    )))
}

fn parse_child_report(stdout: &str) -> ChildReport {
    stdout
        .lines()
        .find_map(|line| line.trim().strip_prefix(CHILD_STRUCTURAL_REPORT))
        .map(parse_child_report_line)
        .unwrap_or_default()
}

fn parse_child_report_line(line: &str) -> ChildReport {
    let mut report = ChildReport::default();
    for field in line.trim_start_matches('\t').split('\t') {
        let Some((name, value)) = field.split_once('=') else {
            continue;
        };
        let Ok(value) = value.parse::<usize>() else {
            continue;
        };
        match name {
            "exact_protos" => report.exact_protos = value,
            "total_protos" => report.total_protos = value,
            "matched_ops" => report.matched_ops = value,
            "total_ops" => report.total_ops = value,
            _ => {}
        }
    }
    report
}

fn corpus_files(limit: usize) -> Vec<PathBuf> {
    let mut files = collect_lua_files(Path::new(GOOD_LUA));
    files.extend(collect_lua_files(Path::new(DEMOJSON)));
    files.sort();
    files.truncate(limit);
    files
}

fn collect_lua_files(root: &Path) -> Vec<PathBuf> {
    if !root.exists() {
        return Vec::new();
    }

    let mut pending = vec![root.to_path_buf()];
    let mut files = Vec::new();
    while let Some(path) = pending.pop() {
        let Ok(entries) = fs::read_dir(path) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                pending.push(path);
            } else if path.extension().is_some_and(|ext| ext == "lua") {
                files.push(path);
            }
        }
    }
    files.sort();
    files
}

fn tools_available() -> bool {
    Path::new(LUA).exists() && Path::new(LUAC).exists()
}

fn compile_lua(source: &Path, bytecode: &Path) -> Result<(), String> {
    let output = Command::new(LUAC)
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

fn record_bucket(buckets: &mut BTreeMap<String, FailureBucket>, source: &Path, message: String) {
    let key = one_line(&message);
    let bucket = buckets.entry(key.clone()).or_insert_with(|| FailureBucket {
        count: 0,
        example: source.to_path_buf(),
        message: key,
    });
    bucket.count += 1;
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

fn percentage(numerator: usize, denominator: usize) -> f64 {
    if denominator == 0 {
        100.0
    } else {
        (numerator as f64 / denominator as f64) * 100.0
    }
}

#[derive(Debug)]
enum FileResult {
    Err(String),
    Crash(String),
}

#[derive(Debug, Default)]
struct ChildReport {
    exact_protos: usize,
    total_protos: usize,
    matched_ops: usize,
    total_ops: usize,
}

struct FailureBucket {
    count: usize,
    example: PathBuf,
    message: String,
}

struct TempPaths {
    bytecode: PathBuf,
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
        Self {
            bytecode: std::env::temp_dir().join(format!("{stem}.luac")),
        }
    }

    fn cleanup(&self) {
        let _ = fs::remove_file(&self.bytecode);
    }
}

impl Drop for TempPaths {
    fn drop(&mut self) {
        self.cleanup();
    }
}
