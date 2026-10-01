//! Reconciles eBPF and SBPF ALU32 semantics.
//!
//! In eBPF every ALU32 instruction zero-extends its 32-bit result. In SBPF
//! (V0 and V3) `add32`, `sub32` and `mul32` sign-extend it instead, so a result
//! with bit 31 set gets `0xFFFFFFFF` in the upper half. For each such def this
//! pass either keeps it (no user reads the upper half, or the result can't have
//! bit 31 set), promotes it to the 64-bit form (when that provably computes the
//! same value), or clears the upper half. Clearing prefers the users: when every
//! user that reads the upper half is a `mov64 rX, rd` copy, each becomes
//! `mov32 rX, rd`, which zero-extends at no extra cost. Otherwise `mov32 rd, rd`
//! is inserted after the def.
//!
//! The pass edits the Cfg in place. Each edit is fed back into the demand
//! analysis here and into [`RangeAnalysis`] before the next def is decided.

use {
    crate::range_analysis::{RangeAnalysis, U32_MAX, imm_value},
    sbpf_common::{
        OpcodeTable, inst_param::Register, instruction::Instruction, opcode::Opcode,
        optype::OperationType,
    },
    sbpf_ir::{
        BlockId, Cfg, CfgFunction, DefId, DefSite, InstKey, InstructionNode, OperandKind, Use,
        graph_engine::WorklistEngine,
    },
    std::collections::HashMap,
};

/// Runs the pass on every function. Returns whether anything changed.
pub fn fix_alu32_extension(cfg: &mut Cfg) -> bool {
    let mut changed = false;
    for function_id in 0..cfg.functions().len() {
        let order = reverse_postorder(cfg, function_id);
        changed |= Alu32Extension::new(&mut cfg.functions[function_id], &order).run();
    }
    changed
}

/// The ALU64 opcode computing the same thing as `opcode` without the 32-bit
/// truncation, for the ALU32 ops that sign-extend in SBPF. `None` for every
/// other opcode.
pub fn sign_extending_alu32(opcode: Opcode) -> Option<Opcode> {
    Some(match opcode {
        Opcode::Add32Imm => Opcode::Add64Imm,
        Opcode::Add32Reg => Opcode::Add64Reg,
        Opcode::Sub32Imm => Opcode::Sub64Imm,
        Opcode::Sub32Reg => Opcode::Sub64Reg,
        Opcode::Mul32Imm => Opcode::Mul64Imm,
        Opcode::Mul32Reg => Opcode::Mul64Reg,
        _ => return None,
    })
}

// ── Lattices ─────────────────────────────────────────────────────────────────

/// How many bits of a def its users read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Demand {
    None,
    Low32,
    Full64,
}

// ── Pass state ───────────────────────────────────────────────────────────────

enum Decision {
    Keep,
    Promote(Opcode),
    /// Turn these `mov64` copies of the def into zero-extending `mov32` copies.
    NarrowCopies(Vec<InstKey>),
    FixUp,
}

struct Alu32Extension<'a> {
    func: &'a mut CfgFunction,
    /// Defs in reverse postorder of their blocks.
    defs_in_order: Vec<DefId>,
    ranges: RangeAnalysis,
    /// Indexed by `DefId`; grown when an edit appends a def.
    demand: Vec<Demand>,
    /// M-defs promoted or fixed up.
    decided: Vec<bool>,
    demand_wl: WorklistEngine<DefId>,
}

impl<'a> Alu32Extension<'a> {
    /// `order` lists the function's blocks (indices into `blocks`) in reverse postorder.
    fn new(func: &'a mut CfgFunction, order: &[usize]) -> Self {
        let defs_in_order: Vec<DefId> = (0..func.defs().len() as u32)
            .map(DefId)
            .filter(|&d| func.def(d).site == DefSite::Entry)
            .chain(order.iter().flat_map(|&local| {
                func.blocks[local]
                    .instructions()
                    .iter()
                    .flat_map(|node| node.defs().iter().copied())
            }))
            .collect();
        let ranges = RangeAnalysis::compute(func, defs_in_order.iter().copied());
        let n = func.defs().len();
        Self {
            func,
            defs_in_order,
            ranges,
            demand: vec![Demand::None; n],
            decided: vec![false; n],
            demand_wl: WorklistEngine::new(),
        }
    }

