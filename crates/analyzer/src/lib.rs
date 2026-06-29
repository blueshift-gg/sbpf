pub mod critical_path;
pub mod dump_cfg;
pub mod remove_dead_functions;

pub use {
    critical_path::{CriticalPathError, CriticalPathResult, critical_path, dump_critical_path},
    dump_cfg::{dump_cfg, dump_cfg_with_critical_path},
    remove_dead_functions::{RemovedFunction, remove_dead_functions},
};
