use {
    crate::{
        CompileError, SbpfArch,
        astnode::{ASTNode, ROData},
        dynsym::{DynamicSymbolMap, RelDynMap, RelocationType},
        parser::ParseResult,
        section::{CodeSection, DataSection},
    },
    either::Either,
    sbpf_common::{
        inst_param::{Number, Register},
        instruction::Instruction,
        opcode::Opcode,
        syscalls_map::murmur3_32,
    },
    std::collections::HashMap,
};

#[derive(Default, Debug)]
pub struct AST {
    pub nodes: Vec<ASTNode>,
    pub rodata_nodes: Vec<ASTNode>,

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

    /// Resolve numeric label references (like "2f" or "1b")
    fn resolve_numeric_label(
        label_ref: &str,
        current_idx: usize,
        numeric_labels: &[(String, u64, usize)],
    ) -> Option<u64> {
        if let Some(direction) = label_ref.chars().last()
            && (direction == 'f' || direction == 'b')
        {
            let label_num = &label_ref[..label_ref.len() - 1];

            if direction == 'f' {
                // search forward from current position
                for (name, offset, node_idx) in numeric_labels {
                    if name == label_num && *node_idx > current_idx {
                        return Some(*offset);
                    }
                }
            } else {
                // search backward from current position
                for (name, offset, node_idx) in numeric_labels.iter().rev() {
                    if name == label_num && *node_idx < current_idx {
                        return Some(*offset);
                    }
                }
            }
        }
        None
    }

