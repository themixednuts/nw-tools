use super::*;

impl<'a> StatementBuilder<'a> {
    pub(super) fn emit_node(
        &mut self,
        node_id: NodeId,
        node: &SsaNode,
    ) -> Result<Option<Stmt>, LuaError> {
        match &node.op {
            SsaOp::Nop | SsaOp::Jump { .. } | SsaOp::Close { .. } => Ok(None),
            SsaOp::Phi { .. } => self.emit_boolean_phi(node),
            SsaOp::Move { .. }
            | SsaOp::LoadK { .. }
            | SsaOp::LoadBool { .. }
            | SsaOp::LoadNil { .. }
            | SsaOp::GetUpval { .. }
            | SsaOp::GetGlobal { .. }
            | SsaOp::GetTable { .. }
            | SsaOp::NewTable { .. }
            | SsaOp::SelfOp { .. }
            | SsaOp::BinOp { .. }
            | SsaOp::UnOp { .. }
            | SsaOp::Concat { .. } => self.emit_value_def(node),
            SsaOp::Closure { .. } => self.emit_closure_value_def(node),
            SsaOp::SetGlobal { src, idx } => {
                let target = self.global_target(*idx)?;
                let value = self.exprs.expr_for_ref(*src, node.pc)?;
                Ok(Some(assign_one(target, value)))
            }
            SsaOp::SetUpval { src, upval } => {
                let target = Expr::Name(self.names.upvalue_name(*upval));
                let value = self.exprs.expr_for_ref(*src, node.pc)?;
                Ok(Some(assign_one(target, value)))
            }
            SsaOp::SetTable { table, key, value } => {
                if self.is_inline_constructor_mutation(node) {
                    return Ok(None);
                }
                let table = self.exprs.expr_for_ref(*table, node.pc)?;
                let key = self.exprs.expr_for_ref(*key, node.pc)?;
                let target = index_expr(table, key);
                let value = self.exprs.expr_for_ref(*value, node.pc)?;
                Ok(Some(assign_one(target, value)))
            }
            SsaOp::Call {
                func,
                args,
                return_count,
                arg_count,
                ..
            } => self.emit_call(node_id, node, *func, args, *arg_count, *return_count),
            SsaOp::TailCall {
                func,
                args,
                arg_count,
                ..
            } => {
                let call = self.call_expr(*func, args, *arg_count, node.pc)?;
                Ok(Some(Stmt::Return(vec![call])))
            }
            SsaOp::Return { values, count, .. } => self.emit_return(node, values, *count),
            SsaOp::VarArg { .. } => self.emit_vararg(node_id, node),
            SsaOp::Branch { .. }
            | SsaOp::ForPrep { .. }
            | SsaOp::ForLoop { .. }
            | SsaOp::TForLoop { .. } => Ok(None),
            SsaOp::SetList {
                table,
                values,
                base,
                count,
                batch,
            } => {
                if self.is_inline_constructor_mutation(node) {
                    return Ok(None);
                }
                self.emit_setlist_fallback(node, *table, values, *base, *count, *batch)
            }
        }
    }

    pub(super) fn emit_setlist_fallback(
        &mut self,
        node: &SsaNode,
        table: SsaRef,
        values: &[SsaRef],
        base: u16,
        count: i32,
        batch: i32,
    ) -> Result<Option<Stmt>, LuaError> {
        if count == 0 {
            return self.emit_open_setlist_fallback(node, table, values, base, batch);
        }
        if values.is_empty() {
            return Ok(None);
        }
        if batch <= 0 {
            return Err(LuaError::Unsupported(format!(
                "invalid SETLIST batch {batch} at pc={}",
                node.pc
            )));
        }

        let table_expr = self.exprs.expr_for_ref(table, node.pc)?;
        if !is_stable_assignment_target(&table_expr) {
            return Err(LuaError::Unsupported(format!(
                "SETLIST fallback needs a materialized table target (pc={} base=R{base})",
                node.pc
            )));
        }

        let first_index = (i64::from(batch) - 1) * LFIELDS_PER_FLUSH + 1;
        let mut targets = Vec::with_capacity(values.len());
        let mut rhs = Vec::with_capacity(values.len());
        let last_index = values.len().saturating_sub(1);
        for (offset, value) in values.iter().copied().enumerate() {
            let offset = i64::try_from(offset).map_err(|_| {
                LuaError::Malformed("SETLIST offset does not fit in i64".to_string())
            })?;
            targets.push(index_expr(
                table_expr.clone(),
                Expr::Integer(first_index + offset),
            ));
            let value_expr = if usize::try_from(offset).ok() == Some(last_index) {
                self.exprs.expr_for_fixed_last_ref(value, node.pc)?
            } else {
                self.exprs.expr_for_ref(value, node.pc)?
            };
            rhs.push(value_expr);
        }

        Ok(Some(Stmt::Assign {
            targets,
            values: rhs,
        }))
    }

