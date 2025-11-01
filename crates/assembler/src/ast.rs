use {
    crate::{
        CompileError,
        astnode::{ASTNode, ROData},
        dynsym::{DynamicSymbolMap, RelDynMap, RelocationType, get_relocation_info},
        parser::ParseResult,
        section::{CodeSection, DataSection},
    },
    either::Either,
    sbpf_common::{
        inst_param::Number,
        instruction::Instruction,
        opcode::Opcode,
        platform::BpfPlatform,
    },
    std::collections::HashMap,
};

#[derive(Default)]
pub struct AST {
    pub nodes: Vec<ASTNode>,
    pub rodata_nodes: Vec<ASTNode>,

    pub entry_label: Option<String>,
    text_size: u64,
    rodata_size: u64,
}

impl AST {
    pub fn new() -> Self {
        Self::default()
    }

    //
    pub fn set_text_size(&mut self, text_size: u64) {
        self.text_size = text_size;
    }

    //
    pub fn set_rodata_size(&mut self, rodata_size: u64) {
        self.rodata_size = rodata_size;
    }

    //
    pub fn get_instruction_at_offset(&mut self, offset: u64) -> Option<&mut Instruction> {
        self.nodes
            .iter_mut()
            .find(|node| match node {
                ASTNode::Instruction {
                    instruction: _,
                    offset: inst_offset,
                    ..
                } => offset == *inst_offset,
                _ => false,
            })
            .map(|node| match node {
                ASTNode::Instruction { instruction, .. } => instruction,
                _ => panic!("Expected Instruction node"),
            })
    }

    //
    pub fn get_rodata_at_offset(&self, offset: u64) -> Option<&ROData> {
        self.rodata_nodes
            .iter()
            .find(|node| match node {
                ASTNode::ROData {
                    rodata: _,
                    offset: rodata_offset,
                    ..
                } => offset == *rodata_offset,
                _ => false,
            })
            .map(|node| match node {
                ASTNode::ROData { rodata, .. } => rodata,
                _ => panic!("Expected ROData node"),
            })
    }

    //
    pub fn build_program<Platform: BpfPlatform>(&mut self) -> Result<ParseResult, Vec<CompileError>> {
        let mut label_offset_map: HashMap<String, u64> = HashMap::new();

        // iterate through text labels and rodata labels and find the pair
        // of each label and offset
        for node in &self.nodes {
            if let ASTNode::Label { label, offset } = node {
                label_offset_map.insert(label.name.clone(), *offset);
            }
        }

        for node in &self.rodata_nodes {
            if let ASTNode::ROData { rodata, offset } = node {
                label_offset_map.insert(rodata.name.clone(), *offset + self.text_size);
            }
        }

        // 1. resolve labels in the intruction nodes for lddw and jump
        // 2. find relocation information

        let program_is_static = !self.nodes.iter().any(|node| matches!(node, ASTNode::Instruction { instruction: inst, .. } if inst.needs_relocation()));
        let mut relocations = RelDynMap::new();
        let mut dynamic_symbols = DynamicSymbolMap::new();

        let mut errors = Vec::new();

        for node in &mut self.nodes {
            if let ASTNode::Instruction {
                instruction: inst,
                offset,
                ..
            } = node
            {
                // For jump/call instructions, replace label with relative offsets
                if inst.is_jump()
                    && let Some(Either::Left(label)) = &inst.off
                {
                    if let Some(target_offset) = label_offset_map.get(label) {
                        let rel_offset = (*target_offset as i64 - *offset as i64) / 8 - 1;
                        inst.off = Some(Either::Right(rel_offset as i16));
                    } else {
                        errors.push(CompileError::UndefinedLabel {
                            label: label.clone(),
                            span: inst.span.clone(),
                            custom_label: None,
                        });
                    }
                } else if inst.opcode == Opcode::Call
                    && let Some(Either::Left(label)) = &inst.imm
                    && let Some(target_offset) = label_offset_map.get(label)
                {
                    let rel_offset = (*target_offset as i64 - *offset as i64) / 8 - 1;
                    inst.imm = Some(Either::Right(Number::Int(rel_offset)));
                }

                if inst.needs_relocation() {
                    let (reloc_type, label) = get_relocation_info(inst);
                    relocations.add_rel_dyn(*offset, reloc_type, label.clone());
                    if reloc_type == RelocationType::RSbfSyscall {
                        dynamic_symbols.add_call_target(label.clone(), *offset);
                    }
                }
                if inst.opcode == Opcode::Lddw
                    && let Some(Either::Left(name)) = &inst.imm
                {
                    let label = name.clone();
                    if let Some(target_offset) = label_offset_map.get(&label) {
                        let ph_count = if program_is_static { 1 } else { 3 };
                        let ph_offset = 64 + (ph_count as u64 * 56) as i64;
                        let abs_offset = *target_offset as i64 + ph_offset;
                        // Replace label with immediate value
                        inst.imm = Some(Either::Right(Number::Addr(abs_offset)));
                    } else {
                        errors.push(CompileError::UndefinedLabel {
                            label: name.clone(),
                            span: inst.span.clone(),
                            custom_label: None,
                        });
                    }
                }
            }
        }

        // Set entry point offset if an entry label was specified
        if let Some(entry_label) = &self.entry_label
            && let Some(offset) = label_offset_map.get(entry_label)
        {
            dynamic_symbols.add_entry_point(entry_label.clone(), *offset);
        }

        if !errors.is_empty() {
            Err(errors)
        } else {
            Ok(ParseResult {
                code_section: CodeSection::new::<Platform>(std::mem::take(&mut self.nodes), self.text_size),
                data_section: DataSection::new(
                    std::mem::take(&mut self.rodata_nodes),
                    self.rodata_size,
                ),
                dynamic_symbols,
                relocation_data: relocations,
                prog_is_static: program_is_static,
            })
        }
    }
}
