//! Linear region fragments used inside structured control-flow regions.

use crate::{LuaError, bytecode::OpcodeTable, ir::SsaFunction};

use super::analysis::NodeId;

/// A straight-line stream of SSA nodes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinearRegion {
    pub nodes: Vec<NodeId>,
}

/// Build a best-effort linear node stream in block order.
///
/// Phase 5 uses [`super::control_flow`] for real structuring; this helper is
/// retained for focused tests and straight-line fragments inside regions.
pub fn linearize(function: &SsaFunction, _table: &OpcodeTable) -> Result<LinearRegion, LuaError> {
    let mut nodes = Vec::new();
    for (current, block) in function.blocks.iter().enumerate() {
        nodes.extend((0..block.nodes.len()).map(|node| NodeId {
            block: current,
            node,
        }));
    }
    Ok(LinearRegion { nodes })
}
