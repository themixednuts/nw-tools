use std::collections::{BTreeMap, BTreeSet};

pub type SymbolSurfacePath = Vec<String>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolSurfaceExport<Symbol> {
    pub module: String,
    pub symbols: BTreeMap<String, Symbol>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolSurfaceModule<Symbol> {
    pub public_symbols: BTreeMap<String, Symbol>,
    pub reexports: Vec<SymbolSurfaceExport<Symbol>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolSurfaceInput<Symbol> {
    pub root_path: SymbolSurfacePath,
    pub local_symbols_by_path: BTreeMap<SymbolSurfacePath, BTreeMap<String, Symbol>>,
    pub direct_reexports_by_path: BTreeMap<SymbolSurfacePath, Vec<SymbolSurfaceExport<Symbol>>>,
    pub child_modules_by_path: BTreeMap<SymbolSurfacePath, BTreeSet<String>>,
}

pub fn plan_symbol_surface<Symbol: Clone>(
    input: SymbolSurfaceInput<Symbol>,
) -> BTreeMap<SymbolSurfacePath, SymbolSurfaceModule<Symbol>> {
    let mut modules = BTreeMap::new();
    collect_symbol_surface_module(
        &input.root_path,
        &input.local_symbols_by_path,
        &input.direct_reexports_by_path,
        &input.child_modules_by_path,
        &mut modules,
    );
    modules
}

fn collect_symbol_surface_module<Symbol: Clone>(
    path: &[String],
    local_symbols_by_path: &BTreeMap<SymbolSurfacePath, BTreeMap<String, Symbol>>,
    direct_reexports_by_path: &BTreeMap<SymbolSurfacePath, Vec<SymbolSurfaceExport<Symbol>>>,
    child_modules_by_path: &BTreeMap<SymbolSurfacePath, BTreeSet<String>>,
    modules: &mut BTreeMap<SymbolSurfacePath, SymbolSurfaceModule<Symbol>>,
) -> BTreeMap<String, Symbol> {
    if let Some(module) = modules.get(path) {
        return module.public_symbols.clone();
    }

    let local_symbols = local_symbols_by_path.get(path).cloned().unwrap_or_default();
    let mut candidates = direct_reexports_by_path
        .get(path)
        .cloned()
        .unwrap_or_default();
    if let Some(child_modules) = child_modules_by_path.get(path) {
        for child in child_modules {
            let mut child_path = path.to_vec();
            child_path.push(child.clone());
            candidates.push(SymbolSurfaceExport {
                module: child.clone(),
                symbols: collect_symbol_surface_module(
                    &child_path,
                    local_symbols_by_path,
                    direct_reexports_by_path,
                    child_modules_by_path,
                    modules,
                ),
            });
        }
    }

    let reexports = retain_public_symbol_reexports(candidates, local_symbols.keys());
    let mut public_symbols = local_symbols;
    for reexport in &reexports {
        public_symbols.extend(
            reexport
                .symbols
                .iter()
                .map(|(name, symbol)| (name.clone(), symbol.clone())),
        );
    }
    modules.insert(
        path.to_vec(),
        SymbolSurfaceModule {
            public_symbols: public_symbols.clone(),
            reexports,
        },
    );
    public_symbols
}

pub fn retain_public_symbol_reexports<'a, Symbol: Clone>(
    candidates: Vec<SymbolSurfaceExport<Symbol>>,
    local_names: impl IntoIterator<Item = &'a String>,
) -> Vec<SymbolSurfaceExport<Symbol>> {
    let local_names = local_names.into_iter().cloned().collect::<BTreeSet<_>>();
    let mut symbol_counts = BTreeMap::<String, usize>::new();
    for candidate in &candidates {
        for symbol in candidate.symbols.keys() {
            *symbol_counts.entry(symbol.clone()).or_default() += 1;
        }
    }

    candidates
        .into_iter()
        .map(|candidate| SymbolSurfaceExport {
            module: candidate.module,
            symbols: candidate
                .symbols
                .into_iter()
                .filter(|(symbol, _)| {
                    symbol_counts.get(symbol).copied() == Some(1) && !local_names.contains(symbol)
                })
                .collect(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_surface_filters_duplicate_child_symbols_without_hiding_unique_symbols() {
        let root = Vec::new();
        let input = SymbolSurfaceInput {
            root_path: root,
            local_symbols_by_path: BTreeMap::new(),
            direct_reexports_by_path: BTreeMap::new(),
            child_modules_by_path: BTreeMap::from([(
                Vec::new(),
                BTreeSet::from(["alpha".to_owned(), "beta".to_owned()]),
            )]),
        };
        let mut input = input;
        input.direct_reexports_by_path.insert(
            vec!["alpha".to_owned()],
            vec![export("alpha_leaf", ["Alpha", "Shared"])],
        );
        input.direct_reexports_by_path.insert(
            vec!["beta".to_owned()],
            vec![export("beta_leaf", ["Beta", "Shared"])],
        );

        let surface = plan_symbol_surface(input);
        let root = surface.get(&Vec::new()).expect("root surface");

        assert_eq!(
            root.public_symbols.keys().cloned().collect::<Vec<_>>(),
            vec!["Alpha", "Beta"]
        );
        assert_eq!(root.reexports[0].module, "alpha");
        assert_eq!(
            root.reexports[0]
                .symbols
                .keys()
                .cloned()
                .collect::<Vec<_>>(),
            vec!["Alpha"]
        );
        assert_eq!(root.reexports[1].module, "beta");
        assert_eq!(
            root.reexports[1]
                .symbols
                .keys()
                .cloned()
                .collect::<Vec<_>>(),
            vec!["Beta"]
        );
    }

    #[test]
    fn local_symbols_block_reexported_symbols_with_the_same_name() {
        let input = SymbolSurfaceInput {
            root_path: Vec::new(),
            local_symbols_by_path: BTreeMap::from([(Vec::new(), symbols(["Shared"]))]),
            direct_reexports_by_path: BTreeMap::from([(
                Vec::new(),
                vec![export("child", ["Child", "Shared"])],
            )]),
            child_modules_by_path: BTreeMap::new(),
        };

        let surface = plan_symbol_surface(input);
        let root = surface.get(&Vec::new()).expect("root surface");

        assert_eq!(
            root.public_symbols.keys().cloned().collect::<Vec<_>>(),
            vec!["Child", "Shared"]
        );
        assert_eq!(
            root.reexports[0]
                .symbols
                .keys()
                .cloned()
                .collect::<Vec<_>>(),
            vec!["Child"]
        );
    }

    fn export<const N: usize>(module: &str, names: [&str; N]) -> SymbolSurfaceExport<()> {
        SymbolSurfaceExport {
            module: module.to_owned(),
            symbols: symbols(names),
        }
    }

    fn symbols<const N: usize>(names: [&str; N]) -> BTreeMap<String, ()> {
        names
            .into_iter()
            .map(|name| (name.to_owned(), ()))
            .collect()
    }
}
