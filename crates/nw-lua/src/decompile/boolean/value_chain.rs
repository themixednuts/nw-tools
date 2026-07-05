//! Value-producing `and`/`or` chain recognition.

use std::collections::BTreeSet;

use crate::{
    decompile::{
        analysis::DecompileAnalysis,
        control_flow::conditionals::{self, PhiSource},
    },
    ir::{RelOp, SsaFunction, SsaOp, SsaRef},
};

use super::{
    branch_at, branch_info, is_pure_value_node,
    short_circuit::{
        ValuePlan, ValuePlanKind, ValueTerm, branch_rel, pure_select_range, selected_operand,
    },
};

pub(super) fn and_or_value_chain(
    function: &SsaFunction,
    expr_analysis: &DecompileAnalysis,
    start: usize,
    pc_map: &[Option<usize>],
) -> Option<ValuePlan> {
    let first = branch_info(function, start, pc_map)?;
    if branch_rel(function, first.node)? == RelOp::TestSet {
        return None;
    }

    for merge in candidate_phi_merges(function, start, first.true_block, first.false_block) {
        for phi in conditionals::phi_sources(function, merge) {
            if !pure_select_range_with_constructor_arms(
                function,
                expr_analysis,
                first.node,
                start,
                merge,
                &phi,
            ) {
                continue;
            }
            let mut parser = ChainParser {
                function,
                expr_analysis,
                pc_map,
                phi: &phi,
                start,
                merge,
                used_sources: BTreeSet::new(),
            };
            let Some(groups) = parser.parse(start) else {
                continue;
            };
            if (groups.len() < 2 && groups.first().is_none_or(|group| group.len() < 2))
                || repeats_earlier_value_as_or_fallback(function, &groups)
                || !parser.used_all_sources()
            {
                continue;
            }
            return Some(ValuePlan {
                start,
                merge,
                dest: phi.dest,
                pc: phi.pc,
                kind: ValuePlanKind::AndOrChain { groups },
            });
        }
    }

    None
}

fn pure_select_range_with_constructor_arms(
    function: &SsaFunction,
    expr_analysis: &DecompileAnalysis,
    branch: crate::decompile::analysis::NodeId,
    start: usize,
    merge: usize,
    phi: &PhiSource,
) -> bool {
    if pure_select_range(function, branch, start, merge) {
        return true;
    }
    start < merge
        && branch.block == start
        && (start..merge).all(|block| {
            let Some(block_ref) = function.blocks.get(block) else {
                return false;
            };
            let first_node = if block == start { branch.node } else { 0 };
            block_ref
                .nodes
                .iter()
                .enumerate()
                .skip(first_node)
                .all(|(node_index, node)| {
                    is_pure_value_node(node)
                        || is_phi_constructor_mutation(
                            function,
                            expr_analysis,
                            phi,
                            block,
                            node_index,
                            node,
                        )
                })
        })
}

fn is_phi_constructor_mutation(
    function: &SsaFunction,
    expr_analysis: &DecompileAnalysis,
    phi: &PhiSource,
    block: usize,
    node_index: usize,
    node: &crate::ir::SsaNode,
) -> bool {
    phi.sources.iter().any(|(source_block, operand)| {
        if *source_block != block {
            return false;
        }
        let Some(def_id) = expr_analysis.def_site(*operand) else {
            return false;
        };
        if def_id.block != block || def_id.node >= node_index {
            return false;
        }
        let Some(def) = function
            .blocks
            .get(def_id.block)
            .and_then(|block| block.nodes.get(def_id.node))
        else {
            return false;
        };
        let Some(table_reg) = def.dest.reg_index() else {
            return false;
        };
        matches!(def.op, SsaOp::NewTable { .. })
            && is_constructor_mutation_for(node, def.dest, table_reg)
    })
}

fn is_constructor_mutation_for(node: &crate::ir::SsaNode, table: SsaRef, table_reg: u16) -> bool {
    match &node.op {
        SsaOp::SetTable {
            table: set_table, ..
        } => *set_table == table || set_table.reg_index() == Some(table_reg),
        SsaOp::SetList {
            table: set_table, ..
        } => *set_table == table || set_table.reg_index() == Some(table_reg),
        _ => false,
    }
}

