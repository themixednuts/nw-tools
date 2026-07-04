mod support;

use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::{
        Mutex, MutexGuard,
        atomic::{AtomicUsize, Ordering},
    },
    time::{SystemTime, UNIX_EPOCH},
};

use support::run_equivalence;

static TEST_LOCK: Mutex<()> = Mutex::new(());
static NEXT_ID: AtomicUsize = AtomicUsize::new(0);

const LUA: &str = r"E:\Projects\lua-5.1.5\src\lua.exe";
const LUAC: &str = r"E:\Projects\lua-5.1.5\src\luac.exe";
const GOOD_LUA: &str = r"E:\Projects\az-rs\resources\fixtures\lua\good-lua";
const DEMOJSON: &str = r"E:\Projects\DEMOJSON";
const DEMOJSON_LIMIT: usize = 300;
const CHILD_OK: &str = "NW_LUA_CORPUS_CHILD_OK";
const CHILD_ERR: &str = "NW_LUA_CORPUS_CHILD_ERR";

#[test]
fn runtime_equivalence_table_constructor_lists() {
    let _guard = test_lock();
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
    let _guard = test_lock();
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
    ];

    for (name, source) in cases {
        let _ = run_equivalence(name, source);
    }
}

#[test]
fn nw_named_regressions_decompile_and_reparse() {
    let _guard = test_lock();
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
fn nw_corpus_decompiles_without_crashes_and_is_idempotent() {
    let _guard = test_lock();
    if !tools_available() {
        eprintln!("skipping NW corpus test; missing Lua 5.1 tools");
        return;
    }
    if !Path::new(GOOD_LUA).exists() || !Path::new(DEMOJSON).exists() {
        eprintln!("skipping NW corpus test; corpus roots are missing");
        return;
    }

    let files = corpus_files();
    assert!(!files.is_empty(), "NW corpus roots contained no Lua files");

    let mut ok = 0usize;
    let mut err = 0usize;
    let mut crash = 0usize;
    let mut idempotent = 0usize;
    let mut buckets = BTreeMap::<String, FailureBucket>::new();
    let worker = Path::new(env!("CARGO_BIN_EXE_nw-lua-corpus-child"));

    for source in &files {
        let paths = TempPaths::new("phase9c_corpus");
        let result = compile_lua(source, &paths.bytecode)
            .map_err(|message| FileResult::Err(format!("luac: {message}")))
            .and_then(|_| run_child_idempotent(worker, &paths.bytecode));

        match result {
            Ok(()) => {
                ok += 1;
                idempotent += 1;
            }
            Err(FileResult::Err(message)) => {
                err += 1;
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
        "Phase 10b NW corpus: ok={ok} err={err} crash={crash} idempotent={idempotent} total={}",
        files.len()
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
    assert_eq!(err, 0, "corpus decompile failed for {err} files");
    assert_eq!(ok, files.len(), "corpus OK count must be 100%");
    assert_eq!(
        idempotent,
        files.len(),
        "corpus idempotency count must be 100%"
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

fn run_child_idempotent(worker: &Path, bytecode: &Path) -> Result<(), FileResult> {
    let output = Command::new(worker)
        .arg("--idempotent")
        .arg(LUAC)
        .arg(bytecode)
        .output()
        .map_err(|err| FileResult::Crash(err.to_string()))?;

    if !output.status.success() {
        return Err(FileResult::Crash(output_summary(&output)));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    if stdout.lines().any(|line| line.trim() == CHILD_OK) {
        return Ok(());
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

fn corpus_files() -> Vec<PathBuf> {
    let mut files = collect_lua_files(Path::new(GOOD_LUA));
    let mut demojson = collect_lua_files(Path::new(DEMOJSON));
    demojson.truncate(DEMOJSON_LIMIT);
    files.extend(demojson);
    files.sort();
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

fn test_lock() -> MutexGuard<'static, ()> {
    TEST_LOCK.lock().expect("corpus test lock poisoned")
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

#[derive(Debug)]
enum FileResult {
    Err(String),
    Crash(String),
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
