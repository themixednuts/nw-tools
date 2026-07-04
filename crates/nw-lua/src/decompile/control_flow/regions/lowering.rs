use crate::{
    LuaError,
    decompile::{
        analysis::{DecompileAnalysis, NodeId},
        ast::{self, Expr, Name, Stmt},
        boolean::BooleanAnalysis,
        naming::NameResolver,
        stmt_build::StatementBuilder,
    },
    ir::{SsaFunction, SsaNode, SsaOp, SsaRef},
};

use super::super::conditionals::{self, PhiSource};
use super::types::{
    Condition, GenericForRegion, IfRegion, NumericForRegion, Region, RegionTree, RepeatRegion,
    WhileRegion,
};

impl RegionTree {
    pub(crate) fn lower(
        &self,
        function: &SsaFunction,
        analysis: &DecompileAnalysis,
        names: &NameResolver<'_>,
        booleans: &BooleanAnalysis,
        builder: &mut StatementBuilder<'_>,
    ) -> Result<ast::Block, LuaError> {
        force_region_loop_phi_values(&self.root, function, builder);
        let stmts = lower_region(&self.root, function, analysis, names, booleans, builder)?;
        Ok(ast::Block::new(stmts))
    }
}

fn lower_region(
    region: &Region,
    function: &SsaFunction,
    analysis: &DecompileAnalysis,
    names: &NameResolver<'_>,
    booleans: &BooleanAnalysis,
    builder: &mut StatementBuilder<'_>,
) -> Result<Vec<Stmt>, LuaError> {
    match region {
        Region::Sequence(regions) => {
            let mut out = Vec::new();
            for region in regions {
                append_stmts(
                    &mut out,
                    lower_region(region, function, analysis, names, booleans, builder)?,
                );
                if matches!(out.last(), Some(Stmt::Return(_) | Stmt::Break)) {
                    break;
                }
            }
            Ok(out)
        }
        Region::Linear(linear) => builder.emit_linear_region(linear),
        Region::If(region) => lower_if(region, function, analysis, names, booleans, builder),
        Region::While(region) => lower_while(region, function, analysis, names, booleans, builder),
        Region::Repeat(region) => {
            lower_repeat(region, function, analysis, names, booleans, builder)
        }
        Region::NumericFor(region) => {
            lower_numeric_for(region, function, analysis, names, booleans, builder)
        }
        Region::GenericFor(region) => {
            lower_generic_for(region, function, analysis, names, booleans, builder)
        }
        Region::Break => Ok(vec![Stmt::Break]),
    }
}

fn lower_if(
    region: &IfRegion,
    function: &SsaFunction,
    analysis: &DecompileAnalysis,
    names: &NameResolver<'_>,
    booleans: &BooleanAnalysis,
    builder: &mut StatementBuilder<'_>,
) -> Result<Vec<Stmt>, LuaError> {
    if let Some(stmts) = lower_boolean_value_if(region, function, analysis, booleans, builder)? {
        return Ok(stmts);
    }
    if let Some(stmts) = lower_simple_phi_value_if(region, function, analysis, booleans, builder)? {
        return Ok(stmts);
    }

    let mut out = builder.emit_linear_region(&region.prefix)?;
    for phi in &region.phis {
        if let Some(stmt) = builder.declare_phi_local(phi.dest, phi.pc) {
            out.push(stmt);
        }
    }

    let mut arms = Vec::new();
    for arm in &region.arms {
        let cond = lower_condition(arm.condition, function, analysis, booleans, builder)?;
        let mut body = lower_region(&arm.body, function, analysis, names, booleans, builder)?;
        append_phi_assignments(&mut body, &region.phis, &arm.blocks, builder)?;
        arms.push((cond, ast::Block::new(body)));
    }

    let else_ = if let Some(else_region) = &region.else_ {
        let mut body = lower_region(else_region, function, analysis, names, booleans, builder)?;
        append_phi_assignments(&mut body, &region.phis, &region.else_blocks, builder)?;
        Some(ast::Block::new(body))
    } else {
        None
    };

    out.push(Stmt::If { arms, else_ });
    Ok(out)
}

