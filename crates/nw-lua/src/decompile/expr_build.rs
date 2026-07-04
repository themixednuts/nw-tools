//! SSA value to compact expression reconstruction for Phase 4.

use bstr::BString;

use crate::{
    LuaError,
    bytecode::OpcodeTable,
    chunk::{Constant, Proto},
    decompile::ast::{self, Expr, Name},
    ir::{self, SsaFunction, SsaNode, SsaOp, SsaRef},
};

use super::{
    analysis::{DecompileAnalysis, NodeId, ValueId, for_each_use, node_has_observable_side_effect},
    boolean::normalize,
    closure, multi,
    naming::{NameResolver, is_valid_identifier},
};

/// Builds expressions while remembering values materialized by statement
/// reconstruction.
#[derive(Debug)]
pub struct ExprBuilder<'a> {
    proto: &'a Proto,
    function: &'a SsaFunction,
    table: &'a OpcodeTable,
    analysis: &'a DecompileAnalysis,
    names: &'a NameResolver<'a>,
    materialized: Vec<Vec<Option<Name>>>,
    visiting: Vec<Vec<bool>>,
}

impl<'a> ExprBuilder<'a> {
    #[must_use]
    pub fn new(
        proto: &'a Proto,
        function: &'a SsaFunction,
        table: &'a OpcodeTable,
        analysis: &'a DecompileAnalysis,
        names: &'a NameResolver<'a>,
    ) -> Self {
        Self {
            proto,
            function,
            table,
            analysis,
            names,
            materialized: vec![Vec::new(); function.num_regs],
            visiting: vec![Vec::new(); function.num_regs],
        }
    }

    /// Remember that a value was emitted as a statement and must be referenced
    /// by name afterward.
    pub fn mark_materialized(&mut self, reference: SsaRef, name: Name) {
        let Some(value) = ValueId::from_ref(reference) else {
            return;
        };
        let reg = usize::from(value.reg);
        let Ok(version) = usize::try_from(value.ver) else {
            return;
        };
        if reg >= self.materialized.len() {
            self.materialized.resize_with(reg + 1, Vec::new);
        }
        if version >= self.materialized[reg].len() {
            self.materialized[reg].resize(version + 1, None);
        }
        self.materialized[reg][version] = Some(name);
    }

    /// Return whether a value should be inlined at its use site.
    #[must_use]
    pub fn can_inline_ref(&self, reference: SsaRef, use_pc: i32) -> bool {
        let Some(node_id) = self.analysis.def_site(reference) else {
            return false;
        };
        let Some(node) = self.analysis.node(self.function, node_id) else {
            return false;
        };

        if self.can_inline_table_constructor(reference, node) {
            return true;
        }
        if self.can_inline_new_table(reference, node) {
            return true;
        }
        self.analysis.use_count(reference) == 1
            && self.analysis.real_use_count(reference) == 1
            && is_inlineable_def(&node.op)
            && !self.is_stable_named_def(node)
            && !matches!(&node.op, SsaOp::NewTable { .. } | SsaOp::Phi { .. })
            && self.inline_preserves_order(reference, node_id, node, use_pc)
    }

    /// Convert an SSA reference to an expression.
    pub fn expr_for_ref(&mut self, reference: SsaRef, use_pc: i32) -> Result<Expr, LuaError> {
        match reference {
            SsaRef::None => Ok(Expr::Nil),
            SsaRef::Const(idx) => self.const_expr(idx),
            SsaRef::Reg { .. } => self.reg_expr(reference, use_pc),
        }
    }

