//! Table-constructor recognition from SSA and `NEWTABLE` compiler hints.

use crate::{
    decompile::analysis::{DecompileAnalysis, NodeId},
    ir::{SsaFunction, SsaNode, SsaOp, SsaRef},
};

/// One complete table-constructor window in a linear SSA block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TableConstructorPlan {
    table: SsaRef,
    end: NodeId,
    setlists: Vec<NodeId>,
    keyed: Vec<NodeId>,
    final_use: Option<NodeId>,
}

impl TableConstructorPlan {
    /// Recognizes a constructor beginning at one `NEWTABLE` definition.
    #[must_use]
    pub(crate) fn recognize(
        function: &SsaFunction,
        analysis: &DecompileAnalysis,
        start: NodeId,
    ) -> Option<Self> {
        Self::from_exact_hints(function, start)
            .or_else(|| Self::from_setlist_boundary(function, start))
            .map(|mut plan| {
                plan.final_use = first_use_after(function, plan.end, plan.table);
                if plan.final_use.is_none() {
                    plan.final_use = analysis
                        .real_uses(plan.table)
                        .iter()
                        .copied()
                        .find(|id| *id != start && !plan.contains_mutation(*id));
                }
                plan
            })
    }

    #[must_use]
    pub(crate) const fn end(&self) -> NodeId {
        self.end
    }

    #[must_use]
    pub(crate) fn setlists(&self) -> &[NodeId] {
        &self.setlists
    }

    #[must_use]
    pub(crate) fn keyed(&self) -> &[NodeId] {
        &self.keyed
    }

    #[must_use]
    pub(crate) fn mutation_count(&self) -> usize {
        self.setlists.len() + self.keyed.len()
    }

    #[must_use]
    pub(crate) const fn final_use(&self) -> Option<NodeId> {
        self.final_use
    }

    fn from_exact_hints(function: &SsaFunction, start: NodeId) -> Option<Self> {
        let block = function.blocks.get(start.block)?;
        let start_node = block.nodes.get(start.node)?;
        let mut frames = vec![ConstructorFrame::from_node(start_node)?];
        if frames[0].is_complete() {
            return None;
        }

        let outer_table = frames[0].table;
        let outer_reg = frames[0].table_reg;
        let mut setlists = Vec::new();
        let mut keyed = Vec::new();
        let mut cursor = start.node + 1;

        loop {
            while frames.last().is_some_and(ConstructorFrame::is_complete) {
                frames.pop();
                if frames.is_empty() {
                    let end = keyed.last().or_else(|| setlists.last()).copied()?;
                    return Some(Self {
                        table: outer_table,
                        end,
                        setlists,
                        keyed,
                        final_use: None,
                    });
                }
            }

            let node = block.nodes.get(cursor)?;
            let id = NodeId {
                block: start.block,
                node: cursor,
            };
            cursor += 1;

            if matches!(node.op, SsaOp::Nop) {
                continue;
            }

            if let Some(frame) = ConstructorFrame::from_node(node) {
                if frame.table_reg <= outer_reg {
                    return None;
                }
                frames.push(frame);
                continue;
            }

            let frame = frames.last_mut()?;
            if is_matching_settable(node, frame.table, frame.table_reg) {
                if frame.remaining_hash == 0 {
                    return None;
                }
                frame.remaining_hash -= 1;
                if frame.table == outer_table {
                    keyed.push(id);
                }
                continue;
            }

            if is_matching_setlist(node, frame.table, frame.table_reg) {
                let SsaOp::SetList { values, count, .. } = &node.op else {
                    unreachable!("matching SETLIST has SETLIST payload");
                };
                if *count == 0 || values.len() > frame.remaining_array {
                    return None;
                }
                frame.remaining_array -= values.len();
                if frame.table == outer_table {
                    setlists.push(id);
                }
                continue;
            }

            if !is_constructor_setup(node, outer_reg) {
                return None;
            }
        }
    }

