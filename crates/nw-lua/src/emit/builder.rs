use bstr::BString;
use full_moon::ast::lua52::{Goto, Label};
use full_moon::ast::lua54::Attribute;
use full_moon::ast::punctuated::{Pair, Punctuated};
use full_moon::ast::span::ContainedSpan;
use full_moon::ast::{
    Assignment, BinOp, Block, Call, Do, ElseIf, Expression, Field, FunctionArgs, FunctionBody,
    FunctionCall, FunctionDeclaration, FunctionName as MoonFunctionName, GenericFor, If, Index,
    LastStmt, LocalAssignment, LocalFunction, MethodCall, NumericFor, Parameter, Prefix, Repeat,
    Return, Stmt, Suffix, TableConstructor, UnOp, Var, VarExpression, While,
};
use full_moon::tokenizer::{StringLiteralQuoteType, Token, TokenReference, TokenType};

use crate::decompile::ast::{Attrib, FunctionName, Name};

pub(super) fn block(stmts: Vec<Stmt>, last_stmt: Option<LastStmt>) -> Block {
    let stmt_count = stmts.len();
    let stmts = stmts
        .into_iter()
        .enumerate()
        .map(|(index, stmt)| {
            let needs_newline = index + 1 < stmt_count || last_stmt.is_some();
            (stmt, needs_newline.then(newline))
        })
        .collect();

    Block::new()
        .with_stmts(stmts)
        .with_last_stmt(last_stmt.map(|stmt| (stmt, None)))
}

pub(super) fn local_assign(
    names: Vec<TokenReference>,
    attribs: Vec<Option<Attribute>>,
    values: Vec<Expression>,
) -> Stmt {
    let mut stmt = LocalAssignment::new(punctuated(names, ", "));

    if !attribs.is_empty() {
        stmt = stmt.with_attributes(attribs);
    }

    if !values.is_empty() {
        stmt = stmt
            .with_equal_token(Some(symbol(" = ")))
            .with_expressions(punctuated(values, ", "));
    }

    Stmt::LocalAssignment(stmt)
}

pub(super) fn assign(vars: Vec<Var>, values: Vec<Expression>) -> Stmt {
    Stmt::Assignment(Assignment::new(
        punctuated(vars, ", "),
        punctuated(values, ", "),
    ))
}

pub(super) fn call_stmt(call: FunctionCall) -> Stmt {
    Stmt::FunctionCall(call)
}

pub(super) fn do_stmt(body: Block) -> Stmt {
    Stmt::Do(
        Do::new()
            .with_do_token(symbol("do\n"))
            .with_block(body)
            .with_end_token(symbol("\nend")),
    )
}

pub(super) fn while_stmt(cond: Expression, body: Block) -> Stmt {
    Stmt::While(
        While::new(cond)
            .with_do_token(symbol(" do\n"))
            .with_block(body)
            .with_end_token(symbol("\nend")),
    )
}

pub(super) fn repeat_stmt(body: Block, cond: Expression) -> Stmt {
    Stmt::Repeat(
        Repeat::new(cond)
            .with_repeat_token(symbol("repeat\n"))
            .with_block(body)
            .with_until_token(symbol("\nuntil ")),
    )
}

pub(super) fn if_stmt(arms: Vec<(Expression, Block)>, else_block: Option<Block>) -> Stmt {
    let mut arms = arms.into_iter();
    let Some((condition, body)) = arms.next() else {
        return do_stmt(else_block.unwrap_or_default());
    };

    let else_if = arms
        .map(|(condition, body)| {
            ElseIf::new(condition)
                .with_else_if_token(symbol("\nelseif "))
                .with_then_token(symbol(" then\n"))
                .with_block(body)
        })
        .collect::<Vec<_>>();

    Stmt::If(
        If::new(condition)
            .with_then_token(symbol(" then\n"))
            .with_block(body)
            .with_else_if((!else_if.is_empty()).then_some(else_if))
            .with_else_token(else_block.as_ref().map(|_| symbol("\nelse\n")))
            .with_else(else_block)
            .with_end_token(symbol("\nend")),
    )
}