    pub(super) fn emit_open_setlist_fallback(
        &mut self,
        node: &SsaNode,
        table: SsaRef,
        values: &[SsaRef],
        base: u16,
        batch: i32,
    ) -> Result<Option<Stmt>, LuaError> {
        if values.is_empty() {
            return Ok(None);
        }
        if batch <= 0 {
            return Err(LuaError::Unsupported(format!(
                "invalid SETLIST batch {batch} at pc={}",
                node.pc
            )));
        }

        let table_expr = self.exprs.expr_for_ref(table, node.pc)?;
        if !is_stable_assignment_target(&table_expr) {
            return Err(LuaError::Unsupported(format!(
                "SETLIST fallback needs a materialized table target (pc={} base=R{base})",
                node.pc
            )));
        }

        let first_index = (i64::from(batch) - 1) * LFIELDS_PER_FLUSH + 1;
        let pack_name = Name::from(format!("__nw_lua_pack_{}", node.pc));
        let values_name = Name::from(format!("__nw_lua_values_{}", node.pc));
        let index_name = Name::from(format!("__nw_lua_index_{}", node.pc));

        let pack_function = Stmt::Function {
            name: pack_name.clone(),
            local: true,
            body: FuncBody::new(
                Vec::new(),
                true,
                ast::Block::new(vec![Stmt::Return(vec![Expr::Table(vec![
                    TableField::Named {
                        name: Name::from("n"),
                        value: Expr::Call {
                            func: Box::new(Expr::Global(BString::from("select"))),
                            args: vec![Expr::Str(BString::from("#")), Expr::VarArg],
                            method: None,
                        },
                    },
                    TableField::List(Expr::VarArg),
                ])])]),
            ),
        };

        let last_index = values.len().saturating_sub(1);
        let args = values
            .iter()
            .copied()
            .enumerate()
            .map(|(index, value)| {
                if index == last_index {
                    self.exprs.expr_for_ref(value, node.pc)
                } else {
                    self.exprs.expr_for_fixed_last_ref(value, node.pc)
                }
            })
            .collect::<Result<Vec<_>, _>>()?;
        let capture_values = Stmt::Local {
            names: vec![values_name.clone()],
            attribs: Vec::new(),
            values: vec![Expr::Call {
                func: Box::new(Expr::Name(pack_name)),
                args,
                method: None,
            }],
        };

        let target_index = if first_index == 1 {
            Expr::Name(index_name.clone())
        } else {
            Expr::Binary {
                op: ast::BinOp::Add,
                lhs: Box::new(Expr::Integer(first_index - 1)),
                rhs: Box::new(Expr::Name(index_name.clone())),
            }
        };
        let copy_loop = Stmt::NumericFor {
            var: index_name.clone(),
            start: Expr::Integer(1),
            stop: Expr::Field {
                obj: Box::new(Expr::Name(values_name.clone())),
                name: Name::from("n"),
            },
            step: None,
            body: ast::Block::new(vec![Stmt::Assign {
                targets: vec![index_expr(table_expr, target_index)],
                values: vec![index_expr(Expr::Name(values_name), Expr::Name(index_name))],
            }]),
        };

        Ok(Some(Stmt::Do(ast::Block::new(vec![
            pack_function,
            capture_values,
            copy_loop,
        ]))))
    }

    pub(super) fn emit_call(
        &mut self,
        node_id: NodeId,
        node: &SsaNode,
        func: SsaRef,
        args: &[SsaRef],
        arg_count: i32,
        return_count: i32,
    ) -> Result<Option<Stmt>, LuaError> {
        let call = self.call_expr(func, args, arg_count, node.pc)?;

        match return_count {
            0 => {
                if self.analysis.real_use_count(node.dest) > 0 {
                    return Ok(None);
                }
                Ok(Some(Stmt::Call(call)))
            }
            1 => Ok(Some(Stmt::Call(call))),
            2 => {
                if self.should_materialize(node) {
                    Ok(Some(self.materialize_value(node.dest, node.pc, call)))
                } else if self.analysis.real_use_count(node.dest) == 0 {
                    Ok(Some(Stmt::Call(call)))
                } else {
                    Ok(None)
                }
            }
            _ => multi::call_results::fixed_call_assignment(
                self,
                node_id,
                node,
                func,
                args,
                arg_count,
                return_count,
            )
            .map(Some),
        }
    }

    pub(super) fn emit_return(
        &mut self,
        node: &SsaNode,
        values: &[SsaRef],
        count: i32,
    ) -> Result<Option<Stmt>, LuaError> {
        if count == 1 && usize::try_from(node.pc).ok() == self.proto.code.len().checked_sub(1) {
            return Ok(None);
        }
        let fixed_last = count != 0;
        let last_index = values.len().saturating_sub(1);
        let values = values
            .iter()
            .copied()
            .enumerate()
            .map(|(index, value)| {
                if fixed_last && index == last_index {
                    self.expr_for_fixed_last_ref(value, node.pc)
                } else {
                    self.exprs.expr_for_ref(value, node.pc)
                }
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Some(Stmt::Return(values)))
    }

    pub(super) fn emit_vararg(
        &mut self,
        node_id: NodeId,
        node: &SsaNode,
    ) -> Result<Option<Stmt>, LuaError> {
        if let SsaOp::VarArg { count, .. } = &node.op
            && *count >= 3
        {
            return multi::vararg::fixed_vararg_assignment(self, node_id, node, *count).map(Some);
        }
        if let SsaOp::VarArg { count: 0, .. } = &node.op
            && self.analysis.real_use_count(node.dest) > 0
        {
            return Ok(None);
        }
        if self.should_materialize(node) {
            return Ok(Some(self.materialize_value(
                node.dest,
                node.pc,
                Expr::VarArg,
            )));
        }
        Ok(None)
    }

    pub(super) fn global_target(&self, idx: u32) -> Result<Expr, LuaError> {
        Ok(global_expr_from_name(self.string_constant(idx)?))
    }

    pub(super) fn string_constant(&self, idx: u32) -> Result<BString, LuaError> {
        let idx = usize::try_from(idx)
            .map_err(|_| LuaError::Malformed("constant index does not fit in usize".to_string()))?;
        let Some(Constant::Str(value)) = self.proto.constants.get(idx) else {
            return Err(LuaError::Malformed(format!(
                "constant index {idx} is not a string"
            )));
        };
        Ok(value.clone())
    }
}
