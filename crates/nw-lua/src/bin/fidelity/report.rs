use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use super::compare::{Category, FileDiff, function_category_hits};

#[derive(Debug, Clone)]
pub struct Example {
    pub path: PathBuf,
    pub function: String,
    pub detail: String,
}

#[derive(Debug, Default)]
pub struct Summary {
    pub roots: Vec<PathBuf>,
    pub limit: usize,
    pub total_files_seen: usize,
    pub processed_files: usize,
    pub source_compile_errors: usize,
    pub decompile_errors: usize,
    pub parse_errors: usize,
    pub original_functions: usize,
    pub divergent_files: usize,
    pub divergent_functions: usize,
    pub file_category_hits: BTreeMap<Category, usize>,
    pub function_category_hits: BTreeMap<Category, usize>,
    pub examples: BTreeMap<Category, Vec<Example>>,
}

impl Summary {
    pub fn add_diff(&mut self, diff: FileDiff) {
        self.processed_files += 1;
        self.original_functions += diff.original_functions;
        if diff.is_divergent() {
            self.divergent_files += 1;
        }
        self.divergent_functions += diff.divergent_functions();

        let file_categories = diff
            .file_categories
            .iter()
            .copied()
            .chain(
                diff.function_diffs
                    .iter()
                    .flat_map(|function| function.categories.iter().copied()),
            )
            .collect::<BTreeSet<_>>();
        for category in file_categories {
            *self.file_category_hits.entry(category).or_default() += 1;
            if diff.file_categories.contains(&category) {
                self.push_example(
                    category,
                    Example {
                        path: diff.path.clone(),
                        function: "<file>".to_owned(),
                        detail: lexical_detail(&diff, category),
                    },
                );
            }
        }

        for (category, count) in function_category_hits(&diff) {
            *self.function_category_hits.entry(category).or_default() += count;
        }

        for func in &diff.function_diffs {
            for category in &func.categories {
                self.push_example(
                    *category,
                    Example {
                        path: diff.path.clone(),
                        function: format!("{}#{}", func.name, func.original_index),
                        detail: metric_detail(func),
                    },
                );
            }
        }
    }

    pub fn print(&self, examples_per_category: usize) {
        println!("nw-lua fidelity differential");
        println!("roots:");
        for root in &self.roots {
            println!("  {}", root.display());
        }
        println!("limit: {}", self.limit);
        println!("files_seen: {}", self.total_files_seen);
        println!("files_processed: {}", self.processed_files);
        println!("source_compile_errors: {}", self.source_compile_errors);
        println!("decompile_errors: {}", self.decompile_errors);
        println!("parse_errors: {}", self.parse_errors);
        println!(
            "files_divergent: {} / {} ({:.2}%)",
            self.divergent_files,
            self.processed_files,
            percent(self.divergent_files, self.processed_files)
        );
        println!(
            "functions_divergent: {} / {} ({:.2}%)",
            self.divergent_functions,
            self.original_functions,
            percent(self.divergent_functions, self.original_functions)
        );
        println!("category,file_hits,file_pct,function_hits,function_pct");
        for category in all_categories() {
            let file_hits = self.file_category_hits.get(&category).copied().unwrap_or(0);
            let function_hits = self
                .function_category_hits
                .get(&category)
                .copied()
                .unwrap_or(0);
            println!(
                "{},{},{:.2},{},{:.2}",
                category.label(),
                file_hits,
                percent(file_hits, self.processed_files),
                function_hits,
                percent(function_hits, self.original_functions)
            );
        }
        println!("examples:");
        for category in all_categories() {
            let Some(examples) = self.examples.get(&category) else {
                continue;
            };
            for example in examples.iter().take(if examples_per_category == 0 {
                usize::MAX
            } else {
                examples_per_category
            }) {
                println!(
                    "{}\t{}\t{}\t{}",
                    category.label(),
                    example.path.display(),
                    example.function,
                    example.detail
                );
            }
        }
    }

    fn push_example(&mut self, category: Category, example: Example) {
        let examples = self.examples.entry(category).or_default();
        if examples.len() < 64 {
            examples.push(example);
        }
    }
}

fn metric_detail(func: &super::compare::FunctionDiff) -> String {
    match &func.decompiled {
        Some(decompiled) => format!(
            "orig(stmt={},ret={},assign={},if={},elseif={},else={},loop={},empty={},ctor={}/{},sugar={}/{}/{},temp={}) decomp(stmt={},ret={},assign={},if={},elseif={},else={},loop={},empty={},ctor={}/{},sugar={}/{}/{},temp={}) decomp_idx={:?}",
            func.original.statements,
            func.original.returns,
            func.original.assignments,
            func.original.ifs,
            func.original.elseifs,
            func.original.elses,
            func.original.loops,
            func.original.empty_branches,
            func.original.table_constructors,
            func.original.table_fields,
            func.original.local_functions,
            func.original.function_declarations,
            func.original.function_value_assignments,
            func.original.synthetic_locals,
            decompiled.statements,
            decompiled.returns,
            decompiled.assignments,
            decompiled.ifs,
            decompiled.elseifs,
            decompiled.elses,
            decompiled.loops,
            decompiled.empty_branches,
            decompiled.table_constructors,
            decompiled.table_fields,
            decompiled.local_functions,
            decompiled.function_declarations,
            decompiled.function_value_assignments,
            decompiled.synthetic_locals,
            func.decompiled_index
        ),
        None => "missing aligned decompiled function".to_owned(),
    }
}

fn lexical_detail(diff: &FileDiff, category: Category) -> String {
    match category {
        Category::ShortCircuitLoss | Category::ShortCircuitGain => format!(
            "and/or original={} decompiled={}",
            diff.lex.original_and_or, diff.lex.decompiled_and_or
        ),
        Category::BogusNotNumber => format!("not-number count={}", diff.lex.bogus_not_number),
        Category::NumberShortCircuit => {
            format!(
                "number-short-circuit count={}",
                diff.lex.number_short_circuit
            )
        }
        Category::UndefinedSyntheticRead => format!(
            "undefined synthetics={}",
            diff.lex
                .undefined_synthetic_reads
                .iter()
                .cloned()
                .collect::<Vec<_>>()
                .join("|")
        ),
        Category::FunctionCount => format!(
            "original_functions={} decompiled_functions={}",
            diff.original_functions, diff.decompiled_functions
        ),
        _ => String::new(),
    }
}

fn all_categories() -> [Category; 16] {
    [
        Category::FunctionCount,
        Category::DroppedReturn,
        Category::StatementCount,
        Category::AssignmentCount,
        Category::AssignmentTargetMismatch,
        Category::ControlFlowCount,
        Category::EmptyDecompiledBranch,
        Category::UnnecessaryControlFlow,
        Category::ConstructorShape,
        Category::DeclarationSugar,
        Category::ExposedTemporary,
        Category::ShortCircuitLoss,
        Category::ShortCircuitGain,
        Category::BogusNotNumber,
        Category::NumberShortCircuit,
        Category::UndefinedSyntheticRead,
    ]
}

fn percent(part: usize, total: usize) -> f64 {
    if total == 0 {
        0.0
    } else {
        (part as f64 / total as f64) * 100.0
    }
}
