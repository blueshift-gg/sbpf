//! Value-range analysis over def-use chains.
//!
//! Tracks, for every def, an interval of the unsigned 64-bit value it writes
//! under SBPF semantics. Ranges start at `TOP` and only narrow, so every
//! intermediate result is sound. After an in-place edit, `update` re-evaluates
//! the affected defs instead of recomputing everything.

use {
    crate::alu32_extension::sign_extending_alu32,
    sbpf_common::{
        OpcodeTable, inst_param::Number, instruction::Instruction, opcode::Opcode,
        optype::OperationType,
    },
    sbpf_ir::{
        CfgFunction, DefId, DefSite, InstructionNode, OperandKind, graph_engine::WorklistEngine,
    },
};

pub const U32_MAX: i128 = u32::MAX as i128;
pub const U64_MAX: i128 = u64::MAX as i128;

/// Interval of the unsigned 64-bit register value. Held in `i128` so that
/// exact arithmetic results can be checked before they are accepted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Range {
    pub lo: i128,
    pub hi: i128,
}

impl Range {
    pub const TOP: Range = Range { lo: 0, hi: U64_MAX };
    pub const U32: Range = Range { lo: 0, hi: U32_MAX };

    pub fn exact(value: i128) -> Range {
        Range::bounded(value, value)
    }

    /// The interval if it is a valid register value, else `TOP`.
    pub fn bounded(lo: i128, hi: i128) -> Range {
        if 0 <= lo && lo <= hi && hi <= U64_MAX {
            Range { lo, hi }
        } else {
            Range::TOP
        }
    }

    pub fn within(self, lo: i128, hi: i128) -> bool {
        lo <= self.lo && self.hi <= hi
    }

    pub fn is_u32(self) -> bool {
        self.within(0, U32_MAX)
    }

    pub fn join(self, other: Range) -> Range {
        Range {
            lo: self.lo.min(other.lo),
            hi: self.hi.max(other.hi),
        }
    }

    /// The value after zero-extending its low 32 bits.
    pub fn zext32(self) -> Range {
        if self.is_u32() { self } else { Range::U32 }
    }

    fn shr(self, k: u32) -> Range {
        Range {
            lo: self.lo >> k,
            hi: self.hi >> k,
        }
    }
}

/// Stop refining a def's range after this many updates. Every update keeps the
/// range sound, so stopping early only costs precision.
const MAX_RANGE_UPDATES: u8 = 8;

/// Per-def ranges for one function, indexed by `DefId`.
pub struct RangeAnalysis {
    range: Vec<Range>,
    updates: Vec<u8>,
    worklist: WorklistEngine<DefId>,
}

impl RangeAnalysis {
    /// Computes ranges for every def of `func`. Visiting `order` first (defs in
    /// reverse postorder) gives straight-line code exact ranges in one sweep.
    pub fn compute(func: &CfgFunction, order: impl IntoIterator<Item = DefId>) -> Self {
        let mut analysis = Self {
            range: Vec::new(),
            updates: Vec::new(),
            worklist: WorklistEngine::new(),
        };
        analysis.update(func, order);
        analysis
    }

    /// Re-evaluates `changed` defs after an in-place edit, and everything that
    /// depends on them. Defs appended by the edit are picked up automatically.
    pub fn update(&mut self, func: &CfgFunction, changed: impl IntoIterator<Item = DefId>) {
        let n = func.defs().len();
        self.range.resize(n, Range::TOP);
        self.updates.resize(n, 0);
        self.worklist.extend(changed);
        while let Some(d) = self.worklist.pop() {
            if self.updates[d.index()] >= MAX_RANGE_UPDATES {
                continue;
            }
            let new = self.transfer(func, d);
            let old = self.range[d.index()];
            if new == old {
                continue;
            }
            // Ranges start at TOP and transfer rules are monotone, so an update
            // may only narrow. Growth means a transfer rule is broken.
            debug_assert!(
                new.within(old.lo, old.hi),
                "range of {d:?} grew from {old:?} to {new:?}"
            );
            self.range[d.index()] = new;
            self.updates[d.index()] += 1;
            for user in &func.def(d).users {
                self.worklist
                    .extend(func.node(user.inst).defs().iter().copied());
            }
        }
    }

    pub fn range(&self, d: DefId) -> Range {
        self.range[d.index()]
    }