fn candidate_phi_merges(
    function: &SsaFunction,
    start: usize,
    true_target: usize,
    false_target: usize,
) -> Vec<usize> {
    let true_start = conditionals::follow_jmp_only(function, true_target, None);
    let false_start = conditionals::follow_jmp_only(function, false_target, None);
    let mut candidates = BTreeSet::new();
    if let Some(merge) = conditionals::find_merge(function, start, true_start, false_start)
        && can_reach_without(function, true_target, merge, start)
        && can_reach_without(function, false_target, merge, start)
    {
        candidates.insert(merge);
    }
    for block in (start + 1)..function.blocks.len() {
        if !conditionals::phi_sources(function, block).is_empty()
            && can_reach_without(function, true_target, block, start)
            && can_reach_without(function, false_target, block, start)
        {
            candidates.insert(block);
        }
    }
    candidates.into_iter().collect()
}

fn can_reach_without(function: &SsaFunction, from: usize, target: usize, forbidden: usize) -> bool {
    let mut stack = vec![from];
    let mut visited = BTreeSet::new();
    while let Some(block) = stack.pop() {
        if block == target {
            return true;
        }
        if block == forbidden || !visited.insert(block) {
            continue;
        }
        if let Some(block_ref) = function.blocks.get(block) {
            stack.extend(block_ref.succs.iter().copied());
        }
    }
    false
}

struct ChainParser<'a> {
    function: &'a SsaFunction,
    expr_analysis: &'a DecompileAnalysis,
    pc_map: &'a [Option<usize>],
    phi: &'a PhiSource,
    start: usize,
    merge: usize,
    used_sources: BTreeSet<usize>,
}