    /// Convert a defining node to the expression it computes.
    pub fn node_expr(&mut self, node: &SsaNode) -> Result<Expr, LuaError> {
        match &node.op {
            SsaOp::Move { src } => self.expr_for_ref(*src, node.pc),
            SsaOp::LoadK { idx } => self.const_expr(*idx),
            SsaOp::LoadBool { value, .. } => Ok(if *value { Expr::True } else { Expr::False }),
            SsaOp::LoadNil { .. } => Ok(Expr::Nil),
            SsaOp::GetUpval { upval } => Ok(Expr::Name(self.names.upvalue_name(*upval))),
            SsaOp::GetGlobal { idx } => self.global_expr(*idx),
            SsaOp::GetTable { table, key } => {
                let obj = self.expr_for_ref(*table, node.pc)?;
                let key = self.expr_for_ref(*key, node.pc)?;
                Ok(index_expr(obj, key))
            }
            SsaOp::NewTable { .. } => self.table_constructor_expr(node),
            SsaOp::SelfOp { table, key, .. } => {
                let obj = self.expr_for_ref(*table, node.pc)?;
                let key = self.expr_for_ref(*key, node.pc)?;
                Ok(index_expr(obj, key))
            }
            SsaOp::BinOp { op, left, right } => Ok(Expr::Binary {
                op: map_bin_op(*op),
                lhs: Box::new(self.expr_for_ref(*left, node.pc)?),
                rhs: Box::new(self.expr_for_ref(*right, node.pc)?),
            }),
            SsaOp::UnOp { op, value } => Ok(Expr::Unary {
                op: map_un_op(*op),
                operand: Box::new(self.expr_for_ref(*value, node.pc)?),
            }),
            SsaOp::Concat { operands } => self.concat_expr(operands, node.pc),
            SsaOp::Branch {
                rel, a, b, invert, ..
            } => self.branch_expr(*rel, *a, *b, *invert, node.pc),
            SsaOp::Call {
                func,
                args,
                arg_count,
                ..
            }
            | SsaOp::TailCall {
                func,
                args,
                arg_count,
                ..
            } => self.call_expr_with_arg_count(*func, args, *arg_count, node.pc),
            SsaOp::Phi { operands, .. } => self.phi_expr(operands, node),
            SsaOp::Closure { .. } => {
                closure::function_expr(self.proto, self.function, self.table, self.names, node)
            }
            SsaOp::VarArg { .. } => Ok(Expr::VarArg),
            SsaOp::Nop
            | SsaOp::SetGlobal { .. }
            | SsaOp::SetUpval { .. }
            | SsaOp::SetTable { .. }
            | SsaOp::Jump { .. }
            | SsaOp::ForPrep { .. }
            | SsaOp::ForLoop { .. }
            | SsaOp::TForLoop { .. }
            | SsaOp::SetList { .. }
            | SsaOp::Close { .. }
            | SsaOp::Return { .. } => {
                if matches!(node.dest, SsaRef::Reg { .. }) {
                    Ok(Expr::Name(self.names.name_for_ref(node.dest, node.pc)))
                } else {
                    Err(LuaError::Unsupported(format!(
                        "cannot use {:?} as an expression in Phase 4",
                        node.op
                    )))
                }
            }
        }
    }

    /// Build a call expression from already-versioned call parts.
    pub fn call_expr(&mut self, func: SsaRef, args: &[SsaRef], pc: i32) -> Result<Expr, LuaError> {
        let arg_count = i32::try_from(args.len() + 1).unwrap_or(i32::MAX);
        self.call_expr_with_arg_count(func, args, arg_count, pc)
    }

    /// Build a call expression, preserving open vs fixed argument semantics.
    pub fn call_expr_with_arg_count(
        &mut self,
        func: SsaRef,
        args: &[SsaRef],
        arg_count: i32,
        pc: i32,
    ) -> Result<Expr, LuaError> {
        if let Some((receiver, method)) = self.method_receiver(func, pc)? {
            let call_args = args.iter().skip(1).copied().collect::<Vec<_>>();
            let args = self.call_args_exprs(&call_args, arg_count, pc)?;
            return Ok(Expr::Call {
                func: Box::new(receiver),
                args,
                method: Some(method),
            });
        }

        let func = self.expr_for_ref(func, pc)?;
        let args = self.call_args_exprs(args, arg_count, pc)?;
        Ok(Expr::Call {
            func: Box::new(func),
            args,
            method: None,
        })
    }

    /// Convert a value in a last-position fixed context to exactly one value.
    pub fn expr_for_fixed_last_ref(
        &mut self,
        reference: SsaRef,
        use_pc: i32,
    ) -> Result<Expr, LuaError> {
        let expr = self.expr_for_ref(reference, use_pc)?;
        if self.last_position_needs_adjustment(reference) {
            Ok(Expr::Paren(Box::new(expr)))
        } else {
            Ok(expr)
        }
    }