    pub fn build_program(&mut self, arch: SbpfArch) -> Result<ParseResult, Vec<CompileError>> {
        let mut label_offset_map: HashMap<String, u64> = HashMap::new();
        let mut numeric_labels: Vec<(String, u64, usize)> = Vec::new();

        // iterate through text labels and rodata labels and find the pair
        // of each label and offset
        for (idx, node) in self.nodes.iter().enumerate() {
            if let ASTNode::Label { label, offset } = node {
                label_offset_map.insert(label.name.clone(), *offset);
                // Also track numeric labels separately for forward/backward resolution
                numeric_labels.push((label.name.clone(), *offset, idx));
            }
        }

        for node in &self.rodata_nodes {
            if let ASTNode::ROData { rodata, offset } = node {
                label_offset_map.insert(rodata.name.clone(), *offset + self.text_size);
            }
        }

        // 1. resolve labels in the intruction nodes for lddw and jump
        // 2. find relocation information

        let mut relocations = RelDynMap::new();
        let mut dynamic_symbols = DynamicSymbolMap::new();

        // Resolve both static and dynamic syscalls.
        for node in self.nodes.iter_mut() {
            if let ASTNode::Instruction {
                instruction: inst,
                offset,
            } = node
                && inst.is_syscall()
                && let Some(Either::Left(syscall_name)) = &inst.imm
            {
                let syscall_name = syscall_name.clone();
                if arch.is_v3() {
                    // Static syscall: src = 0, imm = hash
                    inst.src = Some(Register { n: 0 });
                    inst.imm = Some(Either::Right(Number::Int(murmur3_32(&syscall_name) as i64)));
                } else {
                    // Dynamic syscall: src = 1, imm = -1
                    inst.src = Some(Register { n: 1 });
                    inst.imm = Some(Either::Right(Number::Int(-1)));

                    // Add relocation for dynamic syscall
                    relocations.add_rel_dyn(
                        *offset,
                        RelocationType::RSbfSyscall,
                        syscall_name.clone(),
                    );
                    dynamic_symbols.add_call_target(syscall_name.clone(), *offset);
                }
            }
        }

        let program_is_static = !self.nodes.iter().any(|node| {
            matches!(node, ASTNode::Instruction { instruction: inst, .. }
                if inst.needs_relocation())
        });

        let mut errors = Vec::new();

        for (idx, node) in self.nodes.iter_mut().enumerate() {
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
                    let target_offset = if let Some(offset) = label_offset_map.get(label) {
                        Some(*offset)
                    } else {
                        // Handle numeric label references
                        Self::resolve_numeric_label(label, idx, &numeric_labels)
                    };

                    if let Some(target_offset) = target_offset {
                        let rel_offset = (target_offset as i64 - *offset as i64) / 8 - 1;
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
                    inst.src = Some(Register { n: 1 });
                    inst.imm = Some(Either::Right(Number::Int(rel_offset)));
                }

                if inst.opcode == Opcode::Lddw
                    && let Some(Either::Left(name)) = &inst.imm
                {
                    let label = name.clone();
                    // Add relocation for lddw (only for v0)
                    if !arch.is_v3() {
                        relocations.add_rel_dyn(
                            *offset,
                            RelocationType::RSbf64Relative,
                            label.clone(),
                        );
                    }

                    if let Some(target_offset) = label_offset_map.get(&label) {
                        let abs_offset = if arch.is_v3() {
                            (*target_offset - self.text_size) as i64
                        } else {
                            let ph_count = if program_is_static { 1 } else { 3 };
                            let ph_offset = 64 + (ph_count as u64 * 56) as i64;
                            *target_offset as i64 + ph_offset
                        };
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

        // Set entry point offset if a GlobalDecl was specified
        let entry_label = self.nodes.iter().find_map(|node| {
            if let ASTNode::GlobalDecl { global_decl } = node {
                Some(global_decl.entry_label.clone())
            } else {
                None
            }
        });
        if let Some(entry_label) = entry_label
            && let Some(offset) = label_offset_map.get(&entry_label)
        {
            dynamic_symbols.add_entry_point(entry_label, *offset);
        }

        if !errors.is_empty() {
            Err(errors)
        } else {
            Ok(ParseResult {
                code_section: CodeSection::new(std::mem::take(&mut self.nodes), self.text_size),
                data_section: DataSection::new(
                    std::mem::take(&mut self.rodata_nodes),
                    self.rodata_size,
                ),
                dynamic_symbols,
                relocation_data: relocations,
                prog_is_static: program_is_static,
                arch,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use {super::*, crate::parser::Token};

    #[test]
    fn test_ast_new() {
        let ast = AST::new();
        assert!(ast.nodes.is_empty());
        assert!(ast.rodata_nodes.is_empty());
        assert_eq!(ast.text_size, 0);
        assert_eq!(ast.rodata_size, 0);
    }

    #[test]
    fn test_ast_set_sizes() {
        let mut ast = AST::new();
        ast.set_text_size(100);
        ast.set_rodata_size(50);
        assert_eq!(ast.text_size, 100);
        assert_eq!(ast.rodata_size, 50);
    }

    #[test]
    fn test_get_instruction_at_offset() {
        let mut ast = AST::new();
        let inst = Instruction {
            opcode: Opcode::Exit,
            dst: None,
            src: None,
            off: None,
            imm: None,
            span: 0..4,
        };
        ast.nodes.push(ASTNode::Instruction {
            instruction: inst,
            offset: 0,
        });

        let found = ast.get_instruction_at_offset(0);
        assert!(found.is_some());
        assert_eq!(found.unwrap().opcode, Opcode::Exit);

        let not_found = ast.get_instruction_at_offset(8);
        assert!(not_found.is_none());
    }

    #[test]
    fn test_get_rodata_at_offset() {
        let mut ast = AST::new();
        let rodata = ROData {
            name: "data".to_string(),
            args: vec![
                Token::Directive("ascii".to_string(), 0..5),
                Token::StringLiteral("test".to_string(), 6..12),
            ],
            span: 0..12,
        };
        ast.rodata_nodes.push(ASTNode::ROData {
            rodata: rodata.clone(),
            offset: 0,
        });

        let found = ast.get_rodata_at_offset(0);
        assert!(found.is_some());
        assert_eq!(found.unwrap().name, "data");
    }

    #[test]
    fn test_resolve_numeric_label_forward() {
        let numeric_labels = vec![("1".to_string(), 16, 2), ("2".to_string(), 32, 4)];

        let result = AST::resolve_numeric_label("1f", 0, &numeric_labels);
        assert_eq!(result, Some(16));

        let result = AST::resolve_numeric_label("2f", 3, &numeric_labels);
        assert_eq!(result, Some(32));
    }

    #[test]
    fn test_resolve_numeric_label_backward() {
        let numeric_labels = vec![("1".to_string(), 16, 2), ("2".to_string(), 32, 4)];

        let result = AST::resolve_numeric_label("1b", 3, &numeric_labels);
        assert_eq!(result, Some(16));

        let result = AST::resolve_numeric_label("2b", 5, &numeric_labels);
        assert_eq!(result, Some(32));
    }

    #[test]
    fn test_build_program_simple() {
        let mut ast = AST::new();
        let inst = Instruction {
            opcode: Opcode::Exit,
            dst: None,
            src: None,
            off: None,
            imm: None,
            span: 0..4,
        };
        ast.nodes.push(ASTNode::Instruction {
            instruction: inst,
            offset: 0,
        });
        ast.set_text_size(8);
        ast.set_rodata_size(0);

        let result = ast.build_program(SbpfArch::V0);
        assert!(result.is_ok());
        let parse_result = result.unwrap();
        assert!(parse_result.prog_is_static);
    }

    #[test]
    fn test_build_program_undefined_label_error() {
        let mut ast = AST::new();

        // Jump to undefined label
        let inst = Instruction {
            opcode: Opcode::Ja,
            dst: None,
            src: None,
            off: Some(Either::Left("undefined_label".to_string())),
            imm: None,
            span: 0..10,
        };
        ast.nodes.push(ASTNode::Instruction {
            instruction: inst,
            offset: 0,
        });
        ast.set_text_size(8);

        let result = ast.build_program(SbpfArch::V0);
        assert!(result.is_err());
    }

    #[test]
    fn test_build_program_static_syscalls_no_relocation() {
        let mut ast = AST::new();

        let syscall_inst = Instruction {
            opcode: Opcode::Call,
            dst: None,
            src: None,
            off: None,
            imm: Some(Either::Left("sol_log_".to_string())),
            span: 0..8,
        };
        ast.nodes.push(ASTNode::Instruction {
            instruction: syscall_inst,
            offset: 0,
        });

        let exit_inst = Instruction {
            opcode: Opcode::Exit,
            dst: None,
            src: None,
            off: None,
            imm: None,
            span: 8..16,
        };
        ast.nodes.push(ASTNode::Instruction {
            instruction: exit_inst,
            offset: 8,
        });

        ast.set_text_size(16);
        ast.set_rodata_size(0);

        let result = ast.build_program(SbpfArch::V3);
        assert!(result.is_ok());
        let parse_result = result.unwrap();

        assert!(parse_result.prog_is_static);
        assert!(parse_result.relocation_data.get_rel_dyns().is_empty());
    }

    #[test]
    fn test_build_program_dynamic_syscalls_with_relocation() {
        let mut ast = AST::new();

        let syscall_inst = Instruction {
            opcode: Opcode::Call,
            dst: None,
            src: None,
            off: None,
            imm: Some(Either::Left("sol_log_".to_string())),
            span: 0..8,
        };
        ast.nodes.push(ASTNode::Instruction {
            instruction: syscall_inst,
            offset: 0,
        });

        let exit_inst = Instruction {
            opcode: Opcode::Exit,
            dst: None,
            src: None,
            off: None,
            imm: None,
            span: 8..16,
        };
        ast.nodes.push(ASTNode::Instruction {
            instruction: exit_inst,
            offset: 8,
        });

        ast.set_text_size(16);
        ast.set_rodata_size(0);

        let result = ast.build_program(SbpfArch::V0);
        assert!(result.is_ok());
        let parse_result = result.unwrap();

        assert!(!parse_result.prog_is_static);
        assert!(!parse_result.relocation_data.get_rel_dyns().is_empty());
    }
}
