use full_moon::ast::{Expression, FunctionCall, Prefix, Stmt as MoonStmt, Suffix, Var};

use crate::decompile::ast::{self, BinOp, Expr, Stmt, TableField, UnOp};

use super::builder;

const PREC_OR: u8 = 1;
const PREC_AND: u8 = 2;
const PREC_COMPARE: u8 = 3;
const PREC_BOR: u8 = 4;
const PREC_BXOR: u8 = 5;
const PREC_BAND: u8 = 6;
const PREC_SHIFT: u8 = 7;
const PREC_CONCAT: u8 = 8;
const PREC_ADD: u8 = 9;
const PREC_MUL: u8 = 10;
const PREC_UNARY: u8 = 11;
const PREC_POW: u8 = 12;

/// Lowers a compact decompiler block into a `full_moon` block.
pub fn lower_block(block: &ast::Block) -> full_moon::ast::Block {
    let mut stmts = Vec::new();
    let mut last_stmt = None;

    for (index, stmt) in block.0.iter().enumerate() {
        match stmt {
            Stmt::Return(values) => {
                debug_assert_eq!(index + 1, block.0.len());
                last_stmt = Some(builder::return_stmt(lower_exprs(values)));
                break;
            }
            Stmt::Break => {
                debug_assert_eq!(index + 1, block.0.len());
                last_stmt = Some(builder::break_stmt());
                break;
            }
            stmt => stmts.push(lower_stmt(stmt)),
        }
    }

    builder::block(stmts, last_stmt)
}

fn lower_stmt(stmt: &Stmt) -> MoonStmt {
    match stmt {
        Stmt::Local {
            names,
            attribs,
            values,
        } => builder::local_assign(
            names.iter().map(builder::identifier).collect(),
            attribs
                .iter()
                .map(|attrib| attrib.map(builder::attribute))
                .collect(),
            lower_exprs(values),
        ),
        Stmt::Assign { targets, values } => {
            builder::assign(targets.iter().map(lower_var).collect(), lower_exprs(values))
        }
        Stmt::Call(expr) => builder::call_stmt(lower_call(expr)),
        Stmt::Do(body) => builder::do_stmt(lower_block(body)),
        Stmt::While { cond, body } => {
            builder::while_stmt(lower_expr(cond, ExprContext::default()), lower_block(body))
        }
        Stmt::Repeat { body, cond } => {
            builder::repeat_stmt(lower_block(body), lower_expr(cond, ExprContext::default()))
        }
        Stmt::If { arms, else_ } => builder::if_stmt(
            arms.iter()
                .map(|(cond, body)| (lower_expr(cond, ExprContext::default()), lower_block(body)))
                .collect(),
            else_.as_ref().map(lower_block),
        ),
        Stmt::NumericFor {
            var,
            start,
            stop,
            step,
            body,
        } => builder::numeric_for(
            builder::identifier(var),
            lower_expr(start, ExprContext::default()),
            lower_expr(stop, ExprContext::default()),
            step.as_ref()
                .map(|step| lower_expr(step, ExprContext::default())),
            lower_block(body),
        ),
        Stmt::GenericFor { names, exprs, body } => builder::generic_for(
            names.iter().map(builder::identifier).collect(),
            lower_exprs(exprs),
            lower_block(body),
        ),
        Stmt::Function { name, body, local } => {
            let body = lower_func_body(body);
            if *local {
                builder::local_function(builder::identifier(name), body)
            } else {
                builder::function_declaration(builder::identifier(name), body)
            }
        }
        Stmt::FunctionDecl { name, body } => {
            builder::qualified_function_declaration(name, lower_func_body(body))
        }
        Stmt::Goto(name) => builder::goto_stmt(builder::identifier(name)),
        Stmt::Label(name) => builder::label_stmt(builder::identifier(name)),
        Stmt::Return(_) | Stmt::Break => unreachable!("last statements are handled by lower_block"),
    }
}

fn lower_exprs(exprs: &[Expr]) -> Vec<Expression> {
    exprs
        .iter()
        .map(|expr| lower_expr(expr, ExprContext::default()))
        .collect()
}