    fn call_args_exprs(
        &mut self,
        args: &[SsaRef],
        arg_count: i32,
        pc: i32,
    ) -> Result<Vec<Expr>, LuaError> {
        let fixed_count = arg_count != 0;
        let last_index = args.len().saturating_sub(1);
        args.iter()
            .copied()
            .enumerate()
            .map(|(index, arg)| {
                if fixed_count && index == last_index {
                    self.expr_for_fixed_last_ref(arg, pc)
                } else {
                    self.expr_for_ref(arg, pc)
                }
            })
            .collect()
    }

    fn reg_expr(&mut self, reference: SsaRef, use_pc: i32) -> Result<Expr, LuaError> {
        if let Some(name) = self.materialized_name(reference) {
            return Ok(Expr::Name(name));
        }

        let Some(node_id) = self.analysis.def_site(reference) else {
            return Ok(Expr::Name(self.names.name_for_ref(reference, use_pc)));
        };
        let Some(node) = self.analysis.node(self.function, node_id) else {
            return Ok(Expr::Name(self.names.name_for_ref(reference, use_pc)));
        };

        if node.dest != reference {
            return self.implicit_def_expr(reference, node, use_pc);
        }

        if !self.can_inline_ref(reference, use_pc) {
            return Ok(Expr::Name(self.names.name_for_ref(reference, use_pc)));
        }

        if self.is_visiting(reference) {
            return Ok(Expr::Name(self.names.name_for_ref(reference, use_pc)));
        }

        self.set_visiting(reference, true);
        let result = self.node_expr(node);
        self.set_visiting(reference, false);
        result
    }

    fn implicit_def_expr(
        &mut self,
        reference: SsaRef,
        node: &SsaNode,
        use_pc: i32,
    ) -> Result<Expr, LuaError> {
        if let SsaOp::SelfOp {
            table, self_reg, ..
        } = &node.op
            && reference.reg_index() == Some(*self_reg)
        {
            return self.expr_for_ref(*table, use_pc);
        }
        Ok(Expr::Name(self.names.name_for_ref(reference, use_pc)))
    }

    fn method_receiver(&mut self, func: SsaRef, pc: i32) -> Result<Option<(Expr, Name)>, LuaError> {
        let Some(node_id) = self.analysis.def_site(func) else {
            return Ok(None);
        };
        let Some(node) = self.analysis.node(self.function, node_id) else {
            return Ok(None);
        };
        let SsaOp::SelfOp { table, key, .. } = &node.op else {
            return Ok(None);
        };
        let key_expr = self.expr_for_ref(*key, pc)?;
        let Some(method) = ident_from_string_expr(&key_expr) else {
            return Ok(None);
        };
        let receiver = self.expr_for_ref(*table, pc)?;
        Ok(Some((receiver, method)))
    }

    fn phi_expr(&mut self, operands: &[SsaRef], node: &SsaNode) -> Result<Expr, LuaError> {
        if let Some(first) = operands.first().copied()
            && operands.iter().all(|operand| *operand == first)
        {
            return self.expr_for_ref(first, node.pc);
        }
        Ok(Expr::Name(self.names.name_for_ref(node.dest, node.pc)))
    }

    fn concat_expr(&mut self, operands: &[SsaRef], pc: i32) -> Result<Expr, LuaError> {
        let Some((&last, rest)) = operands.split_last() else {
            return Ok(Expr::Str(BString::from(Vec::new())));
        };
        let mut expr = self.expr_for_ref(last, pc)?;
        for operand in rest.iter().rev() {
            expr = Expr::Binary {
                op: ast::BinOp::Concat,
                lhs: Box::new(self.expr_for_ref(*operand, pc)?),
                rhs: Box::new(expr),
            };
        }
        Ok(expr)
    }

