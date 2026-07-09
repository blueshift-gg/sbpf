//! Generic traversal and fixed-point iteration engines used by SBPF IR.

pub mod dfs;
pub mod worklist;

pub use {
    dfs::{DfsEngine, DfsGraph, DfsVisitor},
    worklist::{Analysis, WorklistEngine, WorklistVisitor, fixed_point_analyze},
};
