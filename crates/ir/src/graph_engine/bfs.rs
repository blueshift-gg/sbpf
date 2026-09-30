//! Breadth-first, visit-once graph traversal.

use {
    crate::graph_engine::dfs::DfsGraph,
    std::collections::{HashSet, VecDeque},
};

/// Visitor called for each node during a BFS traversal.
///
/// The `enqueue` callback lets the visitor inject additional nodes that are not
/// direct graph successors — for example, when discovering a new callee function
/// and wanting to add *all* of its blocks rather than just the one reachable via
/// a CFG edge.
pub trait BfsVisitor<N> {
    fn visit(&mut self, node: N, enqueue: &mut dyn FnMut(N));
}

/// Blanket impl so that a plain `FnMut(N)` closure works as a visitor when no
/// extra enqueueing is needed.
impl<N, F: FnMut(N)> BfsVisitor<N> for F {
    fn visit(&mut self, node: N, _enqueue: &mut dyn FnMut(N)) {
        self(node);
    }
}

/// Stateful BFS engine. Each node is visited at most once.
///
/// Usage pattern:
/// ```ignore
/// let mut engine = BfsEngine::new(graph);
/// engine.initialize(seeds);
/// engine.run(&mut visitor);
/// let visited = engine.visited();
/// ```
pub struct BfsEngine<'a, G: DfsGraph> {
    graph: &'a G,
    pending: HashSet<G::Node>,
    queue: VecDeque<G::Node>,
    visited: HashSet<G::Node>,
}

impl<'a, G: DfsGraph> BfsEngine<'a, G> {
    pub fn new(graph: &'a G) -> Self {
        Self {
            graph,
            pending: HashSet::new(),
            queue: VecDeque::new(),
            visited: HashSet::new(),
        }
    }

    /// Seed the queue with an initial set of nodes.
    pub fn initialize(&mut self, items: impl IntoIterator<Item = G::Node>) -> &mut Self {
        for item in items {
            self.enqueue(item);
        }
        self
    }

    /// Add a single node to the queue. No-op if already visited or pending.
    pub fn enqueue(&mut self, item: G::Node) -> &mut Self {
        if !self.visited.contains(&item) && self.pending.insert(item) {
            self.queue.push_back(item);
        }
        self
    }

    /// Process the queue until empty.
    ///
    /// For each dequeued node, the visitor is called with `(node, enqueue)`.
    /// After the visitor returns, all direct graph successors are also enqueued.
    /// The `enqueue` callback lets the visitor inject additional nodes beyond
    /// those covered by graph edges.
    pub fn run<V: BfsVisitor<G::Node>>(&mut self, visitor: &mut V) {
        while let Some(node) = self.queue.pop_front() {
            self.pending.remove(&node);
            if !self.visited.insert(node) {
                continue;
            }

            // Collect any extra nodes the visitor wants to enqueue.
            let mut extra = Vec::new();
            visitor.visit(node, &mut |item| extra.push(item));

            // Enqueue direct graph successors.
            for &successor in self.graph.successors(node) {
                if !self.visited.contains(&successor) && self.pending.insert(successor) {
                    self.queue.push_back(successor);
                }
            }
            // Enqueue visitor-requested extras.
            for item in extra {
                if !self.visited.contains(&item) && self.pending.insert(item) {
                    self.queue.push_back(item);
                }
            }
        }
    }

    /// Returns the set of nodes visited since construction (or last reset).
    pub fn visited(&self) -> &HashSet<G::Node> {
        &self.visited
    }
}

#[cfg(test)]
mod tests {
    use {super::*, crate::graph_engine::dfs::DfsGraph};

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
    fn test_bfs_visit_uses_fifo_order() {
        let graph = TestGraph {
            successors: vec![vec![1, 2], vec![3], vec![3], vec![]],
        };
        let mut visited = Vec::new();

        BfsEngine::new(&graph)
            .initialize([0])
            .run(&mut |node| visited.push(node));

        assert_eq!(visited, vec![0, 1, 2, 3]);
    }

    #[test]
    fn test_bfs_visit_suppresses_duplicates() {
        let graph = TestGraph {
            successors: vec![vec![1, 1], vec![]],
        };
        let mut visited = Vec::new();

        BfsEngine::new(&graph)
            .initialize([0, 0])
            .run(&mut |node| visited.push(node));

        assert_eq!(visited, vec![0, 1]);
    }

    #[test]
    fn test_bfs_enqueue_adds_extra_nodes() {
        // Graph: 0 -> 1. Visitor on block 0 also enqueues block 2 (not a graph successor).
        let graph = TestGraph {
            successors: vec![vec![1], vec![], vec![]],
        };

        struct FanOutVisitor(Vec<usize>);
        impl BfsVisitor<usize> for FanOutVisitor {
            fn visit(&mut self, node: usize, enqueue: &mut dyn FnMut(usize)) {
                self.0.push(node);
                if node == 0 {
                    enqueue(2);
                }
            }
        }

        let mut visitor = FanOutVisitor(Vec::new());
        BfsEngine::new(&graph).initialize([0]).run(&mut visitor);

        assert_eq!(visitor.0, vec![0, 1, 2]);
    }

    #[test]
    fn test_bfs_visited_reflects_processed_nodes() {
        let graph = TestGraph {
            successors: vec![vec![1, 2], vec![], vec![]],
        };

        let mut engine = BfsEngine::new(&graph);
        engine.initialize([0]).run(&mut |_| {});

        assert!(engine.visited().contains(&0));
        assert!(engine.visited().contains(&1));
        assert!(engine.visited().contains(&2));
        assert!(!engine.visited().contains(&3));
    }
}
