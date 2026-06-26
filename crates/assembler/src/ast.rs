use {
    crate::{
        CompileError, SbpfArch,
        astnode::{ASTNode, ROData},
        dynsym::{DynamicSymbolMap, RelDynMap, RelocationType},
        optimizer,
        parser::ProgramLayout,
        section::{CodeSection, DataSection},
    },
    either::Either,
    sbpf_common::{
        inst_param::{Number, Register},
        instruction::Instruction,
        opcode::Opcode,
    },
    std::{
        collections::{HashMap, HashSet},
        path::PathBuf,
    },
    syscall_map::murmur3_32,
};

type LabelOffsetMap = HashMap<String, u64>;
type NumericLabel = (String, u64, usize);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptimizationConfig {
    Disabled,
    Enabled { cfg_dump_dir: Option<PathBuf> },
}

impl Default for OptimizationConfig {
    fn default() -> Self {
        Self::disabled()
    }
}

impl OptimizationConfig {
    pub fn disabled() -> Self {
        Self::Disabled
    }

    pub fn enabled() -> Self {
        Self::Enabled { cfg_dump_dir: None }
    }

    pub fn with_cfg_dump_dir(self, path: impl Into<PathBuf>) -> Self {
        match self {
            Self::Enabled { .. } => Self::Enabled {
                cfg_dump_dir: Some(path.into()),
            },
            Self::Disabled => Self::Disabled,
        }
    }
}

#[derive(Default, Debug)]
pub struct AST {
    pub nodes: Vec<ASTNode>,
    pub rodata_nodes: Vec<ASTNode>,

    function_entries: HashSet<String>,
    text_size: u64,
    rodata_size: u64,
}

impl AST {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_function_entry(&mut self, name: String) {
        self.function_entries.insert(name);
    }

