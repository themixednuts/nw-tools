use super::*;

impl<'a> ExprBuilder<'a> {
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

    pub(super) fn implicit_def_expr(
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

    pub(super) fn phi_expr(
        &mut self,
        operands: &[SsaRef],
        node: &SsaNode,
    ) -> Result<Expr, LuaError> {
        if let Some(plan) = self.booleans.value_for_phi(node.dest) {
            return self.expr_for_value_plan(plan);
        }
        if let Some(first) = operands.first().copied()
            && operands.iter().all(|operand| *operand == first)
        {
            return self.expr_for_ref(first, node.pc);
        }
        Ok(Expr::Name(self.names.name_for_ref(node.dest, node.pc)))
    }

    pub(super) fn concat_expr(&mut self, operands: &[SsaRef], pc: i32) -> Result<Expr, LuaError> {
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

    pub(super) fn const_expr(&self, idx: u32) -> Result<Expr, LuaError> {
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

    pub(super) fn global_expr(&self, idx: u32) -> Result<Expr, LuaError> {
        let expr = self.const_expr(idx)?;
        let Expr::Str(name) = expr else {
            return Err(LuaError::Malformed(format!(
                "global name constant {idx} is not a string"
            )));
        };
        Ok(global_expr_from_name(name))
    }

    pub(super) fn materialized_name(&self, reference: SsaRef) -> Option<Name> {
        let value = ValueId::from_ref(reference)?;
        self.materialized
            .get(usize::from(value.reg))
            .and_then(|versions| versions.get(usize::try_from(value.ver).ok()?))
            .cloned()
            .flatten()
    }
}
