mod canonicalize;

pub(crate) use canonicalize::{
    canonicalize_control_flow_targets, remove_temp_control_flow_target_labels,
};
use {
    crate::{ast::AST, astnode::ASTNode},
    sbpf_analyze::remove_dead_functions,
    sbpf_common::instruction::Instruction,
    sbpf_ir::{Cfg, CfgRodata, InputNode, control_flow_graph},
    std::collections::{HashMap, HashSet},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CfgDumpStage {
    BeforeDfe,
    AfterDfe,
}

impl CfgDumpStage {
    pub fn file_name(self) -> &'static str {
        match self {
            Self::BeforeDfe => "dfe-before.dot",
            Self::AfterDfe => "dfe-after.dot",
        }
    }
}

/// Runs every CFG optimization on a single CFG built from the AST, then writes
/// the result back once: dead-function elimination, then the ALU32 extension
/// fix-up.
pub fn optimize(ast: &mut AST) {
    optimize_with_observer(ast, |_, _| {});
}

/// Like [`optimize`], and exposes the CFG before and after dead-function
/// elimination. The observer owns any optional diagnostics or I/O, keeping the
/// passes themselves pure.
pub fn optimize_with_observer<F>(ast: &mut AST, mut observe: F)
where
    F: FnMut(CfgDumpStage, &Cfg),
{
    let mut cfg = cfg_for_ast(ast);
    observe(CfgDumpStage::BeforeDfe, &cfg);
    remove_dead_functions(&mut cfg);
    observe(CfgDumpStage::AfterDfe, &cfg);
    sbpf_analyze::fix_alu32_extension(&mut cfg);
    write_cfg_to_ast(ast, &cfg);
}

/// Removes functions not reachable from the entry via `call imm`.
pub fn eliminate_unreachable_functions(ast: &mut AST) {
    let mut cfg = cfg_for_ast(ast);
    remove_dead_functions(&mut cfg);
    write_cfg_to_ast(ast, &cfg);
}

/// Makes eBPF's ALU32 zero-extension hold under SBPF, where `add32`, `sub32` and
/// `mul32` sign-extend. Each such instruction is kept, promoted to its 64-bit form,
/// or zero-extended. Assumes the AST follows eBPF semantics (LLVM output).
pub fn fix_alu32_extension(ast: &mut AST) {
    let mut cfg = cfg_for_ast(ast);
    sbpf_analyze::fix_alu32_extension(&mut cfg);
    write_cfg_to_ast(ast, &cfg);
}

/// Builds the CFG for the AST's text section. Each CFG node records the index of
/// the AST node it came from, which [`write_cfg_to_ast`] uses to map back.
pub(crate) fn cfg_for_ast(ast: &AST) -> Cfg {
    let function_entries = function_entries(ast);
    let entry_label = ast.nodes.iter().find_map(|node| {
        if let ASTNode::GlobalDecl { global_decl } = node {
            Some(global_decl.entry_label.as_str())
        } else {
            None
        }
    });
    let nodes = ast.nodes.iter().map(|node| match node {
        ASTNode::Label { label, .. } => InputNode::Label(label.name.as_str()),
        ASTNode::Instruction { instruction, .. } => InputNode::Instruction(instruction),
        _ => InputNode::Other,
    });
    let mut cfg = control_flow_graph(nodes, &function_entries, entry_label);
    if !ast.rodata_nodes.is_empty() {
        let rodata = ast.rodata_nodes.iter().filter_map(|node| {
            let ASTNode::ROData { rodata, offset } = node else {
                return None;
            };
            Some(CfgRodata::new(&rodata.name, *offset, rodata.get_size()))
        });
        let references = ast
            .rodata_relocations()
            .iter()
            .map(|relocation| (relocation.offset, relocation.target.as_str()));
        cfg.set_rodata(rodata, references);
    }
    cfg
}

