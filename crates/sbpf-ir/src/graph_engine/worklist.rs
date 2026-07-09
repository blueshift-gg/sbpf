//! Worklist traversal and fixed-point analysis.

use {
    crate::graph_engine::dfs::DfsGraph,
    std::collections::{HashMap, HashSet, VecDeque},
};

/// Visitor called for each node during a worklist traversal.
///
/// The `enqueue` callback lets the visitor inject additional nodes that are not
/// direct graph successors — for example, when discovering a new callee function
/// and wanting to add *all* of its blocks rather than just the one reachable via
/// a CFG edge.
pub trait WorklistVisitor<N> {
    fn visit(&mut self, node: N, enqueue: &mut dyn FnMut(N));
}

/// Blanket impl so that a plain `FnMut(N)` closure works as a visitor when no
/// extra enqueueing is needed.
impl<N, F: FnMut(N)> WorklistVisitor<N> for F {
    fn visit(&mut self, node: N, _enqueue: &mut dyn FnMut(N)) {
        self(node);
    }
}

/// Stateful BFS worklist engine.
///
/// Usage pattern:
/// ```ignore
/// let mut engine = WorklistEngine::new(graph);
/// engine.initialize(seeds);
/// engine.run(&mut visitor);
/// let visited = engine.visited();
/// ```
pub struct WorklistEngine<'a, G: DfsGraph> {
    graph: &'a G,
    pending: HashSet<G::Node>,
    worklist: VecDeque<G::Node>,
    visited: HashSet<G::Node>,
}

impl<'a, G: DfsGraph> WorklistEngine<'a, G> {
    pub fn new(graph: &'a G) -> Self {
        Self {
            graph,
            pending: HashSet::new(),
            worklist: VecDeque::new(),
            visited: HashSet::new(),
        }
    }

    /// Seed the worklist with an initial set of nodes.
    pub fn initialize(&mut self, items: impl IntoIterator<Item = G::Node>) -> &mut Self {
        for item in items {
            self.enqueue(item);
        }
        self
    }

    /// Add a single node to the worklist. No-op if already visited or pending.
    pub fn enqueue(&mut self, item: G::Node) -> &mut Self {
        if !self.visited.contains(&item) && self.pending.insert(item) {
            self.worklist.push_back(item);
        }
        self
    }

    /// Process the worklist until empty.
    ///
    /// For each dequeued node, the visitor is called with `(node, enqueue)`.
    /// After the visitor returns, all direct graph successors are also enqueued.
    /// The `enqueue` callback lets the visitor inject additional nodes beyond
    /// those covered by graph edges.
    pub fn run<V: WorklistVisitor<G::Node>>(&mut self, visitor: &mut V) {
        while let Some(node) = self.worklist.pop_front() {
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
                    self.worklist.push_back(successor);
                }
            }
            // Enqueue visitor-requested extras.
            for item in extra {
                if !self.visited.contains(&item) && self.pending.insert(item) {
                    self.worklist.push_back(item);
                }
            }
        }
    }

    /// Returns the set of nodes visited since construction (or last reset).
    pub fn visited(&self) -> &HashSet<G::Node> {
        &self.visited
    }
}

// ── Fixed-point analysis ─────────────────────────────────────────────────────

/// Lattice-based fixed-point analysis over a graph.
pub trait Analysis<N> {
    type State: Clone + PartialEq;

    fn transfer(&mut self, node: N, state: &Self::State) -> Self::State;

    fn join(&self, a: &Self::State, b: &Self::State) -> Self::State;
}