fn lower_boolean_value_if(
    region: &IfRegion,
    function: &SsaFunction,
    analysis: &DecompileAnalysis,
    booleans: &BooleanAnalysis,
    builder: &mut StatementBuilder<'_>,
) -> Result<Option<Vec<Stmt>>, LuaError> {
    let [phi] = region.phis.as_slice() else {
        return Ok(None);
    };
    let [arm] = region.arms.as_slice() else {
        return Ok(None);
    };
    let Some((true_block, false_block)) = branch_targets(function, arm.condition.branch) else {
        return Ok(None);
    };
    let true_block = conditionals::follow_jmp_only(function, true_block, region.merge);
    let false_block = conditionals::follow_jmp_only(function, false_block, region.merge);
    let Some(true_value) = phi_bool_for_block(function, analysis, phi, true_block) else {
        return Ok(None);
    };
    let Some(false_value) = phi_bool_for_block(function, analysis, phi, false_block) else {
        return Ok(None);
    };
    if true_value == false_value {
        return Ok(None);
    }

    let mut condition = arm.condition;
    if !true_value {
        condition.inverted = !condition.inverted;
    }
    let value = lower_condition(condition, function, analysis, booleans, builder)?;
    let mut out = builder.emit_linear_region(&region.prefix)?;
    out.push(builder.materialize_value(phi.dest, phi.pc, value));
    Ok(Some(out))
}

fn lower_simple_phi_value_if(
    region: &IfRegion,
    function: &SsaFunction,
    analysis: &DecompileAnalysis,
    booleans: &BooleanAnalysis,
    builder: &mut StatementBuilder<'_>,
) -> Result<Option<Vec<Stmt>>, LuaError> {
    let [phi] = region.phis.as_slice() else {
        return Ok(None);
    };
    let [arm] = region.arms.as_slice() else {
        return Ok(None);
    };
    let Some((true_block, false_block)) = branch_targets(function, arm.condition.branch) else {
        return Ok(None);
    };
    let true_block = conditionals::follow_jmp_only(function, true_block, region.merge);
    let false_block = conditionals::follow_jmp_only(function, false_block, region.merge);
    let Some(true_operand) = phi_operand_for_block(phi, true_block) else {
        return Ok(None);
    };
    let Some(false_operand) = phi_operand_for_block(phi, false_block) else {
        return Ok(None);
    };
    if true_operand == false_operand
        || !block_is_phi_value_only(function, analysis, true_block, true_operand)
        || !block_is_phi_value_only(function, analysis, false_block, false_operand)
    {
        return Ok(None);
    }

    let mut out = builder.emit_linear_region(&region.prefix)?;
    if let Some(stmt) = builder.declare_phi_local(phi.dest, phi.pc) {
        out.push(stmt);
    }
    let condition = lower_condition(arm.condition, function, analysis, booleans, builder)?;
    let true_value = phi_operand_expr(function, analysis, builder, true_operand)?;
    let false_value = phi_operand_expr(function, analysis, builder, false_operand)?;
    out.push(Stmt::If {
        arms: vec![(
            condition,
            ast::Block::new(vec![
                builder.materialize_value(phi.dest, phi.pc, true_value),
            ]),
        )],
        else_: Some(ast::Block::new(vec![builder.materialize_value(
            phi.dest,
            phi.pc,
            false_value,
        )])),
    });
    Ok(Some(out))
}

fn lower_while(
    region: &WhileRegion,
    function: &SsaFunction,
    analysis: &DecompileAnalysis,
    names: &NameResolver<'_>,
    booleans: &BooleanAnalysis,
    builder: &mut StatementBuilder<'_>,
) -> Result<Vec<Stmt>, LuaError> {
    let mut out = builder.emit_linear_region(&region.prefix)?;
    let cond = lower_condition(region.condition, function, analysis, booleans, builder)?;
    let body = lower_region(&region.body, function, analysis, names, booleans, builder)?;
    out.push(Stmt::While {
        cond,
        body: ast::Block::new(body),
    });
    Ok(out)
}

fn lower_repeat(
    region: &RepeatRegion,
    function: &SsaFunction,
    analysis: &DecompileAnalysis,
    names: &NameResolver<'_>,
    booleans: &BooleanAnalysis,
    builder: &mut StatementBuilder<'_>,
) -> Result<Vec<Stmt>, LuaError> {
    let body = lower_region(&region.body, function, analysis, names, booleans, builder)?;
    let cond = lower_condition(region.condition, function, analysis, booleans, builder)?;
    Ok(vec![Stmt::Repeat {
        body: ast::Block::new(body),
        cond,
    }])
}

