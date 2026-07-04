use std::{
    collections::BTreeSet,
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
const CHILD_UNDEFINED_SYNTHETIC: &str = "NW_LUA_UNDEFINED_SYNTHETIC";

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
    let core_undefined = undefined_synthetic_reads(&core_source);
    let idiomatic_undefined = undefined_synthetic_reads(&idiomatic_source);
    let undefined_synthetics = core_undefined.len() + idiomatic_undefined.len();
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
        print_undefined_synthetic_examples("core", &core_undefined);
        print_undefined_synthetic_examples("idiomatic", &idiomatic_undefined);
        print_structural_report(&original, &recompiled, undefined_synthetics);

        fs::write(&paths.source, &idiomatic_source).map_err(|err| err.to_string())?;
        compile_lua(luac, &paths.source, &paths.bytecode)
            .map_err(|err| format!("idiomatic recompile: {err}"))?;
        Ok(())
    })();

    paths.cleanup();
    result
}

fn print_undefined_synthetic_examples(kind: &str, names: &[String]) {
    for name in names.iter().take(8) {
        println!("{CHILD_UNDEFINED_SYNTHETIC}\tkind={kind}\tname={name}");
    }
}

fn structural_signature(bytes: &[u8]) -> Result<Vec<ProtoSignature>, String> {
    let chunk = nw_lua::parse_chunk(bytes).map_err(|err| err.to_string())?;
    let table = OpcodeTable::builtin(chunk.header.version).map_err(|err| err.to_string())?;
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

fn print_structural_report(
    original: &[ProtoSignature],
    recompiled: &[ProtoSignature],
    undefined_synthetics: usize,
) {
    let report = structural_report(original, recompiled);
    println!(
        "{CHILD_STRUCTURAL_REPORT}\toriginal_protos={}\trecompiled_protos={}\texact_protos={}\ttotal_protos={}\tmatched_ops={}\ttotal_ops={}\tundefined_synthetics={}",
        report.original_protos,
        report.recompiled_protos,
        report.exact_protos,
        report.total_protos,
        report.matched_ops,
        report.total_ops,
        undefined_synthetics
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

fn undefined_synthetic_reads(source: &str) -> Vec<String> {
    let tokens = lex_tokens(source);
    let mut scanner = SyntheticScopeScan {
        tokens: &tokens,
        index: 0,
        scopes: vec![BTreeSet::new()],
        missing: BTreeSet::new(),
    };
    scanner.run();
    scanner.missing.into_iter().collect()
}

struct SyntheticScopeScan<'a> {
    tokens: &'a [String],
    index: usize,
    scopes: Vec<BTreeSet<String>>,
    missing: BTreeSet<String>,
}

impl SyntheticScopeScan<'_> {
    fn run(&mut self) {
        while self.index < self.tokens.len() {
            match self.current() {
                Some("local") => self.scan_local(),
                Some("function") => self.scan_function(false),
                Some("for") => self.scan_for(),
                Some("then" | "repeat") => self.push_scope(BTreeSet::new()),
                Some("else") => {
                    self.pop_scope();
                    self.push_scope(BTreeSet::new());
                }
                Some("elseif" | "until" | "end") => self.pop_scope(),
                Some(token) if is_synthetic_name(token) => self.scan_synthetic_use(),
                _ => {}
            }
            self.index += 1;
        }
    }

    fn current(&self) -> Option<&str> {
        self.tokens.get(self.index).map(String::as_str)
    }

    fn scan_local(&mut self) {
        self.index += 1;
        if self.current() == Some("function") {
            self.index += 1;
            if let Some(name) = self.tokens.get(self.index)
                && is_synthetic_name(name)
            {
                self.define(name);
            }
            self.scan_function(true);
            return;
        }

        while let Some(token) = self.tokens.get(self.index) {
            if token == "=" || statement_boundary(token) {
                self.index = self.index.saturating_sub(1);
                return;
            }
            if is_synthetic_name(token) {
                self.define(token);
            }
            self.index += 1;
        }
    }

    fn scan_function(&mut self, local_function: bool) {
        if !local_function {
            self.scan_function_name();
        }
        let params = self.collect_function_params();
        self.push_scope(params);
    }

    fn scan_function_name(&mut self) {
        self.index += 1;
        let mut saw_path_separator = false;
        while let Some(token) = self.tokens.get(self.index) {
            if token == "(" {
                self.index = self.index.saturating_sub(1);
                return;
            }
            if token == "." || token == ":" {
                saw_path_separator = true;
            } else if is_synthetic_name(token) {
                if saw_path_separator
                    || self
                        .tokens
                        .get(self.index + 1)
                        .is_some_and(|next| next == "." || next == ":")
                {
                    self.read(token);
                } else {
                    self.define(token);
                }
            }
            self.index += 1;
        }
    }

    fn collect_function_params(&mut self) -> BTreeSet<String> {
        while self
            .tokens
            .get(self.index)
            .is_some_and(|token| token != "(")
        {
            self.index += 1;
        }
        let mut params = BTreeSet::new();
        while let Some(token) = self.tokens.get(self.index) {
            if token == ")" {
                break;
            }
            if is_synthetic_name(token) {
                params.insert(token.clone());
            }
            self.index += 1;
        }
        params
    }

    fn scan_for(&mut self) {
        self.index += 1;
        let mut names = BTreeSet::new();
        while let Some(token) = self.tokens.get(self.index) {
            if token == "=" || token == "in" || statement_boundary(token) {
                break;
            }
            if is_synthetic_name(token) {
                names.insert(token.clone());
            }
            self.index += 1;
        }
        self.index = self.index.saturating_sub(1);
        self.push_scope(names);
    }

    fn scan_synthetic_use(&mut self) {
        let token = self.tokens[self.index].clone();
        if self.previous_is_field_separator() {
            return;
        }
        if self.is_bare_assignment_target() {
            self.define(&token);
            return;
        }
        self.read(&token);
    }

    fn is_bare_assignment_target(&self) -> bool {
        if self.previous_is_field_separator() {
            return false;
        }
        match self.tokens.get(self.index + 1).map(String::as_str) {
            Some("=" | ",") => self.assignment_follows(),
            _ => false,
        }
    }

    fn assignment_follows(&self) -> bool {
        let mut cursor = self.index + 1;
        while let Some(token) = self.tokens.get(cursor).map(String::as_str) {
            match token {
                "=" => return true,
                "," => cursor += 1,
                token if is_identifier(token) => cursor += 1,
                _ => return false,
            }
        }
        false
    }

    fn previous_is_field_separator(&self) -> bool {
        self.index > 0
            && self
                .tokens
                .get(self.index - 1)
                .is_some_and(|token| token == "." || token == ":")
    }

    fn push_scope(&mut self, names: BTreeSet<String>) {
        self.scopes.push(names);
    }

    fn pop_scope(&mut self) {
        if self.scopes.len() > 1 {
            self.scopes.pop();
        }
    }

    fn define(&mut self, name: &str) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name.to_string());
        }
    }

    fn read(&mut self, name: &str) {
        if !self.scopes.iter().rev().any(|scope| scope.contains(name)) {
            self.missing.insert(name.to_string());
        }
    }
}