/// Writes a CFG built by [`cfg_for_ast`] back into the AST, then recomputes
/// offsets:
/// - labels and instructions no longer in the CFG (e.g. removed functions) are
///   dropped;
/// - each instruction takes its CFG node's current form, followed by any
///   instructions a pass inserted after it;
/// - every other node (e.g. `GlobalDecl`) is kept as is.
pub(crate) fn write_cfg_to_ast(ast: &mut AST, cfg: &Cfg) {
    let live: HashSet<usize> = cfg
        .all_blocks()
        .flat_map(|(_, block)| block.node_ids().iter().copied())
        .collect();

    let mut rewritten: HashMap<usize, Vec<Instruction>> = HashMap::new();
    for (_, block) in cfg.all_blocks() {
        let mut last_source = None;
        for node in block.instructions() {
            let Some(instruction) = node.instruction() else {
                continue;
            };
            match node.source_node_id() {
                Some(id) => {
                    rewritten.insert(id, vec![instruction.clone()]);
                    last_source = Some(id);
                }
                None => {
                    let id = last_source
                        .expect("inserted instructions follow a source instruction in their block");
                    rewritten
                        .get_mut(&id)
                        .expect("source instruction was recorded")
                        .push(instruction.clone());
                }
            }
        }
    }

    ast.nodes = std::mem::take(&mut ast.nodes)
        .into_iter()
        .enumerate()
        .flat_map(|(idx, node)| match node {
            ASTNode::Label { .. } if !live.contains(&idx) => vec![],
            ASTNode::Instruction { .. } => rewritten
                .remove(&idx)
                .unwrap_or_default()
                .into_iter()
                .map(|instruction| ASTNode::Instruction {
                    instruction,
                    offset: 0,
                })
                .collect(),
            node => vec![node],
        })
        .collect();

    assign_offsets(ast);
}

/// Recomputes byte offsets for all labels and instructions in the AST from
/// scratch, in node order. Called after any pass that alters the node list so
/// there is a single authoritative place that assigns offsets.
pub fn assign_offsets(ast: &mut AST) {
    let mut text_offset = 0u64;
    let mut text_size = 0u64;
    for node in &mut ast.nodes {
        match node {
            ASTNode::Label { offset, .. } => *offset = text_offset,
            ASTNode::Instruction {
                instruction,
                offset,
            } => {
                *offset = text_offset;
                let size = instruction.get_size();
                text_offset += size;
                text_size += size;
            }
            _ => {}
        }
    }
    ast.set_text_size(text_size);
}

