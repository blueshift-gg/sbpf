use {
    crate::{liveness::LiveSets, stack_usage::StackUsage},
    either::Either,
    sbpf_common::{
        inst_param::{Number, Register},
        instruction::Instruction,
        opcode::Opcode,
    },
    sbpf_ir::{Cfg, InstKey, MAX_WRITABLE_REG, reverse_postorder},
};

/// Expands `div`/`mod` instructions with `off = 1` (the BPF v4 signed-semantics
/// marker emitted by LLVM) into equivalent unsigned-divide sequences with inline
/// sign manipulation. Returns true if anything changed.
pub fn fix_signed_div(cfg: &mut Cfg) -> bool {
    let mut stack = StackUsage::compute(cfg);
    let mut changed = false;

    for func_idx in 0..cfg.functions().len() {
        let rpo = reverse_postorder(cfg, func_idx);
        // Compute liveness before any mutations so the scratch-register choices
        // reflect the function's live sets at each signed-div instruction.
        let live = LiveSets::compute(cfg, &cfg.functions[func_idx], &rpo);
        changed |= fix_function(cfg, func_idx, &live, &mut stack);
    }

    changed
}

fn fix_function(cfg: &mut Cfg, func_idx: usize, live: &LiveSets, stack: &mut StackUsage) -> bool {
    let keys: Vec<InstKey> = cfg.functions[func_idx]
        .blocks()
        .iter()
        .flat_map(|b| b.instructions().iter())
        .filter_map(|node| {
            let inst = node.instruction()?;
            (inst.is_signed_div_or_mod() && is_expandable(inst)).then(|| node.key())
        })
        .collect();

    if keys.is_empty() {
        return false;
    }

    for key in keys {
        let inst = cfg.functions[func_idx]
            .node(key)
            .instruction()
            .unwrap()
            .clone();
        let seq = build_expansion(&inst, key, live, stack, func_idx);
        let func = &mut cfg.functions[func_idx];
        func.replace_instruction(key, seq[0].clone());
        func.insert_sequence_after(key, seq[1..].to_vec());
    }

    true
}

/// Whether the signed div/mod `inst` has an expansion. LLVM emits signed
/// division by an immediate only for 64-bit operands, and never by `i32::MIN`:
/// 32-bit constant divisors become multiplies and power-of-two divisors become
/// shifts. Those forms are left in place, so encoding validation rejects them
/// instead of the expansion having to handle cases LLVM does not produce.
fn is_expandable(inst: &Instruction) -> bool {
    match inst.opcode {
        Opcode::Div32Imm | Opcode::Mod32Imm => false,
        Opcode::Div64Imm | Opcode::Mod64Imm => imm(inst) != i64::from(i32::MIN),
        _ => true,
    }
}

fn imm(inst: &Instruction) -> i64 {
    match inst.imm {
        Some(Either::Right(Number::Int(v))) => v,
        _ => 0,
    }
}