fn lex_tokens(source: &str) -> Vec<String> {
    let bytes = source.as_bytes();
    let mut tokens = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        let byte = bytes[index];
        if byte.is_ascii_whitespace() {
            index += 1;
        } else if is_ident_start(byte) {
            let start = index;
            index += 1;
            while index < bytes.len() && is_ident_continue(bytes[index]) {
                index += 1;
            }
            tokens.push(source[start..index].to_string());
        } else if byte == b'\'' || byte == b'"' {
            index = skip_quoted(bytes, index);
        } else if bytes[index..].starts_with(b"--[[") {
            index = skip_until(bytes, index + 4, b"]]");
        } else if bytes[index..].starts_with(b"--") {
            index = skip_line(bytes, index + 2);
        } else if index + 2 <= bytes.len()
            && matches!(&bytes[index..index + 2], b"==" | b"~=" | b"<=" | b">=")
        {
            tokens.push(source[index..index + 2].to_string());
            index += 2;
        } else {
            tokens.push(source[index..index + 1].to_string());
            index += 1;
        }
    }
    tokens
}

fn skip_quoted(bytes: &[u8], start: usize) -> usize {
    let quote = bytes[start];
    let mut index = start + 1;
    while index < bytes.len() {
        if bytes[index] == b'\\' {
            index += 2;
        } else if bytes[index] == quote {
            return index + 1;
        } else {
            index += 1;
        }
    }
    bytes.len()
}

fn skip_until(bytes: &[u8], mut index: usize, needle: &[u8]) -> usize {
    while index + needle.len() <= bytes.len() {
        if bytes[index..].starts_with(needle) {
            return index + needle.len();
        }
        index += 1;
    }
    bytes.len()
}

fn skip_line(bytes: &[u8], mut index: usize) -> usize {
    while index < bytes.len() && bytes[index] != b'\n' {
        index += 1;
    }
    index
}

fn statement_boundary(token: &str) -> bool {
    matches!(
        token,
        ";" | "then"
            | "do"
            | "else"
            | "elseif"
            | "end"
            | "repeat"
            | "until"
            | "while"
            | "for"
            | "if"
            | "return"
            | "local"
    )
}

fn is_synthetic_name(token: &str) -> bool {
    has_prefixed_number(token, "arg", false)
        || has_prefixed_number(token, "up", false)
        || has_prefixed_number(token, "v", true)
}

fn has_prefixed_number(token: &str, prefix: &str, allow_suffix: bool) -> bool {
    let Some(rest) = token.strip_prefix(prefix) else {
        return false;
    };
    if allow_suffix && let Some((head, tail)) = rest.split_once('_') {
        return !head.is_empty()
            && !tail.is_empty()
            && head.bytes().all(|byte| byte.is_ascii_digit())
            && tail.bytes().all(|byte| byte.is_ascii_digit());
    }
    !rest.is_empty() && rest.bytes().all(|byte| byte.is_ascii_digit())
}

fn is_identifier(token: &str) -> bool {
    let mut bytes = token.bytes();
    let Some(first) = bytes.next() else {
        return false;
    };
    is_ident_start(first) && bytes.all(is_ident_continue)
}

fn is_ident_start(byte: u8) -> bool {
    byte == b'_' || byte.is_ascii_alphabetic()
}

fn is_ident_continue(byte: u8) -> bool {
    is_ident_start(byte) || byte.is_ascii_digit()
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
