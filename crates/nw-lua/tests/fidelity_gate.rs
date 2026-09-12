mod support;

use std::{collections::BTreeMap, process::Command};

use support::{demojson_root, good_lua_root, luac_path};
const FIDELITY_GATE_SAMPLE_LIMIT: usize = 80;
const FIDELITY_HEAVY_SAMPLE_LIMIT: usize = 300;

#[test]
fn fidelity_gate_high_severity_regressions_stay_zero() {
    run_fidelity_gate(FIDELITY_GATE_SAMPLE_LIMIT);
}

#[test]
#[ignore = "runs the larger source-vs-decompile fidelity sweep"]
fn fidelity_gate_heavy_high_severity_regressions_stay_zero() {
    run_fidelity_gate(FIDELITY_HEAVY_SAMPLE_LIMIT);
}

fn run_fidelity_gate(limit: usize) {
    let Some(luac) = luac_path() else {
        eprintln!(
            "skipping fidelity gate; set {} to the reference luac.exe",
            support::LUAC_EXE_ENV
        );
        return;
    };
    let (Some(good_lua), Some(demojson)) = (good_lua_root(), demojson_root()) else {
        eprintln!(
            "skipping fidelity gate; corpus roots are missing (set {} and {})",
            support::GOOD_LUA_ROOT_ENV,
            support::DEMOJSON_ROOT_ENV
        );
        return;
    };

    let output = Command::new(env!("CARGO_BIN_EXE_nw-lua-fidelity"))
        .arg("--luac")
        .arg(luac)
        .arg("--root")
        .arg(good_lua)
        .arg("--root")
        .arg(demojson)
        .arg("--limit")
        .arg(limit.to_string())
        .arg("--examples")
        .arg("0")
        .output()
        .expect("run nw-lua-fidelity");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "nw-lua-fidelity failed with status {}\nstdout:\n{stdout}\nstderr:\n{stderr}",
        output.status
    );

    let processed = parse_counter(&stdout, "files_processed")
        .unwrap_or_else(|| panic!("fidelity output did not include files_processed:\n{stdout}"));
    assert_eq!(
        processed, limit,
        "fidelity gate must run the fixed bounded sample"
    );

    let categories = parse_categories(&stdout);
    assert_zero(&categories, "dropped_return");
    assert_zero(&categories, "empty_decompiled_branch");
    assert_zero(&categories, "bogus_not_number");
    assert_zero(&categories, "undefined_synthetic_read");
}

fn parse_counter(stdout: &str, name: &str) -> Option<usize> {
    stdout.lines().find_map(|line| {
        line.strip_prefix(name)?
            .trim_start_matches(':')
            .trim()
            .parse()
            .ok()
    })
}

fn parse_categories(stdout: &str) -> BTreeMap<String, CategoryHits> {
    let mut categories = BTreeMap::new();
    let mut in_table = false;
    for line in stdout.lines() {
        if line == "category,file_hits,file_pct,function_hits,function_pct" {
            in_table = true;
            continue;
        }
        if !in_table || line == "examples:" {
            if line == "examples:" {
                break;
            }
            continue;
        }

        let fields = line.split(',').collect::<Vec<_>>();
        if fields.len() != 5 {
            continue;
        }
        let Ok(file_hits) = fields[1].parse() else {
            continue;
        };
        let Ok(function_hits) = fields[3].parse() else {
            continue;
        };
        categories.insert(
            fields[0].to_owned(),
            CategoryHits {
                file_hits,
                function_hits,
            },
        );
    }
    categories
}

fn assert_zero(categories: &BTreeMap<String, CategoryHits>, label: &str) {
    let hits = categories
        .get(label)
        .unwrap_or_else(|| panic!("fidelity output did not include category {label}"));
    assert_eq!(
        (hits.file_hits, hits.function_hits),
        (0, 0),
        "{label} must stay at zero on the fidelity gate sample"
    );
}

#[derive(Debug)]
struct CategoryHits {
    file_hits: usize,
    function_hits: usize,
}
