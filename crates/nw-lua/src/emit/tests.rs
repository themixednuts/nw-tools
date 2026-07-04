use bstr::BString;

use crate::decompile::ast::{BinOp, Block, Expr, FuncBody, Name, Stmt, TableField, UnOp};
use crate::to_source;

fn assert_emit(block: Block, expected: &str) {
    let emitted = to_source(&block).expect("emit succeeds");
    assert_eq!(emitted, expected);
    assert!(full_moon::parse(&emitted).is_ok());
}

fn block(stmts: Vec<Stmt>) -> Block {
    Block::new(stmts)
}

fn name(value: &str) -> Name {
    Name::from(value)
}

fn var(value: &str) -> Expr {
    Expr::Name(name(value))
}

fn int(value: i64) -> Expr {
    Expr::Integer(value)
}

fn call(function: Expr, args: Vec<Expr>) -> Expr {
    Expr::Call {
        func: Box::new(function),
        args,
        method: None,
    }
}

fn method_call(receiver: Expr, method: &str, args: Vec<Expr>) -> Expr {
    Expr::Call {
        func: Box::new(receiver),
        args,
        method: Some(name(method)),
    }
}

fn local(name: &str, values: Vec<Expr>) -> Stmt {
    Stmt::Local {
        names: vec![self::name(name)],
        attribs: Vec::new(),
        values,
    }
}

fn assign(targets: Vec<Expr>, values: Vec<Expr>) -> Stmt {
    Stmt::Assign { targets, values }
}

fn bin(op: BinOp, lhs: Expr, rhs: Expr) -> Expr {
    Expr::Binary {
        op,
        lhs: Box::new(lhs),
        rhs: Box::new(rhs),
    }
}

fn unary(op: UnOp, operand: Expr) -> Expr {
    Expr::Unary {
        op,
        operand: Box::new(operand),
    }
}

fn expr_local(expr: Expr) -> Block {
    block(vec![local("y", vec![expr])])
}

#[test]
fn emits_local_call() {
    assert_emit(
        block(vec![local(
            "x",
            vec![call(var("f"), vec![var("a"), var("b")])],
        )]),
        "local x = f(a, b)\n",
    );
}

#[test]
fn emits_if_elseif_else() {
    assert_emit(
        block(vec![Stmt::If {
            arms: vec![
                (
                    var("c"),
                    block(vec![local(
                        "x",
                        vec![call(var("f"), vec![var("a"), var("b")])],
                    )]),
                ),
                (
                    var("d"),
                    block(vec![local(
                        "x",
                        vec![call(var("f"), vec![var("d"), var("b")])],
                    )]),
                ),
            ],
            else_: Some(block(vec![local(
                "x",
                vec![call(var("f"), vec![var("c"), var("b")])],
            )])),
        }]),
        concat!(
            "if c then\n",
            "\tlocal x = f(a, b)\n",
            "elseif d then\n",
            "\tlocal x = f(d, b)\n",
            "else\n",
            "\tlocal x = f(c, b)\n",
            "end\n",
        ),
    );
}

#[test]
fn emits_numeric_for() {
    assert_emit(
        block(vec![Stmt::NumericFor {
            var: name("i"),
            start: int(1),
            stop: var("n"),
            step: None,
            body: block(vec![local(
                "x",
                vec![call(var("f"), vec![var("i"), var("n")])],
            )]),
        }]),
        concat!("for i = 1, n do\n", "\tlocal x = f(i, n)\n", "end\n",),
    );
}

#[test]
fn emits_local_function_and_call() {
    assert_emit(
        block(vec![
            Stmt::Function {
                name: name("g"),
                body: FuncBody::new(
                    vec![name("a"), name("b")],
                    false,
                    block(vec![Stmt::Return(vec![bin(
                        BinOp::Add,
                        var("a"),
                        var("b"),
                    )])]),
                ),
                local: true,
            },
            Stmt::Call(call(var("g"), vec![int(1), int(2)])),
        ]),
        concat!(
            "local function g(a, b)\n",
            "\treturn a + b\n",
            "end\n",
            "g(1, 2)\n",
        ),
    );
}

#[test]
fn emits_short_circuit_local() {
    assert_emit(
        expr_local(bin(
            BinOp::Or,
            bin(BinOp::And, var("a"), var("b")),
            var("c"),
        )),
        "local y = a and b or c\n",
    );
}

#[test]
fn emits_multiple_assignment() {
    assert_emit(
        block(vec![assign(
            vec![var("a"), var("b")],
            vec![var("b"), var("a")],
        )]),
        "a, b = b, a\n",
    );
}

