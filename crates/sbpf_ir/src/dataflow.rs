use sbpf_common::{instruction::Instruction, opcode::Opcode};

pub type InstId = usize;

#[derive(Debug, Clone, PartialEq)]
pub struct InstructionNode {
    pub opcode: Opcode,
    source_node_id: Option<usize>,
    instruction: Option<Instruction>,
}

impl InstructionNode {
    pub fn new(opcode: Opcode) -> Self {
        Self {
            opcode,
            source_node_id: None,
            instruction: None,
        }
    }

    pub fn from_instruction(source_node_id: usize, instruction: Instruction) -> Self {
        Self {
            opcode: instruction.opcode,
            source_node_id: Some(source_node_id),
            instruction: Some(instruction),
        }
    }

    pub fn source_node_id(&self) -> Option<usize> {
        self.source_node_id
    }

    pub fn instruction(&self) -> Option<&Instruction> {
        self.instruction.as_ref()
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