fn lower_numeric_for(
    region: &NumericForRegion,
    function: &SsaFunction,
    analysis: &DecompileAnalysis,
    names: &NameResolver<'_>,
    booleans: &BooleanAnalysis,
    builder: &mut StatementBuilder<'_>,
) -> Result<Vec<Stmt>, LuaError> {
    let base = region.info.base;
    let prep_pc = node(function, region.info.prep_node).map_or(0, |node| node.pc);
    let setup_defs = [
        region.info.start_node,
        region.info.stop_node,
        region.info.step_node,
    ]
    .into_iter()
    .flatten()
    .filter_map(|id| node(function, id))
    .filter_map(|node| node.dest.reg_index().map(|reg| (reg, node.pc)))
    .collect::<Vec<_>>();
    let mut out = builder.emit_node_ids(region.prefix.nodes.iter().copied(), |node| {
        matches!(&node.op, SsaOp::ForPrep { base: loop_base, .. } if *loop_base == base)
            || node
                .dest
                .reg_index()
                .is_some_and(|reg| setup_defs.contains(&(reg, node.pc)))
    })?;
    let start = expr_for_optional_node(function, builder, region.info.start_node, base, prep_pc)?;
    let stop = expr_for_optional_node(
        function,
        builder,
        region.info.stop_node,
        base.saturating_add(1),
        prep_pc,
    )?;
    let step = expr_for_optional_node(
        function,
        builder,
        region.info.step_node,
        base.saturating_add(2),
        prep_pc,
    )?;
    let var = loop_var_name(
        names,
        base.saturating_add(3),
        function,
        region.info.body_start,
    );
    let body = lower_region(&region.body, function, analysis, names, booleans, builder)?;
    out.push(Stmt::NumericFor {
        var,
        start,
        stop,
        step: (!is_one(&step)).then_some(step),
        body: ast::Block::new(body),
    });
    Ok(out)
}

fn lower_generic_for(
    region: &GenericForRegion,
    function: &SsaFunction,
    analysis: &DecompileAnalysis,
    names: &NameResolver<'_>,
    booleans: &BooleanAnalysis,
    builder: &mut StatementBuilder<'_>,
) -> Result<Vec<Stmt>, LuaError> {
    let base = region.info.base;
    let skip_end = base.saturating_add(2 + region.info.count.max(0) as u16);
    let setup_range = generic_setup_pc_range(function, region.info.call_node, base, skip_end);
    let mut out = builder.emit_node_ids(region.prefix.nodes.iter().copied(), |node| {
        setup_range.is_some_and(|(start, end)| {
            node.pc >= start
                && node.pc <= end
                && (node
                    .dest
                    .reg_index()
                    .is_some_and(|reg| reg >= base && reg <= skip_end)
                    || matches!(&node.op, SsaOp::SetList { base: table_reg, .. } if *table_reg >= base && *table_reg <= skip_end))
        })
    })?;

    let exprs = generic_for_exprs(region, function, analysis, builder)?;
    let var_base = base.saturating_add(3);
    let var_names = (0..region.info.count.max(0))
        .map(|offset| {
            loop_var_name(
                names,
                var_base.saturating_add(u16::try_from(offset).unwrap_or(0)),
                function,
                region.info.body_start,
            )
        })
        .collect();
    let body = lower_region(&region.body, function, analysis, names, booleans, builder)?;
    out.push(Stmt::GenericFor {
        names: var_names,
        exprs,
        body: ast::Block::new(body),
    });
    Ok(out)
}

fn lower_condition(
    condition: Condition,
    function: &SsaFunction,
    _analysis: &DecompileAnalysis,
    booleans: &BooleanAnalysis,
    builder: &mut StatementBuilder<'_>,
) -> Result<Expr, LuaError> {
    if let Some(start) = condition.compound
        && let Some(chain) = booleans.condition_chain(start)
    {
        return builder.compound_condition(chain, condition.inverted);
    }
    let Some(node) = node(function, condition.branch) else {
        return Ok(Expr::True);
    };
    builder.condition_for_branch(node, condition.inverted)
}

fn append_phi_assignments(
    body: &mut Vec<Stmt>,
    phis: &[PhiSource],
    blocks: &[usize],
    builder: &mut StatementBuilder<'_>,
) -> Result<(), LuaError> {
    if matches!(body.last(), Some(Stmt::Return(_) | Stmt::Break)) {
        return Ok(());
    }
    for phi in phis {
        let Some(operand) = unique_phi_operand(phi, blocks) else {
            continue;
        };
        if let Some(stmt) = builder.phi_assignment(phi.dest, operand, phi.pc)? {
            body.push(stmt);
        }
    }
    Ok(())
}

fn unique_phi_operand(phi: &PhiSource, blocks: &[usize]) -> Option<SsaRef> {
    let mut result = None;
    for (block, operand) in &phi.sources {
        if !blocks.contains(block) {
            continue;
        }
        if result.is_some_and(|current| current != *operand) {
            return None;
        }
        result = Some(*operand);
    }
    result
}

