use {
    sbpf_common::{OpcodeTable, instruction::Instruction, opcode::Opcode, optype::OperationType},
    smallvec::SmallVec,
};

pub type InstId = usize;

/// Register number. Only r0–r9 are tracked; r10 (frame pointer) is read-only.
pub type Reg = u8;

/// Highest register that can be defined.
pub const MAX_WRITABLE_REG: Reg = 9;

/// Registers a call passes arguments in, and clobbers.
pub const CALL_ARG_REGS: [Reg; 5] = [1, 2, 3, 4, 5];

// ── Instruction operand tables ───────────────────────────────────────────────

/// Role of a register read within its instruction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OperandKind {
    /// The instruction's `dst` field.
    Dst,
    /// The instruction's `src` field.
    Src,
    /// An ABI read not named by the instruction: call arguments r1–r5, exit's r0.
    Implicit,
}

/// Registers an instruction reads and writes.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Operands {
    pub uses: SmallVec<[(Reg, OperandKind); 3]>,
    pub defs: SmallVec<[Reg; 1]>,
    /// Registers a call leaves undefined (r1–r5).
    pub clobbers: SmallVec<[Reg; 5]>,
}

/// Returns the registers `instruction` reads and writes. Uses of r10 are omitted.
pub fn operands(instruction: &Instruction) -> Operands {
    let dst = instruction.dst.as_ref().map(|r| r.n);
    let src = instruction.src.as_ref().map(|r| r.n);
    let reads_dst = !matches!(
        instruction.opcode,
        Opcode::Mov32Imm | Opcode::Mov32Reg | Opcode::Mov64Imm | Opcode::Mov64Reg
    );

    let mut ops = Operands::default();
    let mut read = |reg: Option<Reg>, kind| {
        if let Some(reg) = reg.filter(|&r| r <= MAX_WRITABLE_REG) {
            ops.uses.push((reg, kind));
        }
    };

    match instruction.opcode.group() {
        OperationType::LoadImmediate => {}
        OperationType::LoadMemory => read(src, OperandKind::Src),
        OperationType::StoreImmediate => read(dst, OperandKind::Dst),
        OperationType::StoreRegister => {
            read(dst, OperandKind::Dst);
            read(src, OperandKind::Src);
        }
        OperationType::BinaryImmediate | OperationType::Unary | OperationType::Endian => {
            if reads_dst {
                read(dst, OperandKind::Dst);
            }
        }
        OperationType::BinaryRegister => {
            if reads_dst {
                read(dst, OperandKind::Dst);
            }
            read(src, OperandKind::Src);
        }
        OperationType::Jump => {}
        OperationType::JumpImmediate | OperationType::Jump32Immediate => {
            read(dst, OperandKind::Dst)
        }
        OperationType::JumpRegister | OperationType::Jump32Register => {
            read(dst, OperandKind::Dst);
            read(src, OperandKind::Src);
        }
        OperationType::CallImmediate | OperationType::CallRegister => {
            // callx carries its target register in `dst`.
            read(dst, OperandKind::Dst);
            for reg in CALL_ARG_REGS {
                read(Some(reg), OperandKind::Implicit);
            }
        }
        OperationType::Exit => read(Some(0), OperandKind::Implicit),
    }

    match instruction.opcode.group() {
        OperationType::LoadImmediate
        | OperationType::LoadMemory
        | OperationType::BinaryImmediate
        | OperationType::BinaryRegister
        | OperationType::Unary
        | OperationType::Endian => {
            if let Some(reg) = dst.filter(|&r| r <= MAX_WRITABLE_REG) {
                ops.defs.push(reg);
            }
        }
        OperationType::CallImmediate | OperationType::CallRegister => {
            ops.defs.push(0);
            ops.clobbers.extend(CALL_ARG_REGS);
        }
        _ => {}
    }

    ops
}

// ── Def-use chains ───────────────────────────────────────────────────────────

/// Stable id of an instruction within its `CfgFunction`. Survives insertions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct InstKey(pub u32);

impl InstKey {
    pub fn index(self) -> usize {
        self.0 as usize
    }
}

/// Index into `CfgFunction::defs`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DefId(pub u32);

impl DefId {
    pub fn index(self) -> usize {
        self.0 as usize
    }
}

/// Names one use: operand `operand` of the instruction `inst`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UseRef {
    pub inst: InstKey,
    pub operand: u8,
}

/// Where a register gets its value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DefSite {
    /// Written by an instruction.
    Inst(InstKey),
    /// Live-in at function entry.
    Entry,
    /// Left undefined by a call.
    CallClobber(InstKey),
}

/// A register definition and the uses it reaches.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Def {
    pub site: DefSite,
    pub reg: Reg,
    pub users: SmallVec<[UseRef; 4]>,
}

/// A register read and the definitions that reach it. More than one reaching
/// def means paths merge here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Use {
    pub reg: Reg,
    pub kind: OperandKind,
    /// Sorted by `DefId`.
    pub reaching: SmallVec<[DefId; 2]>,
}

// ── Instruction node ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub struct InstructionNode {
    pub opcode: Opcode,
    source_node_id: Option<usize>,
    instruction: Option<Instruction>,
    key: InstKey,
    defs: SmallVec<[DefId; 1]>,
    uses: SmallVec<[Use; 3]>,
}

