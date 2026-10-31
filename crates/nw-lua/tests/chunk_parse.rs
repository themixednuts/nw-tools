use nw_lua::{
    LuaError,
    chunk::{Constant, Proto},
    parse_chunk,
    version::LuaTarget,
};

const SHOPCOMMON: &[u8] = include_bytes!("fixtures/shopcommon.luac");

#[test]
fn parses_shopcommon_lua_51_chunk() {
    let chunk = parse_chunk(SHOPCOMMON).expect("shopcommon chunk parses");

    assert_eq!(chunk.header.version, LuaTarget::V51);
    assert_eq!(chunk.header.format, 0);
    assert_eq!(chunk.header.instruction_size, 4);
    assert!(!chunk.root.code.is_empty());
    assert!(!chunk.root.constants.is_empty());
    assert!(count_nested_protos(&chunk.root) > 0);
    assert!(chunk.root.max_stack > 0);
    assert!(chunk.root.num_params <= chunk.root.max_stack);

    let mut string_constants = 0;
    visit_protos(&chunk.root, &mut |proto| {
        assert!(proto.max_stack > 0);
        assert!(proto.num_params <= proto.max_stack);
        for constant in &proto.constants {
            match constant {
                Constant::Nil | Constant::Boolean(_) | Constant::Number(_) => {}
                Constant::Integer(_) => {
                    panic!("Lua 5.1 chunks should not contain integer constants")
                }
                Constant::Str(bytes) => {
                    string_constants += 1;
                    let _ = bytes.as_slice();
                }
            }
        }
    });
    assert!(string_constants > 0);
}

#[test]
fn skips_the_script_asset_prefix_on_game_chunks() {
    let prefixed = [&[0x04, 0x00][..], SHOPCOMMON].concat();

    assert_eq!(
        parse_chunk(&prefixed).expect("prefixed chunk parses"),
        parse_chunk(SHOPCOMMON).expect("bare chunk parses"),
    );
}

#[test]
fn rejects_other_bytes_before_the_chunk() {
    for prefix in [[0x04, 0x01], [0x03, 0x00], [0x00, 0x00]] {
        let bytes = [&prefix[..], SHOPCOMMON].concat();
        assert!(matches!(parse_chunk(&bytes), Err(LuaError::BadMagic)));
    }
    assert!(matches!(
        parse_chunk(&[0x04, 0x00, 0x00, 0x00, 0x00, 0x00]),
        Err(LuaError::BadMagic)
    ));
}

fn count_nested_protos(proto: &Proto) -> usize {
    proto.protos.len() + proto.protos.iter().map(count_nested_protos).sum::<usize>()
}

fn visit_protos(proto: &Proto, visitor: &mut impl FnMut(&Proto)) {
    visitor(proto);
    for nested in &proto.protos {
        visit_protos(nested, visitor);
    }
}