fn force_phi_values(
    builder: &mut StatementBuilder<'_>,
    phis: &[PhiSource],
    exclude_reg: impl Fn(u16) -> bool,
) {
    for phi in phis {
        let Some(reg) = phi.dest.reg_index() else {
            continue;
        };
        if exclude_reg(reg) {
            continue;
        }
        builder.force_materialized(phi.dest);
        for (_, operand) in &phi.sources {
            builder.force_materialized(*operand);
        }
    }
}

fn force_region_loop_phi_values(
    region: &Region,
    function: &SsaFunction,
    builder: &mut StatementBuilder<'_>,
) {
    match region {
        Region::Sequence(regions) => {
            for region in regions {
                force_region_loop_phi_values(region, function, builder);
            }
        }
        Region::If(region) => {
            for arm in &region.arms {
                force_region_loop_phi_values(&arm.body, function, builder);
            }
            if let Some(else_) = &region.else_ {
                force_region_loop_phi_values(else_, function, builder);
            }
        }
        Region::While(region) => {
            let phis = conditionals::phi_sources(function, region.condition.branch.block);
            force_phi_values(builder, &phis, |_| false);
            force_region_loop_phi_values(&region.body, function, builder);
        }
        Region::Repeat(region) => {
            force_region_loop_phi_values(&region.body, function, builder);
        }
        Region::NumericFor(region) => {
            let base = region.info.base;
            let phis = conditionals::phi_sources(function, region.info.loop_block);
            force_phi_values(builder, &phis, |reg| {
                reg >= base && reg <= base.saturating_add(3)
            });
            force_region_loop_phi_values(&region.body, function, builder);
        }
        Region::GenericFor(region) => {
            let base = region.info.base;
            let skip_end = base.saturating_add(2 + region.info.count.max(0) as u16);
            let phis = conditionals::phi_sources(function, region.info.tfor_block);
            force_phi_values(builder, &phis, |reg| reg >= base && reg <= skip_end);
            force_region_loop_phi_values(&region.body, function, builder);
        }
        Region::Linear(_) | Region::Break => {}
    }
}

fn expr_for_optional_node(
    function: &SsaFunction,
    builder: &mut StatementBuilder<'_>,
    id: Option<NodeId>,
    reg: u16,
    pc: i32,
) -> Result<Expr, LuaError> {
    if let Some(node) = id.and_then(|id| node(function, id)) {
        return builder.expr_for_node(node);
    }
    builder.expr_for_ref(SsaRef::Reg { reg, ver: 0 }, pc)
}

fn generic_for_exprs(
    region: &GenericForRegion,
    function: &SsaFunction,
    analysis: &DecompileAnalysis,
    builder: &mut StatementBuilder<'_>,
) -> Result<Vec<Expr>, LuaError> {
    if let Some(call) = region.info.call_node.and_then(|id| node(function, id)) {
        if let SsaOp::Call { func, args, .. } = &call.op {
            let func = generic_setup_ref_expr(function, analysis, builder, *func, call.pc)?;
            let args = args
                .iter()
                .copied()
                .map(|arg| generic_setup_ref_expr(function, analysis, builder, arg, call.pc))
                .collect::<Result<Vec<_>, _>>()?;
            return Ok(vec![Expr::Call {
                func: Box::new(func),
                args,
                method: None,
            }]);
        }
        return Ok(vec![builder.expr_for_node(call)?]);
    }

    let base = region.info.base;
    let mut exprs = Vec::with_capacity(3);
    for offset in 0..3 {
        let reg = base.saturating_add(offset);
        let expr = if let Some(id) = generic_setup_def(function, region, reg)
            && let Some(node) = node(function, id)
        {
            builder.expr_for_node(node)?
        } else {
            builder.expr_for_ref(SsaRef::Reg { reg, ver: 0 }, 0)?
        };
        exprs.push(expr);
    }
    Ok(exprs)
}

fn generic_setup_ref_expr(
    function: &SsaFunction,
    analysis: &DecompileAnalysis,
    builder: &mut StatementBuilder<'_>,
    reference: SsaRef,
    pc: i32,
) -> Result<Expr, LuaError> {
    if let Some(id) = analysis.def_site(reference)
        && let Some(node) = node(function, id)
        && node.pc <= pc
    {
        return builder.expr_for_node(node);
    }
    builder.expr_for_ref(reference, pc)
}