pub(super) fn numeric_for(
    var: TokenReference,
    start: Expression,
    stop: Expression,
    step: Option<Expression>,
    body: Block,
) -> Stmt {
    let mut stmt = NumericFor::new(var, start, stop)
        .with_do_token(symbol(" do\n"))
        .with_block(body)
        .with_end_token(symbol("\nend"));

    if let Some(step) = step {
        stmt = stmt
            .with_end_step_comma(Some(symbol(", ")))
            .with_step(Some(step));
    }

    Stmt::NumericFor(stmt)
}

pub(super) fn generic_for(names: Vec<TokenReference>, exprs: Vec<Expression>, body: Block) -> Stmt {
    Stmt::GenericFor(
        GenericFor::new(punctuated(names, ", "), punctuated(exprs, ", "))
            .with_do_token(symbol(" do\n"))
            .with_block(body)
            .with_end_token(symbol("\nend")),
    )
}

pub(super) fn local_function(name: TokenReference, body: FunctionBody) -> Stmt {
    Stmt::LocalFunction(LocalFunction::new(name).with_body(body))
}

pub(super) fn function_declaration(name: TokenReference, body: FunctionBody) -> Stmt {
    Stmt::FunctionDeclaration(FunctionDeclaration::new(function_name(name)).with_body(body))
}

pub(super) fn qualified_function_declaration(name: &FunctionName, body: FunctionBody) -> Stmt {
    Stmt::FunctionDeclaration(
        FunctionDeclaration::new(qualified_function_name(name)).with_body(body),
    )
}

pub(super) fn goto_stmt(name: TokenReference) -> Stmt {
    Stmt::Goto(Goto::new(name).with_goto_token(symbol("goto ")))
}

pub(super) fn label_stmt(name: TokenReference) -> Stmt {
    Stmt::Label(Label::new(name))
}

pub(super) fn return_stmt(values: Vec<Expression>) -> LastStmt {
    LastStmt::Return(Return::new().with_returns(punctuated(values, ", ")))
}

pub(super) fn break_stmt() -> LastStmt {
    LastStmt::Break(symbol("break"))
}

pub(super) fn function_body(
    params: Vec<TokenReference>,
    is_vararg: bool,
    body: Block,
) -> FunctionBody {
    let mut params = params.into_iter().map(Parameter::Name).collect::<Vec<_>>();

    if is_vararg {
        params.push(Parameter::Ellipsis(symbol("...")));
    }

    FunctionBody::new()
        .with_parameters_parentheses(ContainedSpan::new(symbol("("), symbol(")\n")))
        .with_parameters(punctuated(params, ", "))
        .with_block(body)
        .with_end_token(symbol("\nend"))
}

pub(super) fn function_call(prefix: Prefix, suffixes: Vec<Suffix>) -> FunctionCall {
    FunctionCall::new(prefix).with_suffixes(suffixes)
}

pub(super) fn anonymous_call(args: Vec<Expression>) -> Suffix {
    Suffix::Call(Call::AnonymousCall(function_args(args)))
}

pub(super) fn method_call(name: TokenReference, args: Vec<Expression>) -> Suffix {
    Suffix::Call(Call::MethodCall(MethodCall::new(name, function_args(args))))
}

pub(super) fn function_args(args: Vec<Expression>) -> FunctionArgs {
    FunctionArgs::Parentheses {
        parentheses: ContainedSpan::new(symbol("("), symbol(")")),
        arguments: punctuated(args, ", "),
    }
}

pub(super) fn var_expression(prefix: Prefix, suffixes: Vec<Suffix>) -> VarExpression {
    VarExpression::new(prefix).with_suffixes(suffixes)
}

pub(super) fn bracket_index(key: Expression) -> Suffix {
    Suffix::Index(Index::Brackets {
        brackets: ContainedSpan::new(symbol("["), symbol("]")),
        expression: key,
    })
}

pub(super) fn dot_index(name: TokenReference) -> Suffix {
    Suffix::Index(Index::Dot {
        dot: symbol("."),
        name,
    })
}

pub(super) fn table(fields: Vec<Field>) -> Expression {
    Expression::TableConstructor(
        TableConstructor::new()
            .with_braces(ContainedSpan::new(symbol("{"), symbol("}")))
            .with_fields(punctuated(fields, ", ")),
    )
}

