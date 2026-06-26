use {
    sbpf_ir::{
        BlockId, Cfg, CfgFunction, FunctionId,
        graph_engine::{WorklistEngine, WorklistVisitor},
    },
    std::collections::HashSet,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemovedFunction {
    pub name: String,
    /// Indices into the original `ast.nodes` array for every node (labels and
    /// instructions) that belonged to this function. Used by the assembler to
    /// strip the corresponding AST nodes after DCE.
    pub node_ids: Vec<usize>,
}

/// Removes functions not reachable from `functions()[0]` via `call imm` edges and returns
/// their names and byte ranges. Dead functions' blocks and instructions are dropped automatically
/// via Rust ownership. Successor/predecessor relationships between live blocks are
/// unchanged and are NOT rebuilt.
pub fn remove_dead_functions(cfg: &mut Cfg) -> Vec<RemovedFunction> {
    if cfg.functions.is_empty() {
        return Vec::new();
    }

    let reachable_funcs = reachable_functions(cfg);

    if reachable_funcs.len() == cfg.functions.len() {
        return Vec::new();
    }

    let old_functions = std::mem::take(&mut cfg.functions);
    let mut removed_functions = Vec::new();

    cfg.functions = old_functions
        .into_iter()
        .enumerate()
        .filter_map(|(fi, func)| {
            if reachable_funcs.contains(&fi) {
                Some(func)
            } else {
                removed_functions.push(RemovedFunction {
                    name: func.name.clone(),
                    node_ids: function_node_ids(&func),
                });
                None
            }
        })
        .collect();

    removed_functions
}

fn function_node_ids(function: &CfgFunction) -> Vec<usize> {
    function
        .blocks()
        .iter()
        .flat_map(|block| block.node_ids().iter().copied())
        .collect()
}

fn reachable_functions(cfg: &Cfg) -> HashSet<FunctionId> {
    // functions()[0] is always the declared entry (guaranteed by control_flow_graph).
    let entry_fi = 0;

    // When the engine crosses into a new function, enqueue ALL of its blocks —
    // not just the entry block. This ensures that unreachable-within-function code
    // (dead blocks inside a live function) is also traversed, so calls from that
    // dead code don't cause the callee to be incorrectly removed.
    struct EnqueueFunctionBlocks<'a> {
        cfg: &'a Cfg,
        enqueued_funcs: HashSet<FunctionId>,
    }

    impl<'a> WorklistVisitor<BlockId> for EnqueueFunctionBlocks<'a> {
        fn visit(&mut self, block_id: BlockId, enqueue: &mut dyn FnMut(BlockId)) {
            let caller_fi = self.cfg.function_of_block(block_id);
            for &succ in self.cfg.successors(block_id) {
                let Some(callee_fi) = self.cfg.function_of_block(succ) else {
                    continue;
                };
                if Some(callee_fi) != caller_fi && self.enqueued_funcs.insert(callee_fi) {
                    for &blk in self.cfg.functions()[callee_fi].block_ids() {
                        enqueue(blk);
                    }
                }
            }
        }
    }

    let mut visitor = EnqueueFunctionBlocks {
        cfg,
        enqueued_funcs: HashSet::from([entry_fi]),
    };

    let mut engine = WorklistEngine::new(cfg);
    engine.initialize(cfg.functions()[entry_fi].block_ids().iter().copied());
    engine.run(&mut visitor);

    // Every function that had at least one block visited is reachable.
    engine
        .visited()
        .iter()
        .filter_map(|&b| cfg.function_of_block(b))
        .collect()
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

    #[test]
    fn test_remove_dead_functions_removes_unreachable_function() {
        let call = call_instruction("callee");
        let entry_exit = instruction(Opcode::Exit, None);
        let dead_exit = instruction(Opcode::Exit, None);
        let callee_exit = instruction(Opcode::Exit, None);
        let nodes = [
            InputNode::Label("entrypoint"),
            InputNode::Instruction(&call),
            InputNode::Instruction(&entry_exit),
            InputNode::Label("dead"),
            InputNode::Instruction(&dead_exit),
            InputNode::Label("callee"),
            InputNode::Instruction(&callee_exit),
        ];
        let function_entries = HashSet::from([
            "entrypoint".to_string(),
            "dead".to_string(),
            "callee".to_string(),
        ]);
        let mut cfg = control_flow_graph(nodes, &function_entries, None);

        let removed = remove_dead_functions(&mut cfg);

        assert_eq!(removed.len(), 1);
        assert_eq!(removed[0].name, "dead");
        assert_eq!(cfg.functions().len(), 2);
        assert_eq!(cfg.functions()[0].name(), "entrypoint");
        assert_eq!(cfg.functions()[1].name(), "callee");
    }

    #[test]
    fn test_remove_dead_functions_keeps_all_reachable() {
        let exit = instruction(Opcode::Exit, None);
        let nodes = [
            InputNode::Label("entrypoint"),
            InputNode::Instruction(&exit),
        ];
        let function_entries = HashSet::from(["entrypoint".to_string()]);
        let mut cfg = control_flow_graph(nodes, &function_entries, None);

        let removed = remove_dead_functions(&mut cfg);

        assert!(removed.is_empty());
        assert_eq!(cfg.functions().len(), 1);
    }

    /// Regression: a `call` in dead code inside a live function must not cause
    /// the callee to be treated as unreachable and removed.
    ///
    /// Layout:
    ///   entrypoint:  ja target   -- skips the call
    ///   [dead block] call hidden  -- never reached via CFG from block 0
    ///   target:      exit
    ///   hidden:      exit         -- only referenced from dead code
    #[test]
    fn test_remove_dead_functions_keeps_callee_of_dead_block_in_live_function() {
        let jump = instruction(Opcode::Ja, Some(Either::Left("target".to_string())));
        let dead_call = call_instruction("hidden");
        let target_exit = instruction(Opcode::Exit, None);
        let hidden_exit = instruction(Opcode::Exit, None);
        let nodes = [
            InputNode::Label("entrypoint"),
            InputNode::Instruction(&jump),
            InputNode::Instruction(&dead_call),
            InputNode::Label("target"),
            InputNode::Instruction(&target_exit),
            InputNode::Label("hidden"),
            InputNode::Instruction(&hidden_exit),
        ];
        let function_entries = HashSet::from(["entrypoint".to_string(), "hidden".to_string()]);
        let mut cfg = control_flow_graph(nodes, &function_entries, None);

        let removed = remove_dead_functions(&mut cfg);

        assert!(
            removed.is_empty(),
            "expected no dead functions, got: {removed:?}"
        );
        assert_eq!(cfg.functions().len(), 2);
    }

    fn instruction(opcode: Opcode, off: Option<Either<String, i16>>) -> Instruction {
        Instruction {
            opcode,
            dst: None,
            src: None,
            off,
            imm: None,
            span: 0..0,
        }
    }

    fn call_instruction(target: &str) -> Instruction {
        Instruction {
            opcode: Opcode::Call,
            dst: None,
            src: None,
            off: None,
            imm: Some(Either::Left(target.to_string())),
            span: 0..0,
        }
    }
}
