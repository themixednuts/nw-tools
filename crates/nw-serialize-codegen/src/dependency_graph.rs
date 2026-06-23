use std::collections::{BTreeMap, BTreeSet};

#[must_use]
pub fn sorted_strongly_connected_components<T>(graph: &BTreeMap<T, BTreeSet<T>>) -> Vec<Vec<T>>
where
    T: Clone + Ord,
{
    let nodes = graph_nodes(graph);
    let finish_order = finishing_order(graph, &nodes);
    let reverse = reversed_graph(graph, &nodes);

    let mut visited = BTreeSet::new();
    let mut components = Vec::new();
    for node in finish_order.into_iter().rev() {
        if visited.contains(&node) {
            continue;
        }

        let mut component = collect_reachable(&reverse, node, &mut visited);
        component.sort();
        components.push(component);
    }
    components.sort();
    components
}

fn graph_nodes<T>(graph: &BTreeMap<T, BTreeSet<T>>) -> BTreeSet<T>
where
    T: Clone + Ord,
{
    let mut nodes = BTreeSet::new();
    for (source, targets) in graph {
        nodes.insert(source.clone());
        nodes.extend(targets.iter().cloned());
    }
    nodes
}

fn finishing_order<T>(graph: &BTreeMap<T, BTreeSet<T>>, nodes: &BTreeSet<T>) -> Vec<T>
where
    T: Clone + Ord,
{
    let mut visited = BTreeSet::new();
    let mut order = Vec::with_capacity(nodes.len());
    for root in nodes {
        if visited.contains(root) {
            continue;
        }

        let mut stack = vec![(root.clone(), false)];
        while let Some((node, expanded)) = stack.pop() {
            if expanded {
                order.push(node);
                continue;
            }

            if !visited.insert(node.clone()) {
                continue;
            }

            stack.push((node.clone(), true));
            if let Some(targets) = graph.get(&node) {
                for target in targets.iter().rev() {
                    if !visited.contains(target) {
                        stack.push((target.clone(), false));
                    }
                }
            }
        }
    }
    order
}

fn reversed_graph<T>(
    graph: &BTreeMap<T, BTreeSet<T>>,
    nodes: &BTreeSet<T>,
) -> BTreeMap<T, BTreeSet<T>>
where
    T: Clone + Ord,
{
    let mut reverse = BTreeMap::<T, BTreeSet<T>>::new();
    for node in nodes {
        reverse.entry(node.clone()).or_default();
    }
    for (source, targets) in graph {
        for target in targets {
            reverse
                .entry(target.clone())
                .or_default()
                .insert(source.clone());
        }
    }
    reverse
}

fn collect_reachable<T>(
    graph: &BTreeMap<T, BTreeSet<T>>,
    root: T,
    visited: &mut BTreeSet<T>,
) -> Vec<T>
where
    T: Clone + Ord,
{
    let mut component = Vec::new();
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        if !visited.insert(node.clone()) {
            continue;
        }

        component.push(node.clone());
        if let Some(targets) = graph.get(&node) {
            for target in targets.iter().rev() {
                if !visited.contains(target) {
                    stack.push(target.clone());
                }
            }
        }
    }
    component
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use super::sorted_strongly_connected_components;

    #[test]
    fn partitions_cycles_and_singletons_deterministically() {
        let graph = BTreeMap::from([
            (1_u32, BTreeSet::from([2])),
            (2, BTreeSet::from([1, 3])),
            (3, BTreeSet::new()),
            (4, BTreeSet::from([5])),
            (5, BTreeSet::from([4])),
        ]);

        assert_eq!(
            sorted_strongly_connected_components(&graph),
            vec![vec![1, 2], vec![3], vec![4, 5]]
        );
    }

    #[test]
    fn includes_nodes_that_only_appear_as_targets() {
        let graph = BTreeMap::from([(1_u32, BTreeSet::from([2]))]);

        assert_eq!(
            sorted_strongly_connected_components(&graph),
            vec![vec![1], vec![2]]
        );
    }

    #[test]
    fn handles_deep_cycles_without_recursive_walks() {
        let mut graph = BTreeMap::new();
        for node in 0_u32..4096 {
            graph.insert(node, BTreeSet::from([(node + 1) % 4096]));
        }

        let components = sorted_strongly_connected_components(&graph);

        assert_eq!(components.len(), 1);
        assert_eq!(components[0].len(), 4096);
        assert_eq!(components[0][0], 0);
        assert_eq!(components[0][4095], 4095);
    }
}