impl InstructionNode {
    pub fn new(opcode: Opcode) -> Self {
        Self {
            opcode,
            source_node_id: None,
            instruction: None,
            key: InstKey::default(),
            defs: SmallVec::new(),
            uses: SmallVec::new(),
        }
    }

    pub fn from_instruction(source_node_id: usize, instruction: Instruction) -> Self {
        Self::with_source(Some(source_node_id), instruction)
    }

    /// A node inserted by a transform, with no AST node behind it.
    pub(crate) fn synthesized(instruction: Instruction) -> Self {
        Self::with_source(None, instruction)
    }

    fn with_source(source_node_id: Option<usize>, instruction: Instruction) -> Self {
        let uses = operands(&instruction)
            .uses
            .into_iter()
            .map(|(reg, kind)| Use {
                reg,
                kind,
                reaching: SmallVec::new(),
            })
            .collect();
        Self {
            opcode: instruction.opcode,
            source_node_id,
            instruction: Some(instruction),
            key: InstKey::default(),
            defs: SmallVec::new(),
            uses,
        }
    }

    pub fn source_node_id(&self) -> Option<usize> {
        self.source_node_id
    }

    pub fn instruction(&self) -> Option<&Instruction> {
        self.instruction.as_ref()
    }

    pub fn key(&self) -> InstKey {
        self.key
    }

    /// Definitions made here. For a call: r0 first, then the r1–r5 clobbers.
    pub fn defs(&self) -> &[DefId] {
        &self.defs
    }

    pub fn uses(&self) -> &[Use] {
        &self.uses
    }

    pub(crate) fn set_key(&mut self, key: InstKey) {
        self.key = key;
    }

    pub(crate) fn set_opcode(&mut self, opcode: Opcode) {
        self.opcode = opcode;
        if let Some(instruction) = &mut self.instruction {
            instruction.opcode = opcode;
        }
    }

    pub(crate) fn defs_mut(&mut self) -> &mut SmallVec<[DefId; 1]> {
        &mut self.defs
    }

    pub(crate) fn uses_mut(&mut self) -> &mut SmallVec<[Use; 3]> {
        &mut self.uses
    }
}

pub trait InstructionVisitor {
    fn visit_instruction_node(&mut self, node: &InstructionNode) {
        walk_instruction_node(self, node);
    }

    fn visit_call(&mut self, node: &InstructionNode, _instruction: &Instruction) {
        self.visit_default(node);
    }

    fn visit_jump(&mut self, node: &InstructionNode, _instruction: &Instruction) {
        self.visit_default(node);
    }

    fn visit_default(&mut self, _node: &InstructionNode) {}
}

pub fn walk_instruction_nodes<'a, I, V>(visitor: &mut V, nodes: I)
where
    I: IntoIterator<Item = &'a InstructionNode>,
    V: InstructionVisitor + ?Sized,
{
    for node in nodes {
        visitor.visit_instruction_node(node);
    }
}

pub fn walk_instruction_node<V>(visitor: &mut V, node: &InstructionNode)
where
    V: InstructionVisitor + ?Sized,
{
    let Some(instruction) = node.instruction() else {
        visitor.visit_default(node);
        return;
    };

    if instruction.opcode == Opcode::Call {
        visitor.visit_call(node, instruction);
    } else if instruction.is_jump() {
        visitor.visit_jump(node, instruction);
    } else {
        visitor.visit_default(node);
    }
}

#[cfg(test)]
mod tests {
    use {super::*, sbpf_common::instruction::Instruction};

    #[test]
    fn test_dataflow_instruction_node_tracks_source() {
        let instruction = instruction(Opcode::Exit);
        let node = InstructionNode::from_instruction(7, instruction.clone());

        assert_eq!(node.opcode, Opcode::Exit);
        assert_eq!(node.source_node_id(), Some(7));
        assert_eq!(node.instruction(), Some(&instruction));
    }

    #[test]
    fn test_dataflow_visitor_dispatches_instruction_kinds() {
        struct Visitor {
            events: Vec<String>,
        }

        impl InstructionVisitor for Visitor {
            fn visit_call(&mut self, node: &InstructionNode, _instruction: &Instruction) {
                self.events
                    .push(format!("call:{}", node.source_node_id().unwrap()));
            }

            fn visit_jump(&mut self, node: &InstructionNode, _instruction: &Instruction) {
                self.events
                    .push(format!("jump:{}", node.source_node_id().unwrap()));
            }

            fn visit_default(&mut self, node: &InstructionNode) {
                self.events.push(format!("default:{}", node.opcode));
            }
        }

        let mut visitor = Visitor { events: Vec::new() };
        let call = InstructionNode::from_instruction(1, instruction(Opcode::Call));
        let jump = InstructionNode::from_instruction(2, instruction(Opcode::Ja));
        let exit = InstructionNode::from_instruction(3, instruction(Opcode::Exit));

        visitor.visit_instruction_node(&call);
        visitor.visit_instruction_node(&jump);
        visitor.visit_instruction_node(&exit);

        assert_eq!(visitor.events, vec!["call:1", "jump:2", "default:exit"]);
    }

    fn instruction(opcode: Opcode) -> Instruction {
        Instruction {
            opcode,
            dst: None,
            src: None,
            off: None,
            imm: None,
            span: 0..0,
        }
    }
}