    fn from_setlist_boundary(function: &SsaFunction, start: NodeId) -> Option<Self> {
        let block = function.blocks.get(start.block)?;
        let start_node = block.nodes.get(start.node)?;
        let SsaOp::NewTable { .. } = start_node.op else {
            return None;
        };
        let table = start_node.dest;
        let table_reg = table.reg_index()?;
        let mut setlists = Vec::new();
        let mut keyed = Vec::new();

        for (node_index, node) in block.nodes.iter().enumerate().skip(start.node + 1) {
            let id = NodeId {
                block: start.block,
                node: node_index,
            };
            if is_matching_setlist(node, table, table_reg) {
                setlists.push(id);
                continue;
            }
            if is_matching_settable(node, table, table_reg) {
                keyed.push(id);
                continue;
            }
            if op_uses_ref(&node.op, table) {
                break;
            }
            if is_constructor_setup(node, table_reg) {
                continue;
            }
            break;
        }

        let end = setlists.last().copied()?;
        keyed.retain(|id| id.node <= end.node);
        Some(Self {
            table,
            end,
            setlists,
            keyed,
            final_use: None,
        })
    }

    fn contains_mutation(&self, id: NodeId) -> bool {
        self.setlists.contains(&id) || self.keyed.contains(&id)
    }
}

#[derive(Debug, Clone, Copy)]
struct ConstructorFrame {
    table: SsaRef,
    table_reg: u16,
    remaining_array: usize,
    remaining_hash: usize,
}

impl ConstructorFrame {
    fn from_node(node: &SsaNode) -> Option<Self> {
        let SsaOp::NewTable {
            array_hint,
            hash_hint,
        } = node.op
        else {
            return None;
        };
        Some(Self {
            table: node.dest,
            table_reg: node.dest.reg_index()?,
            remaining_array: array_hint.exact_field_count()?,
            remaining_hash: hash_hint.exact_field_count()?,
        })
    }

    const fn is_complete(&self) -> bool {
        self.remaining_array == 0 && self.remaining_hash == 0
    }
}

fn first_use_after(function: &SsaFunction, end: NodeId, table: SsaRef) -> Option<NodeId> {
    function.blocks[end.block]
        .nodes
        .iter()
        .enumerate()
        .skip(end.node + 1)
        .find(|(_, node)| op_uses_ref(&node.op, table))
        .map(|(node, _)| NodeId {
            block: end.block,
            node,
        })
}

fn op_uses_ref(op: &SsaOp, needle: SsaRef) -> bool {
    let mut found = false;
    op.visit_uses(|reference, _| found |= reference == needle);
    found
}

pub(crate) fn is_matching_setlist(node: &SsaNode, table: SsaRef, table_reg: u16) -> bool {
    matches!(
        &node.op,
        SsaOp::SetList {
            table: setlist_table,
            base,
            ..
        } if *setlist_table == table || *base == table_reg
    )
}

pub(crate) fn is_matching_settable(node: &SsaNode, table: SsaRef, table_reg: u16) -> bool {
    matches!(
        &node.op,
        SsaOp::SetTable {
            table: settable_table,
            value,
            ..
        } if (*settable_table == table || settable_table.reg_index() == Some(table_reg))
            && value.reg_index() != Some(table_reg)
    )
}

fn is_constructor_setup(node: &SsaNode, table_reg: u16) -> bool {
    if matches!(
        &node.op,
        SsaOp::SetTable { table, .. } if table.reg_index().is_some_and(|reg| reg > table_reg)
    ) {
        return true;
    }
    if matches!(
        &node.op,
        SsaOp::SetList { base, .. } if *base > table_reg
    ) {
        return true;
    }
    let Some(dest_reg) = node.dest.reg_index() else {
        return false;
    };
    dest_reg > table_reg
        && matches!(
            &node.op,
            SsaOp::LoadK { .. }
                | SsaOp::LoadLiteral { .. }
                | SsaOp::LoadBool { .. }
                | SsaOp::LoadNil { .. }
                | SsaOp::GetGlobal { .. }
                | SsaOp::GetTable { .. }
                | SsaOp::GetUpval { .. }
                | SsaOp::Move { .. }
                | SsaOp::BinOp { .. }
                | SsaOp::UnOp { .. }
                | SsaOp::Concat { .. }
                | SsaOp::SelfOp { .. }
                | SsaOp::Call { .. }
                | SsaOp::VarArg { .. }
                | SsaOp::Closure { .. }
                | SsaOp::NewTable { .. }
        )
}
