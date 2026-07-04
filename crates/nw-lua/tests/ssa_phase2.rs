use std::collections::VecDeque;

use nw_lua::{
    bytecode::OpcodeTable,
    chunk::Proto,
    ir::{SsaFunction, SsaOp, build_ssa, dump::dump_function},
    parse_chunk, ssa_dump,
    version::LuaVersion,
};

const SHOPCOMMON: &[u8] = include_bytes!("fixtures/shopcommon.luac");

#[test]
fn builds_ssa_for_all_shopcommon_protos_with_well_formed_cfgs() {
    let chunk = parse_chunk(SHOPCOMMON).expect("shopcommon chunk parses");
    let table = lua51_table();
    let mut proto_count = 0;
    let mut phi_count = 0;

    visit_protos(&chunk.root, &mut |proto| {
        let function = build_ssa(proto, &table);
        assert_well_formed_cfg(&function);
        for block in &function.blocks {
            for node in &block.nodes {
                if matches!(node.op, SsaOp::Phi { .. }) {
                    assert!(
                        block.preds.len() >= 2,
                        "phi in BB{} with fewer than two predecessors",
                        block.index
                    );
                    phi_count += 1;
                }
            }
        }
        proto_count += 1;
    });

    assert!(proto_count > 1);
    assert!(phi_count > 0);
}

#[test]
fn ssa_dump_for_shopcommon_is_deterministic() {
    let first = ssa_dump(SHOPCOMMON).expect("first SSA dump succeeds");
    let second = ssa_dump(SHOPCOMMON).expect("second SSA dump succeeds");

    assert_eq!(first, second);
    assert!(first.contains("-- SSA Dump --"));
    assert!(first.contains("== ssa function"));
}

#[test]
fn first_nested_proto_ssa_dump_matches_snapshot() {
    let chunk = parse_chunk(SHOPCOMMON).expect("shopcommon chunk parses");
    let table = lua51_table();
    let nested = chunk.root.protos.first().expect("fixture has nested proto");
    let function = build_ssa(nested, &table);
    let dump = dump_function(&function);

    let expected = r#"== ssa function (?):59..64 ==
   params=1 is_vararg=0 maxstack=4 blocks=1
BB0 [pc 0..14] preds:[] succs:[] idom=-1
  [   0] GETGLOBAL R1_1 := G[K0]
  [   1] MOVE R2_1 := R0_0
  [   2] CALL R1_2 := base=R1 argc=2 retc=1 func=R1_1 args:[R2_1]
  [   3] GETGLOBAL R1_3 := G[K1]
  [   4] GETTABLE R1_4 := R1_3[K2]
  [   5] GETTABLE R1_5 := R1_4[K3]
  [   6] LOADK R2_2 := K4
  [   7] MOVE R3_1 := R0_0
  [   8] CALL R1_6 := base=R1 argc=3 retc=1 func=R1_5 args:[R2_2, R3_1]
  [   9] GETGLOBAL R1_7 := G[K5]
  [  10] GETTABLE R1_8 := R1_7[K2]
  [  11] GETTABLE R1_9 := R1_8[K6]
  [  12] LOADK R2_3 := K7
  [  13] CALL R1_10 := base=R1 argc=2 retc=1 func=R1_9 args:[R2_3]
  [  14] RETURN base=R0 count=1 values:[]

"#;
    assert_eq!(dump, expected);
}

fn lua51_table() -> OpcodeTable {
    OpcodeTable::builtin(LuaVersion::V51).expect("Lua 5.1 table exists")
}

fn visit_protos(proto: &Proto, visitor: &mut impl FnMut(&Proto)) {
    visitor(proto);
    for nested in &proto.protos {
        visit_protos(nested, visitor);
    }
}

fn assert_well_formed_cfg(function: &SsaFunction) {
    if function.blocks.is_empty() {
        return;
    }

    assert_eq!(function.blocks[0].index, 0);
    for block in &function.blocks {
        if block.index != 0 {
            assert!(
                !block.preds.is_empty(),
                "function {}..{} BB{} [pc {}..{}] has no predecessors; blocks={:?}",
                function.line_defined,
                function.last_line_defined,
                block.index,
                block.start_pc,
                block.end_pc,
                function
                    .blocks
                    .iter()
                    .map(|block| (
                        block.index,
                        block.start_pc,
                        block.end_pc,
                        &block.preds,
                        &block.succs
                    ))
                    .collect::<Vec<_>>()
            );
            assert!(
                block.idom.is_some(),
                "BB{} has no immediate dominator",
                block.index
            );
        }
        for &succ in &block.succs {
            assert!(succ < function.blocks.len());
            assert!(
                function.blocks[succ].preds.contains(&block.index),
                "BB{} -> BB{} missing reverse pred",
                block.index,
                succ
            );
        }
        for &pred in &block.preds {
            assert!(pred < function.blocks.len());
            assert!(
                function.blocks[pred].succs.contains(&block.index),
                "BB{} <- BB{} missing reverse succ",
                block.index,
                pred
            );
        }
    }

    let mut seen = vec![false; function.blocks.len()];
    let mut queue = VecDeque::from([0]);
    seen[0] = true;
    while let Some(block) = queue.pop_front() {
        for &succ in &function.blocks[block].succs {
            if !seen[succ] {
                seen[succ] = true;
                queue.push_back(succ);
            }
        }
    }
    assert!(
        seen.iter().all(|reachable| *reachable),
        "all blocks should be reachable from entry"
    );
}
