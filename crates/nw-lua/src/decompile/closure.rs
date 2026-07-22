//! Closure and upvalue reconstruction.

use bstr::BString;

use crate::{
    LuaError,
    bytecode::OpcodeTable,
    chunk::Proto,
    decompile::ast::{Expr, FuncBody, Name, Stmt},
    ir::{self, SsaFunction, SsaNode, SsaOp, UpvalueCapture},
};

use super::{
    decompile_proto_with_names,
    naming::{NameResolver, is_valid_identifier},
    stmt_build::StatementBuilder,
};

/// Build an anonymous function expression for a `CLOSURE` node.
pub(crate) fn function_expr(
    parent_proto: &Proto,
    parent_function: &SsaFunction,
    table: &OpcodeTable,
    parent_names: &NameResolver<'_>,
    node: &SsaNode,
) -> Result<Expr, LuaError> {
    Ok(Expr::Function(function_body(
        parent_proto,
        parent_function,
        table,
        parent_names,
        node,
    )?))
}

/// Emit a local-function statement when a closure initializes a local slot.
pub(crate) fn try_local_function(
    builder: &mut StatementBuilder<'_>,
    node: &SsaNode,
) -> Result<Option<Stmt>, LuaError> {
    let Some(reg) = node.dest.reg_index() else {
        return Ok(None);
    };
    let pc = builder.materialization_pc(node.dest).unwrap_or(node.pc);
    let Some(binding) = builder.binding_for_def(reg, pc) else {
        return Ok(None);
    };
    if !builder.claim_declaration(node.dest) {
        return Ok(None);
    }

    let body = function_body(
        builder.proto(),
        builder.function(),
        builder.table(),
        builder.names(),
        node,
    )?;
    let name = builder.name_for_binding_def(&binding, node.dest);
    builder.activate(node.dest);
    Ok(Some(Stmt::Function {
        name,
        body,
        local: true,
    }))
}

fn function_body(
    parent_proto: &Proto,
    parent_function: &SsaFunction,
    table: &OpcodeTable,
    parent_names: &NameResolver<'_>,
    node: &SsaNode,
) -> Result<FuncBody, LuaError> {
    let SsaOp::Closure { proto, upvalues } = &node.op else {
        return Err(LuaError::Unsupported(
            "attempted to build a function from a non-closure node".to_string(),
        ));
    };
    let proto_idx = usize::try_from(*proto)
        .map_err(|_| LuaError::Malformed(format!("closure proto index {proto} is invalid")))?;
    let Some(sub_proto) = parent_proto.protos.get(proto_idx) else {
        return Err(LuaError::Malformed(format!(
            "closure proto index {proto_idx} out of range"
        )));
    };
    if upvalues.len() != usize::from(sub_proto.nups) {
        return Err(LuaError::Malformed(format!(
            "closure at pc {} has {} upvalue bindings, expected {}",
            node.pc,
            upvalues.len(),
            sub_proto.nups
        )));
    }

    let upvalue_names = resolve_upvalue_names(parent_function, parent_names, node, upvalues);
    let param_overrides = parameter_overrides(sub_proto, &upvalue_names);
    let sub_ssa = ir::build_ssa(sub_proto, table);
    let sub_names = NameResolver::with_overrides(
        sub_proto,
        &sub_ssa,
        parent_names.child_function_id(proto_idx),
        upvalue_names,
        param_overrides,
    );
    let body = decompile_proto_with_names(sub_proto, &sub_ssa, table, &sub_names)?;
    let params = (0..sub_proto.num_params)
        .map(|reg| sub_names.parameter_name(reg))
        .collect::<Vec<_>>();

    Ok(FuncBody::new(params, is_vararg(sub_proto), body))
}

fn resolve_upvalue_names(
    _parent_function: &SsaFunction,
    parent_names: &NameResolver<'_>,
    node: &SsaNode,
    upvalues: &[UpvalueCapture],
) -> Vec<Name> {
    let binding_pc = closure_binding_pc(node);
    upvalues
        .iter()
        .map(|capture| match capture {
            UpvalueCapture::ParentLocal(reference) => reference
                .reg_index()
                .and_then(|reg| parent_names.binding_for_def(reg, binding_pc))
                .map_or_else(
                    || parent_names.collapsed_name_for_ref(*reference, binding_pc),
                    |binding| parent_names.name_for_binding_def(&binding, *reference),
                ),
            UpvalueCapture::ParentUpvalue(upvalue) => parent_names.upvalue_name(*upvalue),
        })
        .enumerate()
        .map(|(index, name)| {
            if is_valid_identifier(&name.0) {
                name
            } else {
                Name::synthetic(BString::from(format!("up{index}")))
            }
        })
        .collect()
}

fn closure_binding_pc(node: &SsaNode) -> i32 {
    let SsaOp::Closure { upvalues, .. } = &node.op else {
        return node.pc;
    };
    node.pc
        .saturating_add(i32::try_from(upvalues.len()).unwrap_or(i32::MAX))
}

fn parameter_overrides(sub_proto: &Proto, upvalue_names: &[Name]) -> Vec<Option<Name>> {
    (0..sub_proto.num_params)
        .map(|reg| {
            let name = default_parameter_name(sub_proto, reg);
            upvalue_names
                .iter()
                .any(|upvalue| upvalue == &name)
                .then(|| Name::from(format!("arg{reg}")))
        })
        .collect()
}

fn default_parameter_name(sub_proto: &Proto, reg: u8) -> Name {
    let index = usize::from(reg);
    if let Some(loc) = sub_proto.loc_vars.get(index)
        && is_valid_identifier(&loc.name)
    {
        return Name::new(loc.name.clone());
    }
    Name::from(format!("a{reg}"))
}

fn is_vararg(proto: &Proto) -> bool {
    proto.is_vararg & 2 != 0
}