pub(super) fn table_list(value: Expression) -> Field {
    Field::NoKey(value)
}

pub(super) fn table_named(name: TokenReference, value: Expression) -> Field {
    Field::NameKey {
        key: name,
        equal: symbol(" = "),
        value,
    }
}

pub(super) fn table_expr_key(key: Expression, value: Expression) -> Field {
    Field::ExpressionKey {
        brackets: ContainedSpan::new(symbol("["), symbol("]")),
        key,
        equal: symbol(" = "),
        value,
    }
}

pub(super) fn binary_expr(lhs: Expression, binop: BinOp, rhs: Expression) -> Expression {
    Expression::BinaryOperator {
        lhs: Box::new(lhs),
        binop,
        rhs: Box::new(rhs),
    }
}

pub(super) fn unary_expr(unop: UnOp, expression: Expression) -> Expression {
    Expression::UnaryOperator {
        unop,
        expression: Box::new(expression),
    }
}

pub(super) fn paren_expr(expression: Expression) -> Expression {
    Expression::Parentheses {
        contained: ContainedSpan::new(symbol("("), symbol(")")),
        expression: Box::new(expression),
    }
}

pub(super) fn anonymous_function(body: FunctionBody) -> Expression {
    Expression::Function(Box::new(
        full_moon::ast::AnonymousFunction::new()
            .with_function_token(symbol("function"))
            .with_body(body),
    ))
}

pub(super) fn name_expr(name: TokenReference) -> Expression {
    Expression::Var(Var::Name(name))
}

pub(super) fn nil_expr() -> Expression {
    Expression::Symbol(symbol("nil"))
}

pub(super) fn bool_expr(value: bool) -> Expression {
    Expression::Symbol(symbol(if value { "true" } else { "false" }))
}

pub(super) fn vararg_expr() -> Expression {
    Expression::Symbol(symbol("..."))
}

pub(super) fn integer_expr(value: i64) -> Expression {
    number_expr_text(value.to_string())
}

pub(super) fn number_expr(value: f64) -> Expression {
    let text = if value.is_finite() {
        value.to_string()
    } else if value.is_sign_negative() {
        "-1e9999".to_string()
    } else {
        "1e9999".to_string()
    };

    number_expr_text(text)
}

pub(super) fn number_expr_text(text: String) -> Expression {
    Expression::Number(TokenReference::new(
        Vec::new(),
        Token::new(TokenType::Number { text: text.into() }),
        Vec::new(),
    ))
}

pub(super) fn string_expr(bytes: &BString) -> Expression {
    Expression::String(TokenReference::new(
        Vec::new(),
        Token::new(TokenType::StringLiteral {
            literal: escape_string(bytes).into(),
            multi_line_depth: 0,
            quote_type: StringLiteralQuoteType::Double,
        }),
        Vec::new(),
    ))
}

pub(super) fn identifier(name: &Name) -> TokenReference {
    identifier_bytes(name.as_bytes())
}

pub(super) fn identifier_bstring(name: &BString) -> TokenReference {
    identifier_bytes(name.as_slice())
}

pub(super) fn attribute(attrib: Attrib) -> Attribute {
    let name = match attrib {
        Attrib::Const => "const",
        Attrib::Close => "close",
    };

    Attribute::new(identifier_bytes(name.as_bytes()))
        .with_brackets(ContainedSpan::new(symbol(" <"), symbol(">")))
}