fn generic_setup_def(
    function: &SsaFunction,
    region: &GenericForRegion,
    reg: u16,
) -> Option<NodeId> {
    let tfor = region.info.tfor_node;
    let start = region.info.entry.min(tfor.block);
    for block in (start..=tfor.block).rev() {
        let block_ref = function.blocks.get(block)?;
        let end = if block == tfor.block {
            tfor.node
        } else {
            block_ref.nodes.len()
        };
        if let Some((node, _)) = block_ref
            .nodes
            .iter()
            .take(end)
            .enumerate()
            .rev()
            .find(|(_, node)| node.dest.reg_index() == Some(reg))
        {
            return Some(NodeId { block, node });
        }
    }
    None
}

fn loop_var_name(names: &NameResolver<'_>, reg: u16, function: &SsaFunction, block: usize) -> Name {
    let pc = function
        .blocks
        .get(block)
        .map_or(0, |block| i32::try_from(block.start_pc).unwrap_or(0));
    names
        .binding_for_use(reg, pc)
        .or_else(|| names.binding_for_def(reg, pc))
        .map_or_else(
            || names.synthetic_reg_name(reg),
            |binding| names.name_for_binding_def(&binding, SsaRef::Reg { reg, ver: 0 }),
        )
}

fn generic_setup_pc_range(
    function: &SsaFunction,
    call_node: Option<NodeId>,
    base: u16,
    skip_end: u16,
) -> Option<(i32, i32)> {
    let call_id = call_node?;
    let block = function.blocks.get(call_id.block)?;
    let call = block.nodes.get(call_id.node)?;
    let mut start = call.pc;
    for node in block.nodes[..call_id.node].iter().rev() {
        let Some(reg) = node.dest.reg_index() else {
            break;
        };
        if reg < base || reg > skip_end {
            break;
        }
        start = node.pc;
    }
    Some((start, call.pc))
}

fn node(function: &SsaFunction, id: NodeId) -> Option<&SsaNode> {
    function
        .blocks
        .get(id.block)
        .and_then(|block| block.nodes.get(id.node))
}

fn branch_targets(function: &SsaFunction, id: NodeId) -> Option<(usize, usize)> {
    let SsaOp::Branch {
        t_true, t_false, ..
    } = &node(function, id)?.op
    else {
        return None;
    };
    Some((
        block_for_pc(function, *t_true)?,
        block_for_pc(function, *t_false)?,
    ))
}

fn block_for_pc(function: &SsaFunction, pc: i32) -> Option<usize> {
    let pc = usize::try_from(pc).ok()?;
    function
        .blocks
        .iter()
        .find(|block| pc >= block.start_pc && pc <= block.end_pc)
        .map(|block| block.index)
}

fn phi_bool_for_block(
    function: &SsaFunction,
    analysis: &DecompileAnalysis,
    phi: &PhiSource,
    block: usize,
) -> Option<bool> {
    let operand = phi_operand_for_block(phi, block)?;
    let node = analysis
        .def_site(operand)
        .and_then(|id| node(function, id))?;
    let SsaOp::LoadBool { value, .. } = &node.op else {
        return None;
    };
    Some(*value)
}

fn phi_operand_for_block(phi: &PhiSource, block: usize) -> Option<SsaRef> {
    phi.sources
        .iter()
        .find_map(|(source, operand)| (*source == block).then_some(*operand))
}

fn block_is_phi_value_only(
    function: &SsaFunction,
    analysis: &DecompileAnalysis,
    block: usize,
    operand: SsaRef,
) -> bool {
    let Some(block_ref) = function.blocks.get(block) else {
        return false;
    };
    let def_site = analysis.def_site(operand);
    block_ref
        .nodes
        .iter()
        .enumerate()
        .all(|(node_index, node)| {
            node.is_meta_only
                || Some(NodeId {
                    block,
                    node: node_index,
                }) == def_site
                || matches!(node.op, SsaOp::Nop | SsaOp::Jump { .. })
        })
}

fn phi_operand_expr(
    function: &SsaFunction,
    analysis: &DecompileAnalysis,
    builder: &mut StatementBuilder<'_>,
    operand: SsaRef,
) -> Result<Expr, LuaError> {
    let Some(def) = analysis.def_site(operand).and_then(|id| node(function, id)) else {
        return builder.expr_for_ref(operand, 0);
    };
    builder.expr_for_node(def)
}

fn is_one(expr: &Expr) -> bool {
    match expr {
        Expr::Integer(1) => true,
        Expr::Number(value) => (*value - 1.0).abs() < f64::EPSILON,
        _ => false,
    }
}

fn append_stmts(out: &mut Vec<Stmt>, stmts: Vec<Stmt>) {
    if matches!(out.last(), Some(Stmt::Return(_) | Stmt::Break)) {
        return;
    }
    out.extend(stmts);
}