    fn table_constructor_expr(&mut self, node: &SsaNode) -> Result<Expr, LuaError> {
        let Some(table_reg) = node.dest.reg_index() else {
            return Ok(Expr::Table(Vec::new()));
        };
        let plan = if self.can_inline_table_constructor(node.dest, node) {
            self.constructor_plan(node, table_reg)
        } else {
            ConstructorPlan::default()
        };
        let mut expr_for_ref = |reference, pc, mode| match mode {
            multi::table_list::ConstructorValueMode::Normal => self.expr_for_ref(reference, pc),
            multi::table_list::ConstructorValueMode::FixedLast => {
                self.expr_for_fixed_last_ref(reference, pc)
            }
        };
        let fields = multi::table_list::fields_from_nodes(
            plan.setlists.iter(),
            plan.keyed.iter(),
            &mut expr_for_ref,
        )?;
        Ok(Expr::Table(fields))
    }

    fn node_position(&self, needle: &SsaNode) -> Option<(usize, usize)> {
        self.function
            .blocks
            .iter()
            .enumerate()
            .find_map(|(block_index, block)| {
                block
                    .nodes
                    .iter()
                    .position(|node| node.pc == needle.pc && node.dest == needle.dest)
                    .map(|node_index| (block_index, node_index))
            })
    }

    fn branch_expr(
        &mut self,
        rel: ir::RelOp,
        a: SsaRef,
        b: SsaRef,
        invert: bool,
        pc: i32,
    ) -> Result<Expr, LuaError> {
        let op = match (rel, invert) {
            (ir::RelOp::Eq, false) => ast::BinOp::Eq,
            (ir::RelOp::Eq, true) => ast::BinOp::Ne,
            (ir::RelOp::Lt, false) => ast::BinOp::Lt,
            (ir::RelOp::Lt, true) => ast::BinOp::Ge,
            (ir::RelOp::Le, false) => ast::BinOp::Le,
            (ir::RelOp::Le, true) => ast::BinOp::Gt,
            (ir::RelOp::Test | ir::RelOp::TestSet, false) => {
                return self.expr_for_ref(a, pc);
            }
            (ir::RelOp::Test | ir::RelOp::TestSet, true) => {
                return Ok(Expr::Unary {
                    op: ast::UnOp::Not,
                    operand: Box::new(self.expr_for_ref(a, pc)?),
                });
            }
        };
        Ok(normalize::normalize(Expr::Binary {
            op,
            lhs: Box::new(self.expr_for_ref(a, pc)?),
            rhs: Box::new(self.expr_for_ref(b, pc)?),
        }))
    }

    fn const_expr(&self, idx: u32) -> Result<Expr, LuaError> {
        let idx = usize::try_from(idx)
            .map_err(|_| LuaError::Malformed("constant index does not fit in usize".to_string()))?;
        let Some(constant) = self.proto.constants.get(idx) else {
            return Err(LuaError::Malformed(format!(
                "constant index {idx} out of range"
            )));
        };
        Ok(match constant {
            Constant::Nil => Expr::Nil,
            Constant::Boolean(value) => {
                if *value {
                    Expr::True
                } else {
                    Expr::False
                }
            }
            Constant::Number(value) => Expr::Number(*value),
            Constant::Integer(value) => Expr::Integer(*value),
            Constant::Str(value) => Expr::Str(value.clone()),
        })
    }

    fn global_expr(&self, idx: u32) -> Result<Expr, LuaError> {
        let expr = self.const_expr(idx)?;
        let Expr::Str(name) = expr else {
            return Err(LuaError::Malformed(format!(
                "global name constant {idx} is not a string"
            )));
        };
        Ok(global_expr_from_name(name))
    }

    fn materialized_name(&self, reference: SsaRef) -> Option<Name> {
        let value = ValueId::from_ref(reference)?;
        self.materialized
            .get(usize::from(value.reg))
            .and_then(|versions| versions.get(usize::try_from(value.ver).ok()?))
            .cloned()
            .flatten()
    }

    fn can_inline_new_table(&self, reference: SsaRef, node: &SsaNode) -> bool {
        matches!(&node.op, SsaOp::NewTable { .. })
            && self.analysis.use_count(reference) == 1
            && !self.analysis.has_mutating_table_use(reference)
            && !self.is_stable_named_def(node)
    }