pub(super) fn binop(op: crate::decompile::ast::BinOp) -> BinOp {
    match op {
        crate::decompile::ast::BinOp::Add => BinOp::Plus(symbol(" + ")),
        crate::decompile::ast::BinOp::Sub => BinOp::Minus(symbol(" - ")),
        crate::decompile::ast::BinOp::Mul => BinOp::Star(symbol(" * ")),
        crate::decompile::ast::BinOp::Div => BinOp::Slash(symbol(" / ")),
        crate::decompile::ast::BinOp::Mod => BinOp::Percent(symbol(" % ")),
        crate::decompile::ast::BinOp::Pow => BinOp::Caret(symbol("^")),
        crate::decompile::ast::BinOp::IDiv => BinOp::DoubleSlash(symbol(" // ")),
        crate::decompile::ast::BinOp::BAnd => BinOp::Ampersand(symbol(" & ")),
        crate::decompile::ast::BinOp::BOr => BinOp::Pipe(symbol(" | ")),
        crate::decompile::ast::BinOp::BXor => BinOp::Tilde(symbol(" ~ ")),
        crate::decompile::ast::BinOp::Shl => BinOp::DoubleLessThan(symbol(" << ")),
        crate::decompile::ast::BinOp::Shr => BinOp::DoubleGreaterThan(symbol(" >> ")),
        crate::decompile::ast::BinOp::Concat => BinOp::TwoDots(symbol(" .. ")),
        crate::decompile::ast::BinOp::Eq => BinOp::TwoEqual(symbol(" == ")),
        crate::decompile::ast::BinOp::Ne => BinOp::TildeEqual(symbol(" ~= ")),
        crate::decompile::ast::BinOp::Lt => BinOp::LessThan(symbol(" < ")),
        crate::decompile::ast::BinOp::Le => BinOp::LessThanEqual(symbol(" <= ")),
        crate::decompile::ast::BinOp::Gt => BinOp::GreaterThan(symbol(" > ")),
        crate::decompile::ast::BinOp::Ge => BinOp::GreaterThanEqual(symbol(" >= ")),
        crate::decompile::ast::BinOp::And => BinOp::And(symbol(" and ")),
        crate::decompile::ast::BinOp::Or => BinOp::Or(symbol(" or ")),
    }
}

pub(super) fn unop(op: crate::decompile::ast::UnOp) -> UnOp {
    match op {
        crate::decompile::ast::UnOp::Neg => UnOp::Minus(symbol("-")),
        crate::decompile::ast::UnOp::Not => UnOp::Not(symbol("not ")),
        crate::decompile::ast::UnOp::Len => UnOp::Hash(symbol("#")),
        crate::decompile::ast::UnOp::BNot => UnOp::Tilde(symbol("~")),
    }
}

pub(super) fn symbol(text: &str) -> TokenReference {
    TokenReference::symbol(text).expect("valid Lua symbol token")
}

fn identifier_bytes(bytes: &[u8]) -> TokenReference {
    TokenReference::new(
        Vec::new(),
        Token::new(TokenType::Identifier {
            identifier: String::from_utf8_lossy(bytes).into_owned().into(),
        }),
        Vec::new(),
    )
}

fn function_name(name: TokenReference) -> MoonFunctionName {
    MoonFunctionName::new(punctuated(vec![name], "."))
}

fn qualified_function_name(name: &FunctionName) -> MoonFunctionName {
    let path = name.path.iter().map(identifier).collect::<Vec<_>>();
    let function_name = MoonFunctionName::new(punctuated(path, "."));
    function_name.with_method(
        name.method
            .as_ref()
            .map(|method| (symbol(":"), identifier(method))),
    )
}

fn punctuated<T>(items: Vec<T>, punctuation: &str) -> Punctuated<T> {
    let last_index = items.len().saturating_sub(1);

    items
        .into_iter()
        .enumerate()
        .map(|(index, item)| {
            if index == last_index {
                Pair::End(item)
            } else {
                Pair::Punctuated(item, symbol(punctuation))
            }
        })
        .collect()
}

fn newline() -> TokenReference {
    TokenReference::new(
        Vec::new(),
        Token::new(TokenType::Whitespace {
            characters: "\n".into(),
        }),
        Vec::new(),
    )
}

fn escape_string(bytes: &BString) -> String {
    let mut escaped = String::new();

    for &byte in bytes.as_slice() {
        match byte {
            b'\n' => escaped.push_str("\\n"),
            b'\r' => escaped.push_str("\\r"),
            b'\t' => escaped.push_str("\\t"),
            b'\\' => escaped.push_str("\\\\"),
            b'"' => escaped.push_str("\\\""),
            0x20..=0x7e => escaped.push(char::from(byte)),
            _ => escaped.push_str(&format!("\\{byte:03}")),
        }
    }

    escaped
}