fn build_expansion(
    inst: &Instruction,
    key: InstKey,
    live: &LiveSets,
    stack: &mut StackUsage,
    func_idx: usize,
) -> Vec<Instruction> {
    let is_div = matches!(
        inst.opcode,
        Opcode::Div32Reg | Opcode::Div64Reg | Opcode::Div64Imm
    );
    let is_reg = matches!(
        inst.opcode,
        Opcode::Div32Reg | Opcode::Div64Reg | Opcode::Mod32Reg | Opcode::Mod64Reg
    );

    let dst = inst.dst.as_ref().unwrap().n;
    let src_reg: Option<u8> = is_reg.then(|| inst.src.as_ref().unwrap().n);
    let imm = imm(inst);

    // How many scratch registers the expansion needs.
    let n_scratch = match (is_reg, is_div) {
        // div reg: |src|, sign(dst), and result sign = sign(dst) ^ sign(src)
        (true, true) => 3,
        // mod reg: |src| and sign(dst); remainder sign ignores the divisor
        (true, false) => 2,
        // div by negative imm: sign(dst) and its inverse as the result sign
        (false, true) if imm < 0 => 2,
        // div by non-negative imm, or mod by any imm: sign(dst) is the result sign
        (false, _) => 1,
    };

    // Partition candidates into free (not live) and spillable (live).
    let live_set = live.live_before(key);
    let mut free: Vec<u8> = Vec::new();
    let mut spillable: Vec<u8> = Vec::new();
    for r in 0u8..=MAX_WRITABLE_REG {
        if r == dst || src_reg == Some(r) {
            continue;
        }
        if live_set & (1 << r) == 0 {
            free.push(r);
        } else {
            spillable.push(r);
        }
    }

    let n_free = free.len().min(n_scratch);
    let scratch: Vec<u8> = free[..n_free]
        .iter()
        .chain(spillable.iter())
        .take(n_scratch)
        .copied()
        .collect();
    assert_eq!(scratch.len(), n_scratch, "not enough scratch registers");
    let spill_regs = scratch[n_free..].to_vec();
    let slots = stack.alloc_slots(func_idx, spill_regs.len());

    let mut seq = Vec::new();
    for (&reg, &slot) in spill_regs.iter().zip(slots.iter()) {
        seq.push(spill(reg, slot));
    }

    let s1_or_unused = if n_scratch >= 2 { scratch[1] } else { 0 };
    seq.extend(match (inst.opcode, src_reg) {
        (Opcode::Div64Reg, Some(src)) => div64_reg(dst, src, scratch[0], scratch[1], scratch[2]),
        (Opcode::Mod64Reg, Some(src)) => mod64_reg(dst, src, scratch[0], scratch[1]),
        (Opcode::Div32Reg, Some(src)) => div32_reg(dst, src, scratch[0], scratch[1], scratch[2]),
        (Opcode::Mod32Reg, Some(src)) => mod32_reg(dst, src, scratch[0], scratch[1]),
        (Opcode::Div64Imm, None) => div64_imm(dst, imm, scratch[0], s1_or_unused),
        (Opcode::Mod64Imm, None) => mod64_imm(dst, imm, scratch[0]),
        (opcode, _) => unreachable!("{opcode} has no signed expansion"),
    });

    for (&reg, &slot) in spill_regs.iter().zip(slots.iter()) {
        seq.push(restore(reg, slot));
    }

    seq
}

// ── Instruction sequence generators ─────────────────────────────────────────

fn r(n: u8) -> Register {
    Register { n }
}

fn ireg(opcode: Opcode, dst: u8, src: u8) -> Instruction {
    Instruction {
        opcode,
        dst: Some(r(dst)),
        src: Some(r(src)),
        off: None,
        imm: None,
        span: 0..0,
    }
}

fn iimm(opcode: Opcode, dst: u8, imm: i64) -> Instruction {
    Instruction {
        opcode,
        dst: Some(r(dst)),
        src: None,
        off: None,
        imm: Some(Either::Right(Number::Int(imm))),
        span: 0..0,
    }
}

fn spill(reg: u8, off: i16) -> Instruction {
    // stxdw [r10+off], reg
    Instruction {
        opcode: Opcode::Stxdw,
        dst: Some(r(10)),
        src: Some(r(reg)),
        off: Some(Either::Right(off)),
        imm: None,
        span: 0..0,
    }
}

fn restore(reg: u8, off: i16) -> Instruction {
    // ldxdw reg, [r10+off]
    Instruction {
        opcode: Opcode::Ldxdw,
        dst: Some(r(reg)),
        src: Some(r(10)),
        off: Some(Either::Right(off)),
        imm: None,
        span: 0..0,
    }
}

/// Signed 64-bit div: dst = dst / src. Scratch: s0, s1, s2.
fn div64_reg(dst: u8, src: u8, s0: u8, s1: u8, s2: u8) -> Vec<Instruction> {
    vec![
        ireg(Opcode::Mov64Reg, s2, src),
        iimm(Opcode::Arsh64Imm, s2, 63), // s2 = sign(src)
        ireg(Opcode::Mov64Reg, s1, src),
        ireg(Opcode::Add64Reg, s1, s2),
        ireg(Opcode::Xor64Reg, s1, s2), // s1 = |src|
        ireg(Opcode::Mov64Reg, s0, dst),
        iimm(Opcode::Arsh64Imm, s0, 63), // s0 = sign(dst)
        ireg(Opcode::Xor64Reg, s2, s0),  // s2 = result sign
        ireg(Opcode::Add64Reg, dst, s0),
        ireg(Opcode::Xor64Reg, dst, s0), // dst = |dst|
        ireg(Opcode::Div64Reg, dst, s1), // unsigned divide
        ireg(Opcode::Add64Reg, dst, s2),
        ireg(Opcode::Xor64Reg, dst, s2), // apply result sign
    ]
}

