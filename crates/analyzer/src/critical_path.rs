use {
    sbpf_ir::{BlockId, Cfg},
    std::collections::{HashMap, HashSet, VecDeque},
};

/// Returned when a function contains a loop — the longest-path DP only works on
/// DAGs, so we bail out instead of producing an incorrect result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CriticalPathError {
    pub function_name: String,
}

/// The critical path through a single function and its total CU cost.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CriticalPathResult {
    pub function_name: String,
    /// Sum of CUs along the critical path.
    pub total_cu: u64,
    /// Blocks on the critical path in order from entry to exit.
    pub path: Vec<BlockId>,
    /// CU cost of every block in the function (1 per instruction).
    pub block_cu: HashMap<BlockId, u64>,
}

/// Computes the critical (maximum-CU) path for each function in `cfg`.
///
/// Each instruction costs 1 CU. Functions that contain loops return
/// `Err(CriticalPathError)` — the caller can skip or warn on those.
pub fn critical_path(cfg: &Cfg) -> Vec<Result<CriticalPathResult, CriticalPathError>> {
    cfg.functions()
        .iter()
        .map(|function| {
            let name = function.name().to_owned();
            let function_blocks: HashSet<BlockId> = function.block_ids().iter().copied().collect();

            // CU per block: 1 CU per instruction (simplified model).
            let block_cu: HashMap<BlockId, u64> = function
                .block_ids()
                .iter()
                .zip(function.blocks().iter())
                .map(|(&id, block)| (id, block.instructions().len() as u64))
                .collect();

            // Kahn's topological sort with cycle detection
            // Only consider edges that stay within this function.
            let mut in_degree: HashMap<BlockId, usize> =
                function_blocks.iter().map(|&id| (id, 0)).collect();

            for &block_id in function.block_ids() {
                for &succ in cfg.successors(block_id) {
                    if function_blocks.contains(&succ) {
                        *in_degree.get_mut(&succ).unwrap() += 1;
                    }
                }
            }

            let mut queue: VecDeque<BlockId> = in_degree
                .iter()
                .filter(|&(_, &deg)| deg == 0)
                .map(|(&id, _)| id)
                .collect();

            let mut topo_order: Vec<BlockId> = Vec::with_capacity(function_blocks.len());

            while let Some(block_id) = queue.pop_front() {
                topo_order.push(block_id);
                for &succ in cfg.successors(block_id) {
                    if function_blocks.contains(&succ) {
                        let deg = in_degree.get_mut(&succ).unwrap();
                        *deg -= 1;
                        if *deg == 0 {
                            queue.push_back(succ);
                        }
                    }
                }
            }

            if topo_order.len() != function_blocks.len() {
                return Err(CriticalPathError {
                    function_name: name,
                });
            }

            // dp[b] = maximum CU cost of any path from an entry block to b.
            // parent[b] = predecessor on that best path (None for entry blocks).
            let mut dp: HashMap<BlockId, u64> = HashMap::with_capacity(function_blocks.len());
            let mut parent: HashMap<BlockId, Option<BlockId>> =
                HashMap::with_capacity(function_blocks.len());

            for &block_id in &topo_order {
                let self_cu = block_cu[&block_id];
                let (best_cost, best_pred) = cfg
                    .predecessors(block_id)
                    .iter()
                    .filter(|&&p| function_blocks.contains(&p))
                    .map(|&p| (dp[&p], Some(p)))
                    .max_by_key(|&(cost, _)| cost)
                    .unwrap_or((0, None));

                dp.insert(block_id, best_cost + self_cu);
                parent.insert(block_id, best_pred);
            }

            // Find the exit block with the highest accumulated CU
            // An exit block has no successors that remain inside this function.
            let exit_block = function_blocks
                .iter()
                .filter(|&&id| {
                    cfg.successors(id)
                        .iter()
                        .all(|s| !function_blocks.contains(s))
                })
                .max_by_key(|&&id| dp[&id])
                .copied();

            let Some(mut current) = exit_block else {
                return Ok(CriticalPathResult {
                    function_name: name,
                    total_cu: 0,
                    path: vec![],
                    block_cu,
                });
            };

            let total_cu = dp[&current];

            let mut path = vec![current];
            while let Some(&Some(pred)) = parent.get(&current) {
                path.push(pred);
                current = pred;
            }
            path.reverse();

            Ok(CriticalPathResult {
                function_name: name,
                total_cu,
                path,
                block_cu,
            })
        })
        .collect()
}