    fn run(mut self) -> bool {
        self.demand_wl.extend(self.defs_in_order.iter().copied());
        self.drain_demand();

        // Upstream defs first, so their edits narrow downstream ranges before
        // those defs are decided.
        let m_defs: Vec<DefId> = self
            .defs_in_order
            .iter()
            .copied()
            .filter(|&d| self.is_m_def(d))
            .collect();

        let mut changed = false;
        for d in m_defs {
            let DefSite::Inst(key) = self.func.def(d).site else {
                continue;
            };
            match self.decide(d, key) {
                Decision::Keep => {}
                Decision::Promote(opcode) => {
                    self.func.replace_opcode(key, opcode);
                    self.decided[d.index()] = true;
                    changed = true;
                    // The 64-bit form reads its inputs in full and yields the exact result.
                    let inputs: Vec<DefId> = self
                        .func
                        .node(key)
                        .uses()
                        .iter()
                        .flat_map(|u| u.reaching.iter().copied())
                        .collect();
                    self.demand_wl.extend(inputs);
                    self.ranges.update(self.func, [d]);
                }
                Decision::NarrowCopies(copies) => {
                    for &copy in &copies {
                        self.func.replace_opcode(copy, Opcode::Mov32Reg);
                    }
                    self.decided[d.index()] = true;
                    changed |= !copies.is_empty();
                    // The copies now write zero-extended values.
                    let copy_defs: Vec<DefId> = copies
                        .iter()
                        .flat_map(|&copy| self.func.node(copy).defs().iter().copied())
                        .collect();
                    self.ranges.update(self.func, copy_defs);
                }
                Decision::FixUp => {
                    let reg = self.func.def(d).reg;
                    let span = self
                        .func
                        .node(key)
                        .instruction()
                        .map(|inst| inst.span.clone())
                        .unwrap_or_default();
                    let fixup = Instruction {
                        opcode: Opcode::Mov32Reg,
                        dst: Some(Register { n: reg }),
                        src: Some(Register { n: reg }),
                        off: None,
                        imm: None,
                        span,
                    };
                    let new_def = self.func.insert_redef_after(d, fixup);
                    self.grow();
                    self.decided[d.index()] = true;
                    changed = true;
                    self.demand_wl.push(new_def);
                    self.ranges.update(self.func, [new_def]);
                }
            }
            self.drain_demand();
        }

        // Each def is decided once. Later edits only narrow ranges, and a
        // promotion raises demand only on inputs known to fit in 32 bits, so a
        // kept def must still be a keep once everything else is done.
        debug_assert!(
            self.defs_in_order.iter().all(|&d| {
                let DefSite::Inst(key) = self.func.def(d).site else {
                    return true;
                };
                !self.is_m_def(d)
                    || self.decided[d.index()]
                    || matches!(self.decide(d, key), Decision::Keep)
            }),
            "a kept def would no longer be kept after later edits"
        );
        changed
    }

    fn grow(&mut self) {
        let n = self.func.defs().len();
        self.demand.resize(n, Demand::None);
        self.decided.resize(n, false);
    }

    fn is_m_def(&self, d: DefId) -> bool {
        match self.func.def(d).site {
            DefSite::Inst(key) => sign_extending_alu32(self.func.node(key).opcode).is_some(),
            _ => false,
        }
    }

    // ── Decision ─────────────────────────────────────────────────────────────

    fn decide(&self, d: DefId, key: InstKey) -> Decision {
        let node = self.func.node(key);
        let Some(promoted) = sign_extending_alu32(node.opcode) else {
            return Decision::Keep;
        };
        let exact = self.ranges.ebpf_result(node);

        // The result never has bit 31 set, so sign- and zero-extension agree.
        if exact.is_some_and(|e| e.within(0, (1 << 31) - 1)) {
            return Decision::Keep;
        }
        // Nobody reads the upper half.
        if self.demand[d.index()] <= Demand::Low32 {
            return Decision::Keep;
        }
        // The 64-bit op computes the same value as the zero-extended 32-bit op.
        if exact.is_some_and(|e| e.within(0, U32_MAX)) {
            return Decision::Promote(promoted);
        }
        // Zero-extend where the upper half is read, if a copy can do it for free.
        if let Some(copies) = self.narrowable_copies(d) {
            return Decision::NarrowCopies(copies);
        }
        Decision::FixUp
    }

    /// The `mov64 rX, rd` copies to turn into `mov32 rX, rd` instead of fixing up
    /// `d` itself, or `None` if that is not possible. Every user of `d` that reads
    /// its upper half must be such a copy. A copy may have other reaching defs, and
    /// `mov32` zero-extends them too, so each must be either sign-extending (its
    /// intended eBPF value is the zero-extended one) or known to fit in 32 bits.
    fn narrowable_copies(&self, d: DefId) -> Option<Vec<InstKey>> {
        let mut copies = Vec::new();
        for &user in &self.func.def(d).users {
            let node = self.func.node(user.inst);
            let used = self.func.use_at(user);
            if self.use_demand(node, used) <= Demand::Low32 {
                continue;
            }
            if node.opcode != Opcode::Mov64Reg {
                return None;
            }
            let zext_is_safe = used
                .reaching
                .iter()
                .all(|&r| self.is_m_def(r) || self.ranges.range(r).is_u32());
            if !zext_is_safe {
                return None;
            }
            copies.push(user.inst);
        }
        Some(copies)
    }

