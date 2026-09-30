//! Generic traversal and fixed-point iteration engines used by SBPF IR.

pub mod bfs;
pub mod dfs;
pub mod worklist;

pub use {
    bfs::{BfsEngine, BfsVisitor},
    dfs::{DfsEngine, DfsGraph, DfsVisitor},
    worklist::WorklistEngine,
};