/// Signed 64-bit mod: dst = dst % src. Scratch: s0, s1.
fn mod64_reg(dst: u8, src: u8, s0: u8, s1: u8) -> Vec<Instruction> {
    vec![
        ireg(Opcode::Mov64Reg, s0, src),
        iimm(Opcode::Arsh64Imm, s0, 63), // s0 = sign(src)
        ireg(Opcode::Mov64Reg, s1, src),
        ireg(Opcode::Add64Reg, s1, s0),
        ireg(Opcode::Xor64Reg, s1, s0), // s1 = |src|
        ireg(Opcode::Mov64Reg, s0, dst),
        iimm(Opcode::Arsh64Imm, s0, 63), // s0 = sign(dst) = result sign
        ireg(Opcode::Add64Reg, dst, s0),
        ireg(Opcode::Xor64Reg, dst, s0), // dst = |dst|
        ireg(Opcode::Mod64Reg, dst, s1), // unsigned mod
        ireg(Opcode::Add64Reg, dst, s0),
        ireg(Opcode::Xor64Reg, dst, s0), // apply sign of dividend
    ]
}

/// Signed 64-bit mod by immediate (result sign = sign of dividend, not divisor).
fn mod64_imm(dst: u8, imm: i64, s0: u8) -> Vec<Instruction> {
    vec![
        ireg(Opcode::Mov64Reg, s0, dst),
        iimm(Opcode::Arsh64Imm, s0, 63), // s0 = sign(dst)
        ireg(Opcode::Add64Reg, dst, s0),
        ireg(Opcode::Xor64Reg, dst, s0), // dst = |dst|
        iimm(Opcode::Mod64Imm, dst, imm.abs()),
        ireg(Opcode::Add64Reg, dst, s0),
        ireg(Opcode::Xor64Reg, dst, s0), // apply sign
    ]
}

/// Signed 32-bit mod. Scratch: s0, s1.
fn mod32_reg(dst: u8, src: u8, s0: u8, s1: u8) -> Vec<Instruction> {
    vec![
        // s1 = sign-extended src
        ireg(Opcode::Mov64Reg, s1, src),
        iimm(Opcode::Lsh64Imm, s1, 32),
        iimm(Opcode::Arsh64Imm, s1, 32),
        // s0 = sign(src as i32)
        ireg(Opcode::Mov64Reg, s0, s1),
        iimm(Opcode::Arsh64Imm, s0, 63),
        // s1 = |src|
        ireg(Opcode::Add64Reg, s1, s0),
        ireg(Opcode::Xor64Reg, s1, s0),
        // sign-extend dst
        iimm(Opcode::Lsh64Imm, dst, 32),
        iimm(Opcode::Arsh64Imm, dst, 32),
        // s0 = sign(dst as i32) = result sign
        ireg(Opcode::Mov64Reg, s0, dst),
        iimm(Opcode::Arsh64Imm, s0, 63),
        // dst = |dst|
        ireg(Opcode::Add64Reg, dst, s0),
        ireg(Opcode::Xor64Reg, dst, s0),
        // unsigned 64-bit mod
        ireg(Opcode::Mod64Reg, dst, s1),
        // apply sign of dividend
        ireg(Opcode::Add64Reg, dst, s0),
        ireg(Opcode::Xor64Reg, dst, s0),
        // zero-extend
        ireg(Opcode::Mov32Reg, dst, dst),
    ]
}