    /// Range of an operand: the join over its reaching defs.
    pub fn operand_range(&self, node: &InstructionNode, kind: OperandKind) -> Option<Range> {
        let used = node.uses().iter().find(|u| u.kind == kind)?;
        Some(
            used.reaching
                .iter()
                .map(|&d| self.range(d))
                .reduce(Range::join)
                .unwrap_or(Range::TOP),
        )
    }

    /// Exact integer result of an `add32/sub32/mul32` node, before truncation, if
    /// its register inputs are known to fit in 32 bits (so their 64-bit and
    /// 32-bit values agree). The immediate is taken sign-extended, as the 64-bit
    /// form would read it.
    pub fn ebpf_result(&self, node: &InstructionNode) -> Option<Range> {
        let inst = node.instruction()?;
        let lhs = self.operand_range(node, OperandKind::Dst)?;
        let rhs = match inst.opcode.group() {
            OperationType::BinaryRegister => self.operand_range(node, OperandKind::Src)?,
            _ => {
                let imm = imm_value(inst)? as i128;
                Range { lo: imm, hi: imm }
            }
        };
        if !lhs.is_u32() || (inst.opcode.group() == OperationType::BinaryRegister && !rhs.is_u32())
        {
            return None;
        }
        Some(match inst.opcode {
            Opcode::Add32Imm | Opcode::Add32Reg => Range {
                lo: lhs.lo + rhs.lo,
                hi: lhs.hi + rhs.hi,
            },
            Opcode::Sub32Imm | Opcode::Sub32Reg => Range {
                lo: lhs.lo - rhs.hi,
                hi: lhs.hi - rhs.lo,
            },
            Opcode::Mul32Imm | Opcode::Mul32Reg => {
                let corners = [
                    lhs.lo * rhs.lo,
                    lhs.lo * rhs.hi,
                    lhs.hi * rhs.lo,
                    lhs.hi * rhs.hi,
                ];
                Range {
                    lo: *corners.iter().min().unwrap(),
                    hi: *corners.iter().max().unwrap(),
                }
            }
            _ => return None,
        })
    }

    /// Range of the value def `d` writes, in the current Cfg.
    fn transfer(&self, func: &CfgFunction, d: DefId) -> Range {
        let DefSite::Inst(key) = func.def(d).site else {
            return Range::TOP;
        };
        let node = func.node(key);
        let Some(inst) = node.instruction() else {
            return Range::TOP;
        };
        let imm = imm_value(inst);
        let dst = || {
            self.operand_range(node, OperandKind::Dst)
                .unwrap_or(Range::TOP)
        };
        let src = || {
            self.operand_range(node, OperandKind::Src)
                .unwrap_or(Range::TOP)
        };
        // The second operand: a register, or the immediate sign-extended to 64 bits.
        let rhs = || match inst.opcode.group() {
            OperationType::BinaryRegister => Some(src()),
            _ => imm.map(|v| Range {
                lo: v as i128,
                hi: v as i128,
            }),
        };

        if sign_extending_alu32(inst.opcode).is_some() {
            // SBPF sign-extends, so only a result below 2^31 is known.
            return match self.ebpf_result(node) {
                Some(e) if e.within(0, (1 << 31) - 1) => e,
                _ => Range::TOP,
            };
        }

        match inst.opcode {
            Opcode::Lddw => imm.map_or(Range::TOP, |v| Range::exact(v as u64 as i128)),
            Opcode::Ldxb => Range::bounded(0, 0xFF),
            Opcode::Ldxh => Range::bounded(0, 0xFFFF),
            Opcode::Ldxw => Range::U32,
            Opcode::Mov64Imm => imm.map_or(Range::TOP, |v| Range::exact(v as u64 as i128)),
            Opcode::Mov32Imm => imm.map_or(Range::U32, |v| Range::exact(v as u32 as i128)),
            Opcode::Mov64Reg => src(),
            Opcode::Mov32Reg => src().zext32(),
            Opcode::And32Imm => match imm {
                Some(m @ 0..) => Range::bounded(0, (m as i128).min(U32_MAX)),
                _ => Range::U32,
            },
            Opcode::And32Reg => Range::bounded(0, dst().zext32().hi.min(src().zext32().hi)),
            Opcode::Rsh32Imm => imm.map_or(Range::U32, |k| dst().zext32().shr(k as u32 & 31)),
            op if op.is_alu32() => Range::U32,
            Opcode::Add64Imm | Opcode::Add64Reg => match rhs() {
                Some(b) => Range::bounded(dst().lo + b.lo, dst().hi + b.hi),
                None => Range::TOP,
            },
            Opcode::Sub64Imm | Opcode::Sub64Reg => match rhs() {
                Some(b) => Range::bounded(dst().lo - b.hi, dst().hi - b.lo),
                None => Range::TOP,
            },
            // Two 64-bit bounds multiply to ~2^128, past i128; overflow is TOP.
            Opcode::Mul64Imm | Opcode::Mul64Reg => match rhs() {
                Some(b) if b.lo >= 0 => {
                    match (dst().lo.checked_mul(b.lo), dst().hi.checked_mul(b.hi)) {
                        (Some(lo), Some(hi)) => Range::bounded(lo, hi),
                        _ => Range::TOP,
                    }
                }
                _ => Range::TOP,
            },
            Opcode::And64Imm => match imm {
                Some(m @ 0..) => Range::bounded(0, m as i128),
                _ => dst(),
            },
            Opcode::And64Reg => Range::bounded(0, dst().hi.min(src().hi)),
            Opcode::Rsh64Imm => imm.map_or(Range::TOP, |k| dst().shr(k as u32 & 63)),
            Opcode::Lsh64Imm => match imm {
                Some(k) => {
                    let k = k as u32 & 63;
                    Range::bounded(dst().lo << k, dst().hi << k)
                }
                None => Range::TOP,
            },
            Opcode::Div64Imm | Opcode::Div64Reg | Opcode::Mod64Imm | Opcode::Mod64Reg => {
                Range::bounded(0, dst().hi)
            }
            Opcode::Le | Opcode::Be => match imm {
                Some(16) => Range::bounded(0, 0xFFFF),
                Some(32) => Range::U32,
                _ => Range::TOP,
            },
            _ => Range::TOP,
        }
    }
}

