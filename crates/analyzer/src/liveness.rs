use {
    sbpf_ir::{BlockId, Cfg, CfgFunction, InstKey, MAX_WRITABLE_REG, operands},
    std::collections::HashMap,
};

/// Bitmask of registers r0–r9 live at a point. Bit `i` set means ri is live.
pub type LiveSet = u16;

fn reg_bit(r: u8) -> LiveSet {
    if r <= MAX_WRITABLE_REG { 1 << r } else { 0 }
}

/// Per-instruction live-before sets computed by backward dataflow.
pub struct LiveSets {
    live_before: HashMap<InstKey, LiveSet>,
}

impl LiveSets {
    /// Computes live-before sets for every instruction in `func`.
    /// `rpo` lists the function's blocks (local indices into `func.blocks()`)
    /// in reverse-postorder, as returned by `sbpf_ir::reverse_postorder`.
    pub fn compute(cfg: &Cfg, func: &CfgFunction, rpo: &[usize]) -> Self {
        if func.blocks().is_empty() {
            return Self {
                live_before: HashMap::new(),
            };
        }

        let block_ids = func.block_ids();
        let global_to_local: HashMap<BlockId, usize> = block_ids
            .iter()
            .enumerate()
            .map(|(local, &global)| (global, local))
            .collect();

        let n = func.blocks().len();
        let mut live_in = vec![0u16; n];

        // Backward dataflow to fixpoint. Process in reverse RPO (leaves → root).
        let mut changed = true;
        while changed {
            changed = false;
            for &local in rpo.iter().rev() {
                let live_out =
                    successors_live_in(cfg, block_ids[local], &global_to_local, &live_in);
                let new_in = block_live_in(func, local, live_out);
                if new_in != live_in[local] {
                    live_in[local] = new_in;
                    changed = true;
                }
            }
        }

        // Second pass: record per-instruction live_before sets.
        let mut live_before = HashMap::new();
        for &local in rpo {
            let live_out = successors_live_in(cfg, block_ids[local], &global_to_local, &live_in);
            let mut live = live_out;
            for node in func.blocks()[local].instructions().iter().rev() {
                let Some(inst) = node.instruction() else {
                    continue;
                };
                let ops = operands(inst);
                let mut kill: LiveSet = 0;
                for &d in ops.defs.iter() {
                    kill |= reg_bit(d);
                }
                for &c in ops.clobbers.iter() {
                    kill |= reg_bit(c);
                }
                let mut uses_mask: LiveSet = 0;
                for &(r, _) in ops.uses.iter() {
                    uses_mask |= reg_bit(r);
                }
                let lb = (live & !kill) | uses_mask;
                live_before.insert(node.key(), lb);
                live = lb;
            }
        }

        Self { live_before }
    }

    pub fn live_before(&self, key: InstKey) -> LiveSet {
        self.live_before.get(&key).copied().unwrap_or(0)
    }

    /// Iterator over registers NOT live before `key`, in r0..=r9 order.
    pub fn free_regs(&self, key: InstKey) -> impl Iterator<Item = u8> {
        let live = self.live_before(key);
        (0u8..=MAX_WRITABLE_REG).filter(move |&r| live & (1 << r) == 0)
    }
}

fn successors_live_in(
    cfg: &Cfg,
    global_id: BlockId,
    global_to_local: &HashMap<BlockId, usize>,
    live_in: &[LiveSet],
) -> LiveSet {
    cfg.intra_successors(global_id)
        .iter()
        .filter_map(|succ| global_to_local.get(succ).map(|&l| live_in[l]))
        .fold(0, |acc, s| acc | s)
}

