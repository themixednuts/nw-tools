use std::{
    env, fs,
    path::{Path, PathBuf},
    process::{self, Command, Output},
    time::{SystemTime, UNIX_EPOCH},
};

const CHILD_OK: &str = "NW_LUA_CORPUS_CHILD_OK";
const CHILD_ERR: &str = "NW_LUA_CORPUS_CHILD_ERR";

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
    let paths = TempPaths::new();

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

struct TempPaths {
    source: PathBuf,
    bytecode: PathBuf,
}

impl TempPaths {
    fn new() -> Self {
        let millis = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time before UNIX_EPOCH")
            .as_millis();
        let stem = format!("nw_lua_child_{}_{}", process::id(), millis);
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