impl ChainParser<'_> {
    fn parse(&mut self, start: usize) -> Option<Vec<Vec<ValueTerm>>> {
        let mut groups = Vec::new();
        let mut current = start;

        for _ in 0..16 {
            let (group, next) = self.parse_group(current)?;
            if group.is_empty() {
                return None;
            }
            groups.push(group);
            match next {
                Some(next) if next != self.merge => current = next,
                _ => return Some(groups),
            }
        }

        None
    }

    fn parse_group(&mut self, start: usize) -> Option<(Vec<ValueTerm>, Option<usize>)> {
        let mut terms = Vec::new();
        let mut current = self.follow_value_target(start);
        let mut next_or = None;

        for _ in 0..16 {
            if let Some(term) = self.source_term(current) {
                terms.push(term);
                return Some((terms, next_or));
            }

            let info = branch_info(self.function, current, self.pc_map)?;
            let rel = branch_rel(self.function, info.node)?;
            if rel == RelOp::TestSet {
                return None;
            }

            let true_target = self.follow_value_target(info.true_block);
            let false_target = self.follow_value_target(info.false_block);
            let true_source = self.source_operand(true_target);
            let false_source = self.source_operand(false_target);
            match (true_source, false_source) {
                (Some(true_operand), Some(false_operand)) => {
                    if let (Some(true_value), Some(false_value)) = (
                        self.source_bool_value(true_target, true_operand),
                        self.source_bool_value(false_target, false_operand),
                    ) && true_value != false_value
                    {
                        self.mark_source_used(true_target, true_operand);
                        self.mark_source_used(false_target, false_operand);
                        let value_target = if true_value {
                            true_target
                        } else {
                            false_target
                        };
                        terms.push(condition_for_target(
                            info.node,
                            value_target,
                            true_target,
                            false_target,
                        )?);
                        return Some((terms, next_or));
                    }
                    if rel == RelOp::Test
                        && self.source_bool_value(false_target, false_operand) == Some(false)
                    {
                        self.mark_source_used(false_target, false_operand);
                        terms.push(ValueTerm::Condition {
                            branch: info.node,
                            inverted: false,
                        });
                        terms.push(self.source_term(true_target)?);
                        return Some((terms, next_or));
                    }
                    if rel == RelOp::Test
                        && self.source_bool_value(true_target, true_operand) == Some(false)
                    {
                        self.mark_source_used(true_target, true_operand);
                        terms.push(ValueTerm::Condition {
                            branch: info.node,
                            inverted: true,
                        });
                        terms.push(self.source_term(false_target)?);
                        return Some((terms, next_or));
                    }
                    if source_matches_branch_value(self.function, info.node, true_operand) {
                        if branch_test_invert(self.function, info.node)? {
                            self.mark_source_used(true_target, true_operand);
                            terms.push(positive_test_condition(self.function, info.node)?);
                            terms.push(self.source_term(false_target)?);
                            return Some((terms, next_or));
                        }
                        terms.push(self.source_term(true_target)?);
                        return Some((terms, Some(false_target)));
                    }
                    if source_matches_branch_value(self.function, info.node, false_operand) {
                        if !branch_test_invert(self.function, info.node)? {
                            self.mark_source_used(false_target, false_operand);
                            terms.push(positive_test_condition(self.function, info.node)?);
                            terms.push(self.source_term(true_target)?);
                            return Some((terms, next_or));
                        }
                        terms.push(self.source_term(false_target)?);
                        return Some((terms, Some(true_target)));
                    }
                    if self
                        .testset_source_term(true_target, true_operand)
                        .is_some()
                    {
                        terms.push(ValueTerm::Condition {
                            branch: info.node,
                            inverted: false,
                        });
                        terms.push(self.source_term(true_target)?);
                        return Some((terms, Some(false_target)));
                    }
                    if self
                        .testset_source_term(false_target, false_operand)
                        .is_some()
                    {
                        terms.push(ValueTerm::Condition {
                            branch: info.node,
                            inverted: true,
                        });
                        terms.push(self.source_term(false_target)?);
                        return Some((terms, Some(true_target)));
                    }
                    return None;
                }
                (Some(true_operand), None) if !terms.is_empty() => {
                    if self.source_bool_value(true_target, true_operand) == Some(true) {
                        self.mark_source_used(true_target, true_operand);
                        terms.push(condition_for_target(
                            info.node,
                            true_target,
                            true_target,
                            false_target,
                        )?);
                    } else {
                        terms.push(self.source_term(true_target)?);
                    }
                    return Some((terms, Some(false_target)));
                }
                (Some(true_operand), None)
                    if source_matches_branch_value(self.function, info.node, true_operand) =>
                {
                    if branch_test_invert(self.function, info.node)? {
                        self.mark_source_used(true_target, true_operand);
                        terms.push(positive_test_condition(self.function, info.node)?);
                        current = false_target;
                        continue;
                    }
                    terms.push(self.source_term(true_target)?);
                    return Some((terms, Some(false_target)));
                }
                (Some(_), None) => return None,
                (None, Some(false_operand)) if !terms.is_empty() => {
                    if self.source_bool_value(false_target, false_operand) == Some(true) {
                        self.mark_source_used(false_target, false_operand);
                        terms.push(condition_for_target(
                            info.node,
                            false_target,
                            true_target,
                            false_target,
                        )?);
                        return Some((terms, Some(true_target)));
                    }
                    if !branch_test_invert(self.function, info.node)? {
                        let _ = false_operand;
                        if !self.set_next_or(&mut next_or, false_target) {
                            return None;
                        }
                        terms.push(positive_test_condition(self.function, info.node)?);
                        current = true_target;
                        continue;
                    }
                    let _ = false_operand;
                    terms.push(self.source_term(false_target)?);
                    return Some((terms, Some(true_target)));
                }
                (None, Some(_)) => {
                    if !self.set_next_or(&mut next_or, false_target) {
                        return None;
                    }
                    terms.push(ValueTerm::Condition {
                        branch: info.node,
                        inverted: false,
                    });
                    current = true_target;
                    continue;
                }
                (None, None) => {}
            }

            if !self.set_next_or(&mut next_or, false_target) {
                return None;
            }
            terms.push(ValueTerm::Condition {
                branch: info.node,
                inverted: false,
            });
            current = true_target;
        }

        None
    }

    fn set_next_or(&self, next_or: &mut Option<usize>, target: usize) -> bool {
        if target == self.merge {
            return false;
        }
        match *next_or {
            Some(current) => current == target,
            None => {
                *next_or = Some(target);
                true
            }
        }
    }

    fn follow_value_target(&self, target: usize) -> usize {
        let mut current = target;
        for _ in 0..self.function.blocks.len().min(64) {
            if current == self.merge
                || phi_operand_from(self.phi, current).is_some()
                || !conditionals::is_jmp_only(self.function, current)
            {
                break;
            }
            let Some(&next) = self.function.blocks[current].succs.first() else {
                break;
            };
            current = next;
        }
        current
    }

    fn source_term(&mut self, block: usize) -> Option<ValueTerm> {
        let operand = self.source_operand(block)?;
        self.used_sources.insert(block);
        self.mark_duplicate_sources(operand);
        if let Some(term) = self.testset_source_term(block, operand) {
            return Some(term);
        }
        if self.available_before_chain(operand) {
            return Some(operand.into());
        }
        let value_block = self
            .expr_analysis
            .def_site(operand)
            .map_or(block, |id| id.block);
        selected_operand(
            self.function,
            self.expr_analysis,
            self.phi.dest,
            value_block,
            operand,
        )
    }

    fn source_operand(&self, block: usize) -> Option<SsaRef> {
        phi_operand_from(self.phi, block)
    }

    fn source_bool_value(&self, block: usize, operand: SsaRef) -> Option<bool> {
        let node_id = self.expr_analysis.def_site(operand)?;
        if node_id.block != block {
            return None;
        }
        let node = self
            .function
            .blocks
            .get(node_id.block)?
            .nodes
            .get(node_id.node)?;
        let SsaOp::LoadBool { value, .. } = node.op else {
            return None;
        };
        Some(value)
    }

    fn used_all_sources(&self) -> bool {
        self.phi
            .sources
            .iter()
            .map(|(source, _)| *source)
            .all(|source| self.used_sources.contains(&source))
    }

    fn mark_source_used(&mut self, block: usize, operand: SsaRef) {
        self.used_sources.insert(block);
        self.mark_duplicate_sources(operand);
    }

    fn testset_source_term(&self, block: usize, operand: SsaRef) -> Option<ValueTerm> {
        let node_id = self.expr_analysis.def_site(operand)?;
        if node_id.block != block {
            return None;
        }
        let SsaOp::Branch {
            rel: RelOp::TestSet,
            a,
            ..
        } = branch_at(self.function, node_id)?.op
        else {
            return None;
        };
        let value_block = self.expr_analysis.def_site(a).map_or(block, |id| id.block);
        selected_operand(
            self.function,
            self.expr_analysis,
            self.phi.dest,
            value_block,
            a,
        )
    }

    fn mark_duplicate_sources(&mut self, operand: SsaRef) {
        for (source, source_operand) in &self.phi.sources {
            if *source_operand == operand
                && conditionals::follow_jmp_only(self.function, *source, Some(self.merge))
                    == self.merge
            {
                self.used_sources.insert(*source);
            }
        }
    }

    fn available_before_chain(&self, operand: SsaRef) -> bool {
        let Some(def_site) = self.expr_analysis.def_site(operand) else {
            return false;
        };
        if def_site.block < self.start {
            return true;
        }
        if def_site.block != self.start {
            return false;
        }
        let Some(start_branch) = branch_info(self.function, self.start, self.pc_map) else {
            return false;
        };
        def_site.node < start_branch.node.node
    }
}

