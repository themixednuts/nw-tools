use std::collections::BTreeSet;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::thread;
use std::time::Duration;

use nw_jobs::{CancellationToken, JobRunner};
use nw_serialize_codegen::{
    CodegenContext, CompileUnit, CompletedCodegenUnits, GoSourceEmitter, RustCodegenPlanner,
    RustSourceEmitter, SerializeCodegenField, SerializeCodegenItem, SerializeCodegenItemKind,
    SerializeCodegenSelection, SerializeCodegenUnit, SerializeCodegenVariant,
    SerializeContextCompiler, TypeScriptSourceEmitter, TypeScriptStandaloneProjectOptions,
    complete_known_missing_reflected_bodies, missing_reflected_bodies_by_type,
};
use nw_serialize_codegen::{MapKind, PointerKind, ReflectedTypeRole, ResolvedType, ScalarType};
use nw_serialize_codegen::{RustCodegenUnit, SequenceKind};
use uuid::uuid;

fn codegen_context() -> CodegenContext {
    CodegenContext::inline()
}

#[test]
#[ignore = "runs external language toolchains against repo-local type output directories"]
fn full_serialize_output_directories_are_self_contained_and_lint_clean() {
    let compile_unit = project_compile_unit();
    assert!(
        compile_unit.codegen_unit.items.len() > 4_300,
        "full serialize.json should produce thousands of type items, got {}",
        compile_unit.codegen_unit.items.len()
    );
    thread::scope(|scope| {
        let rust = scope.spawn(|| validate_rust(&compile_unit));
        let go = scope.spawn(|| validate_go(&compile_unit));
        let typescript = scope.spawn(|| validate_typescript(&compile_unit));

        join_validation("Rust", rust);
        join_validation("Go", go);
        join_validation("TypeScript", typescript);
    });
}

#[test]
#[ignore = "rewrites repo-local type output directories without external lint/check toolchains"]
fn full_serialize_output_directories_are_regenerated() {
    let selection = output_selection();
    let compile_unit = project_compile_unit();
    let unit = compile_unit.selected_codegen_unit(selection);
    assert!(
        !unit.items.is_empty(),
        "serialize.json selection {selection:?} should produce at least one type item"
    );
    eprintln!(
        "regenerating serialize output directories with {selection:?}: {} type items",
        unit.items.len()
    );
    let regen_units = regeneration_codegen_units(&compile_unit, selection);
    thread::scope(|scope| {
        let rust = scope.spawn(|| write_selected_rust_output(&regen_units));
        let go = scope.spawn(|| write_selected_go_output(&regen_units));
        let typescript = scope.spawn(|| write_selected_typescript_output(&regen_units));

        join_regeneration("Rust", rust);
        join_regeneration("Go", go);
        join_regeneration("TypeScript", typescript);
    });
}

#[test]
#[ignore = "rewrites repo-local selected Rust type output without external lint/check toolchains"]
fn default_runtime_rust_output_directory_is_regenerated() {
    let compile_unit = project_compile_unit();
    let selection = SerializeCodegenSelection::RuntimeRoots;
    let unit = compile_unit.selected_codegen_unit(selection);
    assert!(
        !unit.items.is_empty(),
        "default selected serialize.json output should include runtime roots"
    );
    eprintln!(
        "regenerating default selected Rust output with {selection:?}: {} type items",
        unit.items.len()
    );
    let regen_units = regeneration_codegen_units(&compile_unit, selection);
    write_selected_rust_output_to_dir(&regen_units, "rust-runtime");
}

#[test]
fn sample_output_unit_keeps_scalar_support_coverage() {
    let unit = sample_unit();
    let context = codegen_context();

    let rust_unit: RustCodegenUnit = RustCodegenPlanner::standalone()
        .plan_serialize_codegen_unit(&unit, &crate::CodegenContext::inline());
    RustSourceEmitter::emit_standalone_project(&rust_unit, &context).expect("sample Rust project");
    GoSourceEmitter
        .emit_standalone_project(&unit, "aztypesvalidation", "aztypesvalidation", &context)
        .expect("sample Go project");
    TypeScriptSourceEmitter
        .emit_standalone_project(&unit, &context)
        .expect("sample TypeScript project");
}

fn validate_rust(compile_unit: &CompileUnit) {
    let project = write_rust_output(compile_unit);

    run_rust(&project, ["fmt", "--all"]);
    run_rust(&project, ["fmt", "--all", "--check"]);
    run_rust(
        &project,
        [
            "clippy",
            "--lib",
            "--tests",
            "--",
            "-D",
            "warnings",
            "-W",
            "clippy::pedantic",
            "-W",
            "clippy::nursery",
        ],
    );
    run_rust(&project, ["test"]);
}

fn write_rust_output(compile_unit: &CompileUnit) -> PathBuf {
    write_rust_project(
        compile_unit
            .emit_standalone_rust_project(&codegen_context())
            .expect("standalone Rust project"),
    )
}

fn write_selected_rust_output(regen_units: &CompletedCodegenUnits) -> PathBuf {
    write_selected_rust_output_to_dir(regen_units, "rust")
}

