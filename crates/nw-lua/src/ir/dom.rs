//! Dominators and dominance frontiers.

use petgraph::{Directed, Graph, algo::dominators::simple_fast, graph::NodeIndex};

use super::BasicBlock;

/// Stored dominator analysis results.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DomInfo {
    pub idom: Vec<Option<usize>>,
    pub dom_children: Vec<Vec<usize>>,
    pub dominance_frontiers: Vec<Vec<usize>>,
}

impl DomInfo {
    /// Return whether `dominator` dominates `block` in the stored dominator tree.
    #[must_use]
    pub fn dominates(&self, dominator: usize, block: usize) -> bool {
        if dominator == block {
            return block < self.idom.len();
        }
        let mut current = self.idom.get(block).copied().flatten();
        let mut remaining = self.idom.len();
        while let Some(candidate) = current {
            if candidate == dominator {
                return true;
            }
            if remaining == 0 {
                return false;
            }
            remaining -= 1;
            current = self.idom.get(candidate).copied().flatten();
        }
        false
    }
}

/// Compute immediate dominators, dominator-tree children, and dominance frontiers.
#[must_use]
pub fn analyze(blocks: &[BasicBlock]) -> DomInfo {
    if blocks.is_empty() {
        return DomInfo {
            idom: Vec::new(),
            dom_children: Vec::new(),
            dominance_frontiers: Vec::new(),
        };
    }

    let (graph, nodes) = build_graph(blocks);
    let dominators = simple_fast(&graph, nodes[0]);
    let mut idom = vec![None; blocks.len()];
    for block in 1..blocks.len() {
        idom[block] = dominators
            .immediate_dominator(nodes[block])
            .map(NodeIndex::index)
            .filter(|dom| *dom != block);
    }

    let mut dom_children = vec![Vec::new(); blocks.len()];
    for (block, parent) in idom.iter().copied().enumerate().skip(1) {
        if let Some(parent) = parent {
            dom_children[parent].push(block);
        }
    }

    let dominance_frontiers = compute_frontiers(blocks, &idom);
    DomInfo {
        idom,
        dom_children,
        dominance_frontiers,
    }
}

/// Copy stored dominator results onto blocks for convenient inspection.
pub fn apply_to_blocks(blocks: &mut [BasicBlock], dom: &DomInfo) {
    for block in blocks {
        block.idom = dom.idom.get(block.index).copied().flatten();
        block.dom_children = dom
            .dom_children
            .get(block.index)
            .cloned()
            .unwrap_or_default();
        block.dominance_frontier = dom
            .dominance_frontiers
            .get(block.index)
            .cloned()
            .unwrap_or_default();
    }
}

fn build_graph(blocks: &[BasicBlock]) -> (Graph<(), (), Directed>, Vec<NodeIndex>) {
    let mut graph = Graph::<(), (), Directed>::new();
    let nodes = (0..blocks.len())
        .map(|_| graph.add_node(()))
        .collect::<Vec<_>>();
    for block in blocks {
        for &succ in &block.succs {
            if succ < nodes.len() {
                graph.add_edge(nodes[block.index], nodes[succ], ());
            }
        }
    }
    (graph, nodes)
}

fn compute_frontiers(blocks: &[BasicBlock], idom: &[Option<usize>]) -> Vec<Vec<usize>> {
    let mut frontiers = vec![Vec::new(); blocks.len()];
    for block in blocks {
        if block.preds.len() < 2 {
            continue;
        }
        for &pred in &block.preds {
            let mut runner = pred;
            while Some(runner) != idom[block.index] {
                if !frontiers[runner].contains(&block.index) {
                    frontiers[runner].push(block.index);
                }
                let Some(next) = idom[runner] else {
                    break;
                };
                if next == runner {
                    break;
                }
                runner = next;
            }
        }
    }
    for frontier in &mut frontiers {
        frontier.sort_unstable();
    }
    frontiers
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diamond_dominators_and_frontiers_are_exact() {
        let blocks = vec![
            BasicBlock::synthetic(0, vec![1, 2], vec![]),
            BasicBlock::synthetic(1, vec![3], vec![0]),
            BasicBlock::synthetic(2, vec![3], vec![0]),
            BasicBlock::synthetic(3, vec![], vec![1, 2]),
        ];

        let dom = analyze(&blocks);

        assert_eq!(dom.idom, vec![None, Some(0), Some(0), Some(0)]);
        assert_eq!(
            dom.dom_children,
            vec![vec![1, 2, 3], vec![], vec![], vec![]]
        );
        assert_eq!(
            dom.dominance_frontiers,
            vec![vec![], vec![3], vec![3], vec![]]
        );
        assert!(dom.dominates(0, 3));
        assert!(!dom.dominates(1, 3));
    }

    #[test]
    fn loop_dominators_and_frontiers_are_exact() {
        let blocks = vec![
            BasicBlock::synthetic(0, vec![1], vec![]),
            BasicBlock::synthetic(1, vec![2, 3], vec![0, 2]),
            BasicBlock::synthetic(2, vec![1], vec![1]),
            BasicBlock::synthetic(3, vec![], vec![1]),
        ];

        let dom = analyze(&blocks);

        assert_eq!(dom.idom, vec![None, Some(0), Some(1), Some(1)]);
        assert_eq!(dom.dom_children, vec![vec![1], vec![2, 3], vec![], vec![]]);
        assert_eq!(
            dom.dominance_frontiers,
            vec![vec![], vec![1], vec![1], vec![]]
        );
        assert!(dom.dominates(1, 2));
        assert!(!dom.dominates(2, 3));
    }
}
