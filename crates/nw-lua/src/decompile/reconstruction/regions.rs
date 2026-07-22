//! Region-tree projections needed by reconstruction planning.

use std::collections::HashSet;

use crate::{
    decompile::{
        analysis::NodeId,
        boolean::BooleanAnalysis,
        control_flow::{RegionTree, conditionals, regions::Region},
    },
    ir::{SsaFunction, SsaNode, SsaOp, SsaRef},
};

pub(super) fn collect_forced_loop_values(
    region: &Region,
    function: &SsaFunction,
    forced: &mut HashSet<SsaRef>,
) {
    match region {
        Region::Sequence(regions) => {
            for region in regions {
                collect_forced_loop_values(region, function, forced);
            }
        }
        Region::If(region) => {
            for arm in &region.arms {
                collect_forced_loop_values(&arm.body, function, forced);
            }
            if let Some(else_) = &region.else_ {
                collect_forced_loop_values(else_, function, forced);
            }
        }
        Region::While(region) => {
            let phis = conditionals::phi_sources(function, region.condition.branch.block);
            force_phi_values(&phis, |_| false, forced);
            collect_forced_loop_values(&region.body, function, forced);
        }
        Region::Repeat(region) => collect_forced_loop_values(&region.body, function, forced),
        Region::NumericFor(region) => {
            let base = region.info.base;
            let phis = conditionals::phi_sources(function, region.info.loop_block);
            force_phi_values(
                &phis,
                |reg| reg >= base && reg <= base.saturating_add(3),
                forced,
            );
            collect_forced_loop_values(&region.body, function, forced);
        }
        Region::GenericFor(region) => {
            let base = region.info.base;
            let end = base.saturating_add(2 + region.info.count.max(0) as u16);
            let phis = conditionals::phi_sources(function, region.info.tfor_block);
            force_phi_values(&phis, |reg| reg >= base && reg <= end, forced);
            collect_forced_loop_values(&region.body, function, forced);
        }
        Region::Value(_) | Region::Linear(_) | Region::Break => {}
    }
}

pub(super) fn collect_control_blocks(
    region: &Region,
    booleans: &BooleanAnalysis,
    blocks: &mut HashSet<usize>,
) {
    match region {
        Region::Sequence(regions) => {
            for region in regions {
                collect_control_blocks(region, booleans, blocks);
            }
        }
        Region::If(region) => {
            for arm in &region.arms {
                collect_condition_blocks(arm.condition, booleans, blocks);
                collect_control_blocks(&arm.body, booleans, blocks);
            }
            if let Some(else_) = &region.else_ {
                collect_control_blocks(else_, booleans, blocks);
            }
        }
        Region::While(region) => {
            collect_condition_blocks(region.condition, booleans, blocks);
            collect_control_blocks(&region.body, booleans, blocks);
        }
        Region::Repeat(region) => {
            collect_condition_blocks(region.condition, booleans, blocks);
            collect_control_blocks(&region.body, booleans, blocks);
        }
        Region::NumericFor(region) => collect_control_blocks(&region.body, booleans, blocks),
        Region::GenericFor(region) => collect_control_blocks(&region.body, booleans, blocks),
        Region::Value(_) | Region::Linear(_) | Region::Break => {}
    }
}

pub(super) fn collect_emittable_nodes(region: &Region, nodes: &mut HashSet<NodeId>) {
    match region {
        Region::Sequence(regions) => {
            for region in regions {
                collect_emittable_nodes(region, nodes);
            }
        }
        Region::Linear(linear) => nodes.extend(linear.nodes.iter().copied()),
        Region::Value(region) => nodes.extend(region.prefix.nodes.iter().copied()),
        Region::If(region) => {
            nodes.extend(region.prefix.nodes.iter().copied());
            for arm in &region.arms {
                collect_emittable_nodes(&arm.body, nodes);
            }
            if let Some(else_) = &region.else_ {
                collect_emittable_nodes(else_, nodes);
            }
        }
        Region::While(region) => {
            nodes.extend(region.prefix.nodes.iter().copied());
            collect_emittable_nodes(&region.body, nodes);
        }
        Region::Repeat(region) => collect_emittable_nodes(&region.body, nodes),
        Region::NumericFor(region) => {
            nodes.extend(region.prefix.nodes.iter().copied());
            collect_emittable_nodes(&region.body, nodes);
        }
        Region::GenericFor(region) => {
            nodes.extend(region.prefix.nodes.iter().copied());
            collect_emittable_nodes(&region.body, nodes);
        }
        Region::Break => {}
    }
}