fn lower_expr(expr: &Expr, context: ExprContext) -> Expression {
    match expr {
        Expr::Nil => builder::nil_expr(),
        Expr::True => builder::bool_expr(true),
        Expr::False => builder::bool_expr(false),
        Expr::VarArg => builder::vararg_expr(),
        Expr::Number(value) => builder::number_expr(*value),
        Expr::Integer(value) => builder::integer_expr(*value),
        Expr::Str(bytes) => builder::string_expr(bytes),
        Expr::Name(name) => builder::name_expr(builder::identifier(name)),
        Expr::Global(name) => builder::name_expr(builder::identifier_bstring(name)),
        Expr::Index { .. } | Expr::Field { .. } => {
            let (prefix, suffixes) = lower_prefix(expr);
            Expression::Var(Var::Expression(Box::new(builder::var_expression(
                prefix, suffixes,
            ))))
        }
        Expr::Call { .. } => Expression::FunctionCall(lower_call(expr)),
        Expr::Function(body) => builder::anonymous_function(lower_func_body(body)),
        Expr::Table(fields) => builder::table(
            fields
                .iter()
                .map(|field| match field {
                    TableField::List(value) => {
                        builder::table_list(lower_expr(value, ExprContext::default()))
                    }
                    TableField::Named { name, value } => builder::table_named(
                        builder::identifier(name),
                        lower_expr(value, ExprContext::default()),
                    ),
                    TableField::ExprKey { key, value } => builder::table_expr_key(
                        lower_expr(key, ExprContext::default()),
                        lower_expr(value, ExprContext::default()),
                    ),
                })
                .collect(),
        ),
        Expr::Binary { op, lhs, rhs } => {
            let lhs = lower_expr(lhs, ExprContext::binary_child(*op, Side::Left));
            let rhs = lower_expr(rhs, ExprContext::binary_child(*op, Side::Right));
            let expression = builder::binary_expr(lhs, builder::binop(*op), rhs);
            paren_for_context(expression, ExprShape::Binary(*op), context)
        }
        Expr::Unary { op, operand } => {
            let operand = lower_expr(operand, ExprContext::unary_child(*op));
            let expression = builder::unary_expr(builder::unop(*op), operand);
            paren_for_context(expression, ExprShape::Unary(*op), context)
        }
        Expr::Paren(inner) => builder::paren_expr(lower_expr(inner, ExprContext::default())),
    }
}

fn lower_func_body(body: &ast::FuncBody) -> full_moon::ast::FunctionBody {
    builder::function_body(
        body.params.iter().map(builder::identifier).collect(),
        body.is_vararg,
        lower_block(&body.body),
    )
}

fn lower_call(expr: &Expr) -> FunctionCall {
    let Expr::Call { .. } = expr else {
        let prefix = Prefix::Expression(Box::new(builder::paren_expr(lower_expr(
            expr,
            ExprContext::default(),
        ))));
        return builder::function_call(prefix, vec![builder::anonymous_call(Vec::new())]);
    };

    let (prefix, suffixes) = lower_prefix(expr);
    builder::function_call(prefix, suffixes)
}

fn lower_var(expr: &Expr) -> Var {
    match expr {
        Expr::Name(name) => Var::Name(builder::identifier(name)),
        Expr::Global(name) => Var::Name(builder::identifier_bstring(name)),
        Expr::Index { .. } | Expr::Field { .. } | Expr::Call { .. } => {
            let (prefix, suffixes) = lower_prefix(expr);
            Var::Expression(Box::new(builder::var_expression(prefix, suffixes)))
        }
        expr => {
            let prefix = Prefix::Expression(Box::new(builder::paren_expr(lower_expr(
                expr,
                ExprContext::default(),
            ))));
            Var::Expression(Box::new(builder::var_expression(prefix, Vec::new())))
        }
    }
}

