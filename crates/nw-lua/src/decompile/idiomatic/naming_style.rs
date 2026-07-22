use std::collections::HashMap;

use bstr::BString;
use heck::{ToLowerCamelCase, ToUpperCamelCase};

use crate::decompile::{
    ast::{
        BindingId, BindingUsage, Block, Expr, FunctionName, Name, Stmt,
        binding_spelling_available_in_block, binding_usages_in_block, rename_binding_in_block,
    },
    naming::is_valid_identifier,
};

use super::engine::{CleanContext, Rewrite, Rule};

pub struct ModuleTableName;

impl Rule for ModuleTableName {
    fn rewrite_block(&self, block: Block, ctx: &CleanContext) -> Rewrite<Block> {
        if !ctx.in_root_function() {
            return Rewrite::unchanged(block);
        }
        let Some(stem) = ctx.module_stem.as_deref() else {
            return Rewrite::unchanged(block);
        };
        let Some(module) = recognized_module_binding(&block) else {
            return Rewrite::unchanged(block);
        };
        if !module.name.is_synthetic() {
            return Rewrite::unchanged(block);
        }

        let candidate = module_pascal_name(stem);
        let candidate_bytes = BString::from(candidate.as_str());
        if !is_valid_identifier(&candidate_bytes)
            || module.name.as_bytes() == candidate_bytes.as_slice()
            || !binding_spelling_available_in_block(
                &block,
                &module.identity,
                candidate_bytes.as_slice(),
            )
        {
            return Rewrite::unchanged(block);
        }

        let mut block = block;
        rename_binding_in_block(&mut block, &module.identity, candidate_bytes.as_slice());
        Rewrite::changed(block)
    }
}

pub struct ConsumerFieldTableName;

impl Rule for ConsumerFieldTableName {
    fn rewrite_block(&self, block: Block, _ctx: &CleanContext) -> Rewrite<Block> {
        let usages = binding_usages_in_block(&block);
        let tables = consumer_field_table_bindings(&block, &usages);
        if tables.is_empty() {
            return Rewrite::unchanged(block);
        }

        let mut block = block;
        let mut changed = false;
        for table in tables {
            let Ok(field) = std::str::from_utf8(table.field.as_bytes()) else {
                continue;
            };
            let candidate = field.to_lower_camel_case();
            let candidate = BString::from(candidate.as_str());
            if table.name.as_bytes() == candidate.as_slice()
                || !is_valid_identifier(&candidate)
                || !binding_spelling_available_in_block(
                    &block,
                    &table.identity,
                    candidate.as_slice(),
                )
            {
                continue;
            }
            rename_binding_in_block(&mut block, &table.identity, candidate.as_slice());
            changed = true;
        }
        if changed {
            Rewrite::changed(block)
        } else {
            Rewrite::unchanged(block)
        }
    }
}

#[derive(Debug)]
struct ModuleBinding {
    identity: BindingId,
    name: Name,
}

#[derive(Debug)]
struct ConsumerFieldTableBinding {
    identity: BindingId,
    name: Name,
    field: Name,
}

fn consumer_field_table_bindings(
    block: &Block,
    usages: &HashMap<BindingId, BindingUsage>,
) -> Vec<ConsumerFieldTableBinding> {
    block
        .0
        .iter()
        .enumerate()
        .filter_map(|(consumer_index, stmt)| {
            let (value, field) = field_store(stmt)?;
            let identity = value.binding()?.clone();
            if !value.is_synthetic() {
                return None;
            }
            let declared = block.0[..consumer_index].iter().any(|stmt| {
                local_table_name(stmt).is_some_and(|name| name.binding() == Some(&identity))
            });
            if !declared {
                return None;
            }
            let usage = usages.get(&identity).copied().unwrap_or_default();
            (usage.receiver_reads() > 0 && usage.value_reads() == 1).then(|| {
                ConsumerFieldTableBinding {
                    identity,
                    name: value.clone(),
                    field: field.clone(),
                }
            })
        })
        .collect()
}

fn field_store(stmt: &Stmt) -> Option<(&Name, &Name)> {
    let Stmt::Assign { targets, values } = stmt else {
        return None;
    };
    let ([Expr::Field { name: field, .. }], [Expr::Name(value)]) =
        (targets.as_slice(), values.as_slice())
    else {
        return None;
    };
    Some((value, field))
}

fn recognized_module_binding(block: &Block) -> Option<ModuleBinding> {
    let returned = returned_name(block)?;
    let identity = returned.binding()?.clone();
    let table_index = block.0.iter().position(|stmt| {
        local_table_name(stmt).is_some_and(|name| name.binding() == Some(&identity))
    })?;
    let has_members = block
        .0
        .iter()
        .enumerate()
        .any(|(index, stmt)| index > table_index && module_member_stmt(stmt, &identity));
    has_members.then(|| ModuleBinding {
        identity,
        name: returned.clone(),
    })
}

