use crate::graph::{SchemaGraph, SchemaGraphDiagnosticCode};
use crate::lint::{Diagnostic, DiagnosticCode, Severity};

pub(super) fn schema_graph_diagnostics(schema_graph: &SchemaGraph) -> Vec<Diagnostic> {
    schema_graph
        .diagnostics
        .iter()
        .map(|diagnostic| Diagnostic {
            severity: Severity::Warning,
            code: match diagnostic.code {
                SchemaGraphDiagnosticCode::MissingWrapperTarget => {
                    DiagnosticCode::MissingWrapperTarget
                }
                SchemaGraphDiagnosticCode::AmbiguousWrapperTarget => {
                    DiagnosticCode::AmbiguousWrapperTarget
                }
            },
            message: diagnostic.message.clone(),
        })
        .collect()
}