fn write_selected_rust_output_to_dir(
    regen_units: &CompletedCodegenUnits,
    output_dir_name: &str,
) -> PathBuf {
    let rust_unit = RustCodegenPlanner::standalone().plan_serialize_codegen_units(
        &regen_units.emitted,
        &regen_units.context,
        &crate::CodegenContext::inline(),
    );
    write_rust_project_to_dir(
        RustSourceEmitter::emit_standalone_project(&rust_unit, &codegen_context())
            .expect("selected standalone Rust project"),
        output_dir_name,
    )
}

fn write_rust_project(rust_project: nw_serialize_codegen::RustStandaloneProject) -> PathBuf {
    write_rust_project_to_dir(rust_project, "rust")
}

fn write_rust_project_to_dir(
    mut rust_project: nw_serialize_codegen::RustStandaloneProject,
    output_dir_name: &str,
) -> PathBuf {
    let project = reset_generated_rust_dir(output_dir_name);
    let generated = rust_project
        .files
        .iter_mut()
        .find(|file| file.path == "src/types/mod.rs")
        .expect("Rust types module");
    generated.source.push_str(r#"

#[cfg(test)]
mod type_semantics_tests {
	use crate::az::{
		asset::AssetId,
		crc::Crc32,
		uuid::{type_ids, Uuid},
	};

	#[test]
	fn validates_az_uuid_crc_and_support_types() {
		assert_eq!(Uuid::create_name(b"hello"), Uuid::parse_str("aaf4c61d-dcc5-58a2-9abe-de0f3b482cd9").expect("hello uuid"));
		assert_eq!(Uuid::combine(type_ids::INT, type_ids::U8), Uuid::parse_str("2554130f-2bb8-5a25-8cc4-319329151f28").expect("combined uuid"));
		assert_eq!(Uuid::template_auto_type_id(32), Uuid::parse_str("cb4e5208-b4cd-5726-8b20-8e49452ed6e8").expect("template uuid"));
		assert_eq!(Uuid::aggregate_type_ids_right(&[type_ids::INT, type_ids::U8]).expect("right aggregate"), Uuid::combine(type_ids::INT, type_ids::U8));
		assert_eq!(Uuid::specialized_template_prefix(type_ids::ASSET_ID, &[type_ids::AZ_UUID]).expect("prefix fold"), Uuid::combine(type_ids::ASSET_ID, type_ids::AZ_UUID));
		assert_eq!(Uuid::specialized_template_postfix(type_ids::ASSET_ID, &[type_ids::AZ_UUID]).expect("postfix fold"), Uuid::combine(type_ids::AZ_UUID, type_ids::ASSET_ID));
		assert_eq!(Crc32::from_str_lower("EditorData").value(), 0xf44f_1a1d);
		assert_eq!(Crc32::from_bytes(b"Editor").value(), 0xcb5d_f48c);
		let asset_id = AssetId::new(Uuid::from_u128(0xC9000220_B6D4_506D_B1A0_08BF4D6845DC), 0x20000);
		assert_eq!(asset_id.to_string(), "{C9000220-B6D4-506D-B1A0-08BF4D6845DC}:20000");
		assert_eq!(asset_id.to_string().parse::<AssetId>().expect("asset id"), asset_id);
	}
}
"#);
    let mut files = rust_project
        .files
        .into_iter()
        .map(|file| GeneratedOutputFile {
            path: file.path,
            source: file.source,
        })
        .collect::<Vec<_>>();
    files.push(GeneratedOutputFile {
        path: "Cargo.toml".to_owned(),
        source: r#"[package]
name = "aztypes-rust-validation"
version = "0.1.0"
edition = "2024"
rust-version = "1.96"

[dependencies]
bevy_ecs = { version = "0.18", features = ["serialize"] }
bevy_color = { version = "0.18", features = ["serialize"] }
bevy_math = { version = "0.18", features = ["serialize"] }
bevy_reflect = { version = "0.18", features = ["smallvec", "uuid"] }
bevy_transform = { version = "0.18", features = ["serialize"] }
sha1 = "0.11"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
smallvec = { version = "1", features = ["serde"] }
uuid = { version = "1.23", features = ["serde", "v4", "v7"] }

[workspace]
"#
        .to_owned(),
    });
    write_generated_files(&project, "rust", files);
    project
}

fn validate_go(compile_unit: &CompileUnit) {
    let project = write_go_output(compile_unit);

    run(&project, "go", ["mod", "tidy"]);
    run(&project, "gofmt", ["-w", "."]);
    let output = run_output(&project, "gofmt", ["-l", "."]);
    assert!(
        String::from_utf8_lossy(&output.stdout).trim().is_empty(),
        "gofmt reported unformatted files:\n{}",
        String::from_utf8_lossy(&output.stdout)
    );
    run(&project, "go", ["test", "-vet=off", "."]);
    run(&project, "go", ["vet", "./..."]);
}

fn write_go_output(compile_unit: &CompileUnit) -> PathBuf {
    write_go_project(
        compile_unit
            .emit_standalone_go_project(
                "aztypesvalidation",
                "aztypesvalidation",
                &codegen_context(),
            )
            .expect("Go standalone project"),
    )
}

fn write_selected_go_output(regen_units: &CompletedCodegenUnits) -> PathBuf {
    write_go_project(
        GoSourceEmitter
            .emit_standalone_project_with_context(
                &regen_units.emitted,
                &regen_units.context,
                "aztypesvalidation",
                "aztypesvalidation",
                &codegen_context(),
            )
            .expect("selected Go standalone project"),
    )
}

fn write_go_project(go_project: nw_serialize_codegen::GoStandaloneProject) -> PathBuf {
    let project = reset_generated_dir("go");
    let mut files = go_project
        .files
        .into_iter()
        .map(|file| GeneratedOutputFile {
            path: file.path,
            source: file.source,
        })
        .collect::<Vec<_>>();
    files.push(GeneratedOutputFile {
        path: "go.mod".to_owned(),
        source: "module aztypesvalidation\n\ngo 1.26\n\nrequire github.com/google/uuid v1.6.0\n"
            .to_owned(),
    });
    files.push(GeneratedOutputFile {
        path: "types_test.go".to_owned(),
        source: go_semantics_test().to_owned(),
    });
    files.push(GeneratedOutputFile {
        path: "compile_all_test.go".to_owned(),
        source: go_compile_all_test(
            files.iter().map(|file| file.path.as_str()),
            "aztypesvalidation",
        ),
    });
    write_generated_files(&project, "go", files);
    project
}

fn go_compile_all_test<'a>(
    relative_paths: impl IntoIterator<Item = &'a str>,
    module_path: &str,
) -> String {
    let mut packages = BTreeSet::new();
    for relative_path in relative_paths {
        let path = Path::new(relative_path);
        if path.extension().and_then(OsStr::to_str) != Some("go") {
            continue;
        }
        let Some(parent) = path.parent() else {
            continue;
        };
        if parent.as_os_str().is_empty() {
            continue;
        }
        let relative_import = parent
            .components()
            .map(|component| component.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join("/");
        packages.insert(format!("{module_path}/{relative_import}"));
    }

    let mut source = String::from("package aztypesvalidation\n\nimport (\n");
    for package in packages {
        source.push_str("\t_ \"");
        source.push_str(&package);
        source.push_str("\"\n");
    }
    source.push_str(")\n");
    source
}

fn validate_typescript(compile_unit: &CompileUnit) {
    let project = write_typescript_output(compile_unit);

    run(&project, "bun", ["install"]);
    let vp = project
        .join("node_modules")
        .join(".bin")
        .join(exe_name("vp"));
    run(&project, &vp, ["fmt", ".", "--write"]);
    run(&project, &vp, ["fmt", ".", "--check"]);
    run(&project, &vp, ["check"]);
    run(&project, &vp, ["pack"]);
    run(&project, "node", ["dist/semantic-check.mjs"]);
}

fn join_validation<T>(name: &str, handle: thread::ScopedJoinHandle<'_, T>) {
    if let Err(payload) = handle.join() {
        eprintln!("{name} validation failed");
        std::panic::resume_unwind(payload);
    }
}

fn join_regeneration<T>(name: &str, handle: thread::ScopedJoinHandle<'_, T>) {
    if let Err(payload) = handle.join() {
        eprintln!("{name} regeneration failed");
        std::panic::resume_unwind(payload);
    }
    eprintln!("{name} output regeneration completed");
}

fn write_typescript_output(compile_unit: &CompileUnit) -> PathBuf {
    write_typescript_project(
        compile_unit
            .emit_standalone_typescript_project_with_options(
                &typescript_project_options(),
                &codegen_context(),
            )
            .expect("TypeScript standalone project"),
    )
}

fn write_selected_typescript_output(regen_units: &CompletedCodegenUnits) -> PathBuf {
    write_typescript_project(
        TypeScriptSourceEmitter
            .emit_standalone_project_with_options_and_context(
                &regen_units.emitted,
                &regen_units.context,
                &typescript_project_options(),
                &codegen_context(),
            )
            .expect("selected TypeScript standalone project"),
    )
}

fn write_typescript_project(
    typescript_project: nw_serialize_codegen::TypeScriptStandaloneProject,
) -> PathBuf {
    let project = reset_generated_dir("ts");
    let mut files = typescript_project
        .files
        .into_iter()
        .map(|file| GeneratedOutputFile {
            path: file.path,
            source: file.source,
        })
        .collect::<Vec<_>>();
    files.push(GeneratedOutputFile {
        path: "src/semantic-check.ts".to_owned(),
        source: typescript_semantics_test().to_owned(),
    });
    write_generated_files(&project, "ts", files);
    project
}

fn typescript_project_options() -> TypeScriptStandaloneProjectOptions {
    TypeScriptStandaloneProjectOptions {
        package_name: "aztypes-typescript-validation".to_owned(),
        pack_entries: vec![
            "src/index.ts".to_owned(),
            "src/semantic-check.ts".to_owned(),
        ],
    }
}

fn project_compile_unit() -> CompileUnit {
    let resources = repo_root().join("resources");
    let compile_unit = SerializeContextCompiler::compile_from_paths_with_class_registration_trace(
        resources.join("serialize.json"),
        Some(resources.join("modules")),
        None::<&Path>,
        None::<&Path>,
        &codegen_context(),
    )
    .expect("compile project serialize context");
    assert!(
        !compile_unit.has_errors(),
        "serialize.json diagnostics contain errors: {:#?}",
        compile_unit.diagnostics
    );
    for (type_id, (owner_name, field_name, reason, count)) in
        missing_reflected_bodies_by_type(&compile_unit.codegen_unit)
            .into_iter()
            .map(|(type_id, missing)| {
                (
                    type_id,
                    (
                        missing.owner_name,
                        missing.field_name,
                        missing.reason,
                        missing.reference_count,
                    ),
                )
            })
    {
        eprintln!(
            "warning: missing reflected type `{type_id}` for `{owner_name}.{field_name}`: {reason} ({count} references)"
        );
    }
    compile_unit
}

fn regeneration_codegen_units(
    compile_unit: &CompileUnit,
    selection: SerializeCodegenSelection,
) -> CompletedCodegenUnits {
    let emitted = compile_unit.selected_codegen_unit(selection);
    let completed =
        complete_known_missing_reflected_bodies(emitted, compile_unit.codegen_unit.clone());
    for placeholder in &completed.placeholders {
        eprintln!(
            "warning: generating placeholder type `{}` ({}) for missing reflected body used by `{}.{}`: {} ({} reference(s))",
            placeholder.source_name,
            placeholder.type_id,
            placeholder.owner_name,
            placeholder.field_name,
            placeholder.reason,
            placeholder.reference_count
        );
    }
    completed
}

fn output_selection() -> SerializeCodegenSelection {
    match std::env::var("NW_SERIALIZE_CODEGEN_SELECTION")
        .unwrap_or_else(|_| "runtime-roots".to_owned())
        .as_str()
    {
        "all" => SerializeCodegenSelection::All,
        "components" | "component-roots" | "component_roots" => {
            SerializeCodegenSelection::Components
        }
        "runtime-roots" | "runtime_roots" | "runtime" => SerializeCodegenSelection::RuntimeRoots,
        value => panic!(
            "unsupported NW_SERIALIZE_CODEGEN_SELECTION `{value}`; expected `all`, `components`, or `runtime-roots`"
        ),
    }
}

fn sample_unit() -> SerializeCodegenUnit {
    SerializeCodegenUnit {
        items: vec![
            SerializeCodegenItem {
                source_type_id: uuid!("11111111-1111-1111-1111-111111111111"),
                source_name: "Example::AddressType".to_owned(),
                role: ReflectedTypeRole::SupportType,
                is_reflection_marker: false,
                is_abstract: Some(false),
                factory: None,
                rtti_base_chain: Vec::new(),
                kind: SerializeCodegenItemKind::Struct,
                enum_underlying_type: None,
                fields: vec![field(
                    "m_label",
                    uuid!("43DA906B-7DEF-4CA8-9790-854106D3F983"),
                    ResolvedType::Scalar(ScalarType::String),
                )],
                variants: Vec::new(),
            },
            SerializeCodegenItem {
                source_type_id: uuid!("22222222-2222-2222-2222-222222222222"),
                source_name: "Example::CounterComponent".to_owned(),
                role: ReflectedTypeRole::AzComponent,
                is_reflection_marker: false,
                is_abstract: Some(false),
                factory: None,
                rtti_base_chain: Vec::new(),
                kind: SerializeCodegenItemKind::Struct,
                enum_underlying_type: None,
                fields: vec![
                    field(
                        "m_targetEntity",
                        uuid!("6383F1D3-BB27-4E6B-A49A-6409B2059EAA"),
                        ResolvedType::Scalar(ScalarType::EntityId),
                    ),
                    field(
                        "m_asset",
                        uuid!("652ED536-3402-439B-AEBE-4A5DBC554085"),
                        ResolvedType::Scalar(ScalarType::AssetId),
                    ),
                    field(
                        "m_tag",
                        uuid!("9F4E062E-06A0-46D4-85DF-E0DA96467D3A"),
                        ResolvedType::Scalar(ScalarType::Crc32),
                    ),
                    field(
                        "m_owner",
                        uuid!("E152C105-A133-4D03-BBF8-3D4B2FBA3E2A"),
                        ResolvedType::Scalar(ScalarType::Uuid),
                    ),
                    field(
                        "m_payload",
                        uuid!("ADFD596B-7177-5519-9752-BC418FE42963"),
                        ResolvedType::ByteStream,
                    ),
                    field(
                        "m_addresses",
                        uuid!("33333333-3333-3333-3333-333333333333"),
                        ResolvedType::Sequence {
                            kind: SequenceKind::Vector,
                            element: Box::new(address_type()),
                            capacity: None,
                        },
                    ),
                    field(
                        "m_routes",
                        uuid!("44444444-4444-4444-4444-444444444444"),
                        ResolvedType::Map {
                            kind: MapKind::UnorderedMap,
                            key: Box::new(ResolvedType::Scalar(ScalarType::EntityId)),
                            value: Box::new(address_type()),
                        },
                    ),
                    field(
                        "m_optionalByte",
                        uuid!("55555555-5555-5555-5555-555555555555"),
                        ResolvedType::Pointer {
                            kind: PointerKind::Shared,
                            target: Box::new(ResolvedType::Scalar(ScalarType::U8)),
                        },
                    ),
                ],
                variants: Vec::new(),
            },
            SerializeCodegenItem {
                source_type_id: uuid!("66666666-6666-6666-6666-666666666666"),
                source_name: "Example::Mode".to_owned(),
                role: ReflectedTypeRole::SupportType,
                is_reflection_marker: false,
                is_abstract: Some(false),
                factory: None,
                rtti_base_chain: Vec::new(),
                kind: SerializeCodegenItemKind::Enum,
                enum_underlying_type: Some(ResolvedType::Scalar(ScalarType::U8)),
                fields: Vec::new(),
                variants: vec![SerializeCodegenVariant {
                    source_name: "Enabled".to_owned(),
                    value_u64: Some(7),
                    value_u32: Some(7),
                    value_i32: Some(7),
                }],
            },
        ],
    }
}

fn field(
    source_name: &str,
    source_type_id: uuid::Uuid,
    resolved_type: ResolvedType,
) -> SerializeCodegenField {
    SerializeCodegenField {
        source_name: source_name.to_owned(),
        source_type_id,
        resolved_type,
        data_size: None,
        offset: None,
        flags: None,
        is_base_class: false,
        is_pointer: false,
        is_dynamic_field: false,
    }
}

fn address_type() -> ResolvedType {
    ResolvedType::Named {
        type_id: uuid!("11111111-1111-1111-1111-111111111111"),
        source_name: "Example::AddressType".to_owned(),
    }
}

fn go_semantics_test() -> &'static str {
    r#"
package aztypesvalidation

import (
	"testing"

	"aztypesvalidation/az/asset"
	"aztypesvalidation/az/crc"
	"aztypesvalidation/az/uuid"
)

func TestAzSemantics(t *testing.T) {
	intType := uuid.MustParse("72039442-eb38-4d42-a1ad-cb68f7e0eef6")
	u8Type := uuid.MustParse("72b9409a-7d1a-4831-9cfe-fcb3fadd3426")
	assetIDType := uuid.MustParse("652ed536-3402-439b-aebe-4a5dbc554085")
	azUuidType := uuid.MustParse("e152c105-a133-4d03-bbf8-3d4b2fba3e2a")

	if got := uuid.CreateName("hello").String(); got != "aaf4c61d-dcc5-58a2-9abe-de0f3b482cd9" {
		t.Fatalf("CreateName mismatch: %s", got)
	}
	if got := uuid.Combine(intType, u8Type).String(); got != "2554130f-2bb8-5a25-8cc4-319329151f28" {
		t.Fatalf("Combine mismatch: %s", got)
	}
	if got := uuid.TemplateAutoTypeID(32).String(); got != "cb4e5208-b4cd-5726-8b20-8e49452ed6e8" {
		t.Fatalf("TemplateAutoTypeID mismatch: %s", got)
	}
	if got, ok := uuid.AggregateTypeIDs([]uuid.Uuid{intType, u8Type}); !ok || got.String() != "2554130f-2bb8-5a25-8cc4-319329151f28" {
		t.Fatalf("AggregateTypeIDs mismatch: %v %s", ok, got.String())
	}
	if got, ok := uuid.AggregateTypeIDsRight([]uuid.Uuid{intType, u8Type}); !ok || got != uuid.Combine(intType, u8Type) {
		t.Fatalf("AggregateTypeIDsRight mismatch: %v %s", ok, got.String())
	}
	if got, ok := uuid.SpecializedTemplatePrefix(assetIDType, []uuid.Uuid{azUuidType}); !ok || got != uuid.Combine(assetIDType, azUuidType) {
		t.Fatalf("SpecializedTemplatePrefix mismatch: %v %s", ok, got.String())
	}
	if got, ok := uuid.SpecializedTemplatePostfix(assetIDType, []uuid.Uuid{azUuidType}); !ok || got != uuid.Combine(azUuidType, assetIDType) {
		t.Fatalf("SpecializedTemplatePostfix mismatch: %v %s", ok, got.String())
	}
	if got := crc.FromStringLower("EditorData").Value(); got != 0xf44f1a1d {
		t.Fatalf("Crc32FromStringLower mismatch: %#x", got)
	}
	if got := crc.FromBytesLower([]byte("EditorData")).Value(); got != 0xf44f1a1d {
		t.Fatalf("Crc32FromBytesLower mismatch: %#x", got)
	}
	if got := crc.FromBytes([]byte("Editor"), false).Value(); got != 0xcb5df48c {
		t.Fatalf("Crc32FromBytes mismatch: %#x", got)
	}
	assetID := asset.New(uuid.MustParse("c9000220-b6d4-506d-b1a0-08bf4d6845dc"), 0x20000)
	if got := assetID.String(); got != "{C9000220-B6D4-506D-B1A0-08BF4D6845DC}:20000" {
		t.Fatalf("AssetId string mismatch: %s", got)
	}
	parsed, err := asset.Parse(assetID.String())
	if err != nil {
		t.Fatalf("ParseAssetId failed: %v", err)
	}
	if parsed != assetID {
		t.Fatalf("AssetId round trip mismatch: %#v != %#v", parsed, assetID)
	}
}
"#
}

