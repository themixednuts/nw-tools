use std::collections::BTreeMap;

use full_moon::ast::{Ast, Block, Expression, LastStmt, Stmt};

#[derive(Debug, Clone, Default)]
pub struct Metrics {
    pub statements: usize,
    pub returns: usize,
    pub assignments: usize,
    pub assignment_targets: BTreeMap<String, usize>,
    pub ifs: usize,
    pub elseifs: usize,
    pub elses: usize,
    pub loops: usize,
    pub empty_branches: usize,
    pub and_ops: usize,
    pub or_ops: usize,
}

#[derive(Debug, Clone)]
pub struct FunctionSig {
    pub index: usize,
    pub name: String,
    pub metrics: Metrics,
}

#[derive(Debug, Clone)]
pub struct FileSig {
    pub functions: Vec<FunctionSig>,
}

pub fn signature(ast: &Ast) -> FileSig {
    let mut collector = Collector::default();
    collector.push_function("<root>".to_owned(), ast.nodes());
    FileSig {
        functions: collector.functions,
    }
}

#[derive(Default)]
struct Collector {
    functions: Vec<FunctionSig>,
    anon_count: usize,
}

impl Collector {
    fn push_function(&mut self, name: String, block: &Block) {
        let index = self.functions.len();
        self.functions.push(FunctionSig {
            index,
            name,
            metrics: metrics_for_block(block),
        });
        self.collect_nested_functions(block);
    }

    fn collect_nested_functions(&mut self, block: &Block) {
        for stmt in block.stmts() {
            self.collect_from_stmt(stmt);
        }
        if let Some(LastStmt::Return(ret)) = block.last_stmt() {
            for expr in ret.returns().iter() {
                self.collect_from_expr(expr);
            }
        }
    }

    fn collect_from_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Assignment(assign) => {
                let targets: Vec<String> = assign.variables().iter().map(compact).collect();
                for (idx, expr) in assign.expressions().iter().enumerate() {
                    if let Expression::Function(func) = expr {
                        let name = targets
                            .get(idx)
                            .cloned()
                            .unwrap_or_else(|| self.next_anon_name());
                        self.push_function(name, func.body().block());
                    } else {
                        self.collect_from_expr(expr);
                    }
                }
            }
            Stmt::Do(stmt) => self.collect_nested_functions(stmt.block()),
            Stmt::FunctionCall(call) => self.collect_from_text(&call.to_string()),
            Stmt::FunctionDeclaration(func) => {
                self.push_function(compact(func.name()), func.body().block());
            }
            Stmt::GenericFor(stmt) => {
                for expr in stmt.expressions().iter() {
                    self.collect_from_expr(expr);
                }
                self.collect_nested_functions(stmt.block());
            }
            Stmt::If(stmt) => {
                self.collect_from_expr(stmt.condition());
                self.collect_nested_functions(stmt.block());
                if let Some(elseifs) = stmt.else_if() {
                    for elseif in elseifs {
                        self.collect_from_expr(elseif.condition());
                        self.collect_nested_functions(elseif.block());
                    }
                }
                if let Some(block) = stmt.else_block() {
                    self.collect_nested_functions(block);
                }
            }
            Stmt::LocalAssignment(assign) => {
                let names: Vec<String> = assign.names().iter().map(compact).collect();
                for (idx, expr) in assign.expressions().iter().enumerate() {
                    if let Expression::Function(func) = expr {
                        let name = names
                            .get(idx)
                            .cloned()
                            .unwrap_or_else(|| self.next_anon_name());
                        self.push_function(name, func.body().block());
                    } else {
                        self.collect_from_expr(expr);
                    }
                }
            }
            Stmt::LocalFunction(func) => {
                self.push_function(compact(func.name()), func.body().block());
            }
            Stmt::NumericFor(stmt) => {
                self.collect_from_expr(stmt.start());
                self.collect_from_expr(stmt.end());
                if let Some(step) = stmt.step() {
                    self.collect_from_expr(step);
                }
                self.collect_nested_functions(stmt.block());
            }
            Stmt::Repeat(stmt) => {
                self.collect_nested_functions(stmt.block());
                self.collect_from_expr(stmt.until());
            }
            Stmt::While(stmt) => {
                self.collect_from_expr(stmt.condition());
                self.collect_nested_functions(stmt.block());
            }
            _ => {}
        }
    }

    fn collect_from_expr(&mut self, expr: &Expression) {
        match expr {
            Expression::BinaryOperator { lhs, rhs, .. } => {
                self.collect_from_expr(lhs);
                self.collect_from_expr(rhs);
            }
            Expression::Parentheses { expression, .. } => self.collect_from_expr(expression),
            Expression::UnaryOperator { expression, .. } => self.collect_from_expr(expression),
            Expression::Function(func) => {
                let name = self.next_anon_name();
                self.push_function(name, func.body().block());
            }
            other => self.collect_from_text(&other.to_string()),
        }
    }

    fn collect_from_text(&mut self, _text: &str) {}

    fn next_anon_name(&mut self) -> String {
        self.anon_count += 1;
        format!("<anon{}>", self.anon_count)
    }
}