/// Returns a human-readable summary of the critical path for every function.
pub fn dump_critical_path(cfg: &Cfg) -> String {
    let results = critical_path(cfg);
    let mut lines: Vec<String> = Vec::new();

    for result in results {
        match result {
            Err(e) => {
                lines.push(format!(
                    "function '{}': skipped (contains loops)",
                    e.function_name
                ));
            }
            Ok(r) if r.path.is_empty() => {
                lines.push(format!(
                    "function '{}': no exit block found",
                    r.function_name
                ));
            }
            Ok(r) => {
                lines.push(format!(
                    "function '{}': critical path {} CU",
                    r.function_name, r.total_cu
                ));
                for (i, &block_id) in r.path.iter().enumerate() {
                    let cu = r.block_cu[&block_id];
                    let label = cfg
                        .block(block_id)
                        .and_then(|b| b.labels().first())
                        .map(|(name, _)| format!(" ({})", name))
                        .unwrap_or_default();
                    lines.push(format!("  [{}] block {}{} — {} CU", i, block_id, label, cu));
                }
            }
        }
    }

    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        either::Either,
        sbpf_common::{instruction::Instruction, opcode::Opcode},
        sbpf_ir::{InputNode, control_flow_graph},
        std::collections::HashSet,
    };

    fn exit_inst() -> Instruction {
        inst(Opcode::Exit, None, None)
    }

    fn nop_inst() -> Instruction {
        inst(Opcode::Mov64Imm, None, None)
    }

    fn jump_inst(target: &str) -> Instruction {
        inst(Opcode::Ja, Some(Either::Left(target.to_string())), None)
    }

    fn call_inst(target: &str) -> Instruction {
        inst(Opcode::Call, None, Some(Either::Left(target.to_string())))
    }

    fn jeq_inst(target: &str) -> Instruction {
        inst(Opcode::JeqImm, Some(Either::Left(target.to_string())), None)
    }

    fn inst(
        opcode: Opcode,
        off: Option<Either<String, i16>>,
        imm: Option<Either<String, sbpf_common::inst_param::Number>>,
    ) -> Instruction {
        Instruction {
            opcode,
            dst: None,
            src: None,
            off,
            imm,
            span: 0..0,
        }
    }

    fn make_cfg(nodes: &[InputNode<'_>], entries: &[&str]) -> Cfg {
        let entries: HashSet<String> = entries.iter().map(|s| s.to_string()).collect();
        control_flow_graph(nodes.iter().copied(), &entries, None)
    }

    #[test]
    fn single_block_function() {
        // entrypoint: exit   → 1 instruction = 1 CU
        let exit = exit_inst();
        let nodes = [
            InputNode::Label("entrypoint"),
            InputNode::Instruction(&exit),
        ];
        let cfg = make_cfg(&nodes, &["entrypoint"]);

        let results = critical_path(&cfg);
        assert_eq!(results.len(), 1);
        let r = results[0].as_ref().unwrap();
        assert_eq!(r.function_name, "entrypoint");
        assert_eq!(r.total_cu, 1);
        assert_eq!(r.path.len(), 1);
    }

    #[test]
    fn linear_chain_sums_all_blocks() {
        // entrypoint: ja mid   (1 CU)
        // mid:        ja done  (1 CU)
        // done:       exit     (1 CU)
        // Critical path = all 3 blocks = 3 CU
        let j1 = jump_inst("mid");
        let j2 = jump_inst("done");
        let ex = exit_inst();
        let nodes = [
            InputNode::Label("entrypoint"),
            InputNode::Instruction(&j1),
            InputNode::Label("mid"),
            InputNode::Instruction(&j2),
            InputNode::Label("done"),
            InputNode::Instruction(&ex),
        ];
        let cfg = make_cfg(&nodes, &["entrypoint"]);

        let results = critical_path(&cfg);
        let r = results[0].as_ref().unwrap();
        assert_eq!(r.total_cu, 3);
        assert_eq!(r.path.len(), 3);
    }

    #[test]
    fn branch_picks_longer_path() {
        // entrypoint: jeq short  ─┐  (1 CU)
        //                         │
        // long_path:  nop         │  (2 CU — nop then exit in same block)
        //             exit        │
        //                         │
        // short:      exit  ──────┘  (1 CU)
        //
        // critical path = entrypoint → long_path = 1 + 2 = 3 CU
        // Note: two exit instructions would split into two 1-CU blocks each,
        // so we use nop (non-terminating) + exit to get one 2-CU block.
        let branch = jeq_inst("short");
        let long1 = nop_inst();
        let long2 = exit_inst();
        let short_ex = exit_inst();
        let nodes = [
            InputNode::Label("entrypoint"),
            InputNode::Instruction(&branch),
            InputNode::Label("long_path"),
            InputNode::Instruction(&long1),
            InputNode::Instruction(&long2),
            InputNode::Label("short"),
            InputNode::Instruction(&short_ex),
        ];
        let cfg = make_cfg(&nodes, &["entrypoint"]);

        let results = critical_path(&cfg);
        let r = results[0].as_ref().unwrap();
        assert_eq!(r.total_cu, 3, "critical path should go through long_path");
    }

    #[test]
    fn loop_returns_error() {
        // A self-call creates a back-edge → cycle → Err
        let self_call = call_inst("entrypoint");
        let ex = exit_inst();
        let nodes = [
            InputNode::Label("entrypoint"),
            InputNode::Instruction(&self_call),
            InputNode::Instruction(&ex),
        ];
        let cfg = make_cfg(&nodes, &["entrypoint"]);

        let results = critical_path(&cfg);
        assert!(
            results[0].is_err(),
            "self-recursive function should return an error"
        );
        let e = results[0].as_ref().unwrap_err();
        assert_eq!(e.function_name, "entrypoint");
    }

    #[test]
    fn multiple_functions_analyzed_independently() {
        // entrypoint: call helper; exit  (2 CU)
        // helper:     exit               (1 CU)
        let call = call_inst("helper");
        let entry_ex = exit_inst();
        let helper_ex = exit_inst();
        let nodes = [
            InputNode::Label("entrypoint"),
            InputNode::Instruction(&call),
            InputNode::Instruction(&entry_ex),
            InputNode::Label("helper"),
            InputNode::Instruction(&helper_ex),
        ];
        let cfg = make_cfg(&nodes, &["entrypoint", "helper"]);

        let results = critical_path(&cfg);
        assert_eq!(results.len(), 2);

        let entry_r = results
            .iter()
            .find(|r| {
                r.as_ref()
                    .map(|r| r.function_name == "entrypoint")
                    .unwrap_or(false)
            })
            .unwrap()
            .as_ref()
            .unwrap();
        assert_eq!(entry_r.total_cu, 2);

        let helper_r = results
            .iter()
            .find(|r| {
                r.as_ref()
                    .map(|r| r.function_name == "helper")
                    .unwrap_or(false)
            })
            .unwrap()
            .as_ref()
            .unwrap();
        assert_eq!(helper_r.total_cu, 1);
    }

    #[test]
    fn dump_critical_path_smoke() {
        let exit = exit_inst();
        let nodes = [
            InputNode::Label("entrypoint"),
            InputNode::Instruction(&exit),
        ];
        let cfg = make_cfg(&nodes, &["entrypoint"]);
        let out = dump_critical_path(&cfg);
        assert!(out.contains("entrypoint"));
        assert!(out.contains("1 CU"));
    }
}