pub(super) fn emission_sequences(
    regions: Option<&RegionTree>,
    function: &SsaFunction,
) -> Vec<Vec<NodeId>> {
    let Some(regions) = regions else {
        return vec![
            function
                .blocks
                .iter()
                .flat_map(|block| {
                    (0..block.nodes.len()).map(|node| NodeId {
                        block: block.index,
                        node,
                    })
                })
                .collect(),
        ];
    };
    let mut sequences = Vec::new();
    collect_emission_sequences(&regions.root, function, &mut sequences);
    sequences
}

fn collect_emission_sequences(
    region: &Region,
    function: &SsaFunction,
    sequences: &mut Vec<Vec<NodeId>>,
) {
    match region {
        Region::Sequence(regions) => {
            for region in regions {
                collect_emission_sequences(region, function, sequences);
            }
        }
        Region::Linear(linear) => push_sequence(sequences, linear.nodes.clone()),
        Region::Value(region) => push_sequence(sequences, region.prefix.nodes.clone()),
        Region::If(region) => {
            push_sequence(sequences, region.prefix.nodes.clone());
            for arm in &region.arms {
                collect_emission_sequences(&arm.body, function, sequences);
            }
            if let Some(else_) = &region.else_ {
                collect_emission_sequences(else_, function, sequences);
            }
        }
        Region::While(region) => {
            push_sequence(sequences, region.prefix.nodes.clone());
            collect_emission_sequences(&region.body, function, sequences);
        }
        Region::Repeat(region) => {
            collect_emission_sequences(&region.body, function, sequences);
        }
        Region::NumericFor(region) => {
            let base = region.info.base;
            let setup_nodes = [
                region.info.start_node,
                region.info.stop_node,
                region.info.step_node,
                Some(region.info.prep_node),
            ]
            .into_iter()
            .flatten()
            .collect::<HashSet<_>>();
            let nodes = region
                .prefix
                .nodes
                .iter()
                .copied()
                .filter(|id| {
                    !setup_nodes.contains(id)
                        && analysis_node(function, *id).is_none_or(|node| {
                            !matches!(
                                &node.op,
                                SsaOp::ForPrep { control, .. } if control.base() == base
                            )
                        })
                })
                .collect();
            push_sequence(sequences, nodes);
            collect_emission_sequences(&region.body, function, sequences);
        }
        Region::GenericFor(region) => {
            let base = region.info.base;
            let skip_end = base.saturating_add(2 + region.info.count.max(0) as u16);
            let setup_nodes = region.setup_nodes.iter().copied().collect::<HashSet<_>>();
            let nodes = region
                .prefix
                .nodes
                .iter()
                .copied()
                .filter(|id| {
                    !setup_nodes.contains(id)
                        && analysis_node(function, *id).is_none_or(|node| {
                            !matches!(
                                &node.op,
                                SsaOp::SetList { base: table_reg, .. }
                                    if *table_reg >= base && *table_reg <= skip_end
                            )
                        })
                })
                .collect();
            push_sequence(sequences, nodes);
            collect_emission_sequences(&region.body, function, sequences);
        }
        Region::Break => {}
    }
}

fn push_sequence(sequences: &mut Vec<Vec<NodeId>>, nodes: Vec<NodeId>) {
    if !nodes.is_empty() {
        sequences.push(nodes);
    }
}

fn analysis_node(function: &SsaFunction, id: NodeId) -> Option<&SsaNode> {
    function
        .blocks
        .get(id.block)
        .and_then(|block| block.nodes.get(id.node))
}

fn collect_condition_blocks(
    condition: crate::decompile::control_flow::regions::Condition,
    booleans: &BooleanAnalysis,
    blocks: &mut HashSet<usize>,
) {
    blocks.insert(condition.branch.block);
    if let Some(start) = condition.compound
        && let Some(chain) = booleans.condition_chain(start)
    {
        blocks.extend(chain.blocks.iter().copied());
    }
}

fn force_phi_values(
    phis: &[conditionals::PhiSource],
    exclude: impl Fn(u16) -> bool,
    forced: &mut HashSet<SsaRef>,
) {
    for phi in phis {
        if phi.dest.reg_index().is_some_and(&exclude) {
            continue;
        }
        forced.insert(phi.dest);
        forced.extend(phi.sources.iter().map(|(_, operand)| *operand));
    }
}