    // ── Demand ───────────────────────────────────────────────────────────────

    fn drain_demand(&mut self) {
        while let Some(d) = self.demand_wl.pop() {
            let new = self
                .func
                .def(d)
                .users
                .iter()
                .map(|&u| self.use_demand(self.func.node(u.inst), self.func.use_at(u)))
                .max()
                .unwrap_or(Demand::None);
            if new <= self.demand[d.index()] {
                continue;
            }
            self.demand[d.index()] = new;
            // The inputs of d's instruction may pass this demand through.
            if let DefSite::Inst(key) = self.func.def(d).site {
                let inputs: Vec<DefId> = self
                    .func
                    .node(key)
                    .uses()
                    .iter()
                    .flat_map(|u| u.reaching.iter().copied())
                    .collect();
                self.demand_wl.extend(inputs);
            }
        }
    }

    /// How many bits of its register `node` reads through `used`.
    fn use_demand(&self, node: &InstructionNode, used: &Use) -> Demand {
        let Some(inst) = node.instruction() else {
            return Demand::Full64;
        };
        // Demand on the instruction's own result, for ops whose low result bits
        // depend only on low input bits.
        let own = || {
            node.defs()
                .first()
                .map_or(Demand::None, |d| self.demand[d.index()])
        };
        let is_src = used.kind == OperandKind::Src;
        match inst.opcode.group() {
            OperationType::StoreRegister if is_src && inst.opcode != Opcode::Stxdw => Demand::Low32,
            OperationType::Jump32Immediate | OperationType::Jump32Register => Demand::Low32,
            OperationType::Endian => match imm_value(inst) {
                Some(16 | 32) => Demand::Low32,
                _ => Demand::Full64,
            },
            OperationType::Unary if inst.opcode == Opcode::Neg32 => Demand::Low32,
            OperationType::Unary => own(),
            OperationType::BinaryImmediate | OperationType::BinaryRegister => {
                if inst.opcode.is_alu32() {
                    return Demand::Low32;
                }
                match inst.opcode {
                    Opcode::Mov64Reg
                    | Opcode::Add64Imm
                    | Opcode::Add64Reg
                    | Opcode::Sub64Imm
                    | Opcode::Sub64Reg
                    | Opcode::Mul64Imm
                    | Opcode::Mul64Reg
                    | Opcode::Lmul64Imm
                    | Opcode::Lmul64Reg
                    | Opcode::Or64Imm
                    | Opcode::Or64Reg
                    | Opcode::Xor64Imm
                    | Opcode::Xor64Reg
                    | Opcode::And64Reg => own(),
                    // A mask with a zero upper half ignores the input's upper half.
                    Opcode::And64Imm => match imm_value(inst) {
                        Some(0..=0x7FFF_FFFF) => own().min(Demand::Low32),
                        _ => own(),
                    },
                    Opcode::Lsh64Imm => match imm_value(inst) {
                        Some(k) if k & 63 >= 32 => own().min(Demand::Low32),
                        _ => own(),
                    },
                    // Shift amounts only use their low bits.
                    Opcode::Lsh64Reg if is_src => Demand::Low32,
                    Opcode::Lsh64Reg => own(),
                    Opcode::Rsh64Reg | Opcode::Arsh64Reg if is_src => Demand::Low32,
                    _ => Demand::Full64,
                }
            }
            // Memory bases, 64-bit jumps, stxdw values, call arguments, exit's r0.
            _ => Demand::Full64,
        }
    }
}