fn typescript_semantics_test() -> &'static str {
    r#"
import { AssetId, Crc32, Uuid, typeIds } from "./index.js";

function assertEqual<T>(actual: T, expected: T, label: string): void {
	if (actual !== expected) {
		throw new Error(`${label}: ${String(actual)} !== ${String(expected)}`);
	}
}

const hello = await Uuid.createName("hello");
assertEqual(hello.toString(), "aaf4c61d-dcc5-58a2-9abe-de0f3b482cd9", "CreateName");

const combined = await Uuid.combine(typeIds.int, typeIds.u8);
assertEqual(combined.toString(), "2554130f-2bb8-5a25-8cc4-319329151f28", "Combine");
const aggregate = await Uuid.aggregateTypeIds([typeIds.int, typeIds.u8]);
assertEqual(aggregate?.toString(), "2554130f-2bb8-5a25-8cc4-319329151f28", "AggregateTypeIds");
const aggregateRight = await Uuid.aggregateTypeIdsRight([typeIds.int, typeIds.u8]);
assertEqual(aggregateRight?.toString(), "2554130f-2bb8-5a25-8cc4-319329151f28", "AggregateTypeIdsRight");
const assetIdType = Uuid.parse("652ed536-3402-439b-aebe-4a5dbc554085");
const azUuidType = Uuid.parse("e152c105-a133-4d03-bbf8-3d4b2fba3e2a");
const prefix = await Uuid.specializedTemplatePrefix(assetIdType, [azUuidType]);
const expectedPrefix = await Uuid.combine(assetIdType, azUuidType);
assertEqual(prefix?.toString(), expectedPrefix.toString(), "SpecializedTemplatePrefix");
const postfix = await Uuid.specializedTemplatePostfix(assetIdType, [azUuidType]);
const expectedPostfix = await Uuid.combine(azUuidType, assetIdType);
assertEqual(postfix?.toString(), expectedPostfix.toString(), "SpecializedTemplatePostfix");
const templateAutoTypeId = await Uuid.templateAutoTypeId(32);
assertEqual(templateAutoTypeId.toString(), "cb4e5208-b4cd-5726-8b20-8e49452ed6e8", "TemplateAutoTypeId");
assertEqual(Crc32.fromStringLower("EditorData").value(), 0xf44f1a1d, "Crc32FromStringLower");
assertEqual(Crc32.fromBytesLower(new TextEncoder().encode("EditorData")).value(), 0xf44f1a1d, "Crc32FromBytesLower");
assertEqual(
	Crc32.fromBytes(new TextEncoder().encode("Editor"), false).value(),
	0xcb5df48c,
	"Crc32FromBytes",
);