/// Run a fixed-point analysis from multiple start nodes, iterating until the
/// per-node state stops changing.
pub fn fixed_point_analyze<G, I, A>(
    graph: &G,
    starts: I,
    initial_state: A::State,
    analysis: &mut A,
) -> HashMap<G::Node, A::State>
where
    G: DfsGraph,
    I: IntoIterator<Item = G::Node>,
    A: Analysis<G::Node>,
{
    let mut states: HashMap<G::Node, A::State> = HashMap::new();
    let mut pending: HashSet<G::Node> = HashSet::new();
    let mut worklist: VecDeque<G::Node> = VecDeque::new();

    for node in starts {
        states.insert(node, initial_state.clone());
        if pending.insert(node) {
            worklist.push_back(node);
        }
    }

    while let Some(node) = worklist.pop_front() {
        pending.remove(&node);

        let Some(input_state) = states.get(&node).cloned() else {
            continue;
        };
        let output_state = analysis.transfer(node, &input_state);

        for successor in graph.successors(node) {
            let next_state = states
                .get(successor)
                .map(|state| analysis.join(state, &output_state))
                .unwrap_or_else(|| output_state.clone());

            if states.get(successor) != Some(&next_state) {
                states.insert(*successor, next_state);
                if pending.insert(*successor) {
                    worklist.push_back(*successor);
                }
            }
        }
    }

    states
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
    fn test_worklist_visit_uses_fifo_order() {
        let graph = TestGraph {
            successors: vec![vec![1, 2], vec![3], vec![3], vec![]],
        };
        let mut visited = Vec::new();

        WorklistEngine::new(&graph)
            .initialize([0])
            .run(&mut |node| visited.push(node));

        assert_eq!(visited, vec![0, 1, 2, 3]);
    }

    #[test]
    fn test_worklist_visit_suppresses_duplicates() {
        let graph = TestGraph {
            successors: vec![vec![1, 1], vec![]],
        };
        let mut visited = Vec::new();

        WorklistEngine::new(&graph)
            .initialize([0, 0])
            .run(&mut |node| visited.push(node));

        assert_eq!(visited, vec![0, 1]);
    }

    #[test]
    fn test_worklist_enqueue_adds_extra_nodes() {
        // Graph: 0 -> 1. Visitor on block 0 also enqueues block 2 (not a graph successor).
        let graph = TestGraph {
            successors: vec![vec![1], vec![], vec![]],
        };

        struct FanOutVisitor(Vec<usize>);
        impl WorklistVisitor<usize> for FanOutVisitor {
            fn visit(&mut self, node: usize, enqueue: &mut dyn FnMut(usize)) {
                self.0.push(node);
                if node == 0 {
                    enqueue(2);
                }
            }
        }

        let mut visitor = FanOutVisitor(Vec::new());
        WorklistEngine::new(&graph)
            .initialize([0])
            .run(&mut visitor);

        assert_eq!(visitor.0, vec![0, 1, 2]);
    }

    #[test]
    fn test_worklist_visited_reflects_processed_nodes() {
        let graph = TestGraph {
            successors: vec![vec![1, 2], vec![], vec![]],
        };

        let mut engine = WorklistEngine::new(&graph);
        engine.initialize([0]).run(&mut |_| {});

        assert!(engine.visited().contains(&0));
        assert!(engine.visited().contains(&1));
        assert!(engine.visited().contains(&2));
        assert!(!engine.visited().contains(&3));
    }

    #[test]
    fn test_fixed_point_analyze_applies_transfer_and_join() {
        struct ReachabilityAnalysis;

        impl Analysis<usize> for ReachabilityAnalysis {
            type State = Vec<usize>;

            fn transfer(&mut self, node: usize, state: &Self::State) -> Self::State {
                let mut state = state.clone();
                if !state.contains(&node) {
                    state.push(node);
                    state.sort_unstable();
                }
                state
            }

            fn join(&self, a: &Self::State, b: &Self::State) -> Self::State {
                let mut state = a.clone();
                for node in b {
                    if !state.contains(node) {
                        state.push(*node);
                    }
                }
                state.sort_unstable();
                state
            }
        }

        let graph = TestGraph {
            successors: vec![vec![1, 2], vec![3], vec![3], vec![]],
        };
        let mut analysis = ReachabilityAnalysis;

        let states = fixed_point_analyze(&graph, [0], Vec::new(), &mut analysis);

        assert_eq!(states.get(&0).unwrap(), &vec![]);
        assert_eq!(states.get(&1).unwrap(), &vec![0]);
        assert_eq!(states.get(&2).unwrap(), &vec![0]);
        assert_eq!(states.get(&3).unwrap(), &vec![0, 1, 2]);
    }
}