fn phi_operand_from(phi: &PhiSource, block: usize) -> Option<SsaRef> {
    phi.sources
        .iter()
        .copied()
        .find_map(|(source, operand)| (source == block).then_some(operand))
}

fn source_matches_branch_value(
    function: &SsaFunction,
    branch: crate::decompile::analysis::NodeId,
    source: SsaRef,
) -> bool {
    let Some(node) = super::branch_at(function, branch) else {
        return false;
    };
    let SsaOp::Branch {
        rel: RelOp::Test,
        a,
        ..
    } = node.op
    else {
        return false;
    };
    source == a
}

fn branch_test_invert(
    function: &SsaFunction,
    branch: crate::decompile::analysis::NodeId,
) -> Option<bool> {
    let node = super::branch_at(function, branch)?;
    let SsaOp::Branch {
        rel: RelOp::Test,
        invert,
        ..
    } = node.op
    else {
        return None;
    };
    Some(invert)
}

fn positive_test_condition(
    function: &SsaFunction,
    branch: crate::decompile::analysis::NodeId,
) -> Option<ValueTerm> {
    Some(ValueTerm::Condition {
        branch,
        inverted: branch_test_invert(function, branch)?,
    })
}

fn repeats_earlier_value_as_or_fallback(function: &SsaFunction, groups: &[Vec<ValueTerm>]) -> bool {
    let mut seen = Vec::new();
    for (group_index, group) in groups.iter().enumerate() {
        if group_index > 0
            && group.len() == 1
            && let Some(reference) = term_test_ref(function, group[0])
            && seen.contains(&reference)
        {
            return true;
        }
        for term in group {
            if let Some(reference) = term_test_ref(function, *term)
                && !seen.contains(&reference)
            {
                seen.push(reference);
            }
        }
    }
    false
}

fn term_test_ref(function: &SsaFunction, term: ValueTerm) -> Option<SsaRef> {
    match term {
        ValueTerm::Ref(reference) => Some(reference),
        ValueTerm::Node(_) => None,
        ValueTerm::Condition { branch, inverted } => {
            let node = branch_at(function, branch)?;
            let SsaOp::Branch {
                rel,
                a,
                invert: node_invert,
                ..
            } = node.op
            else {
                return None;
            };
            (matches!(rel, RelOp::Test | RelOp::TestSet) && !(node_invert ^ inverted)).then_some(a)
        }
    }
}

fn condition_for_target(
    branch: crate::decompile::analysis::NodeId,
    target: usize,
    true_target: usize,
    false_target: usize,
) -> Option<ValueTerm> {
    Some(ValueTerm::Condition {
        branch,
        inverted: if target == true_target {
            false
        } else if target == false_target {
            true
        } else {
            return None;
        },
    })
}