#[test]
fn emits_while_and_repeat() {
    assert_emit(
        block(vec![
            Stmt::While {
                cond: var("a"),
                body: block(vec![assign(
                    vec![var("x")],
                    vec![bin(BinOp::Add, var("x"), int(1))],
                )]),
            },
            Stmt::Repeat {
                body: block(vec![assign(
                    vec![var("x")],
                    vec![bin(BinOp::Sub, var("x"), int(1))],
                )]),
                cond: bin(BinOp::Eq, var("x"), int(0)),
            },
        ]),
        concat!(
            "while a do\n",
            "\tx = x + 1\n",
            "end\n",
            "repeat\n",
            "\tx = x - 1\n",
            "until x == 0\n",
        ),
    );
}

#[test]
fn emits_generic_for() {
    assert_emit(
        block(vec![Stmt::GenericFor {
            names: vec![name("k"), name("v")],
            exprs: vec![call(var("pairs"), vec![var("t")])],
            body: block(vec![Stmt::Call(call(
                var("print"),
                vec![var("k"), var("v")],
            ))]),
        }]),
        concat!("for k, v in pairs(t) do\n", "\tprint(k, v)\n", "end\n",),
    );
}

#[test]
fn emits_method_call() {
    assert_emit(
        block(vec![Stmt::Call(method_call(
            var("obj"),
            "method",
            vec![var("x")],
        ))]),
        "obj:method(x)\n",
    );
}

#[test]
fn emits_table_constructor() {
    assert_emit(
        block(vec![local(
            "t",
            vec![Expr::Table(vec![
                TableField::List(int(1)),
                TableField::List(int(2)),
                TableField::Named {
                    name: name("x"),
                    value: int(3),
                },
                TableField::ExprKey {
                    key: var("k"),
                    value: var("v"),
                },
            ])],
        )]),
        "local t = { 1, 2, x = 3, [k] = v }\n",
    );
}

#[test]
fn emits_nested_if() {
    assert_emit(
        block(vec![Stmt::If {
            arms: vec![(
                var("a"),
                block(vec![Stmt::If {
                    arms: vec![(var("b"), block(vec![Stmt::Return(vec![var("c")])]))],
                    else_: None,
                }]),
            )],
            else_: None,
        }]),
        concat!(
            "if a then\n",
            "\tif b then\n",
            "\t\treturn c\n",
            "\tend\n",
            "end\n",
        ),
    );
}

#[test]
fn precedence_add_mul() {
    assert_emit(
        expr_local(bin(
            BinOp::Add,
            var("a"),
            bin(BinOp::Mul, var("b"), var("c")),
        )),
        "local y = a + b * c\n",
    );
}

#[test]
fn precedence_parens_add_before_mul() {
    assert_emit(
        expr_local(bin(
            BinOp::Mul,
            bin(BinOp::Add, var("a"), var("b")),
            var("c"),
        )),
        "local y = (a + b) * c\n",
    );
}

#[test]
fn precedence_unary_power() {
    assert_emit(
        expr_local(unary(UnOp::Neg, bin(BinOp::Pow, var("x"), int(2)))),
        "local y = -x ^ 2\n",
    );
}

#[test]
fn precedence_power_unary_rhs() {
    assert_emit(
        expr_local(bin(BinOp::Pow, int(2), unary(UnOp::Neg, var("x")))),
        "local y = 2 ^ -x\n",
    );
}

#[test]
fn precedence_concat_right_assoc() {
    assert_emit(
        expr_local(bin(
            BinOp::Concat,
            var("a"),
            bin(BinOp::Concat, var("b"), var("c")),
        )),
        "local y = a .. b .. c\n",
    );
}

#[test]
fn precedence_not_comparison() {
    assert_emit(
        expr_local(unary(UnOp::Not, bin(BinOp::Eq, var("a"), var("b")))),
        "local y = not (a == b)\n",
    );
}

#[test]
fn precedence_and_or() {
    assert_emit(
        expr_local(bin(
            BinOp::Or,
            bin(BinOp::And, var("a"), var("b")),
            var("c"),
        )),
        "local y = a and b or c\n",
    );
}

#[test]
fn precedence_or_before_and() {
    assert_emit(
        expr_local(bin(
            BinOp::And,
            bin(BinOp::Or, var("a"), var("b")),
            var("c"),
        )),
        "local y = (a or b) and c\n",
    );
}

#[test]
fn emits_strings_from_bytes() {
    assert_emit(
        block(vec![local(
            "s",
            vec![Expr::Str(BString::from(vec![b'a', b'\n', 0xff]))],
        )]),
        "local s = \"a\\n\\255\"\n",
    );
}