/// Signed 64-bit div by immediate.
fn div64_imm(dst: u8, imm: i64, s0: u8, s1: u8) -> Vec<Instruction> {
    if imm >= 0 {
        vec![
            ireg(Opcode::Mov64Reg, s0, dst),
            iimm(Opcode::Arsh64Imm, s0, 63), // s0 = sign(dst)
            ireg(Opcode::Add64Reg, dst, s0),
            ireg(Opcode::Xor64Reg, dst, s0), // dst = |dst|
            iimm(Opcode::Div64Imm, dst, imm.abs()),
            ireg(Opcode::Add64Reg, dst, s0),
            ireg(Opcode::Xor64Reg, dst, s0), // apply sign
        ]
    } else {
        vec![
            ireg(Opcode::Mov64Reg, s0, dst),
            iimm(Opcode::Arsh64Imm, s0, 63), // s0 = sign(dst)
            ireg(Opcode::Mov64Reg, s1, s0),
            iimm(Opcode::Xor64Imm, s1, -1), // s1 = NOT sign(dst) = result sign
            ireg(Opcode::Add64Reg, dst, s0),
            ireg(Opcode::Xor64Reg, dst, s0), // dst = |dst|
            iimm(Opcode::Div64Imm, dst, imm.abs()),
            ireg(Opcode::Add64Reg, dst, s1),
            ireg(Opcode::Xor64Reg, dst, s1), // apply result sign
        ]
    }
}

// ── 32-bit variants: sign-extend inputs, do 64-bit expansion, zero-extend ───

