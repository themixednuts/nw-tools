use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use super::ast_sig::{FileSig, FunctionSig, Metrics};
use super::lex::LexFindings;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Category {
    FunctionCount,
    DroppedReturn,
    StatementCount,
    AssignmentCount,
    AssignmentTargetMismatch,
    ControlFlowCount,
    EmptyDecompiledBranch,
    UnnecessaryControlFlow,
    ConstructorShape,
    DeclarationSugar,
    ExposedTemporary,
    ShortCircuitLoss,
    ShortCircuitGain,
    BogusNotNumber,
    NumberShortCircuit,
    UndefinedSyntheticRead,
}

impl Category {
    pub fn label(self) -> &'static str {
        match self {
            Category::FunctionCount => "function_count",
            Category::DroppedReturn => "dropped_return",
            Category::StatementCount => "statement_count",
            Category::AssignmentCount => "assignment_count",
            Category::AssignmentTargetMismatch => "assignment_target_mismatch",
            Category::ControlFlowCount => "control_flow_count",
            Category::EmptyDecompiledBranch => "empty_decompiled_branch",
            Category::UnnecessaryControlFlow => "unnecessary_control_flow",
            Category::ConstructorShape => "constructor_shape",
            Category::DeclarationSugar => "declaration_sugar",
            Category::ExposedTemporary => "exposed_temporary",
            Category::ShortCircuitLoss => "short_circuit_loss",
            Category::ShortCircuitGain => "short_circuit_gain",
            Category::BogusNotNumber => "bogus_not_number",
            Category::NumberShortCircuit => "number_short_circuit",
            Category::UndefinedSyntheticRead => "undefined_synthetic_read",
        }
    }
}

#[derive(Debug, Clone)]
pub struct FunctionDiff {
    pub name: String,
    pub original_index: usize,
    pub decompiled_index: Option<usize>,
    pub categories: BTreeSet<Category>,
    pub original: Metrics,
    pub decompiled: Option<Metrics>,
}

#[derive(Debug, Clone)]
pub struct FileDiff {
    pub path: PathBuf,
    pub original_functions: usize,
    pub decompiled_functions: usize,
    pub function_diffs: Vec<FunctionDiff>,
    pub file_categories: BTreeSet<Category>,
    pub lex: LexFindings,
}

impl FileDiff {
    pub fn is_divergent(&self) -> bool {
        !self.file_categories.is_empty()
            || self
                .function_diffs
                .iter()
                .any(|diff| !diff.categories.is_empty())
    }

    pub fn divergent_functions(&self) -> usize {
        self.function_diffs
            .iter()
            .filter(|diff| !diff.categories.is_empty())
            .count()
    }
}

pub fn compare_file(
    path: PathBuf,
    original: &FileSig,
    decompiled: &FileSig,
    lex: LexFindings,
) -> FileDiff {
    let mut used = BTreeSet::new();
    let mut file_categories = BTreeSet::new();
    let mut function_diffs = Vec::new();

    if original.functions.len() != decompiled.functions.len() {
        file_categories.insert(Category::FunctionCount);
    }

    let original_name_counts = name_counts(original);
    let decompiled_name_counts = name_counts(decompiled);
    let original_names = original_name_counts
        .keys()
        .copied()
        .collect::<BTreeSet<_>>();

    for original_func in &original.functions {
        let decompiled_idx = align_function(
            original_func,
            decompiled,
            &used,
            &original_names,
            &original_name_counts,
            &decompiled_name_counts,
        );
        if let Some(idx) = decompiled_idx {
            used.insert(idx);
        }

        let decompiled_func = decompiled_idx.and_then(|idx| decompiled.functions.get(idx));
        let categories = match decompiled_func {
            Some(func) => compare_metrics(&original_func.metrics, &func.metrics),
            None => BTreeSet::from([Category::FunctionCount]),
        };

        function_diffs.push(FunctionDiff {
            name: original_func.name.clone(),
            original_index: original_func.index,
            decompiled_index: decompiled_idx,
            categories,
            original: original_func.metrics.clone(),
            decompiled: decompiled_func.map(|func| func.metrics.clone()),
        });
    }

    if lex.decompiled_and_or < lex.original_and_or {
        file_categories.insert(Category::ShortCircuitLoss);
    } else if lex.decompiled_and_or > lex.original_and_or {
        file_categories.insert(Category::ShortCircuitGain);
    }
    if lex.bogus_not_number > 0 {
        file_categories.insert(Category::BogusNotNumber);
    }
    if lex.number_short_circuit > 0 {
        file_categories.insert(Category::NumberShortCircuit);
    }
    if !lex.undefined_synthetic_reads.is_empty() {
        file_categories.insert(Category::UndefinedSyntheticRead);
    }

    FileDiff {
        path,
        original_functions: original.functions.len(),
        decompiled_functions: decompiled.functions.len(),
        function_diffs,
        file_categories,
        lex,
    }
}