fn lower_prefix(expr: &Expr) -> (Prefix, Vec<Suffix>) {
    match expr {
        Expr::Name(name) => (Prefix::Name(builder::identifier(name)), Vec::new()),
        Expr::Global(name) => (Prefix::Name(builder::identifier_bstring(name)), Vec::new()),
        Expr::Index { obj, key } => {
            let (prefix, mut suffixes) = lower_prefix(obj);
            suffixes.push(builder::bracket_index(lower_expr(
                key,
                ExprContext::default(),
            )));
            (prefix, suffixes)
        }
        Expr::Field { obj, name } => {
            let (prefix, mut suffixes) = lower_prefix(obj);
            suffixes.push(builder::dot_index(builder::identifier(name)));
            (prefix, suffixes)
        }
        Expr::Call { func, args, method } => {
            let (prefix, mut suffixes) = lower_prefix(func);
            let args = lower_exprs(args);
            suffixes.push(match method {
                Some(name) => builder::method_call(builder::identifier(name), args),
                None => builder::anonymous_call(args),
            });
            (prefix, suffixes)
        }
        expr => (
            Prefix::Expression(Box::new(builder::paren_expr(lower_expr(
                expr,
                ExprContext::default(),
            )))),
            Vec::new(),
        ),
    }
}

fn paren_for_context(expression: Expression, shape: ExprShape, context: ExprContext) -> Expression {
    if context.needs_parens(shape) {
        builder::paren_expr(expression)
    } else {
        expression
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct ExprContext {
    parent: Option<ParentExpr>,
}

impl ExprContext {
    fn binary_child(op: BinOp, side: Side) -> Self {
        Self {
            parent: Some(ParentExpr::Binary { op, side }),
        }
    }

    fn unary_child(op: UnOp) -> Self {
        Self {
            parent: Some(ParentExpr::Unary(op)),
        }
    }

    fn needs_parens(self, child: ExprShape) -> bool {
        match self.parent {
            None => false,
            Some(ParentExpr::Unary(parent_op)) => unary_operand_needs_parens(parent_op, child),
            Some(ParentExpr::Binary { op, side }) => binary_child_needs_parens(op, side, child),
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum ParentExpr {
    Binary { op: BinOp, side: Side },
    Unary(UnOp),
}

#[derive(Debug, Clone, Copy)]
enum ExprShape {
    Binary(BinOp),
    Unary(UnOp),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Side {
    Left,
    Right,
}

fn unary_operand_needs_parens(parent: UnOp, child: ExprShape) -> bool {
    match child {
        ExprShape::Binary(op) => precedence(op) <= PREC_UNARY,
        ExprShape::Unary(UnOp::Neg) if parent == UnOp::Neg => true,
        ExprShape::Unary(_) => false,
    }
}

fn binary_child_needs_parens(parent: BinOp, side: Side, child: ExprShape) -> bool {
    let child_prec = child.precedence();
    let parent_prec = precedence(parent);

    if child_prec < parent_prec {
        return !(side == Side::Right && parent == BinOp::Pow && child.is_unary());
    }

    if child_prec > parent_prec {
        return false;
    }

    match side {
        Side::Left => is_right_assoc(parent),
        Side::Right => !is_right_assoc(parent),
    }
}

impl ExprShape {
    fn precedence(self) -> u8 {
        match self {
            ExprShape::Binary(op) => precedence(op),
            ExprShape::Unary(_) => PREC_UNARY,
        }
    }

    fn is_unary(self) -> bool {
        matches!(self, ExprShape::Unary(_))
    }
}

fn precedence(op: BinOp) -> u8 {
    match op {
        BinOp::Or => PREC_OR,
        BinOp::And => PREC_AND,
        BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => PREC_COMPARE,
        BinOp::BOr => PREC_BOR,
        BinOp::BXor => PREC_BXOR,
        BinOp::BAnd => PREC_BAND,
        BinOp::Shl | BinOp::Shr => PREC_SHIFT,
        BinOp::Concat => PREC_CONCAT,
        BinOp::Add | BinOp::Sub => PREC_ADD,
        BinOp::Mul | BinOp::Div | BinOp::Mod | BinOp::IDiv => PREC_MUL,
        BinOp::Pow => PREC_POW,
    }
}

fn is_right_assoc(op: BinOp) -> bool {
    matches!(op, BinOp::Pow | BinOp::Concat)
}
