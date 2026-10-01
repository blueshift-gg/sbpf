//! Re-visitable worklist for fixed-point iteration.

use std::{
    collections::{HashSet, VecDeque},
    hash::Hash,
};

/// FIFO worklist that de-duplicates pending items.
///
/// Unlike [`BfsEngine`](crate::graph_engine::BfsEngine), an item may be pushed
/// again after it has been popped, so it can drive fixed-point iterations where
/// a node must be revisited whenever its inputs change.
///
/// Usage pattern:
/// ```ignore
/// let mut worklist = WorklistEngine::new();
/// worklist.extend(seeds);
/// while let Some(node) = worklist.pop() {
///     if update(node) {
///         worklist.extend(dependents(node));
///     }
/// }
/// ```
#[derive(Debug, Clone)]
pub struct WorklistEngine<N> {
    pending: HashSet<N>,
    queue: VecDeque<N>,
}

impl<N> Default for WorklistEngine<N> {
    fn default() -> Self {
        Self {
            pending: HashSet::new(),
            queue: VecDeque::new(),
        }
    }
}

impl<N: Copy + Eq + Hash> WorklistEngine<N> {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add an item. No-op if it is already pending.
    pub fn push(&mut self, item: N) {
        if self.pending.insert(item) {
            self.queue.push_back(item);
        }
    }

    /// Add several items, skipping those already pending.
    pub fn extend(&mut self, items: impl IntoIterator<Item = N>) {
        for item in items {
            self.push(item);
        }
    }

    /// Remove the oldest pending item.
    pub fn pop(&mut self) -> Option<N> {
        let item = self.queue.pop_front()?;
        self.pending.remove(&item);
        Some(item)
    }

    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_worklist_pops_in_fifo_order() {
        let mut worklist = WorklistEngine::new();
        worklist.extend([3, 1, 2]);

        assert_eq!(worklist.pop(), Some(3));
        assert_eq!(worklist.pop(), Some(1));
        assert_eq!(worklist.pop(), Some(2));
        assert_eq!(worklist.pop(), None);
        assert!(worklist.is_empty());
    }

    #[test]
    fn test_worklist_suppresses_pending_duplicates() {
        let mut worklist = WorklistEngine::new();
        worklist.extend([1, 2, 1, 2]);

        assert_eq!(worklist.pop(), Some(1));
        assert_eq!(worklist.pop(), Some(2));
        assert_eq!(worklist.pop(), None);
    }

    #[test]
    fn test_worklist_allows_repush_after_pop() {
        let mut worklist = WorklistEngine::new();
        worklist.push(1);

        assert_eq!(worklist.pop(), Some(1));
        worklist.push(1);
        assert_eq!(worklist.pop(), Some(1));
        assert_eq!(worklist.pop(), None);
    }
}
