//! Lua source materialization through `full_moon` and `stylua`.

mod builder;
mod lower;
mod validate;

#[cfg(test)]
mod tests;

use stylua_lib::{Config, OutputVerification, format_code};

use crate::{LuaError, decompile};

pub use lower::lower_block;

const VERIFICATION_STACK_SIZE: usize = 64 * 1024 * 1024;

/// Emits formatted Lua source from a compact decompiler IR block.
///
/// # Errors
///
/// Returns [`LuaError`] if the raw or formatted Lua fails to parse, or if
/// StyLua rejects the emitted source.
pub fn to_source(block: &decompile::ast::Block) -> Result<String, LuaError> {
    validate::block(block)?;
    let lowered = lower_block(block);
    let raw = lowered.to_string();

    let formatted = format_source(&raw)?;

    assert_parse("formatted stylua output", &formatted)?;

    Ok(formatted)
}

fn format_source(raw: &str) -> Result<String, LuaError> {
    run_with_large_stack("stylua format", || {
        format_code(raw, Config::default(), None, OutputVerification::None)
            .map_err(|error| format!("stylua rejected emitted Lua: {error}"))
    })
}

fn assert_parse(stage: &'static str, source: &str) -> Result<(), LuaError> {
    run_with_large_stack(stage, || {
        full_moon::parse(source)
            .map(|_| ())
            .map_err(|errors| format!("{stage} did not parse: {errors:#?}"))
    })
}

fn run_with_large_stack<T>(
    stage: &'static str,
    f: impl FnOnce() -> Result<T, String> + Send,
) -> Result<T, LuaError>
where
    T: Send,
{
    std::thread::scope(|scope| {
        std::thread::Builder::new()
            .name(format!("nw-lua-{stage}"))
            .stack_size(VERIFICATION_STACK_SIZE)
            .spawn_scoped(scope, f)
            .map_err(|error| LuaError::Emit(format!("failed to spawn {stage} worker: {error}")))?
            .join()
            .map_err(|_| LuaError::Emit(format!("{stage} worker panicked")))?
            .map_err(LuaError::Emit)
    })
}