pub fn function_category_hits(diff: &FileDiff) -> BTreeMap<Category, usize> {
    let mut hits = BTreeMap::new();
    for function in &diff.function_diffs {
        for category in &function.categories {
            *hits.entry(*category).or_default() += 1;
        }
    }
    hits
}

fn name_counts(file: &FileSig) -> BTreeMap<&str, usize> {
    file.functions
        .iter()
        .filter(|func| !func.name.starts_with('<'))
        .fold(BTreeMap::new(), |mut counts, func| {
            *counts.entry(func.name.as_str()).or_default() += 1;
            counts
        })
}

fn align_function(
    original: &FunctionSig,
    decompiled: &FileSig,
    used: &BTreeSet<usize>,
    original_names: &BTreeSet<&str>,
    original_name_counts: &BTreeMap<&str, usize>,
    decompiled_name_counts: &BTreeMap<&str, usize>,
) -> Option<usize> {
    if !original.name.starts_with('<')
        && original_name_counts
            .get(original.name.as_str())
            .copied()
            .unwrap_or(0)
            == 1
        && decompiled_name_counts
            .get(original.name.as_str())
            .copied()
            .unwrap_or(0)
            == 1
        && let Some(found) = decompiled
            .functions
            .iter()
            .find(|func| !used.contains(&func.index) && func.name == original.name)
    {
        return Some(found.index);
    }
    if !original.name.starts_with('<') {
        return None;
    }

    decompiled
        .functions
        .get(original.index)
        .filter(|func| fallback_candidate(original, func, used, original_names))
        .map(|func| func.index)
        .or_else(|| {
            decompiled
                .functions
                .iter()
                .find(|func| fallback_candidate(original, func, used, original_names))
                .map(|func| func.index)
        })
}

fn fallback_candidate(
    original: &FunctionSig,
    candidate: &FunctionSig,
    used: &BTreeSet<usize>,
    original_names: &BTreeSet<&str>,
) -> bool {
    if used.contains(&candidate.index) {
        return false;
    }
    candidate.name == original.name
        || candidate.name.starts_with('<')
        || !original_names.contains(candidate.name.as_str())
}

fn compare_metrics(original: &Metrics, decompiled: &Metrics) -> BTreeSet<Category> {
    let mut categories = BTreeSet::new();
    if original.returns > decompiled.returns {
        categories.insert(Category::DroppedReturn);
    }
    if original.statements != decompiled.statements {
        categories.insert(Category::StatementCount);
    }
    if original.assignments != decompiled.assignments {
        categories.insert(Category::AssignmentCount);
    }
    if original.assignment_targets != decompiled.assignment_targets {
        categories.insert(Category::AssignmentTargetMismatch);
    }
    if original.ifs != decompiled.ifs
        || original.elseifs != decompiled.elseifs
        || original.elses != decompiled.elses
        || original.loops != decompiled.loops
    {
        categories.insert(Category::ControlFlowCount);
    }
    if decompiled.empty_branches > original.empty_branches {
        categories.insert(Category::EmptyDecompiledBranch);
    }
    let original_control = original.ifs + original.elseifs + original.elses + original.loops;
    let decompiled_control =
        decompiled.ifs + decompiled.elseifs + decompiled.elses + decompiled.loops;
    if decompiled_control > original_control {
        categories.insert(Category::UnnecessaryControlFlow);
    }
    if original.table_constructors != decompiled.table_constructors
        || original.table_fields != decompiled.table_fields
    {
        categories.insert(Category::ConstructorShape);
    }
    if original.local_functions != decompiled.local_functions
        || original.function_declarations != decompiled.function_declarations
        || original.function_value_assignments != decompiled.function_value_assignments
    {
        categories.insert(Category::DeclarationSugar);
    }
    if decompiled.synthetic_locals > original.synthetic_locals {
        categories.insert(Category::ExposedTemporary);
    }
    if decompiled.and_ops + decompiled.or_ops < original.and_ops + original.or_ops {
        categories.insert(Category::ShortCircuitLoss);
    } else if decompiled.and_ops + decompiled.or_ops > original.and_ops + original.or_ops {
        categories.insert(Category::ShortCircuitGain);
    }
    categories
}