fn metrics_for_block(block: &Block) -> Metrics {
    let mut metrics = Metrics::default();
    count_block(block, &mut metrics);
    metrics
}

fn count_block(block: &Block, metrics: &mut Metrics) {
    for stmt in block.stmts() {
        metrics.statements += 1;
        count_stmt(stmt, metrics);
        if stmt_always_returns(stmt) {
            return;
        }
    }
    if let Some(last) = block.last_stmt() {
        metrics.statements += 1;
        if let LastStmt::Return(ret) = last {
            metrics.returns += 1;
            for expr in ret.returns().iter() {
                count_expr(expr, metrics);
            }
        }
    }
}

fn block_always_returns(block: &Block) -> bool {
    if block
        .last_stmt()
        .is_some_and(|last| matches!(last, LastStmt::Return(_)))
    {
        return true;
    }
    block.stmts().any(stmt_always_returns)
}

fn stmt_always_returns(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::Do(stmt) => block_always_returns(stmt.block()),
        Stmt::If(stmt) => {
            block_always_returns(stmt.block())
                && stmt.else_if().is_none_or(|elseifs| {
                    elseifs
                        .iter()
                        .all(|elseif| block_always_returns(elseif.block()))
                })
                && stmt.else_block().is_some_and(block_always_returns)
        }
        _ => false,
    }
}

fn count_stmt(stmt: &Stmt, metrics: &mut Metrics) {
    match stmt {
        Stmt::Assignment(assign) => {
            metrics.assignments += 1;
            for target in assign.variables().iter() {
                *metrics
                    .assignment_targets
                    .entry(compact(target))
                    .or_default() += 1;
            }
            for expr in assign.expressions().iter() {
                count_expr(expr, metrics);
            }
        }
        Stmt::Do(stmt) => count_block(stmt.block(), metrics),
        Stmt::FunctionCall(_) => {}
        Stmt::FunctionDeclaration(_) => {}
        Stmt::GenericFor(stmt) => {
            metrics.loops += 1;
            for expr in stmt.expressions().iter() {
                count_expr(expr, metrics);
            }
            count_branch(stmt.block(), metrics);
        }
        Stmt::If(stmt) => {
            metrics.ifs += 1;
            count_expr(stmt.condition(), metrics);
            count_branch(stmt.block(), metrics);
            if let Some(elseifs) = stmt.else_if() {
                metrics.elseifs += elseifs.len();
                for elseif in elseifs {
                    count_expr(elseif.condition(), metrics);
                    count_branch(elseif.block(), metrics);
                }
            }
            if let Some(block) = stmt.else_block() {
                metrics.elses += 1;
                count_branch(block, metrics);
            }
        }
        Stmt::LocalAssignment(assign) => {
            if assign.equal_token().is_some() {
                metrics.assignments += 1;
                for target in assign.names().iter() {
                    *metrics
                        .assignment_targets
                        .entry(compact(target))
                        .or_default() += 1;
                }
            }
            for expr in assign.expressions().iter() {
                count_expr(expr, metrics);
            }
        }
        Stmt::LocalFunction(_) => {}
        Stmt::NumericFor(stmt) => {
            metrics.loops += 1;
            count_expr(stmt.start(), metrics);
            count_expr(stmt.end(), metrics);
            if let Some(step) = stmt.step() {
                count_expr(step, metrics);
            }
            count_branch(stmt.block(), metrics);
        }
        Stmt::Repeat(stmt) => {
            metrics.loops += 1;
            count_branch(stmt.block(), metrics);
            count_expr(stmt.until(), metrics);
        }
        Stmt::While(stmt) => {
            metrics.loops += 1;
            count_expr(stmt.condition(), metrics);
            count_branch(stmt.block(), metrics);
        }
        _ => {}
    }
}

fn count_branch(block: &Block, metrics: &mut Metrics) {
    if block_is_empty(block) {
        metrics.empty_branches += 1;
    }
    count_block(block, metrics);
}

fn block_is_empty(block: &Block) -> bool {
    block.stmts().next().is_none() && block.last_stmt().is_none()
}

fn count_expr(expr: &Expression, metrics: &mut Metrics) {
    match expr {
        Expression::BinaryOperator { lhs, binop, rhs } => {
            let op = compact(binop);
            if op == "and" {
                metrics.and_ops += 1;
            } else if op == "or" {
                metrics.or_ops += 1;
            }
            count_expr(lhs, metrics);
            count_expr(rhs, metrics);
        }
        Expression::Parentheses { expression, .. } => count_expr(expression, metrics),
        Expression::UnaryOperator { expression, .. } => count_expr(expression, metrics),
        Expression::Function(_) => {}
        _ => {}
    }
}

fn compact(value: &impl ToString) -> String {
    value
        .to_string()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}
