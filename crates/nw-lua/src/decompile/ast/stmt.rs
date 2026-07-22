use super::{Attrib, Block, Expr, FuncBody, FunctionName, Name};

/// Statements in the compact decompiler IR.
#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
    Local {
        names: Vec<Name>,
        attribs: Vec<Option<Attrib>>,
        values: Vec<Expr>,
    },
    Assign {
        targets: Vec<Expr>,
        values: Vec<Expr>,
    },
    Call(Expr),
    Do(Block),
    While {
        cond: Expr,
        body: Block,
    },
    Repeat {
        body: Block,
        cond: Expr,
    },
    If {
        arms: Vec<(Expr, Block)>,
        else_: Option<Block>,
    },
    NumericFor {
        var: Name,
        start: Expr,
        stop: Expr,
        step: Option<Box<Expr>>,
        body: Block,
    },
    GenericFor {
        names: Vec<Name>,
        exprs: Vec<Expr>,
        body: Block,
    },
    Function {
        name: Name,
        body: FuncBody,
        local: bool,
    },
    FunctionDecl {
        name: FunctionName,
        body: FuncBody,
    },
    Return(Vec<Expr>),
    Break,
    Goto(Name),
    Label(Name),
}