    fn can_inline_table_constructor(&self, reference: SsaRef, node: &SsaNode) -> bool {
        if !matches!(&node.op, SsaOp::NewTable { .. }) || self.is_stable_named_def(node) {
            return false;
        }
        let Some(table_reg) = node.dest.reg_index() else {
            return false;
        };
        let plan = self.constructor_plan(node, table_reg);
        plan.mutation_count > 0
            && plan.final_use.is_some()
            && self.analysis.facts(reference).mutating_table_uses == plan.mutation_count
            && self.analysis.real_use_count(reference) == plan.mutation_count + 1
    }

    fn constructor_plan(&self, node: &SsaNode, table_reg: u16) -> ConstructorPlan {
        let Some((block, node_index)) = self.node_position(node) else {
            return ConstructorPlan::default();
        };

        let mut plan = ConstructorPlan::default();
        for (offset, current) in self.function.blocks[block]
            .nodes
            .iter()
            .skip(node_index + 1)
            .enumerate()
        {
            if current.is_meta_only {
                break;
            }
            let current_id = NodeId {
                block,
                node: node_index + offset + 1,
            };
            if multi::table_list::is_matching_setlist(current, node.dest, table_reg) {
                plan.setlists.push(current.clone());
                plan.mutation_count += 1;
                continue;
            }
            if multi::table_list::is_matching_settable(current, node.dest, table_reg) {
                plan.keyed.push(current.clone());
                plan.mutation_count += 1;
                continue;
            }
            if op_uses_ref(&current.op, node.dest) {
                plan.final_use = Some(current_id);
                break;
            }
            if multi::table_list::is_constructor_setup(current, table_reg) {
                if node_has_observable_side_effect(&current.op) {
                    return ConstructorPlan::default();
                }
                continue;
            }
            break;
        }
        plan
    }

    fn inline_preserves_order(
        &self,
        reference: SsaRef,
        def_id: NodeId,
        node: &SsaNode,
        _use_pc: i32,
    ) -> bool {
        let Some(use_id) = self.analysis.single_real_use(reference) else {
            return false;
        };
        let Some(use_node) = self.analysis.node(self.function, use_id) else {
            return false;
        };
        if self.dependencies_redefined_before_use(&node.op, node.pc, use_node.pc) {
            return false;
        }
        if is_pure_def(&node.op) || !self.analysis.has_side_effect_between(def_id, use_id) {
            return true;
        }
        self.use_preserves_intervening_effects(reference, def_id, use_id, use_node)
    }

    fn dependencies_redefined_before_use(&self, op: &SsaOp, def_pc: i32, use_pc: i32) -> bool {
        let mut redefined = false;
        for_each_use(op, |reference| {
            if let Some(reg) = reference.reg_index()
                && self.analysis.has_later_def_before(reg, def_pc, use_pc)
            {
                redefined = true;
            }
        });
        redefined
    }

    fn use_preserves_intervening_effects(
        &self,
        reference: SsaRef,
        def_id: NodeId,
        use_id: NodeId,
        use_node: &SsaNode,
    ) -> bool {
        if def_id.block != use_id.block || def_id.node >= use_id.node {
            return false;
        }
        let eval_refs = direct_eval_order_refs(&use_node.op);
        let Some(reference_index) = eval_refs.iter().position(|operand| *operand == reference)
        else {
            return false;
        };
        let block = &self.function.blocks[def_id.block];
        for current in &block.nodes[def_id.node + 1..use_id.node] {
            if !node_has_observable_side_effect(&current.op) {
                continue;
            }
            let Some(effect_index) = current.dest.reg_index().and_then(|_| {
                eval_refs
                    .iter()
                    .position(|operand| *operand == current.dest)
            }) else {
                let Some(table) = constructor_mutation_table(&current.op) else {
                    return false;
                };
                let Some(table_index) = eval_refs.iter().position(|operand| *operand == table)
                else {
                    return false;
                };
                if table_index <= reference_index {
                    return false;
                }
                continue;
            };
            if effect_index <= reference_index {
                return false;
            }
        }
        true
    }

