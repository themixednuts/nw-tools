use super::*;
use crate::decompile::reconstruction::ReconstructionPlan;
use crate::{
    bytecode::OpcodeTable,
    chunk::Proto,
    ir::{BasicBlock, SsaFunction, SsaOp, SsaRef, dom},
    version::LuaTarget,
};
use bstr::BString;

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
    let table = OpcodeTable::builtin(LuaTarget::V51);
    let booleans = BooleanAnalysis::empty();
    let plan = ReconstructionPlan::build(
        &proto, &function, &table, &analysis, &names, &booleans, None,
    );
    let mut builder = ExprBuilder::new(
        &proto, &function, &table, &analysis, &names, &booleans, &plan,
    );

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
    let table = OpcodeTable::builtin(LuaTarget::V51);
    let booleans = BooleanAnalysis::empty();
    let plan = ReconstructionPlan::build(
        &proto, &function, &table, &analysis, &names, &booleans, None,
    );
    let mut builder = ExprBuilder::new(
        &proto, &function, &table, &analysis, &names, &booleans, &plan,
    );

    assert!(!builder.can_inline_ref(reg(0, 1), 1));
    assert_eq!(
        builder.expr_for_ref(reg(0, 1), 1).expect("expr builds"),
        Expr::Name(Name::from("l0"))
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
        version: LuaTarget::V51,
    }
}

fn function_with_nodes(nodes: Vec<SsaNode>) -> SsaFunction {
    let mut block = BasicBlock::new(0, 0, nodes.len().saturating_sub(1));
    block.nodes = nodes;
    SsaFunction {
        source: BString::from(Vec::new()),
        line_defined: 0,
        last_line_defined: 0,
        version: LuaTarget::V51,
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
    }
}