    pub(crate) fn function_entries(&self) -> &HashSet<String> {
        &self.function_entries
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
    pub(crate) fn resolve_numeric_label(
        label_ref: &str,
        current_idx: usize,
        numeric_labels: &[NumericLabel],
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
}

pub fn build_program(
    mut ast: AST,
    arch: SbpfArch,
    optimization: OptimizationConfig,
) -> Result<ProgramLayout, Vec<CompileError>> {
    let optimization = run_optimizations(&mut ast, &optimization);
    let mut errors = optimization.errors;

    let (label_offset_map, numeric_labels) = label_offset_map(&ast);
    let program_is_static = arch.is_v3()
        || !ast.nodes.iter().any(|node| {
            matches!(node, ASTNode::Instruction { instruction: inst, .. }
                if inst.is_syscall()
                || (inst.opcode == Opcode::Lddw && matches!(&inst.imm, Some(Either::Left(_)))))
        });

    let label_resolution = resolve_label_references(
        &mut ast,
        arch,
        program_is_static,
        &label_offset_map,
        &numeric_labels,
    );
    errors.extend(label_resolution.errors);

    optimizer::remove_temp_control_flow_target_labels(
        &mut ast.nodes,
        &optimization.labels_to_remove,
    );

    if !errors.is_empty() {
        Err(errors)
    } else {
        Ok(ProgramLayout {
            code_section: CodeSection::new(std::mem::take(&mut ast.nodes), ast.text_size),
            data_section: DataSection::new(std::mem::take(&mut ast.rodata_nodes), ast.rodata_size),
            dynamic_symbols: label_resolution.dynamic_symbols,
            relocation_data: label_resolution.relocations,
            prog_is_static: program_is_static,
            arch,
            debug_sections: Vec::default(),
        })
    }
}

#[derive(Default)]
struct OptimizationOutcome {
    labels_to_remove: HashSet<String>,
    errors: Vec<CompileError>,
}

fn run_optimizations(ast: &mut AST, config: &OptimizationConfig) -> OptimizationOutcome {
    let OptimizationConfig::Enabled { cfg_dump_dir } = config else {
        return OptimizationOutcome::default();
    };

    // Normalize numeric and relative control-flow targets to labels before running
    // CFG-based optimizations. Synthetic labels are tracked so they can be removed
    // after optimization, and the pass is skipped if normalization fails.
    let canonicalized_targets = optimizer::canonicalize_control_flow_targets(&mut ast.nodes);
    let labels_to_remove = canonicalized_targets.labels_to_remove;
    let mut errors = Vec::new();

    if canonicalized_targets.errors.is_empty() {
        if let Some(dump_dir) = cfg_dump_dir.as_deref() {
            let mut dump_errors = Vec::new();
            if let Err(error) = std::fs::create_dir_all(dump_dir) {
                dump_errors.push((dump_dir.to_path_buf(), error));
                optimizer::eliminate_unreachable_functions(ast);
            } else {
                optimizer::eliminate_unreachable_functions_with_observer(ast, |stage, cfg| {
                    let path = dump_dir.join(stage.file_name());
                    if let Err(error) = std::fs::write(&path, sbpf_transform::dump_cfg(cfg)) {
                        dump_errors.push((path, error));
                    }
                });
            }
            for (path, error) in dump_errors {
                errors.push(CompileError::BytecodeError {
                    error: format!("failed to write CFG dump '{}': {error}", path.display()),
                    span: 0..0,
                    custom_label: None,
                });
            }
        } else {
            optimizer::eliminate_unreachable_functions(ast);
        }
    }

    OptimizationOutcome {
        labels_to_remove,
        errors,
    }
}

#[derive(Default)]
struct LabelResolution {
    dynamic_symbols: DynamicSymbolMap,
    relocations: RelDynMap,
    errors: Vec<CompileError>,
}

fn resolve_label_references(
    ast: &mut AST,
    arch: SbpfArch,
    program_is_static: bool,
    label_offset_map: &LabelOffsetMap,
    numeric_labels: &[NumericLabel],
) -> LabelResolution {
    let mut relocations = RelDynMap::new();
    let mut dynamic_symbols = DynamicSymbolMap::new();
    let mut errors = Vec::new();

    // Resolve both static and dynamic syscalls.
    for node in ast.nodes.iter_mut() {
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
                relocations.add_rel_dyn(*offset, RelocationType::RSbfSyscall, syscall_name.clone());
                dynamic_symbols.add_call_target(syscall_name.clone(), *offset);
            }
        }
    }

    for (idx, node) in ast.nodes.iter_mut().enumerate() {
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
                    AST::resolve_numeric_label(label, idx, numeric_labels)
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
                    relocations.add_rel_dyn(*offset, RelocationType::RSbf64Relative, label.clone());
                }