    fn is_stable_named_def(&self, node: &SsaNode) -> bool {
        let Some(reg) = node.dest.reg_index() else {
            return false;
        };
        let Some(binding) = self.names.binding_for_def(reg, node.pc) else {
            return false;
        };
        if self.value_used_only_before_binding(node.dest, binding.start_pc) {
            return false;
        }
        !self
            .analysis
            .has_later_def_before(reg, node.pc, binding.start_pc)
    }

    fn value_used_only_before_binding(&self, reference: SsaRef, binding_start_pc: i32) -> bool {
        let uses = self.analysis.real_uses(reference);
        !uses.is_empty()
            && uses.iter().all(|use_id| {
                self.analysis
                    .node(self.function, *use_id)
                    .is_some_and(|use_node| use_node.pc < binding_start_pc)
            })
    }

    fn last_position_needs_adjustment(&self, reference: SsaRef) -> bool {
        let Some(node_id) = self.analysis.def_site(reference) else {
            return false;
        };
        let Some(node) = self.analysis.node(self.function, node_id) else {
            return false;
        };
        matches!(
            node.op,
            SsaOp::Call {
                return_count: 2,
                ..
            } | SsaOp::VarArg { count: 2, .. }
        )
    }

    fn is_visiting(&self, reference: SsaRef) -> bool {
        let Some(value) = ValueId::from_ref(reference) else {
            return false;
        };
        self.visiting
            .get(usize::from(value.reg))
            .and_then(|versions| versions.get(usize::try_from(value.ver).ok()?))
            .copied()
            .unwrap_or(false)
    }

    fn set_visiting(&mut self, reference: SsaRef, visiting: bool) {
        let Some(value) = ValueId::from_ref(reference) else {
            return;
        };
        let reg = usize::from(value.reg);
        let Ok(version) = usize::try_from(value.ver) else {
            return;
        };
        if reg >= self.visiting.len() {
            self.visiting.resize_with(reg + 1, Vec::new);
        }
        if version >= self.visiting[reg].len() {
            self.visiting[reg].resize(version + 1, false);
        }
        self.visiting[reg][version] = visiting;
    }
}

#[derive(Debug, Default)]
struct ConstructorPlan {
    setlists: Vec<SsaNode>,
    keyed: Vec<SsaNode>,
    mutation_count: usize,
    final_use: Option<NodeId>,
}

fn is_inlineable_def(op: &SsaOp) -> bool {
    matches!(
        op,
        SsaOp::Move { .. }
            | SsaOp::LoadK { .. }
            | SsaOp::LoadBool { .. }
            | SsaOp::LoadNil { .. }
            | SsaOp::GetUpval { .. }
            | SsaOp::GetGlobal { .. }
            | SsaOp::GetTable { .. }
            | SsaOp::SelfOp { .. }
            | SsaOp::BinOp { .. }
            | SsaOp::UnOp { .. }
            | SsaOp::Concat { .. }
            | SsaOp::Call { .. }
            | SsaOp::Closure { .. }
            | SsaOp::VarArg { .. }
    )
}

fn is_pure_def(op: &SsaOp) -> bool {
    matches!(
        op,
        SsaOp::Move { .. }
            | SsaOp::LoadK { .. }
            | SsaOp::LoadBool { .. }
            | SsaOp::LoadNil { .. }
            | SsaOp::Closure { .. }
            | SsaOp::VarArg { .. }
    )
}

fn op_uses_ref(op: &SsaOp, needle: SsaRef) -> bool {
    let mut found = false;
    for_each_use(op, |reference| {
        if reference == needle {
            found = true;
        }
    });
    found
}

fn constructor_mutation_table(op: &SsaOp) -> Option<SsaRef> {
    match op {
        SsaOp::SetTable { table, .. } | SsaOp::SetList { table, .. } => Some(*table),
        _ => None,
    }
}