/// The function's blocks, as indices into `blocks`, in reverse postorder over
/// intra-function edges. Blocks unreachable from the entry follow in source order.
fn reverse_postorder(cfg: &Cfg, function_id: usize) -> Vec<usize> {
    let func = &cfg.functions()[function_id];
    let local_of: HashMap<BlockId, usize> = func
        .block_ids()
        .iter()
        .enumerate()
        .map(|(local, &id)| (id, local))
        .collect();
    let mut visited = vec![false; func.blocks().len()];
    let mut postorder = Vec::with_capacity(func.blocks().len());
    let mut stack: Vec<(usize, usize)> = Vec::new();
    if !func.blocks().is_empty() {
        visited[0] = true;
        stack.push((0, 0));
    }
    while let Some((local, next)) = stack.last_mut() {
        let successors = cfg.intra_successors(func.block_ids()[*local]);
        if let Some(&succ) = successors.get(*next) {
            *next += 1;
            if let Some(&succ_local) = local_of.get(&succ)
                && !visited[succ_local]
            {
                visited[succ_local] = true;
                stack.push((succ_local, 0));
            }
        } else {
            postorder.push(*local);
            stack.pop();
        }
    }
    postorder.reverse();
    postorder.extend((0..func.blocks().len()).filter(|&local| !visited[local]));
    postorder
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        either::Either,
        sbpf_common::inst_param::Number,
        sbpf_ir::{InputNode, control_flow_graph},
        std::collections::HashSet,
    };

    enum Src {
        Label(&'static str),
        Inst(Instruction),
    }

    /// One resulting instruction as (opcode, dst, src).
    type Row = (Opcode, Option<u8>, Option<u8>);

    fn build(program: &[Src]) -> Cfg {
        let nodes = program.iter().map(|node| match node {
            Src::Label(label) => InputNode::Label(label),
            Src::Inst(inst) => InputNode::Instruction(inst),
        });
        control_flow_graph(nodes, &HashSet::new(), None)
    }

    fn inst(opcode: Opcode, dst: Option<u8>, src: Option<u8>, imm: Option<i64>) -> Src {
        Src::Inst(Instruction {
            opcode,
            dst: dst.map(|n| Register { n }),
            src: src.map(|n| Register { n }),
            off: None,
            imm: imm.map(|v| Either::Right(Number::Int(v))),
            span: 0..0,
        })
    }

    fn mem(opcode: Opcode, dst: u8, src: u8, off: i16) -> Src {
        Src::Inst(Instruction {
            opcode,
            dst: Some(Register { n: dst }),
            src: Some(Register { n: src }),
            off: Some(Either::Right(off)),
            imm: None,
            span: 0..0,
        })
    }

    fn jump(opcode: Opcode, dst: u8, src: Option<u8>, imm: Option<i64>, target: &str) -> Src {
        Src::Inst(Instruction {
            opcode,
            dst: Some(Register { n: dst }),
            src: src.map(|n| Register { n }),
            off: Some(Either::Left(target.to_string())),
            imm: imm.map(|v| Either::Right(Number::Int(v))),
            span: 0..0,
        })
    }

    fn exit() -> Src {
        inst(Opcode::Exit, None, None, None)
    }

    /// Runs the pass and returns the resulting instructions as (opcode, dst, src).
    fn run(program: &[Src]) -> (bool, Vec<Row>) {
        let mut cfg = build(program);
        let changed = fix_alu32_extension(&mut cfg);
        let listing = cfg
            .all_instructions()
            .map(|(_, node)| {
                let inst = node.instruction().unwrap();
                (
                    inst.opcode,
                    inst.dst.as_ref().map(|r| r.n),
                    inst.src.as_ref().map(|r| r.n),
                )
            })
            .collect();
        (changed, listing)
    }

    fn opcodes(listing: &[Row]) -> Vec<Opcode> {
        listing.iter().map(|(op, _, _)| *op).collect()
    }

    fn fixup(reg: u8) -> Row {
        (Opcode::Mov32Reg, Some(reg), Some(reg))
    }

    #[test]
    fn test_keeps_def_read_by_jmp32() {
        let (changed, _) = run(&[
            Src::Label("entrypoint"),
            inst(Opcode::Add32Reg, Some(1), Some(2), None),
            jump(Opcode::Jeq32Imm, 1, None, Some(0), "done"),
            inst(Opcode::Mov64Imm, Some(0), None, Some(1)),
            Src::Label("done"),
            exit(),
        ]);
        assert!(!changed);
    }

    #[test]
    fn test_keeps_def_stored_as_word() {
        let (changed, _) = run(&[
            Src::Label("entrypoint"),
            inst(Opcode::Add32Reg, Some(1), Some(2), None),
            mem(Opcode::Stxw, 10, 1, -4),
            exit(),
        ]);
        assert!(!changed);
    }

    #[test]
    fn test_keeps_def_masked_to_low_bits() {
        let (changed, _) = run(&[
            Src::Label("entrypoint"),
            inst(Opcode::Add32Reg, Some(1), Some(2), None),
            inst(Opcode::And64Imm, Some(1), None, Some(0xFFFF)),
            inst(Opcode::Mov64Reg, Some(0), Some(1), None),
            exit(),
        ]);
        assert!(!changed);
    }

    #[test]
    fn test_keeps_result_below_bit_31() {
        // Two bytes summed can't reach bit 31, so sign- and zero-extension agree.
        let (changed, _) = run(&[
            Src::Label("entrypoint"),
            mem(Opcode::Ldxb, 1, 10, -1),
            mem(Opcode::Ldxb, 2, 10, -2),
            inst(Opcode::Add32Reg, Some(1), Some(2), None),
            mem(Opcode::Stxdw, 10, 1, -16),
            exit(),
        ]);
        assert!(!changed);
    }

    #[test]
    fn test_keeps_dead_def() {
        let (changed, _) = run(&[
            Src::Label("entrypoint"),
            inst(Opcode::Add32Reg, Some(1), Some(2), None),
            inst(Opcode::Mov64Reg, Some(3), Some(1), None),
            exit(),
        ]);
        assert!(!changed);
    }

    #[test]
    fn test_promotes_when_result_fits_in_32_bits() {
        // 0x9000_0000 - 1 has bit 31 set but no borrow, so sub64 gives the same value.
        let (changed, listing) = run(&[
            Src::Label("entrypoint"),
            inst(Opcode::Lddw, Some(1), None, Some(0x9000_0000)),
            inst(Opcode::Sub32Imm, Some(1), None, Some(1)),
            mem(Opcode::Stxdw, 10, 1, -8),
            exit(),
        ]);
        assert!(changed);
        assert_eq!(
            opcodes(&listing),
            [Opcode::Lddw, Opcode::Sub64Imm, Opcode::Stxdw, Opcode::Exit]
        );
    }

    #[test]
    fn test_fixes_up_unknown_inputs_read_by_jmp64() {
        let (changed, listing) = run(&[
            Src::Label("entrypoint"),
            inst(Opcode::Sub32Reg, Some(1), Some(2), None),
            jump(Opcode::JgtReg, 1, Some(3), None, "done"),
            inst(Opcode::Mov64Imm, Some(0), None, Some(1)),
            Src::Label("done"),
            exit(),
        ]);
        assert!(changed);
        assert_eq!(listing[0].0, Opcode::Sub32Reg);
        assert_eq!(listing[1], fixup(1));
        assert_eq!(listing.len(), 5);
    }

    #[test]
    fn test_narrows_copy_after_join() {
        // The copy's other reaching def is the constant 0, which zero-extension
        // leaves unchanged, so the copy can clear the upper half for both paths.
        let (changed, listing) = run(&[
            Src::Label("entrypoint"),
            inst(Opcode::Mov64Imm, Some(1), None, Some(0)),
            jump(Opcode::JeqImm, 2, None, Some(0), "join"),
            inst(Opcode::Mul32Reg, Some(1), Some(3), None),
            Src::Label("join"),
            inst(Opcode::Mov64Reg, Some(0), Some(1), None),
            exit(),
        ]);
        assert!(changed);
        assert_eq!(
            listing,
            [
                (Opcode::Mov64Imm, Some(1), None),
                (Opcode::JeqImm, Some(2), None),
                (Opcode::Mul32Reg, Some(1), Some(3)),
                (Opcode::Mov32Reg, Some(0), Some(1)),
                (Opcode::Exit, None, None),
            ]
        );
    }

    #[test]
    fn test_fixes_up_def_when_copy_also_carries_a_64_bit_value() {
        // r1 may arrive with its upper half set from the caller, so the copy
        // must stay 64-bit and the mul32 is fixed up on its own path instead.
        let (changed, listing) = run(&[
            Src::Label("entrypoint"),
            jump(Opcode::JeqImm, 2, None, Some(0), "join"),
            inst(Opcode::Mul32Reg, Some(1), Some(3), None),
            Src::Label("join"),
            inst(Opcode::Mov64Reg, Some(0), Some(1), None),
            exit(),
        ]);
        assert!(changed);
        assert_eq!(
            opcodes(&listing),
            [
                Opcode::JeqImm,
                Opcode::Mul32Reg,
                Opcode::Mov32Reg,
                Opcode::Mov64Reg,
                Opcode::Exit
            ]
        );
        assert_eq!(listing[2], fixup(1));
    }

    #[test]
    fn test_narrows_single_copy_instead_of_fixing_up() {
        let (changed, listing) = run(&[
            Src::Label("entrypoint"),
            inst(Opcode::Add32Reg, Some(1), Some(2), None),
            inst(Opcode::Mov64Reg, Some(0), Some(1), None),
            exit(),
        ]);
        assert!(changed);
        assert_eq!(
            listing,
            [
                (Opcode::Add32Reg, Some(1), Some(2)),
                (Opcode::Mov32Reg, Some(0), Some(1)),
                (Opcode::Exit, None, None),
            ]
        );
    }

    #[test]
    fn test_narrows_copy_after_loop_instead_of_fixing_up_inside() {
        // Inside the loop r4 is only read as 32 bits; only the copy after the
        // loop reads all 64, so no instruction is added to the loop body.
        let (changed, listing) = run(&[
            Src::Label("entrypoint"),
            mem(Opcode::Ldxw, 4, 10, -4),
            Src::Label("loop"),
            inst(Opcode::Add32Imm, Some(4), None, Some(-1)),
            inst(Opcode::Add32Imm, Some(1), None, Some(1)),
            jump(Opcode::Jlt32Imm, 1, None, Some(5), "loop"),
            inst(Opcode::Mov64Reg, Some(0), Some(4), None),
            exit(),
        ]);
        assert!(changed);
        assert_eq!(
            listing,
            [
                (Opcode::Ldxw, Some(4), Some(10)),
                (Opcode::Add32Imm, Some(4), None),
                (Opcode::Add32Imm, Some(1), None),
                (Opcode::Jlt32Imm, Some(1), None),
                (Opcode::Mov32Reg, Some(0), Some(4)),
                (Opcode::Exit, None, None),
            ]
        );
    }

    #[test]
    fn test_fixes_up_def_when_another_user_is_not_a_copy() {
        let (changed, listing) = run(&[
            Src::Label("entrypoint"),
            inst(Opcode::Add32Reg, Some(1), Some(2), None),
            inst(Opcode::Mov64Reg, Some(0), Some(1), None),
            jump(Opcode::JgtReg, 1, Some(3), None, "done"),
            inst(Opcode::Mov64Imm, Some(0), None, Some(1)),
            Src::Label("done"),
            exit(),
        ]);
        assert!(changed);
        assert_eq!(listing[1], fixup(1));
        assert_eq!(listing[2], (Opcode::Mov64Reg, Some(0), Some(1)));
    }

    #[test]
    fn test_fixes_up_loop_counter_compared_in_64_bits() {
        let (changed, listing) = run(&[
            Src::Label("entrypoint"),
            inst(Opcode::Mov64Imm, Some(1), None, Some(0)),
            Src::Label("loop"),
            inst(Opcode::Add32Imm, Some(1), None, Some(1)),
            jump(Opcode::JneImm, 1, None, Some(10), "loop"),
            exit(),
        ]);
        assert!(changed);
        assert_eq!(listing[2], fixup(1));
    }

    #[test]
    fn test_keeps_loop_counter_compared_in_32_bits() {
        let (changed, _) = run(&[
            Src::Label("entrypoint"),
            inst(Opcode::Mov64Imm, Some(1), None, Some(0)),
            Src::Label("loop"),
            inst(Opcode::Add32Imm, Some(1), None, Some(1)),
            jump(Opcode::Jne32Imm, 1, None, Some(10), "loop"),
            exit(),
        ]);
        assert!(!changed);
    }

    #[test]
    fn test_fixup_enables_downstream_promotion() {
        // The fix-up bounds r1 to 32 bits, so after rsh64 by 1 it is below 2^31
        // and the add32 below can be promoted instead of fixed up.
        let (changed, listing) = run(&[
            Src::Label("entrypoint"),
            inst(Opcode::Sub32Reg, Some(1), Some(2), None),
            inst(Opcode::Rsh64Imm, Some(1), None, Some(1)),
            inst(Opcode::Add32Imm, Some(1), None, Some(0x4000_0000)),
            inst(Opcode::Mov64Reg, Some(0), Some(1), None),
            exit(),
        ]);
        assert!(changed);
        assert_eq!(
            opcodes(&listing),
            [
                Opcode::Sub32Reg,
                Opcode::Mov32Reg,
                Opcode::Rsh64Imm,
                Opcode::Add64Imm,
                Opcode::Mov64Reg,
                Opcode::Exit
            ]
        );
    }

    #[test]
    fn test_call_argument_is_fully_demanded() {
        let (changed, listing) = run(&[
            Src::Label("entrypoint"),
            inst(Opcode::Add32Reg, Some(1), Some(2), None),
            Src::Inst(Instruction {
                opcode: Opcode::Call,
                dst: None,
                src: None,
                off: None,
                imm: Some(Either::Left("sol_log_".to_string())),
                span: 0..0,
            }),
            exit(),
        ]);
        assert!(changed);
        assert_eq!(listing[1], fixup(1));
    }

    #[test]
    fn test_second_run_changes_nothing() {
        let program = [
            Src::Label("entrypoint"),
            inst(Opcode::Sub32Reg, Some(1), Some(2), None),
            inst(Opcode::Rsh64Imm, Some(1), None, Some(1)),
            inst(Opcode::Add32Imm, Some(1), None, Some(0x4000_0000)),
            inst(Opcode::Mov64Reg, Some(0), Some(1), None),
            exit(),
        ];
        let (_, first) = run(&program);
        let rerun: Vec<Src> = std::iter::once(Src::Label("entrypoint"))
            .chain(first.iter().map(|&(op, dst, src)| {
                let imm = match op {
                    Opcode::Rsh64Imm => Some(1),
                    Opcode::Add64Imm => Some(0x4000_0000),
                    _ => None,
                };
                inst(op, dst, src, imm)
            }))
            .collect();
        let (changed, second) = run(&rerun);
        assert!(!changed);
        assert_eq!(first, second);
    }

    // ── Differential check against the real instruction executors ──────────

    /// Registers only; enough for straight-line ALU programs.
    struct RegVm {
        regs: [u64; 11],
        pc: usize,
    }

    impl sbpf_common::execute::Vm for RegVm {
        fn get_register(&self, reg: usize) -> u64 {
            self.regs[reg]
        }
        fn set_register(&mut self, reg: usize, value: u64) {
            self.regs[reg] = value;
        }
        fn get_pc(&self) -> usize {
            self.pc
        }
        fn set_pc(&mut self, pc: usize) {
            self.pc = pc;
        }
        fn read_u8(&self, _: u64) -> ExecResult<u8> {
            Err(ExecutionError::InvalidInstruction)
        }
        fn read_u16(&self, _: u64) -> ExecResult<u16> {
            Err(ExecutionError::InvalidInstruction)
        }
        fn read_u32(&self, _: u64) -> ExecResult<u32> {
            Err(ExecutionError::InvalidInstruction)
        }
        fn read_u64(&self, _: u64) -> ExecResult<u64> {
            Err(ExecutionError::InvalidInstruction)
        }
        fn write_u8(&mut self, _: u64, _: u8) -> ExecResult<()> {
            Err(ExecutionError::InvalidInstruction)
        }
        fn write_u16(&mut self, _: u64, _: u16) -> ExecResult<()> {
            Err(ExecutionError::InvalidInstruction)
        }
        fn write_u32(&mut self, _: u64, _: u32) -> ExecResult<()> {
            Err(ExecutionError::InvalidInstruction)
        }
        fn write_u64(&mut self, _: u64, _: u64) -> ExecResult<()> {
            Err(ExecutionError::InvalidInstruction)
        }
        fn get_call_depth(&self) -> usize {
            0
        }
        fn max_call_depth(&self) -> usize {
            0
        }
        fn push_frame(&mut self, _: usize, _: [u64; 4], _: u64) -> ExecResult<()> {
            Err(ExecutionError::InvalidInstruction)
        }
        fn pop_frame(&mut self) -> Option<(usize, [u64; 4], u64)> {
            None
        }
        fn halt(&mut self, _: u64) {}
        fn get_stack_frame_size(&self) -> u64 {
            0
        }
        fn handle_syscall(&mut self, _: &str) -> ExecResult<u64> {
            Err(ExecutionError::InvalidInstruction)
        }
    }

    use sbpf_common::{
        errors::ExecutionError,
        execute::{
            ExecutionResult as ExecResult, execute_binary_immediate, execute_binary_register,
            execute_load_immediate,
        },
    };

    /// Runs a straight-line program (ending in `exit`) and returns r0. With
    /// `ebpf`, results of `add32/sub32/mul32` are zero-extended as eBPF requires;
    /// otherwise the SBPF executor's own semantics apply.
    fn execute(program: &[Instruction], inputs: [u64; 4], ebpf: bool) -> u64 {
        let mut vm = RegVm {
            regs: [0; 11],
            pc: 0,
        };
        vm.regs[1..5].copy_from_slice(&inputs);
        for inst in program {
            let result = match inst.opcode.group() {
                OperationType::Exit => break,
                OperationType::LoadImmediate => execute_load_immediate(&mut vm, inst),
                OperationType::BinaryImmediate => execute_binary_immediate(&mut vm, inst),
                OperationType::BinaryRegister => execute_binary_register(&mut vm, inst),
                other => panic!("unsupported in differential test: {other:?}"),
            };
            result.unwrap();
            if ebpf && sign_extending_alu32(inst.opcode).is_some() {
                let dst = inst.dst.as_ref().unwrap().n as usize;
                vm.regs[dst] &= u32::MAX as u64;
            }
        }
        vm.regs[0]
    }

    fn instructions(program: &[Src]) -> Vec<Instruction> {
        program
            .iter()
            .filter_map(|node| match node {
                Src::Inst(inst) => Some(inst.clone()),
                Src::Label(_) => None,
            })
            .collect()
    }

    #[test]
    fn test_rewritten_programs_match_ebpf_semantics() {
        let programs: Vec<Vec<Src>> = vec![
            // Fix-up read by exit.
            vec![
                Src::Label("entrypoint"),
                inst(Opcode::Add32Reg, Some(1), Some(2), None),
                inst(Opcode::Mov64Reg, Some(0), Some(1), None),
                exit(),
            ],
            // Fix-up enabling a downstream promotion.
            vec![
                Src::Label("entrypoint"),
                inst(Opcode::Sub32Reg, Some(1), Some(2), None),
                inst(Opcode::Rsh64Imm, Some(1), None, Some(1)),
                inst(Opcode::Add32Imm, Some(1), None, Some(0x4000_0000)),
                inst(Opcode::Mov64Reg, Some(0), Some(1), None),
                exit(),
            ],
            // Promotion of a constant with bit 31 set.
            vec![
                Src::Label("entrypoint"),
                inst(Opcode::Lddw, Some(1), None, Some(0x9000_0000)),
                inst(Opcode::Sub32Imm, Some(1), None, Some(1)),
                inst(Opcode::Add64Reg, Some(1), Some(3), None),
                inst(Opcode::Mov64Reg, Some(0), Some(1), None),
                exit(),
            ],
            // Kept: only the low half survives the mask.
            vec![
                Src::Label("entrypoint"),
                inst(Opcode::Mul32Reg, Some(1), Some(2), None),
                inst(Opcode::And64Imm, Some(1), None, Some(0xFFFF)),
                inst(Opcode::Mov64Reg, Some(0), Some(1), None),
                exit(),
            ],
            // Demand passed through mov64 and add64 into r0.
            vec![
                Src::Label("entrypoint"),
                inst(Opcode::Mul32Imm, Some(1), None, Some(-3)),
                inst(Opcode::Mov64Reg, Some(3), Some(1), None),
                inst(Opcode::Add64Reg, Some(3), Some(4), None),
                inst(Opcode::Mov64Reg, Some(0), Some(3), None),
                exit(),
            ],
            // A 32-bit consumer between the def and a 64-bit one.
            vec![
                Src::Label("entrypoint"),
                inst(Opcode::Add32Reg, Some(1), Some(2), None),
                inst(Opcode::Xor32Imm, Some(1), None, Some(0x5555)),
                inst(Opcode::Add32Reg, Some(3), Some(1), None),
                inst(Opcode::Mov64Reg, Some(0), Some(3), None),
                exit(),
            ],
        ];
        let values = [
            0,
            1,
            0x7FFF_FFFF,
            0x8000_0000,
            0xFFFF_FFFF,
            0x1_0000_0000,
            0x1234_5678_9ABC_DEF0,
            u64::MAX,
        ];

        for program in &programs {
            let original = instructions(program);
            let mut cfg = build(program);
            fix_alu32_extension(&mut cfg);
            let rewritten: Vec<Instruction> = cfg
                .all_instructions()
                .map(|(_, node)| node.instruction().unwrap().clone())
                .collect();

            for &a in &values {
                for &b in &values {
                    for &c in &values {
                        for &e in &values {
                            let inputs = [a, b, c, e];
                            assert_eq!(
                                execute(&rewritten, inputs, false),
                                execute(&original, inputs, true),
                                "inputs {inputs:x?}\noriginal {:?}\nrewritten {:?}",
                                original.iter().map(|i| i.opcode).collect::<Vec<_>>(),
                                rewritten.iter().map(|i| i.opcode).collect::<Vec<_>>(),
                            );
                        }
                    }
                }
            }
        }
    }

    /// Sanity check for the harness: without the pass, SBPF and eBPF disagree.
    #[test]
    fn test_unrewritten_program_differs_from_ebpf_semantics() {
        let program = instructions(&[
            Src::Label("entrypoint"),
            inst(Opcode::Add32Reg, Some(1), Some(2), None),
            inst(Opcode::Mov64Reg, Some(0), Some(1), None),
            exit(),
        ]);
        let inputs = [0x8000_0000, 0, 0, 0];
        assert_eq!(execute(&program, inputs, true), 0x8000_0000);
        assert_eq!(execute(&program, inputs, false), 0xFFFF_FFFF_8000_0000);
    }
}
