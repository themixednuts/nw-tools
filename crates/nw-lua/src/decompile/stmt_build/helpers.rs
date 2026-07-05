use super::*;

pub(crate) fn local_one(name: Name, value: Expr) -> Stmt {
    let values = if matches!(value, Expr::Nil) {
        Vec::new()
    } else {
        vec![value]
    };
    Stmt::Local {
        names: vec![name],
        attribs: Vec::new(),
        values,
    }
}

pub(crate) fn assign_one(target: Expr, value: Expr) -> Stmt {
    Stmt::Assign {
        targets: vec![target],
        values: vec![value],
    }
}

pub(super) fn is_stable_assignment_target(expr: &Expr) -> bool {
    match expr {
        Expr::Name(_) | Expr::Global(_) => true,
        Expr::Field { obj, .. } | Expr::Index { obj, .. } => is_stable_assignment_target(obj),
        _ => false,
    }
}