fn direct_eval_order_refs(op: &SsaOp) -> Vec<SsaRef> {
    match op {
        SsaOp::Move { src } | SsaOp::UnOp { value: src, .. } => vec![*src],
        SsaOp::GetTable { table, key } => vec![*table, *key],
        SsaOp::SetGlobal { src, .. } | SsaOp::SetUpval { src, .. } => vec![*src],
        SsaOp::SetTable { table, key, value } => vec![*table, *key, *value],
        SsaOp::SelfOp { table, key, .. } => vec![*table, *key],
        SsaOp::BinOp { left, right, .. } => vec![*left, *right],
        SsaOp::Concat { operands } => operands.clone(),
        SsaOp::Branch { a, b, .. } => vec![*a, *b],
        SsaOp::Call { func, args, .. } | SsaOp::TailCall { func, args, .. } => {
            let mut refs = Vec::with_capacity(args.len() + 1);
            refs.push(*func);
            refs.extend(args.iter().copied());
            refs
        }
        SsaOp::Return { values, .. } | SsaOp::SetList { values, .. } => values.clone(),
        SsaOp::Phi { operands, .. } => operands.clone(),
        SsaOp::Nop
        | SsaOp::LoadK { .. }
        | SsaOp::LoadBool { .. }
        | SsaOp::LoadNil { .. }
        | SsaOp::GetUpval { .. }
        | SsaOp::GetGlobal { .. }
        | SsaOp::NewTable { .. }
        | SsaOp::Jump { .. }
        | SsaOp::ForPrep { .. }
        | SsaOp::ForLoop { .. }
        | SsaOp::TForLoop { .. }
        | SsaOp::Close { .. }
        | SsaOp::Closure { .. }
        | SsaOp::VarArg { .. } => Vec::new(),
    }
}

fn map_bin_op(op: ir::BinOp) -> ast::BinOp {
    match op {
        ir::BinOp::Add => ast::BinOp::Add,
        ir::BinOp::Sub => ast::BinOp::Sub,
        ir::BinOp::Mul => ast::BinOp::Mul,
        ir::BinOp::Div => ast::BinOp::Div,
        ir::BinOp::Mod => ast::BinOp::Mod,
        ir::BinOp::Pow => ast::BinOp::Pow,
        ir::BinOp::IDiv => ast::BinOp::IDiv,
        ir::BinOp::BAnd => ast::BinOp::BAnd,
        ir::BinOp::BOr => ast::BinOp::BOr,
        ir::BinOp::BXor => ast::BinOp::BXor,
        ir::BinOp::Shl => ast::BinOp::Shl,
        ir::BinOp::Shr => ast::BinOp::Shr,
    }
}

fn map_un_op(op: ir::UnOp) -> ast::UnOp {
    match op {
        ir::UnOp::Neg => ast::UnOp::Neg,
        ir::UnOp::Not => ast::UnOp::Not,
        ir::UnOp::Len => ast::UnOp::Len,
        ir::UnOp::BNot => ast::UnOp::BNot,
    }
}

pub(crate) fn index_expr(obj: Expr, key: Expr) -> Expr {
    if let Some(name) = ident_from_string_expr(&key) {
        Expr::Field {
            obj: Box::new(obj),
            name,
        }
    } else {
        Expr::Index {
            obj: Box::new(obj),
            key: Box::new(key),
        }
    }
}

pub(crate) fn global_expr_from_name(name: BString) -> Expr {
    if is_valid_identifier(&name) {
        Expr::Global(name)
    } else {
        Expr::Index {
            obj: Box::new(Expr::Global(BString::from("_G"))),
            key: Box::new(Expr::Str(name)),
        }
    }
}

pub(crate) fn ident_from_string_expr(expr: &Expr) -> Option<Name> {
    let Expr::Str(bytes) = expr else {
        return None;
    };
    is_valid_identifier(bytes).then(|| Name::new(bytes.clone()))
}

#[cfg(test)]
mod tests {
    use bstr::BString;

    use super::*;
    use crate::{
        bytecode::OpcodeTable,
        chunk::Proto,
        ir::{BasicBlock, SsaFunction, SsaOp, SsaRef, dom},
        version::LuaVersion,
    };