const assetId = new AssetId(Uuid.parse("c9000220-b6d4-506d-b1a0-08bf4d6845dc"), 0x20000);
assertEqual(assetId.toString(), "{C9000220-B6D4-506D-B1A0-08BF4D6845DC}:20000", "AssetId.toString");
assertEqual(AssetId.parse(assetId.toString()).guid.toString(), assetId.guid.toString(), "AssetId.guid");
assertEqual(Uuid.NIL.toString(), "00000000-0000-0000-0000-000000000000", "NilUuid");
"#
}

fn run<I, S>(cwd: &Path, program: impl AsRef<Path>, args: I)
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let args = collect_args(args);
    let command = format_command(program.as_ref(), &args);
    let output = run_output_with_args_and_env(cwd, program.as_ref(), &args, &[]);
    assert_command_success(cwd, &command, &output);
}

fn run_rust<I, S>(cwd: &Path, args: I)
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let args = collect_args(args);
    let program = cargo();
    let command = format_command(program.as_ref(), &args);
    let output = run_output_with_args_and_env(
        cwd,
        program.as_ref(),
        &args,
        &[("RUSTFLAGS", "-Cdebuginfo=0")],
    );
    assert_command_success(cwd, &command, &output);
}

fn assert_command_success(cwd: &Path, command: &str, output: &Output) {
    assert!(
        output.status.success(),
        "command failed in {}: `{command}` status={}\nstdout:\n{}\nstderr:\n{}",
        cwd.display(),
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct GeneratedOutputFile {
    path: String,
    source: String,
}

fn write_generated_files(project: &Path, language: &str, mut files: Vec<GeneratedOutputFile>) {
    files.sort_by(|left, right| left.path.cmp(&right.path));
    assert_unique_generated_paths(&files);
    let desired_paths = files
        .iter()
        .map(|file| file.path.clone())
        .collect::<BTreeSet<_>>();
    let existing_generated_files = collect_existing_generated_files(project, language);
    let stale_files = existing_generated_files
        .into_iter()
        .filter(|path| !desired_paths.contains(&relative_path_text(path)))
        .collect::<Vec<_>>();

    let runner = JobRunner::automatic();
    let cancel = CancellationToken::new();
    remove_stale_generated_files(project, &runner, &cancel, &stale_files);
    create_generated_parent_dirs(project, &runner, &cancel, &files);

    let writes = runner.map_until_cancelled(&files, &cancel, |file| {
        write_generated_file_if_changed(project, file)
    });
    assert!(
        !writes.was_cancelled(),
        "generated output writes were unexpectedly cancelled"
    );
    let mut stats = GeneratedWriteStats::default();
    for result in writes.into_completed() {
        match result {
            Ok(result) => stats.merge(result),
            Err(error) => panic!("write generated output: {error}"),
        }
    }
    prune_empty_generated_dirs(project, language);
    eprintln!(
        "wrote {}: {} changed, {} unchanged, {} stale removed",
        project.display(),
        stats.changed,
        stats.unchanged,
        stale_files.len()
    );
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct GeneratedWriteStats {
    changed: usize,
    unchanged: usize,
}

impl GeneratedWriteStats {
    fn merge(&mut self, other: Self) {
        self.changed += other.changed;
        self.unchanged += other.unchanged;
    }
}

fn assert_unique_generated_paths(files: &[GeneratedOutputFile]) {
    let mut seen = BTreeSet::new();
    for file in files {
        assert!(
            seen.insert(file.path.as_str()),
            "duplicate generated output path: {}",
            file.path
        );
    }
}

fn create_generated_parent_dirs(
    project: &Path,
    runner: &JobRunner,
    cancel: &CancellationToken,
    files: &[GeneratedOutputFile],
) {
    let dirs = files
        .iter()
        .filter_map(|file| Path::new(&file.path).parent())
        .filter(|path| !path.as_os_str().is_empty())
        .map(Path::to_path_buf)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let created = runner.map_until_cancelled(&dirs, cancel, |dir| {
        let path = project.join(dir);
        fs::create_dir_all(&path).map_err(|error| {
            format!(
                "create type output parent directory {}: {error}",
                path.display()
            )
        })
    });
    assert!(
        !created.was_cancelled(),
        "generated output directory creation was unexpectedly cancelled"
    );
    for result in created.into_completed() {
        if let Err(error) = result {
            panic!("{error}");
        }
    }
}

fn write_generated_file_if_changed(
    project: &Path,
    file: &GeneratedOutputFile,
) -> Result<GeneratedWriteStats, String> {
    let path = project.join(&file.path);
    let source_bytes = file.source.as_bytes();
    let source_hash = blake3::hash(source_bytes);
    if existing_file_matches_hash(&path, source_bytes.len() as u64, source_hash)? {
        return Ok(GeneratedWriteStats {
            changed: 0,
            unchanged: 1,
        });
    }

    fs::write(&path, source_bytes)
        .map_err(|error| format!("write type output file {}: {error}", path.display()))?;
    Ok(GeneratedWriteStats {
        changed: 1,
        unchanged: 0,
    })
}

fn existing_file_matches_hash(
    path: &Path,
    expected_len: u64,
    expected_hash: blake3::Hash,
) -> Result<bool, String> {
    match fs::metadata(path) {
        Ok(metadata) if metadata.len() != expected_len => return Ok(false),
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(error) => {
            return Err(format!(
                "inspect existing type output file {}: {error}",
                path.display()
            ));
        }
    }

    let mut file = fs::File::open(path)
        .map_err(|error| format!("open existing type output file {}: {error}", path.display()))?;
    let mut hasher = blake3::Hasher::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let bytes_read = file.read(&mut buffer).map_err(|error| {
            format!("read existing type output file {}: {error}", path.display())
        })?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
    }
    Ok(hasher.finalize() == expected_hash)
}

fn collect_existing_generated_files(project: &Path, language: &str) -> Vec<PathBuf> {
    generated_roots(language)
        .into_iter()
        .flat_map(|relative| {
            let path = project.join(relative);
            collect_existing_files_under(project, &path)
        })
        .collect()
}

fn generated_roots(language: &str) -> Vec<&'static str> {
    match language {
        "rust" => vec!["Cargo.toml", "Cargo.lock", "src"],
        "go" => vec![
            "go.mod",
            "go.sum",
            "types.go",
            "types_test.go",
            "compile_all_test.go",
            "az",
            "types",
        ],
        "ts" => vec![
            "bun.lock",
            "package.json",
            "tsconfig.json",
            "vite.config.ts",
            "src",
            "dist",
        ],
        _ => unreachable!("language was validated before syncing generated output"),
    }
}

fn collect_existing_files_under(project: &Path, path: &Path) -> Vec<PathBuf> {
    if !path.exists() {
        return Vec::new();
    }
    if path.is_file() {
        return vec![
            path.strip_prefix(project)
                .unwrap_or_else(|error| {
                    panic!(
                        "strip generated output root from {}: {error}",
                        path.display()
                    )
                })
                .to_path_buf(),
        ];
    }

    let mut files = Vec::new();
    let entries = fs::read_dir(path).unwrap_or_else(|error| {
        panic!(
            "read generated output directory {}: {error}",
            path.display()
        )
    });
    for entry in entries {
        let entry = entry.unwrap_or_else(|error| {
            panic!("read generated output entry in {}: {error}", path.display())
        });
        files.extend(collect_existing_files_under(project, &entry.path()));
    }
    files
}

fn remove_stale_generated_files(
    project: &Path,
    runner: &JobRunner,
    cancel: &CancellationToken,
    stale_files: &[PathBuf],
) {
    let removed = runner.map_until_cancelled(stale_files, cancel, |relative_path| {
        remove_file_with_retry(&project.join(relative_path))
    });
    assert!(
        !removed.was_cancelled(),
        "generated output stale-file removal was unexpectedly cancelled"
    );
    for result in removed.into_completed() {
        if let Err(error) = result {
            panic!("{error}");
        }
    }
}

fn remove_file_with_retry(path: &Path) -> Result<(), String> {
    let mut last_error = None;
    for _ in 0..120 {
        match fs::remove_file(path) {
            Ok(()) => return Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(error) => {
                last_error = Some(error);
                thread::sleep(Duration::from_millis(500));
            }
        }
    }
    let error = last_error.expect("remove_file should have either succeeded or returned error");
    Err(format!(
        "remove type output file {}: {error}",
        path.display()
    ))
}

fn prune_empty_generated_dirs(project: &Path, language: &str) {
    for relative in generated_roots(language) {
        let path = project.join(relative);
        if path.is_dir() {
            prune_empty_dirs(&path);
        }
    }
}

fn prune_empty_dirs(path: &Path) -> bool {
    let Ok(entries) = fs::read_dir(path) else {
        return false;
    };
    let mut is_empty = true;
    for entry in entries {
        let entry = entry.unwrap_or_else(|error| {
            panic!("read generated output entry in {}: {error}", path.display())
        });
        let child = entry.path();
        if child.is_dir() {
            if !prune_empty_dirs(&child) {
                is_empty = false;
            }
        } else {
            is_empty = false;
        }
    }
    if is_empty {
        let _ = fs::remove_dir(path);
    }
    is_empty
}

fn relative_path_text(path: &Path) -> String {
    path.components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

fn run_output<I, S>(cwd: &Path, program: impl AsRef<Path>, args: I) -> Output
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let args = collect_args(args);
    run_output_with_args(cwd, program.as_ref(), &args)
}

fn run_output_with_args(cwd: &Path, program: &Path, args: &[OsString]) -> Output {
    run_output_with_args_and_env(cwd, program, args, &[])
}

fn run_output_with_args_and_env(
    cwd: &Path,
    program: &Path,
    args: &[OsString],
    envs: &[(&str, &str)],
) -> Output {
    let command = format_command(program, args);
    let timeout = command_timeout();
    let mut child = Command::new(program)
        .args(args)
        .envs(envs.iter().copied())
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|error| panic!("spawn command in {}: `{command}`: {error}", cwd.display()));
    let start = std::time::Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_status)) => {
                return child.wait_with_output().unwrap_or_else(|error| {
                    panic!(
                        "collect command output in {}: `{command}`: {error}",
                        cwd.display()
                    )
                });
            }
            Ok(None) if start.elapsed() >= timeout => {
                let kill_result = child.kill();
                let output = child.wait_with_output().unwrap_or_else(|error| {
                    panic!(
                        "collect timed-out command output in {}: `{command}`: {error}",
                        cwd.display()
                    )
                });
                let kill_note = kill_result
                    .err()
                    .map(|error| format!("; kill failed: {error}"))
                    .unwrap_or_default();
                panic!(
                    "command timed out after {}s in {}: `{command}`{kill_note}\nstdout:\n{}\nstderr:\n{}",
                    timeout.as_secs(),
                    cwd.display(),
                    String::from_utf8_lossy(&output.stdout),
                    String::from_utf8_lossy(&output.stderr)
                );
            }
            Ok(None) => thread::sleep(Duration::from_millis(100)),
            Err(error) => panic!("poll command in {}: `{command}`: {error}", cwd.display()),
        }
    }
}