fn returned_name(block: &Block) -> Option<&Name> {
    let Some(Stmt::Return(values)) = block.0.last() else {
        return None;
    };
    let [Expr::Name(name)] = values.as_slice() else {
        return None;
    };
    Some(name)
}

fn local_table_name(stmt: &Stmt) -> Option<&Name> {
    let Stmt::Local {
        names,
        attribs,
        values,
    } = stmt
    else {
        return None;
    };
    let ([name], [Expr::Table(_)]) = (names.as_slice(), values.as_slice()) else {
        return None;
    };
    attribs.is_empty().then_some(name)
}

fn module_member_stmt(stmt: &Stmt, module: &BindingId) -> bool {
    match stmt {
        Stmt::Assign { targets, .. } => targets.iter().any(|target| {
            target_base_name(target).is_some_and(|name| name.binding() == Some(module))
        }),
        Stmt::FunctionDecl { name, .. } => {
            path_base(name).is_some_and(|name| name.binding() == Some(module))
        }
        _ => false,
    }
}

fn target_base_name(expr: &Expr) -> Option<&Name> {
    match expr {
        Expr::Name(name) => Some(name),
        Expr::Field { obj, .. } | Expr::Index { obj, .. } => target_base_name(obj),
        _ => None,
    }
}

fn path_base(name: &FunctionName) -> Option<&Name> {
    name.path.first()
}

fn module_pascal_name(stem: &str) -> String {
    normalized_module_stem(stem).to_upper_camel_case()
}

fn normalized_module_stem(stem: &str) -> String {
    stem.chars()
        .map(|ch| {
            if ch == '-' || ch == '.' || ch.is_whitespace() {
                '_'
            } else {
                ch
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decompile::ast::FunctionId;

    fn bound(name: &str, binding: &BindingId) -> Name {
        Name::synthetic(name).with_binding(binding.clone())
    }

    fn table_local(name: Name) -> Stmt {
        Stmt::Local {
            names: vec![name],
            attribs: Vec::new(),
            values: vec![Expr::Table(Vec::new())],
        }
    }

    fn field(obj: Expr, name: &str) -> Expr {
        Expr::Field {
            obj: Box::new(obj),
            name: Name::new(name),
        }
    }

    #[test]
    fn consumer_field_names_synthetic_table_binding() {
        let function = FunctionId::root();
        let options = BindingId::debug_local(&function, 0);
        let table = BindingId::synthetic(&function, 1);
        let options_name = Name::new("Options").with_binding(options);
        let table_name = bound("l1", &table);
        let block = Block::new(vec![
            table_local(options_name.clone()),
            table_local(table_name.clone()),
            Stmt::Assign {
                targets: vec![field(Expr::Name(table_name.clone()), "Width")],
                values: vec![Expr::Integer(10)],
            },
            Stmt::Assign {
                targets: vec![field(Expr::Name(options_name), "Properties")],
                values: vec![Expr::Name(table_name)],
            },
        ]);

        let rewrite = ConsumerFieldTableName.rewrite_block(block, &CleanContext::new(None));

        assert!(rewrite.changed);
        let Stmt::Local { names, .. } = &rewrite.value.0[1] else {
            panic!("expected table local")
        };
        assert_eq!(names[0].as_bytes(), b"properties");
        let Stmt::Assign { values, .. } = &rewrite.value.0[3] else {
            panic!("expected field store")
        };
        let [Expr::Name(value)] = values.as_slice() else {
            panic!("expected stored table binding")
        };
        assert_eq!(value.as_bytes(), b"properties");
    }

    #[test]
    fn consumer_field_name_skips_numeric_and_dynamic_index_stores() {
        let function = FunctionId::root();
        for key in [Expr::Integer(1), Expr::Name(Name::new("key"))] {
            let options = BindingId::debug_local(&function, 0);
            let table = BindingId::synthetic(&function, 1);
            let options_name = Name::new("Options").with_binding(options);
            let table_name = bound("l1", &table);
            let block = Block::new(vec![
                table_local(options_name.clone()),
                table_local(table_name.clone()),
                Stmt::Assign {
                    targets: vec![field(Expr::Name(table_name.clone()), "Width")],
                    values: vec![Expr::Integer(10)],
                },
                Stmt::Assign {
                    targets: vec![Expr::Index {
                        obj: Box::new(Expr::Name(options_name.clone())),
                        key: Box::new(key),
                    }],
                    values: vec![Expr::Name(table_name)],
                },
            ]);

            let rewrite = ConsumerFieldTableName.rewrite_block(block, &CleanContext::new(None));

            assert!(!rewrite.changed);
            let Stmt::Local { names, .. } = &rewrite.value.0[1] else {
                panic!("expected table local")
            };
            assert_eq!(names[0].as_bytes(), b"l1");
        }
    }
}