fn block_live_in(func: &CfgFunction, local: usize, live_out: LiveSet) -> LiveSet {
    let mut live = live_out;
    for node in func.blocks()[local].instructions().iter().rev() {
        let Some(inst) = node.instruction() else {
            continue;
        };
        let ops = operands(inst);
        let mut kill: LiveSet = 0;
        for &d in ops.defs.iter() {
            kill |= reg_bit(d);
        }
        for &c in ops.clobbers.iter() {
            kill |= reg_bit(c);
        }
        let mut uses_mask: LiveSet = 0;
        for &(r, _) in ops.uses.iter() {
            uses_mask |= reg_bit(r);
        }
        live = (live & !kill) | uses_mask;
    }
    live
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        either::Either,
        sbpf_common::{
            inst_param::{Number, Register},
            instruction::Instruction,
            opcode::Opcode,
        },
        sbpf_ir::{InputNode, control_flow_graph, reverse_postorder},
        std::collections::HashSet,
    };

    fn inst(opcode: Opcode, dst: Option<u8>, src: Option<u8>, imm: Option<i64>) -> Instruction {
        Instruction {
            opcode,
            dst: dst.map(|n| Register { n }),
            src: src.map(|n| Register { n }),
            off: None,
            imm: imm.map(|v| Either::Right(Number::Int(v))),
            span: 0..0,
        }
    }

    fn build_cfg(nodes: Vec<InputNode<'_>>) -> Cfg {
        control_flow_graph(nodes, &HashSet::new(), None)
    }

    #[test]
    fn test_liveness_dead_def_is_free() {
        // r1 is defined by mov64 (pure def, no read of r1) but never read after → not live before mov64.
        // exit has an implicit use of r0, so r0 is live before exit and before mov64.
        let mov = inst(Opcode::Mov64Imm, Some(1), None, Some(5));
        let exit = inst(Opcode::Exit, None, None, None);
        let cfg = build_cfg(vec![
            InputNode::Label("entrypoint"),
            InputNode::Instruction(&mov),
            InputNode::Instruction(&exit),
        ]);
        let rpo = reverse_postorder(&cfg, 0);
        let func = &cfg.functions()[0];
        let live = LiveSets::compute(&cfg, func, &rpo);
        let mov_key = func.blocks()[0].instructions()[0].key();
        // r0 is live before mov64 (exit needs it); r1 is NOT live (pure def, never used after).
        assert_eq!(
            live.live_before(mov_key) & (1 << 1),
            0,
            "r1 should be dead before mov"
        );
        assert!(live.free_regs(mov_key).any(|r| r == 1), "r1 should be free");
    }

    #[test]
    fn test_liveness_used_register_is_live() {
        // r1 is written by mov64, then read by add64 r2, r1.
        let mov = inst(Opcode::Mov64Imm, Some(1), None, Some(42));
        let add = inst(Opcode::Add64Reg, Some(2), Some(1), None);
        let exit = inst(Opcode::Exit, None, None, None);
        let cfg = build_cfg(vec![
            InputNode::Label("entrypoint"),
            InputNode::Instruction(&mov),
            InputNode::Instruction(&add),
            InputNode::Instruction(&exit),
        ]);
        let rpo = reverse_postorder(&cfg, 0);
        let func = &cfg.functions()[0];
        let live = LiveSets::compute(&cfg, func, &rpo);
        // add64 r2, r1 reads r1, so r1 is live before the add.
        let add_key = func.blocks()[0].instructions()[1].key();
        assert_ne!(
            live.live_before(add_key) & (1 << 1),
            0,
            "r1 should be live before add"
        );
        // r1 is NOT live before the initial mov (it's about to be overwritten with no prior read).
        let mov_key = func.blocks()[0].instructions()[0].key();
        assert_eq!(
            live.live_before(mov_key) & (1 << 1),
            0,
            "r1 not live before its own def"
        );
    }

    #[test]
    fn test_liveness_call_clobbers_args() {
        // After a call, r1-r5 are clobbered → not live after the call.
        // Before the call, r1-r5 are uses → live before the call.
        let call = inst(Opcode::Call, None, None, Some(0));
        let exit = inst(Opcode::Exit, None, None, None);
        let cfg = build_cfg(vec![
            InputNode::Label("entrypoint"),
            InputNode::Instruction(&call),
            InputNode::Instruction(&exit),
        ]);
        let rpo = reverse_postorder(&cfg, 0);
        let func = &cfg.functions()[0];
        let live = LiveSets::compute(&cfg, func, &rpo);
        let exit_key = func.blocks()[0].instructions()[1].key();
        // After the call r1-r5 are clobbered, so they should not be live before exit.
        for r in 1u8..=5 {
            assert_eq!(
                live.live_before(exit_key) & (1 << r),
                0,
                "r{r} should not be live before exit after a call"
            );
        }
    }
}
