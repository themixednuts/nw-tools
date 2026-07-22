//! Semantics-preserving AST cleanup for idiomatic Lua output.

use bstr::BString;

use crate::{chunk::Proto, decompile::ast::Block};

pub mod engine;

mod naming_style;
mod structure;
mod sugar;

use engine::{CleanContext, Engine, Rule};

/// Builds cleanup context from the current prototype.
pub fn context_for_proto(proto: &Proto) -> CleanContext {
    context_for_proto_with_fallback(proto, None)
}

/// Builds cleanup context with a file-stem fallback for stripped chunks.
pub fn context_for_proto_with_fallback(
    proto: &Proto,
    fallback_module_stem: Option<&str>,
) -> CleanContext {
    CleanContext::new(
        module_stem(&proto.source).or_else(|| fallback_module_stem.and_then(clean_stem)),
    )
}

/// Applies the idiomatic cleanup pass to a decompiled block.
pub fn clean(block: Block, ctx: CleanContext) -> Block {
    let rules: [&dyn Rule; 11] = [
        &naming_style::ModuleTableName,
        &naming_style::ConsumerFieldTableName,
        &sugar::AssignmentFunctionSugar,
        &sugar::LocalFunctionSugar,
        &sugar::RecursiveLocalFunctionSugar,
        &sugar::MethodDeclarationSugar,
        &structure::ElseIfChain,
        &structure::DropElseAfterExit,
        &structure::EarlyReturnGuard,
        &structure::EmptyBranchCleanup,
        &structure::RedundantDo,
    ];
    Engine::new(&rules).run(block, ctx)
}

fn module_stem(source: &BString) -> Option<String> {
    let source = std::str::from_utf8(source.as_slice()).ok()?;
    let source = source.trim_start_matches('@');
    let file = source
        .rsplit(['\\', '/'])
        .next()
        .filter(|part| !part.is_empty())?;
    let stem = file.rsplit_once('.').map_or(file, |(stem, _)| stem);
    clean_stem(stem)
}

fn clean_stem(stem: &str) -> Option<String> {
    let stem = stem.trim();
    (!stem.is_empty()).then(|| stem.to_string())
}