                if let Some(target_offset) = label_offset_map.get(&label) {
                    let abs_offset = if arch.is_v3() {
                        (*target_offset - ast.text_size) as i64
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
    let entry_label = ast.nodes.iter().find_map(|node| {
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

    LabelResolution {
        dynamic_symbols,
        relocations,
        errors,
    }
}

fn label_offset_map(ast: &AST) -> (LabelOffsetMap, Vec<NumericLabel>) {
    let mut label_offset_map = HashMap::new();
    let mut numeric_labels = Vec::new();

    for (idx, node) in ast.nodes.iter().enumerate() {
        if let ASTNode::Label { label, offset } = node {
            label_offset_map.insert(label.name.clone(), *offset);
            numeric_labels.push((label.name.clone(), *offset, idx));
        }
    }

    for node in &ast.rodata_nodes {
        if let ASTNode::ROData { rodata, offset } = node {
            label_offset_map.insert(rodata.name.clone(), *offset + ast.text_size);
        }
    }

    (label_offset_map, numeric_labels)
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{astnode::Label, parser::Token},
    };

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
        ast.nodes
            .push(instruction_node(Opcode::Exit, 0, None, None));

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
    fn test_canonicalize_numeric_jump_target_to_label() {
        let mut nodes = vec![
            instruction_node(Opcode::Ja, 0, Some(Either::Left("1f".to_string())), None),
            label_node("1", 8),
            instruction_node(Opcode::Exit, 8, None, None),
        ];

        let canonicalized = optimizer::canonicalize_control_flow_targets(&mut nodes);

        assert!(canonicalized.errors.is_empty());
        assert!(canonicalized.labels_to_remove.is_empty());
        let ASTNode::Instruction { instruction, .. } = &nodes[0] else {
            panic!("expected instruction");
        };
        assert_eq!(instruction.off, Some(Either::Left("1".to_string())));
    }

    #[test]
    fn test_canonicalize_relative_jump_target_to_synthetic_label() {
        let mut nodes = vec![
            instruction_node(Opcode::Ja, 0, Some(Either::Right(1)), None),
            instruction_node(Opcode::Exit, 8, None, None),
            instruction_node(Opcode::Exit, 16, None, None),
        ];

        let canonicalized = optimizer::canonicalize_control_flow_targets(&mut nodes);

        assert!(canonicalized.errors.is_empty());
        assert_eq!(canonicalized.labels_to_remove.len(), 1);

        let ASTNode::Instruction { instruction, .. } = &nodes[0] else {
            panic!("expected instruction");
        };
        let Some(Either::Left(label)) = &instruction.off else {
            panic!("expected canonical label target");
        };
        assert!(label.starts_with("temp_"));
        assert!(canonicalized.labels_to_remove.contains(label));
        assert!(
            matches!(&nodes[2], ASTNode::Label { label: target_label, offset }
                if target_label.name == *label && *offset == 16)
        );
    }

    #[test]
    fn test_canonicalize_rejects_invalid_relative_jump_target() {
        let mut nodes = vec![
            instruction_node(Opcode::Ja, 0, Some(Either::Right(1)), None),
            instruction_node(Opcode::Exit, 8, None, None),
        ];

        let canonicalized = optimizer::canonicalize_control_flow_targets(&mut nodes);

        assert_eq!(canonicalized.errors.len(), 1);
        assert!(canonicalized.labels_to_remove.is_empty());
        assert_eq!(nodes.len(), 2);
        let ASTNode::Instruction { instruction, .. } = &nodes[0] else {
            panic!("expected instruction");
        };
        assert_eq!(instruction.off, Some(Either::Right(1)));
    }

    #[test]
    fn test_dce_recomputes_relative_call_target_after_removing_code() {
        let mut ast = AST::new();
        ast.add_function_entry("entrypoint".to_string());
        ast.add_function_entry("dead".to_string());
        ast.add_function_entry("target".to_string());
        ast.nodes = vec![
            label_node("entrypoint", 0),
            internal_call_node(0, 2),
            instruction_node(Opcode::Exit, 8, None, None),
            label_node("dead", 16),
            instruction_node(Opcode::Exit, 16, None, None),
            label_node("target", 24),
            instruction_node(Opcode::Exit, 24, None, None),
        ];
        ast.set_text_size(32);

        let program_layout =
            build_program(ast, SbpfArch::V0, OptimizationConfig::enabled()).unwrap();
        let nodes = program_layout.code_section.get_nodes();

        assert_eq!(
            nodes
                .iter()
                .filter(|node| matches!(node, ASTNode::Instruction { .. }))
                .count(),
            3
        );
        assert!(matches!(
            &nodes[1],
            ASTNode::Instruction { instruction, offset }
                if instruction.opcode == Opcode::Call
                    && instruction.src == Some(Register { n: 1 })
                    && instruction.imm == Some(Either::Right(Number::Int(1)))
                    && *offset == 0
        ));
        assert!(matches!(
            &nodes[3],
            ASTNode::Label { label, offset } if label.name == "target" && *offset == 16
        ));
    }

    #[test]
    fn test_optimize_ast_removes_temp_jump_target_labels() {
        let mut ast = AST::new();
        ast.nodes = vec![
            instruction_node(Opcode::Ja, 0, Some(Either::Right(1)), None),
            instruction_node(Opcode::Exit, 8, None, None),
            instruction_node(Opcode::Exit, 16, None, None),
        ];
        ast.set_text_size(24);
        ast.set_rodata_size(0);

        let result = build_program(ast, SbpfArch::V0, OptimizationConfig::enabled());

        assert!(result.is_ok());
        let program_layout = result.unwrap();
        assert!(!program_layout.code_section.get_nodes().iter().any(|node| {
            matches!(node, ASTNode::Label { label, .. }
                if label
                    .name
                    .starts_with("temp_"))
        }));
        let ASTNode::Instruction { instruction, .. } = &program_layout.code_section.get_nodes()[0]
        else {
            panic!("expected instruction");
        };
        assert_eq!(instruction.off, Some(Either::Right(1)));
    }

    #[test]
    fn test_build_program_simple() {
        let mut ast = AST::new();
        ast.nodes
            .push(instruction_node(Opcode::Exit, 0, None, None));
        ast.set_text_size(8);
        ast.set_rodata_size(0);

        let result = build_program(ast, SbpfArch::V0, OptimizationConfig::default());
        assert!(result.is_ok());
        let parse_result = result.unwrap();
        assert!(parse_result.prog_is_static);
    }

    #[test]
    fn test_build_program_undefined_label_error() {
        let mut ast = AST::new();

        // Jump to undefined label
        ast.nodes.push(instruction_node(
            Opcode::Ja,
            0,
            Some(Either::Left("undefined_label".to_string())),
            None,
        ));
        ast.set_text_size(8);

        let result = build_program(ast, SbpfArch::V0, OptimizationConfig::default());
        assert!(result.is_err());
    }

    #[test]
    fn test_build_program_static_syscalls_no_relocation() {
        let mut ast = AST::new();

        ast.nodes.push(instruction_node(
            Opcode::Call,
            0,
            None,
            Some(Either::Left("sol_log_".to_string())),
        ));
        ast.nodes
            .push(instruction_node(Opcode::Exit, 8, None, None));

        ast.set_text_size(16);
        ast.set_rodata_size(0);

        let result = build_program(ast, SbpfArch::V3, OptimizationConfig::default());
        assert!(result.is_ok());
        let parse_result = result.unwrap();

        assert!(parse_result.prog_is_static);
        assert!(parse_result.relocation_data.get_rel_dyns().is_empty());
    }

    #[test]
    fn test_build_program_dynamic_syscalls_with_relocation() {
        let mut ast = AST::new();

        ast.nodes.push(instruction_node(
            Opcode::Call,
            0,
            None,
            Some(Either::Left("sol_log_".to_string())),
        ));
        ast.nodes
            .push(instruction_node(Opcode::Exit, 8, None, None));

        ast.set_text_size(16);
        ast.set_rodata_size(0);

        let result = build_program(ast, SbpfArch::V0, OptimizationConfig::default());
        assert!(result.is_ok());
        let parse_result = result.unwrap();

        assert!(!parse_result.prog_is_static);
        assert!(!parse_result.relocation_data.get_rel_dyns().is_empty());
    }

    fn label_node(name: &str, offset: u64) -> ASTNode {
        ASTNode::Label {
            label: Label {
                name: name.to_string(),
                span: 0..0,
            },
            offset,
        }
    }

    fn instruction_node(
        opcode: Opcode,
        offset: u64,
        off: Option<Either<String, i16>>,
        imm: Option<Either<String, Number>>,
    ) -> ASTNode {
        ASTNode::Instruction {
            instruction: Instruction {
                opcode,
                dst: None,
                src: None,
                off,
                imm,
                span: 0..0,
            },
            offset,
        }
    }

    fn internal_call_node(offset: u64, relative_offset: i64) -> ASTNode {
        ASTNode::Instruction {
            instruction: Instruction {
                opcode: Opcode::Call,
                dst: None,
                src: Some(Register { n: 1 }),
                off: None,
                imm: Some(Either::Right(Number::Int(relative_offset))),
                span: 0..0,
            },
            offset,
        }
    }
}