fn collect_args<I, S>(args: I) -> Vec<OsString>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    args.into_iter()
        .map(|arg| arg.as_ref().to_os_string())
        .collect()
}

fn command_timeout() -> Duration {
    std::env::var("NW_SERIALIZE_CODEGEN_COMMAND_TIMEOUT_SECS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|seconds| *seconds > 0)
        .map(Duration::from_secs)
        .unwrap_or_else(|| Duration::from_secs(20 * 60))
}

fn format_command(program: &Path, args: &[OsString]) -> String {
    std::iter::once(program.as_os_str())
        .chain(args.iter().map(OsString::as_os_str))
        .map(|part| part.to_string_lossy())
        .collect::<Vec<_>>()
        .join(" ")
}

fn cargo() -> &'static str {
    option_env!("CARGO").unwrap_or("cargo")
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("repo root")
        .to_path_buf()
}

fn reset_generated_dir(language: &str) -> PathBuf {
    assert!(
        matches!(language, "go" | "rust" | "ts"),
        "unexpected type output language directory: {language}"
    );

    let root = repo_root();
    let tmp_root = root.join("tmp");
    let output_dir = tmp_root.join(language);
    assert!(
        tmp_root.starts_with(&root) && output_dir.starts_with(&tmp_root),
        "refusing to reset type output outside repo tmp: {}",
        output_dir.display()
    );

    fs::create_dir_all(&output_dir).unwrap_or_else(|err| {
        panic!(
            "create type output directory {}: {err}",
            output_dir.display()
        )
    });
    output_dir
}

fn reset_generated_rust_dir(output_dir_name: &str) -> PathBuf {
    assert!(
        output_dir_name == "rust" || output_dir_name.starts_with("rust-"),
        "unexpected Rust output directory: {output_dir_name}"
    );

    let root = repo_root();
    let tmp_root = root.join("tmp");
    let output_dir = tmp_root.join(output_dir_name);
    assert!(
        tmp_root.starts_with(&root) && output_dir.starts_with(&tmp_root),
        "refusing to reset type output outside repo tmp: {}",
        output_dir.display()
    );

    fs::create_dir_all(&output_dir).unwrap_or_else(|err| {
        panic!(
            "create Rust type output directory {}: {err}",
            output_dir.display()
        )
    });
    output_dir
}

fn exe_name(name: &str) -> String {
    if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_owned()
    }
}
