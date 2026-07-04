use bstr::BString;

use super::{BinOp, FuncBody, Name, TableField, UnOp};

/// Expressions in the compact decompiler IR.
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Nil,
    True,
    False,
    VarArg,
    Number(f64),
    Integer(i64),
    Str(BString),
    Name(Name),
    Global(BString),
    Index {
        obj: Box<Expr>,
        key: Box<Expr>,
    },
    Field {
        obj: Box<Expr>,
        name: Name,
    },
    Call {
        func: Box<Expr>,
        args: Vec<Expr>,
        method: Option<Name>,
    },
    Function(FuncBody),
    Table(Vec<TableField>),
    Binary {
        op: BinOp,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
    },
    Unary {
        op: UnOp,
        operand: Box<Expr>,
    },
    Paren(Box<Expr>),
}
