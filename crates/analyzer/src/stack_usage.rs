use {
    either::Either,
    sbpf_ir::{Cfg, CfgFunction},
};

/// Tracks the deepest (most negative) R10-relative offset used by each function,
/// so the signed-div pass can allocate spill slots below the existing frame.
pub struct StackUsage {
    /// Per-function next available spill offset from R10 (negative, 8-byte aligned).
    bases: Vec<i16>,
}

impl StackUsage {
    pub fn compute(cfg: &Cfg) -> Self {
        let bases = cfg.functions().iter().map(func_spill_base).collect();
        Self { bases }
    }

    /// Allocates `count` 8-byte spill slots for `func_idx`.
    /// Returns their R10-relative offsets (e.g. [-56, -64, …]).
    pub fn alloc_slots(&mut self, func_idx: usize, count: usize) -> Vec<i16> {
        if count == 0 {
            return Vec::new();
        }
        let slots = (0..count as i16)
            .map(|i| self.bases[func_idx] - i * 8)
            .collect();
        self.bases[func_idx] -= count as i16 * 8;
        slots
    }
}

/// Returns the first available spill offset for `func`: 8 bytes below the
/// lowest R10-relative memory access offset already present, or -8 if none.
fn func_spill_base(func: &CfgFunction) -> i16 {
    let mut min_off: i16 = 0;
    for block in func.blocks() {
        for node in block.instructions() {
            let Some(inst) = node.instruction() else {
                continue;
            };
            let Some(Either::Right(off)) = &inst.off else {
                continue;
            };
            // Load: src == r10.  Store: dst == r10.
            let is_r10 = inst.src.as_ref().is_some_and(|r| r.n == 10)
                || inst.dst.as_ref().is_some_and(|r| r.n == 10);
            if is_r10 && *off < min_off {
                min_off = *off;
            }
        }
    }
    min_off.saturating_sub(8)
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        either::Either,
        sbpf_common::{inst_param::Register, instruction::Instruction, opcode::Opcode},
        sbpf_ir::{InputNode, control_flow_graph},
        std::collections::HashSet,
    };

    fn mem(opcode: Opcode, dst: u8, src: u8, off: i16) -> Instruction {
        Instruction {
            opcode,
            dst: Some(Register { n: dst }),
            src: Some(Register { n: src }),
            off: Some(Either::Right(off)),
            imm: None,
            span: 0..0,
        }
    }

    fn exit() -> Instruction {
        Instruction {
            opcode: Opcode::Exit,
            dst: None,
            src: None,
            off: None,
            imm: None,
            span: 0..0,
        }
    }

    fn build(instructions: &[Instruction]) -> Cfg {
        let nodes: Vec<InputNode<'_>> = std::iter::once(InputNode::Label("entrypoint"))
            .chain(instructions.iter().map(InputNode::Instruction))
            .collect();
        control_flow_graph(nodes, &HashSet::new(), None)
    }

    #[test]
    fn test_no_stack_uses_base_is_minus_8() {
        let cfg = build(&[exit()]);
        let usage = StackUsage::compute(&cfg);
        assert_eq!(usage.bases[0], -8);
    }

    #[test]
    fn test_deepest_offset_sets_base() {
        let cfg = build(&[
            mem(Opcode::Stxdw, 10, 1, -8),
            mem(Opcode::Stxdw, 10, 2, -16),
            mem(Opcode::Stxdw, 10, 3, -48),
            exit(),
        ]);
        let usage = StackUsage::compute(&cfg);
        assert_eq!(usage.bases[0], -56); // -48 - 8
    }

    #[test]
    fn test_alloc_slots_returns_consecutive_offsets() {
        let cfg = build(&[mem(Opcode::Stxdw, 10, 1, -8), exit()]);
        let mut usage = StackUsage::compute(&cfg);
        let slots = usage.alloc_slots(0, 3);
        assert_eq!(slots, vec![-16, -24, -32]);
        // Next alloc starts below the last slot.
        let more = usage.alloc_slots(0, 1);
        assert_eq!(more, vec![-40]);
    }
}