/// The numeric immediate as the VM reads it, if there is one (labels are not
/// resolved yet). Only `lddw` carries 64 bits; every other immediate is a
/// 32-bit field that is sign-extended.
pub fn imm_value(inst: &Instruction) -> Option<i64> {
    let value = match inst.imm.as_ref()?.as_ref().right()? {
        Number::Int(v) | Number::Addr(v) => *v,
    };
    Some(if inst.opcode == Opcode::Lddw {
        value
    } else {
        value as i32 as i64
    })
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        either::Either,
        sbpf_common::inst_param::Register,
        sbpf_ir::{Cfg, InputNode, control_flow_graph},
        std::collections::HashSet,
    };

    enum Src {
        Label(&'static str),
        Inst(Instruction),
    }

    fn build(program: &[Src]) -> Cfg {
        let nodes = program.iter().map(|node| match node {
            Src::Label(label) => InputNode::Label(label),
            Src::Inst(inst) => InputNode::Instruction(inst),
        });
        control_flow_graph(nodes, &HashSet::new(), None)
    }

    fn inst(opcode: Opcode, dst: u8, src: Option<u8>, imm: Option<i64>) -> Src {
        Src::Inst(Instruction {
            opcode,
            dst: Some(Register { n: dst }),
            src: src.map(|n| Register { n }),
            off: None,
            imm: imm.map(|v| Either::Right(Number::Int(v))),
            span: 0..0,
        })
    }

    fn jump(opcode: Opcode, dst: u8, imm: i64, target: &str) -> Src {
        Src::Inst(Instruction {
            opcode,
            dst: Some(Register { n: dst }),
            src: None,
            off: Some(Either::Left(target.to_string())),
            imm: Some(Either::Right(Number::Int(imm))),
            span: 0..0,
        })
    }

    fn exit() -> Src {
        Src::Inst(Instruction {
            opcode: Opcode::Exit,
            dst: None,
            src: None,
            off: None,
            imm: None,
            span: 0..0,
        })
    }

    /// Ranges of the first def of each instruction, in instruction order.
    fn ranges(cfg: &Cfg, analysis: &RangeAnalysis) -> Vec<Option<Range>> {
        cfg.all_instructions()
            .map(|(_, node)| node.defs().first().map(|&d| analysis.range(d)))
            .collect()
    }

    fn all_defs(cfg: &Cfg) -> Vec<DefId> {
        (0..cfg.functions()[0].defs().len() as u32)
            .map(DefId)
            .collect()
    }

    #[test]
    fn test_straight_line_ranges_are_exact() {
        let cfg = build(&[
            Src::Label("entrypoint"),
            inst(Opcode::Ldxb, 1, Some(10), None),
            inst(Opcode::Add64Imm, 1, None, Some(10)),
            inst(Opcode::Mov32Imm, 2, None, Some(-1)),
            inst(Opcode::Mov64Imm, 3, None, Some(-1)),
            inst(Opcode::Rsh64Imm, 3, None, Some(60)),
            exit(),
        ]);
        let analysis = RangeAnalysis::compute(&cfg.functions()[0], all_defs(&cfg));

        assert_eq!(
            ranges(&cfg, &analysis),
            [
                Some(Range { lo: 0, hi: 0xFF }),
                Some(Range { lo: 10, hi: 0x109 }),
                Some(Range::exact(U32_MAX)),
                Some(Range::exact(U64_MAX)),
                Some(Range::exact(0xF)),
                None,
            ]
        );
    }

    #[test]
    fn test_merge_joins_ranges() {
        let cfg = build(&[
            Src::Label("entrypoint"),
            inst(Opcode::Mov64Imm, 1, None, Some(5)),
            jump(Opcode::JeqImm, 2, 0, "join"),
            inst(Opcode::Mov64Imm, 1, None, Some(100)),
            Src::Label("join"),
            inst(Opcode::Mov64Reg, 3, Some(1), None),
            exit(),
        ]);
        let analysis = RangeAnalysis::compute(&cfg.functions()[0], all_defs(&cfg));

        assert_eq!(ranges(&cfg, &analysis)[3], Some(Range { lo: 5, hi: 100 }));
    }

    #[test]
    fn test_mul64_of_wide_ranges_is_top_without_overflow() {
        // Both inputs are unknown 64-bit values: the bound product (~2^128)
        // does not fit in i128 and must become TOP, not panic or wrap.
        let cfg = build(&[
            Src::Label("entrypoint"),
            inst(Opcode::Mul64Reg, 1, Some(2), None),
            inst(Opcode::Ldxb, 3, Some(10), None),
            inst(Opcode::Mul64Imm, 3, None, Some(1000)),
            exit(),
        ]);
        let analysis = RangeAnalysis::compute(&cfg.functions()[0], all_defs(&cfg));

        let r = ranges(&cfg, &analysis);
        assert_eq!(r[0], Some(Range::TOP));
        assert_eq!(r[2], Some(Range { lo: 0, hi: 255_000 }));
    }

    #[test]
    fn test_sign_extending_op_is_known_only_below_bit_31() {
        let cfg = build(&[
            Src::Label("entrypoint"),
            inst(Opcode::Mov64Imm, 1, None, Some(5)),
            inst(Opcode::Add32Imm, 1, None, Some(3)),
            inst(Opcode::Lddw, 2, None, Some(0x9000_0000)),
            inst(Opcode::Add32Imm, 2, None, Some(1)),
            exit(),
        ]);
        let analysis = RangeAnalysis::compute(&cfg.functions()[0], all_defs(&cfg));

        let r = ranges(&cfg, &analysis);
        assert_eq!(r[1], Some(Range::exact(8)));
        assert_eq!(r[3], Some(Range::TOP));
    }

    #[test]
    fn test_update_after_edit_narrows_dependents() {
        let mut cfg = build(&[
            Src::Label("entrypoint"),
            inst(Opcode::Add32Reg, 1, Some(2), None),
            inst(Opcode::Rsh64Imm, 1, None, Some(1)),
            exit(),
        ]);
        let func = &mut cfg.functions[0];
        let mut analysis = RangeAnalysis::compute(func, (0..func.defs().len() as u32).map(DefId));
        let add = func.blocks()[0].instructions()[0].defs()[0];
        let shift = func.blocks()[0].instructions()[1].defs()[0];
        assert_eq!(analysis.range(shift), Range::TOP.shr(1));

        let fixup = Instruction {
            opcode: Opcode::Mov32Reg,
            dst: Some(Register { n: 1 }),
            src: Some(Register { n: 1 }),
            off: None,
            imm: None,
            span: 0..0,
        };
        let new_def = func.insert_redef_after(add, fixup);
        analysis.update(func, [new_def]);

        assert_eq!(analysis.range(new_def), Range::U32);
        assert_eq!(analysis.range(shift), Range::U32.shr(1));
    }
}
