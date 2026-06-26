//! Depth-first graph traversal.

use std::{collections::HashSet, hash::Hash};

pub trait DfsGraph {
    type Node: Copy + Eq + Hash;

    fn successors(&self, node: Self::Node) -> &[Self::Node];
}

/// Visitor called for each node during a DFS traversal.
pub trait DfsVisitor<N> {
    fn visit(&mut self, node: N);
}

/// Blanket impl so that a plain `FnMut(N)` closure works as a visitor.
impl<N, F: FnMut(N)> DfsVisitor<N> for F {
    fn visit(&mut self, node: N) {
        self(node);
    }
}

pub struct DfsEngine<'a, G> {
    graph: &'a G,
}

impl<'a, G> DfsEngine<'a, G>
where
    G: DfsGraph,
{
    pub fn new(graph: &'a G) -> Self {
        Self { graph }
    }

    pub fn visit<V>(&self, start: G::Node, visitor: &mut V)
    where
        V: DfsVisitor<G::Node>,
    {
        self.visit_many([start], visitor);
    }

    pub fn visit_many<I, V>(&self, starts: I, visitor: &mut V)
    where
        I: IntoIterator<Item = G::Node>,
        V: DfsVisitor<G::Node>,
    {
        let mut visited = HashSet::new();
        let mut stack = starts.into_iter().collect::<Vec<_>>();

        while let Some(node) = stack.pop() {
            if !visited.insert(node) {
                continue;
            }

            visitor.visit(node);

            for successor in self.graph.successors(node).iter().rev() {
                stack.push(*successor);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestGraph {
        successors: Vec<Vec<usize>>,
    }

    impl DfsGraph for TestGraph {
        type Node = usize;

        fn successors(&self, node: Self::Node) -> &[Self::Node] {
            self.successors
                .get(node)
                .map(Vec::as_slice)
                .unwrap_or_default()
        }
    }

    #[test]
    fn test_dfs_visit() {
        let graph = TestGraph {
            successors: vec![vec![1, 2], vec![3], vec![3], vec![]],
        };
        let mut visited = Vec::new();

        DfsEngine::new(&graph).visit(0, &mut |node| visited.push(node));

        assert_eq!(visited, vec![0, 1, 3, 2]);
    }
}
