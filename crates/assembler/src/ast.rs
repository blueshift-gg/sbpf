use crate::astnode::{ASTNode, ROData};
use crate::instruction::Instruction;
use crate::dynsym::{RelDynMap, DynamicSymbolMap, RelocationType};
use crate::lexer::{ImmediateValue, Token};
use crate::opcode::Opcode;
use crate::parser::ParseResult;
use crate::section::{DataSection, CodeSection};
use crate::CompileError;

use std::collections::HashMap;

pub struct AST {
    pub nodes: Vec<ASTNode>,
    pub rodata_nodes: Vec<ASTNode>,

    pub entry_label: Option<String>,
    text_size: u64,
    rodata_size: u64,
}

impl AST {
    pub fn new() -> Self {
        Self { nodes: Vec::new(), rodata_nodes: Vec::new(), entry_label: None, text_size: 0, rodata_size: 0 }
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
        self.nodes.iter_mut().find(|node| match node {
            ASTNode::Instruction { instruction: _, offset: inst_offset, .. } => offset == *inst_offset,
            _ => false,
        }).map(|node| match node {
            ASTNode::Instruction { instruction, .. } => instruction,
            _ => panic!("Expected Instruction node"),
        })
    }
    //
    pub fn get_rodata_at_offset(&self, offset: u64) -> Option<&ROData> {
        self.rodata_nodes.iter().find(|node| match node {
            ASTNode::ROData { rodata: _, offset: rodata_offset, .. } => offset == *rodata_offset,
            _ => false,
        }).map(|node| match node {
            ASTNode::ROData { rodata, .. } => rodata,
            _ => panic!("Expected ROData node"),
        })
    }
    //
    pub fn build_program(&mut self) -> Result<ParseResult, Vec<CompileError>> {
        let mut label_offset_map : HashMap<String, u64> = HashMap::new();

        // iterate through text labels and rodata labels and find the pair
        // of each label and offset
        for node in &self.nodes {
            match node {
                ASTNode::Label { label, offset } => {
                    label_offset_map.insert(label.name.clone(), *offset);
                }
                _ => {}
            }
        }

        for node in &self.rodata_nodes {
            match node {
                ASTNode::ROData { rodata, offset } => {
                    label_offset_map.insert(rodata.name.clone(), *offset + self.text_size);
                }
                _ => {}
            }
        }

        // 1. resolve labels in the intruction nodes for lddw and jump
        // 2. find relocation information

        let mut program_is_static = true;
        let mut relocations = RelDynMap::new();
        let mut dynamic_symbols = DynamicSymbolMap::new();

        let mut errors = Vec::new();

        for node in &mut self.nodes {
            match node {
                ASTNode::Instruction { instruction: inst, offset, .. } => {
                    // For jump/call instructions, replace label with relative offsets
                    if inst.is_jump() || inst.opcode == Opcode::Call {
                        if let Some(Token::Identifier(label, span)) = inst.operands.last() {
                            let label = label.clone();
                            if let Some(target_offset) = label_offset_map.get(&label) {
                                let rel_offset = (*target_offset as i64 - *offset as i64) / 8 - 1;
                                let last_idx = inst.operands.len() - 1;
                                inst.operands[last_idx] = Token::ImmediateValue(ImmediateValue::Int(rel_offset), span.clone());
                            } else if inst.is_jump() {
                                // only error out unresolved jump labels, since call 
                                // labels could exist externally
                                errors.push(CompileError::UndefinedLabel { label: label.clone(), span: span.clone(), custom_label: None });
                            }
                        }
                    }
                    // This has to be done before resolving lddw labels since lddw 
                    // operand needs to be absolute offset values
                    if inst.needs_relocation() {
                        program_is_static = false;
                        let (reloc_type, label) = inst.get_relocation_info();
                        relocations.add_rel_dyn(*offset, reloc_type, label.clone());
                        if reloc_type == RelocationType::RSbfSyscall {
                            dynamic_symbols.add_call_target(label.clone(), *offset);
                        }
                    }
                    if inst.opcode == Opcode::Lddw {
                        if let Some(Token::Identifier(name, span)) = inst.operands.last() {
                            let label = name.clone();
                            if let Some(target_offset) = label_offset_map.get(&label) {
                                let ph_count = if program_is_static { 1 } else { 3 };
                                let ph_offset = 64 + (ph_count as u64 * 56) as i64;
                                let abs_offset = *target_offset as i64 + ph_offset;
                                // Replace label with immediate value
                                let last_idx = inst.operands.len() - 1;
                                inst.operands[last_idx] = Token::ImmediateValue(ImmediateValue::Addr(abs_offset), span.clone());
                            }  else {
                                errors.push(CompileError::UndefinedLabel { label: name.clone(), span: span.clone(), custom_label: None });
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        // Set entry point offset if an entry label was specified
        if let Some(entry_label) = &self.entry_label {
            if let Some(offset) = label_offset_map.get(entry_label) {
                dynamic_symbols.add_entry_point(entry_label.clone(), *offset);
            }
        }

        if !errors.is_empty() {
            return Err(errors);
        } else {
            Ok(ParseResult {
                code_section: CodeSection::new(std::mem::take(&mut self.nodes), self.text_size),
                data_section: DataSection::new(std::mem::take(&mut self.rodata_nodes), self.rodata_size),
                dynamic_symbols: dynamic_symbols,
                relocation_data: relocations,
                prog_is_static: program_is_static,
            })
        }
    }
}