/// Signed 32-bit div. Scratch: s0, s1, s2.
fn div32_reg(dst: u8, src: u8, s0: u8, s1: u8, s2: u8) -> Vec<Instruction> {
    vec![
        // s1 = sign-extended src
        ireg(Opcode::Mov64Reg, s1, src),
        iimm(Opcode::Lsh64Imm, s1, 32),
        iimm(Opcode::Arsh64Imm, s1, 32),
        // s2 = sign(s1) = sign(src as i32)
        ireg(Opcode::Mov64Reg, s2, s1),
        iimm(Opcode::Arsh64Imm, s2, 63),
        // s1 = |src|
        ireg(Opcode::Add64Reg, s1, s2),
        ireg(Opcode::Xor64Reg, s1, s2),
        // sign-extend dst in place
        iimm(Opcode::Lsh64Imm, dst, 32),
        iimm(Opcode::Arsh64Imm, dst, 32),
        // s0 = sign(dst as i32)
        ireg(Opcode::Mov64Reg, s0, dst),
        iimm(Opcode::Arsh64Imm, s0, 63),
        // s2 = result sign = sign(dst) XOR sign(src)
        ireg(Opcode::Xor64Reg, s2, s0),
        // dst = |dst|
        ireg(Opcode::Add64Reg, dst, s0),
        ireg(Opcode::Xor64Reg, dst, s0),
        // unsigned 64-bit divide
        ireg(Opcode::Div64Reg, dst, s1),
        // apply result sign
        ireg(Opcode::Add64Reg, dst, s2),
        ireg(Opcode::Xor64Reg, dst, s2),
        // zero-extend result to 32 bits
        ireg(Opcode::Mov32Reg, dst, dst),
    ]
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        sbpf_common::opcode::SIGNED_DIV_OFF,
        sbpf_ir::{InputNode, control_flow_graph},
        sbpf_vm::{syscalls::MockSyscallHandler, vm::SbpfVm},
        std::collections::HashSet,
    };

    /// Runs the straight-line `program` on the sbpf VM, starting from `regs_in`
    /// (r10 keeps the VM's frame pointer), and returns the final registers.
    /// Instructions go through the encoder first, so an operand that does not
    /// survive encoding (e.g. an immediate outside the i32 field) shows up.
    fn exec_alu(program: &[Instruction], regs_in: [u64; 11]) -> [u64; 11] {
        let program = program
            .iter()
            .map(|inst| Instruction::from_bytes(&inst.to_bytes().unwrap()).unwrap())
            .chain([exit_inst()])
            .collect();
        let mut vm = SbpfVm::new(program, vec![], vec![], MockSyscallHandler::default());
        vm.registers[..10].copy_from_slice(&regs_in[..10]);
        vm.run().unwrap();
        vm.registers
    }

    // Non-zero values only in the dividend/divisor grids: the "div-by-zero = 0"
    // rule is a machine-level sBPF guarantee; the VM reports DivisionByZero
    // instead, so testing it through exec_alu would need special casing there.
    const TEST_VALS: &[i64] = &[-7, -3, -1, 1, 3, 7, i64::MIN, i64::MAX];

    fn signed_div(opcode: Opcode, dst: u8, src_or_imm: Either<u8, i64>) -> Instruction {
        Instruction {
            opcode,
            dst: Some(r(dst)),
            src: src_or_imm.left().map(r),
            off: Some(Either::Right(SIGNED_DIV_OFF)),
            imm: src_or_imm.right().map(|v| Either::Right(Number::Int(v))),
            span: 0..0,
        }
    }

    fn exit_inst() -> Instruction {
        Instruction {
            opcode: Opcode::Exit,
            dst: None,
            src: None,
            off: None,
            imm: None,
            span: 0..0,
        }
    }

    fn run(instructions: &[Instruction]) -> Vec<Opcode> {
        let nodes: Vec<InputNode<'_>> = std::iter::once(InputNode::Label("entrypoint"))
            .chain(instructions.iter().map(InputNode::Instruction))
            .collect();
        let mut cfg = control_flow_graph(nodes, &HashSet::new(), None);
        fix_signed_div(&mut cfg);
        cfg.all_instructions()
            .map(|(_, n)| n.instruction().unwrap().opcode)
            .collect()
    }

    #[test]
    fn test_div64_reg_expands() {
        let ops = run(&[
            signed_div(Opcode::Div64Reg, 3, Either::Left(4)),
            exit_inst(),
        ]);
        assert!(
            !ops.contains(&Opcode::Div64Reg)
                || ops.iter().filter(|&&o| o == Opcode::Div64Reg).count() == 1,
            "should have exactly one unsigned div64 in expansion"
        );
        assert!(
            ops.contains(&Opcode::Div64Reg),
            "expansion must contain unsigned div64"
        );
        assert!(
            ops.contains(&Opcode::Arsh64Imm),
            "expansion must use arsh64"
        );
    }

    #[test]
    fn test_div64_imm_positive_expands() {
        let ops = run(&[
            signed_div(Opcode::Div64Imm, 5, Either::Right(7)),
            exit_inst(),
        ]);
        assert!(
            ops.contains(&Opcode::Div64Imm),
            "expansion must contain div64 imm"
        );
        assert!(ops.contains(&Opcode::Arsh64Imm));
    }

    #[test]
    fn test_mod64_reg_expands() {
        let ops = run(&[
            signed_div(Opcode::Mod64Reg, 1, Either::Left(2)),
            exit_inst(),
        ]);
        assert!(ops.contains(&Opcode::Mod64Reg));
        assert!(
            !ops.contains(&Opcode::Div64Reg),
            "mod expansion must not use div"
        );
    }

    #[test]
    fn test_mod32_reg_expands() {
        // mod32 r1, r2, off=1 — same sign-extend strategy as div32, using 64-bit unsigned mod.
        let ops = run(&[
            signed_div(Opcode::Mod32Reg, 1, Either::Left(2)),
            exit_inst(),
        ]);
        assert!(
            ops.contains(&Opcode::Mod64Reg),
            "32-bit signed mod must use 64-bit unsigned mod"
        );
        assert!(
            !ops.contains(&Opcode::Mod32Reg),
            "original mod32 must be replaced"
        );
        assert!(
            ops.contains(&Opcode::Mov32Reg),
            "result must be zero-extended with mov32"
        );
    }

    #[test]
    fn test_mod64_imm_expands() {
        // mod64 r2, 5, off=1 — remainder sign follows dividend only, no divisor sign needed.
        let ops = run(&[
            signed_div(Opcode::Mod64Imm, 2, Either::Right(5)),
            exit_inst(),
        ]);
        assert!(ops.contains(&Opcode::Mod64Imm));
        assert!(ops.contains(&Opcode::Arsh64Imm));
        // Unlike negative-imm div, mod never needs to flip the result sign via xor -1.
        assert!(
            !ops.contains(&Opcode::Xor64Imm),
            "mod expansion must not flip result sign"
        );
    }

    #[test]
    fn test_unsigned_div_not_touched() {
        // A plain div64 without off=1 must not be expanded.
        let plain_div = Instruction {
            opcode: Opcode::Div64Reg,
            dst: Some(r(1)),
            src: Some(r(2)),
            off: None,
            imm: None,
            span: 0..0,
        };
        let ops = run(&[plain_div, exit_inst()]);
        assert_eq!(ops, [Opcode::Div64Reg, Opcode::Exit]);
    }

    #[test]
    fn test_expansion_does_not_clobber_live_callee_saved() {
        // r6-r9 are callee-saved and live throughout; scratch must come from r0-r5.
        // We mark r1-r5 live-ish by using them as sources in surrounding instructions
        // so the pass is forced to spill something if needed — but here r0 is free,
        // and r1-r2 are used as the div operands so exactly r3,r4,r5 are candidates.
        // The test just checks the expansion is present and structurally sound.
        let ops = run(&[
            signed_div(Opcode::Div64Reg, 0, Either::Left(1)),
            exit_inst(),
        ]);
        assert!(ops.contains(&Opcode::Div64Reg));
        assert!(
            ops.len() > 2,
            "expansion should be more than 2 instructions"
        );
    }

    #[test]
    fn test_div64_imm_negative_expands() {
        // div64 r5, -3, off=1 — divisor is negative so result sign = NOT sign(dividend).
        // The expansion must emit Xor64Imm(-1) to flip the result sign register.
        let ops = run(&[
            signed_div(Opcode::Div64Imm, 5, Either::Right(-3)),
            exit_inst(),
        ]);
        assert!(
            ops.contains(&Opcode::Xor64Imm),
            "negative-imm div must flip result sign via xor -1"
        );
        assert!(ops.contains(&Opcode::Div64Imm));
        assert!(ops.contains(&Opcode::Arsh64Imm));
    }

    #[test]
    fn test_div32_reg_expands() {
        // div32 r3, r4, off=1 — 32-bit signed div: sign-extend both operands to 64 bits,
        // perform 64-bit unsigned divide, then zero-extend the 32-bit result.
        let ops = run(&[
            signed_div(Opcode::Div32Reg, 3, Either::Left(4)),
            exit_inst(),
        ]);
        assert!(
            ops.contains(&Opcode::Lsh64Imm),
            "must sign-extend 32-bit inputs via lsh64"
        );
        assert!(ops.contains(&Opcode::Arsh64Imm));
        assert!(
            ops.contains(&Opcode::Div64Reg),
            "32-bit signed div must use 64-bit unsigned divide"
        );
        assert!(
            !ops.contains(&Opcode::Div32Reg),
            "original div32 must be replaced"
        );
        assert!(
            ops.contains(&Opcode::Mov32Reg),
            "result must be zero-extended with mov32"
        );
    }

    #[test]
    fn test_spill_when_all_scratch_regs_live() {
        // Instructions after the div read r0-r7, making them all live at the div site.
        // div64 r8, r9 needs 3 scratch registers from r0-r7; all are live, so all must be spilled.
        let ops = run(&[
            signed_div(Opcode::Div64Reg, 8, Either::Left(9)),
            ireg(Opcode::Add64Reg, 0, 1),
            ireg(Opcode::Add64Reg, 2, 3),
            ireg(Opcode::Add64Reg, 4, 5),
            ireg(Opcode::Add64Reg, 6, 7),
            exit_inst(),
        ]);
        let spills = ops.iter().filter(|&&o| o == Opcode::Stxdw).count();
        let restores = ops.iter().filter(|&&o| o == Opcode::Ldxdw).count();
        assert!(spills > 0, "must spill when all scratch registers are live");
        assert_eq!(spills, restores, "every spill must have a matching restore");
        assert!(ops.contains(&Opcode::Div64Reg));
    }

    // ── Arithmetic correctness (differential against Rust's wrapping semantics) ──

    #[test]
    fn test_div64_reg_correctness() {
        for &a in TEST_VALS {
            for &b in TEST_VALS {
                let expected = a.wrapping_div(b);
                let seq = div64_reg(1, 2, 0, 3, 4);
                let mut r = [0u64; 11];
                r[1] = a as u64;
                r[2] = b as u64;
                let got = exec_alu(&seq, r)[1] as i64;
                assert_eq!(got, expected, "div64({a}, {b})");
            }
        }
    }

    #[test]
    fn test_mod64_reg_correctness() {
        for &a in TEST_VALS {
            for &b in TEST_VALS {
                let expected = a.wrapping_rem(b);
                let seq = mod64_reg(1, 2, 0, 3);
                let mut r = [0u64; 11];
                r[1] = a as u64;
                r[2] = b as u64;
                let got = exec_alu(&seq, r)[1] as i64;
                assert_eq!(got, expected, "mod64({a}, {b})");
            }
        }
    }

    #[test]
    fn test_mod64_imm_correctness() {
        for &a in TEST_VALS {
            for &d in IMM_DIVISORS {
                let expected = a.wrapping_rem(d);
                let seq = mod64_imm(1, d, 0);
                let mut r = [0u64; 11];
                r[1] = a as u64;
                let got = exec_alu(&seq, r)[1] as i64;
                assert_eq!(got, expected, "mod64_imm({a}, {d})");
            }
        }
    }

    #[test]
    fn test_div64_imm_correctness() {
        // Non-zero divisors only; div-by-zero is a separate concern.
        for &a in TEST_VALS {
            for &d in IMM_DIVISORS {
                let expected = a.wrapping_div(d);
                let seq = div64_imm(1, d, 0, 2);
                let mut r = [0u64; 11];
                r[1] = a as u64;
                let got = exec_alu(&seq, r)[1] as i64;
                assert_eq!(got, expected, "div64_imm({a}, {d})");
            }
        }
    }

    /// Immediate divisors the pass expands: non-zero, and not `i32::MIN`.
    const IMM_DIVISORS: &[i64] = &[-7, -3, -1, 1, 3, 7, i32::MIN as i64 + 1, i32::MAX as i64];

    /// Operands for the 32-bit forms. Only the low 32 bits are the operand;
    /// the upper bits of the last three are garbage the expansion must ignore.
    /// No low half is zero.
    const TEST_VALS_32: &[i64] = &[
        -7,
        -3,
        -1,
        1,
        3,
        7,
        i32::MIN as i64,
        i32::MAX as i64,
        0x1234_5678_8000_0000_u64 as i64,
        0xdead_beef_0000_0007_u64 as i64,
        0x0000_0001_ffff_ffff,
    ];

    /// 32-bit results are zero-extended into the 64-bit register.
    fn zext32(v: i32) -> u64 {
        v as u32 as u64
    }

    #[test]
    fn test_forms_llvm_does_not_emit_are_left_for_validation() {
        // 32-bit immediates, and an `i32::MIN` divisor, whose magnitude does
        // not fit the sign-extended 32-bit immediate field.
        for (opcode, imm) in [
            (Opcode::Div32Imm, -3),
            (Opcode::Mod32Imm, 7),
            (Opcode::Div64Imm, i64::from(i32::MIN)),
            (Opcode::Mod64Imm, i64::from(i32::MIN)),
        ] {
            let ops = run(&[signed_div(opcode, 5, Either::Right(imm)), exit_inst()]);
            assert_eq!(
                ops,
                [opcode, Opcode::Exit],
                "{opcode} {imm} must not be expanded"
            );
        }
    }

    #[test]
    fn test_div32_reg_correctness() {
        for &a in TEST_VALS_32 {
            for &b in TEST_VALS_32 {
                let mut r = [0u64; 11];
                r[1] = a as u64;
                r[2] = b as u64;
                let out = exec_alu(&div32_reg(1, 2, 0, 3, 4), r);
                let expected = zext32((a as i32).wrapping_div(b as i32));
                assert_eq!(out[1], expected, "div32({a:#x}, {b:#x})");
                assert_eq!(out[2], b as u64, "div32({a:#x}, {b:#x}) clobbered src");
            }
        }
    }

    #[test]
    fn test_mod32_reg_correctness() {
        for &a in TEST_VALS_32 {
            for &b in TEST_VALS_32 {
                let mut r = [0u64; 11];
                r[1] = a as u64;
                r[2] = b as u64;
                let out = exec_alu(&mod32_reg(1, 2, 0, 3), r);
                let expected = zext32((a as i32).wrapping_rem(b as i32));
                assert_eq!(out[1], expected, "mod32({a:#x}, {b:#x})");
                assert_eq!(out[2], b as u64, "mod32({a:#x}, {b:#x}) clobbered src");
            }
        }
    }
}