fn function_entries(ast: &AST) -> HashSet<String> {
    ast.function_entries().clone()
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{
            SbpfArch,
            ast::{OptimizationConfig, build_program},
            astnode::{GlobalDecl, Label, ROData},
            parser::Token,
        },
        either::Either,
        sbpf_common::{
            inst_param::{Number, Register},
            instruction::Instruction,
            opcode::Opcode,
        },
    };

    #[test]
    fn test_optimizer_preserves_unreachable_blocks_in_reachable_function() {
        let mut ast = AST::new();
        ast.add_function_entry("entrypoint".to_string());
        ast.nodes = vec![
            label_node("entrypoint", 0),
            instruction_node(
                Opcode::Ja,
                None,
                0,
                Some(Either::Left("target".to_string())),
            ),
            instruction_node(Opcode::Mov64Imm, Some(0), 8, None),
            label_node("target", 16),
            instruction_node(Opcode::Exit, None, 16, None),
        ];
        ast.set_text_size(24);

        eliminate_unreachable_functions(&mut ast);

        assert_eq!(ast.nodes.len(), 5);
        assert!(matches!(
            &ast.nodes[1],
            ASTNode::Instruction { instruction, offset }
                if instruction.opcode == Opcode::Ja
                    && instruction.off == Some(Either::Left("target".to_string()))
                    && *offset == 0
        ));
        assert!(matches!(
            &ast.nodes[2],
            ASTNode::Instruction { instruction, offset }
                if instruction.opcode == Opcode::Mov64Imm && *offset == 8
        ));
        assert!(matches!(
            &ast.nodes[3],
            ASTNode::Label { label, offset } if label.name == "target" && *offset == 16
        ));
        assert!(matches!(
            &ast.nodes[4],
            ASTNode::Instruction { instruction, offset }
                if instruction.opcode == Opcode::Exit && *offset == 16
        ));
    }

    #[test]
    fn test_optimizer_removes_uncalled_function_only() {
        let mut ast = AST::new();
        ast.add_function_entry("entrypoint".to_string());
        ast.add_function_entry("dead".to_string());
        ast.add_function_entry("callee".to_string());
        ast.nodes = vec![
            label_node("entrypoint", 0),
            call_node("callee", 0),
            instruction_node(Opcode::Exit, None, 8, None),
            label_node("dead", 16),
            instruction_node(Opcode::Exit, None, 16, None),
            label_node("callee", 24),
            instruction_node(Opcode::Exit, None, 24, None),
        ];
        ast.set_text_size(32);

        eliminate_unreachable_functions(&mut ast);

        assert!(
            !ast.nodes
                .iter()
                .any(|node| matches!(node, ASTNode::Label { label, .. } if label.name == "dead"))
        );
        assert!(
            ast.nodes
                .iter()
                .any(|node| matches!(node, ASTNode::Label { label, offset }
                if label.name == "callee" && *offset == 16))
        );
        assert_eq!(
            ast.nodes
                .iter()
                .filter(|node| matches!(node, ASTNode::Instruction { .. }))
                .count(),
            3
        );
    }

    #[test]
    fn test_optimizer_uses_declared_entry_when_it_is_not_first() {
        let mut ast = AST::new();
        ast.add_function_entry("helper".to_string());
        ast.add_function_entry("dead".to_string());
        ast.add_function_entry("entrypoint".to_string());
        ast.nodes = vec![
            ASTNode::GlobalDecl {
                global_decl: GlobalDecl {
                    entry_label: "entrypoint".to_string(),
                    span: 0..0,
                },
            },
            label_node("helper", 0),
            instruction_node(Opcode::Exit, None, 0, None),
            label_node("dead", 8),
            instruction_node(Opcode::Exit, None, 8, None),
            label_node("entrypoint", 16),
            call_node("helper", 16),
            instruction_node(Opcode::Exit, None, 24, None),
        ];
        ast.set_text_size(32);

        let cfg = cfg_for_ast(&ast);
        assert_eq!(cfg.functions()[0].name(), "entrypoint");
        assert_eq!(cfg.functions()[0].entry_block_id(), Some(2)); // entrypoint is block 2 in source order

        eliminate_unreachable_functions(&mut ast);

        assert!(ast.nodes.iter().any(
            |node| matches!(node, ASTNode::Label { label, .. } if label.name == "entrypoint")
        ));
        assert!(
            ast.nodes
                .iter()
                .any(|node| matches!(node, ASTNode::Label { label, .. } if label.name == "helper"))
        );
        assert!(
            !ast.nodes
                .iter()
                .any(|node| matches!(node, ASTNode::Label { label, .. } if label.name == "dead"))
        );
    }

    #[test]
    fn test_optimizer_preserves_function_targeted_by_rodata_relocation() {
        let mut ast = AST::new();
        ast.add_function_entry("entrypoint".to_string());
        ast.add_function_entry("target".to_string());
        ast.add_function_entry("dead".to_string());
        ast.nodes = vec![
            ASTNode::GlobalDecl {
                global_decl: GlobalDecl {
                    entry_label: "entrypoint".to_string(),
                    span: 0..0,
                },
            },
            label_node("entrypoint", 0),
            instruction_node(Opcode::Exit, None, 0, None),
            label_node("target", 8),
            instruction_node(Opcode::Exit, None, 8, None),
            label_node("dead", 16),
            instruction_node(Opcode::Exit, None, 16, None),
        ];
        ast.rodata_nodes.push(ASTNode::ROData {
            rodata: ROData {
                name: "pointer".to_string(),
                args: vec![
                    Token::Directive("quad".to_string(), 0..0),
                    Token::VectorLiteral(vec![Number::Int(0)], 0..0),
                ],
                span: 0..0,
            },
            offset: 0,
        });
        ast.set_text_size(24);
        ast.set_rodata_size(8);
        ast.add_rodata_relocation(0, "target".to_string());

        eliminate_unreachable_functions(&mut ast);

        assert!(ast.nodes.iter().any(
            |node| matches!(node, ASTNode::Label { label, .. } if label.name == "entrypoint")
        ));
        assert!(
            ast.nodes
                .iter()
                .any(|node| matches!(node, ASTNode::Label { label, .. } if label.name == "target"))
        );
        assert!(
            !ast.nodes
                .iter()
                .any(|node| matches!(node, ASTNode::Label { label, .. } if label.name == "dead"))
        );
    }

    #[test]
    fn test_write_back_drops_dead_functions_and_recomputes_offsets_cumulatively() {
        let mut ast = AST::new();
        for name in ["entrypoint", "dead_a", "live", "dead_b", "target"] {
            ast.add_function_entry(name.to_string());
        }
        ast.nodes = vec![
            label_node("entrypoint", 0),
            call_node("live", 0),
            call_node("target", 8),
            instruction_node(Opcode::Exit, None, 16, None),
            label_node("dead_a", 24),
            instruction_node(Opcode::Exit, None, 24, None),
            label_node("live", 32),
            instruction_node(Opcode::Exit, None, 32, None),
            label_node("dead_b", 40),
            instruction_node(Opcode::Exit, None, 40, None),
            label_node("target", 48),
            instruction_node(Opcode::Exit, None, 48, None),
        ];
        ast.set_text_size(56);

        eliminate_unreachable_functions(&mut ast);

        assert_eq!(
            listing(&ast.nodes),
            [
                ("entrypoint:".to_string(), 0),
                ("call".to_string(), 0),
                ("call".to_string(), 8),
                ("exit".to_string(), 16),
                ("live:".to_string(), 24),
                ("exit".to_string(), 24),
                ("target:".to_string(), 32),
                ("exit".to_string(), 32),
            ]
        );
    }

    #[test]
    fn test_assign_offsets_recomputes_from_scratch() {
        let mut ast = AST::new();
        ast.nodes = vec![
            label_node("entrypoint", 999),
            instruction_node(Opcode::Exit, None, 999, None),
            label_node("next", 999),
            instruction_node(Opcode::Exit, None, 999, None),
        ];

        assign_offsets(&mut ast);

        assert!(matches!(
            &ast.nodes[0],
            ASTNode::Label { label, offset } if label.name == "entrypoint" && *offset == 0
        ));
        assert!(matches!(
            &ast.nodes[1],
            ASTNode::Instruction { offset, .. } if *offset == 0
        ));
        assert!(matches!(
            &ast.nodes[2],
            ASTNode::Label { label, offset } if label.name == "next" && *offset == 8
        ));
    }

    fn alu_node(opcode: Opcode, dst: u8, src: Option<u8>, imm: Option<i64>) -> ASTNode {
        ASTNode::Instruction {
            instruction: Instruction {
                opcode,
                dst: Some(Register { n: dst }),
                src: src.map(|n| Register { n }),
                off: None,
                imm: imm.map(|v| Either::Right(Number::Int(v))),
                span: 0..0,
            },
            offset: 0,
        }
    }

    /// `add32 r1, r2` whose full 64-bit result is shifted and returned, then a
    /// later label. The shift reads r1 directly, so the def itself is fixed up.
    fn alu32_program() -> AST {
        let mut ast = AST::new();
        ast.add_function_entry("entrypoint".to_string());
        ast.nodes = vec![
            label_node("entrypoint", 0),
            alu_node(Opcode::Add32Reg, 1, Some(2), None),
            instruction_node(Opcode::Ja, None, 8, Some(Either::Left("done".to_string()))),
            label_node("done", 16),
            alu_node(Opcode::Lsh64Imm, 1, None, Some(1)),
            alu_node(Opcode::Mov64Reg, 0, Some(1), None),
            instruction_node(Opcode::Exit, None, 32, None),
        ];
        ast.set_text_size(40);
        ast
    }

    fn listing(nodes: &[ASTNode]) -> Vec<(String, u64)> {
        nodes
            .iter()
            .filter_map(|node| match node {
                ASTNode::Label { label, offset } => Some((format!("{}:", label.name), *offset)),
                ASTNode::Instruction {
                    instruction,
                    offset,
                } => Some((instruction.opcode.to_string(), *offset)),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn test_fix_alu32_extension_inserts_fixup_and_reassigns_offsets() {
        let mut ast = alu32_program();

        fix_alu32_extension(&mut ast);

        assert_eq!(
            listing(&ast.nodes),
            [
                ("entrypoint:".to_string(), 0),
                ("add32".to_string(), 0),
                ("mov32".to_string(), 8),
                ("ja".to_string(), 16),
                ("done:".to_string(), 24),
                ("lsh64".to_string(), 24),
                ("mov64".to_string(), 32),
                ("exit".to_string(), 40),
            ]
        );
        let ASTNode::Instruction { instruction, .. } = &ast.nodes[2] else {
            panic!("expected the inserted instruction");
        };
        assert_eq!(instruction.opcode, Opcode::Mov32Reg);
        assert_eq!(instruction.dst, Some(Register { n: 1 }));
        assert_eq!(instruction.src, Some(Register { n: 1 }));
    }

    #[test]
    fn test_fix_alu32_extension_narrows_copy_in_place() {
        let mut ast = AST::new();
        ast.add_function_entry("entrypoint".to_string());
        ast.nodes = vec![
            label_node("entrypoint", 0),
            alu_node(Opcode::Add32Reg, 1, Some(2), None),
            alu_node(Opcode::Mov64Reg, 0, Some(1), None),
            instruction_node(Opcode::Exit, None, 16, None),
        ];
        ast.set_text_size(24);

        fix_alu32_extension(&mut ast);

        assert_eq!(
            listing(&ast.nodes),
            [
                ("entrypoint:".to_string(), 0),
                ("add32".to_string(), 0),
                ("mov32".to_string(), 8),
                ("exit".to_string(), 16),
            ]
        );
        let ASTNode::Instruction { instruction, .. } = &ast.nodes[2] else {
            panic!("expected the narrowed copy");
        };
        assert_eq!(instruction.opcode, Opcode::Mov32Reg);
        assert_eq!(instruction.dst, Some(Register { n: 0 }));
        assert_eq!(instruction.src, Some(Register { n: 1 }));
    }

    #[test]
    fn test_fix_alu32_extension_promotes_in_place() {
        let mut ast = AST::new();
        ast.add_function_entry("entrypoint".to_string());
        ast.nodes = vec![
            label_node("entrypoint", 0),
            alu_node(Opcode::Lddw, 1, None, Some(0x9000_0000)),
            alu_node(Opcode::Sub32Imm, 1, None, Some(1)),
            alu_node(Opcode::Mov64Reg, 0, Some(1), None),
            instruction_node(Opcode::Exit, None, 32, None),
        ];
        ast.set_text_size(40);

        fix_alu32_extension(&mut ast);

        assert_eq!(
            listing(&ast.nodes),
            [
                ("entrypoint:".to_string(), 0),
                ("lddw".to_string(), 0),
                ("sub64".to_string(), 16),
                ("mov64".to_string(), 24),
                ("exit".to_string(), 32),
            ]
        );
    }

    #[test]
    fn test_fix_alu32_extension_leaves_unaffected_program_untouched() {
        let mut ast = AST::new();
        ast.add_function_entry("entrypoint".to_string());
        ast.nodes = vec![
            label_node("entrypoint", 0),
            alu_node(Opcode::Add32Reg, 1, Some(2), None),
            alu_node(Opcode::Mov32Reg, 0, Some(1), None),
            instruction_node(Opcode::Exit, None, 16, None),
        ];
        assign_offsets(&mut ast);
        let before = listing(&ast.nodes);

        fix_alu32_extension(&mut ast);

        assert_eq!(listing(&ast.nodes), before);
    }

    #[test]
    fn test_build_program_runs_fixup_only_when_optimizing() {
        for arch in [SbpfArch::V0, SbpfArch::V3] {
            let disabled =
                build_program(alu32_program(), arch, OptimizationConfig::disabled()).unwrap();
            assert!(
                !listing(disabled.code_section.get_nodes())
                    .iter()
                    .any(|(op, _)| op == "mov32"),
                "{arch:?}: disabled must not rewrite"
            );

            let enabled =
                build_program(alu32_program(), arch, OptimizationConfig::enabled()).unwrap();
            let ops: Vec<String> = listing(enabled.code_section.get_nodes())
                .into_iter()
                .map(|(op, _)| op)
                .collect();
            assert_eq!(
                ops,
                [
                    "entrypoint:",
                    "add32",
                    "mov32",
                    "ja",
                    "done:",
                    "lsh64",
                    "mov64",
                    "exit"
                ],
                "{arch:?}"
            );
        }
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
        dst: Option<u8>,
        offset: u64,
        off: Option<Either<String, i16>>,
    ) -> ASTNode {
        ASTNode::Instruction {
            instruction: Instruction {
                opcode,
                dst: dst.map(|n| Register { n }),
                src: None,
                off,
                imm: None,
                span: 0..0,
            },
            offset,
        }
    }

    fn call_node(target: &str, offset: u64) -> ASTNode {
        ASTNode::Instruction {
            instruction: Instruction {
                opcode: Opcode::Call,
                dst: None,
                src: None,
                off: None,
                imm: Some(Either::Left(target.to_string())),
                span: 0..0,
            },
            offset,
        }
    }
}
