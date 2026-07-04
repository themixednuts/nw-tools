use nw_lua::decompile;

const NUMERIC_FOR: &[u8] = include_bytes!("fixtures/control_flow/numeric_for.luac");
const WHILE_LOOP: &[u8] = include_bytes!("fixtures/control_flow/while.luac");
const REPEAT_LOOP: &[u8] = include_bytes!("fixtures/control_flow/repeat.luac");
const IF_ELSE_PHI: &[u8] = include_bytes!("fixtures/control_flow/if_else_phi.luac");
const IF_ELSEIF_ELSE: &[u8] = include_bytes!("fixtures/control_flow/if_elseif_else.luac");
const GENERIC_FOR: &[u8] = include_bytes!("fixtures/control_flow/generic_for.luac");
const NESTED_FOR_IF: &[u8] = include_bytes!("fixtures/control_flow/nested_for_if.luac");

#[test]
fn control_flow_fixtures_decompile_and_reparse() {
    let cases = [
        ("numeric_for", NUMERIC_FOR, &["for", "do", "end"][..]),
        ("while", WHILE_LOOP, &["while", "do", "end"][..]),
        ("repeat", REPEAT_LOOP, &["repeat", "until"][..]),
        ("if_else_phi", IF_ELSE_PHI, &["if", "else", "end"][..]),
        (
            "if_elseif_else",
            IF_ELSEIF_ELSE,
            &["if", "elseif", "else", "end"][..],
        ),
        ("generic_for", GENERIC_FOR, &["for", "in", "do", "end"][..]),
        ("nested_for_if", NESTED_FOR_IF, &["for", "if", "end"][..]),
    ];

    for (name, bytes, keywords) in cases {
        let source = decompile(bytes).unwrap_or_else(|error| panic!("{name}: {error}"));
        full_moon::parse(&source).unwrap_or_else(|errors| {
            panic!("{name}: emitted source did not parse:\n{source}\n{errors:#?}")
        });
        for keyword in keywords {
            assert!(
                source.contains(keyword),
                "{name}: expected keyword {keyword:?} in\n{source}"
            );
        }
    }
}

#[test]
fn if_else_phi_declares_once_and_assigns_in_both_arms() {
    let source = decompile(IF_ELSE_PHI).expect("decompile succeeds");
    assert_eq!(source.matches("local r").count(), 1, "{source}");
    assert!(source.contains("r = 1"), "{source}");
    assert!(source.contains("r = 2"), "{source}");
    assert!(source.contains("return r"), "{source}");
}