    #[test]
    fn single_use_temp_inlines_into_expression() {
        let proto = proto_with_constants(vec![Constant::Number(1.0), Constant::Number(2.0)]);
        let function = function_with_nodes(vec![
            SsaNode::with_dest(0, -1, reg(0, 1), SsaOp::LoadK { idx: 0 }),
            SsaNode::with_dest(
                1,
                -1,
                reg(1, 1),
                SsaOp::BinOp {
                    op: ir::BinOp::Add,
                    left: reg(0, 1),
                    right: SsaRef::Const(1),
                },
            ),
            SsaNode::new(
                2,
                -1,
                SsaOp::Return {
                    values: vec![reg(1, 1)],
                    base: 1,
                    count: 2,
                },
            ),
        ]);
        let analysis = super::super::analysis::analyze(&function);
        let names = NameResolver::new(&proto, &function);
        let table = OpcodeTable::builtin(LuaVersion::V51).expect("Lua 5.1 opcode table");
        let mut builder = ExprBuilder::new(&proto, &function, &table, &analysis, &names);

        assert!(builder.can_inline_ref(reg(0, 1), 1));
        let expr = builder.expr_for_ref(reg(1, 1), 1).expect("expr builds");

        assert_eq!(
            expr,
            Expr::Binary {
                op: ast::BinOp::Add,
                lhs: Box::new(Expr::Number(1.0)),
                rhs: Box::new(Expr::Number(2.0)),
            }
        );
    }

    #[test]
    fn multi_use_temp_materializes_as_name() {
        let proto = proto_with_constants(vec![Constant::Number(1.0), Constant::Number(2.0)]);
        let function = function_with_nodes(vec![
            SsaNode::with_dest(0, -1, reg(0, 1), SsaOp::LoadK { idx: 0 }),
            SsaNode::with_dest(
                1,
                -1,
                reg(1, 1),
                SsaOp::BinOp {
                    op: ir::BinOp::Add,
                    left: reg(0, 1),
                    right: SsaRef::Const(1),
                },
            ),
            SsaNode::with_dest(
                2,
                -1,
                reg(2, 1),
                SsaOp::BinOp {
                    op: ir::BinOp::Mul,
                    left: reg(0, 1),
                    right: SsaRef::Const(1),
                },
            ),
        ]);
        let analysis = super::super::analysis::analyze(&function);
        let names = NameResolver::new(&proto, &function);
        let table = OpcodeTable::builtin(LuaVersion::V51).expect("Lua 5.1 opcode table");
        let mut builder = ExprBuilder::new(&proto, &function, &table, &analysis, &names);

        assert!(!builder.can_inline_ref(reg(0, 1), 1));
        builder.mark_materialized(reg(0, 1), Name::from("v0"));

        assert_eq!(
            builder.expr_for_ref(reg(0, 1), 1).expect("expr builds"),
            Expr::Name(Name::from("v0"))
        );
    }

    fn reg(reg: u16, ver: u32) -> SsaRef {
        SsaRef::Reg { reg, ver }
    }

    fn proto_with_constants(constants: Vec<Constant>) -> Proto {
        Proto {
            source: BString::from(Vec::new()),
            line_defined: 0,
            last_line_defined: 0,
            code: Vec::new(),
            line_info: Vec::new(),
            constants,
            upvalues: Vec::new(),
            protos: Vec::new(),
            loc_vars: Vec::new(),
            nups: 0,
            max_stack: 4,
            num_params: 0,
            is_vararg: 0,
            version: LuaVersion::V51,
        }
    }

    fn function_with_nodes(nodes: Vec<SsaNode>) -> SsaFunction {
        let mut block = BasicBlock::new(0, 0, nodes.len().saturating_sub(1));
        block.nodes = nodes;
        SsaFunction {
            source: BString::from(Vec::new()),
            line_defined: 0,
            last_line_defined: 0,
            version: LuaVersion::V51,
            num_params: 0,
            is_vararg: 0,
            max_stack: 4,
            num_regs: 4,
            instructions: Vec::new(),
            blocks: vec![block],
            dom: dom::DomInfo {
                idom: Vec::new(),
                dom_children: Vec::new(),
                dominance_frontiers: Vec::new(),
            },
            def_sites: crate::ir::ssa::DefSites {
                blocks_by_reg: Vec::new(),
                defines: Vec::new(),
                use_before_def: Vec::new(),
                live_in: Vec::new(),
            },
            implicit_defs: Vec::new(),
        }
    }
}
