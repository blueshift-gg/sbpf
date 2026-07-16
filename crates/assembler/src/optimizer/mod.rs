mod canonicalize;

pub(crate) use canonicalize::{
    canonicalize_control_flow_targets, remove_temp_control_flow_target_labels,
};
use {
    crate::{ast::AST, astnode::ASTNode},
    sbpf_ir::{Cfg, InputNode, control_flow_graph},
    sbpf_analyze::remove_dead_functions,
    std::collections::HashSet,
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

/// Removes functions not reachable from the entry via `call imm`.
pub fn eliminate_unreachable_functions(ast: &mut AST) {
    eliminate_unreachable_functions_with_observer(ast, |_, _| {});
}

/// Removes unreachable functions and exposes the CFG before and after the pass.
/// The observer owns any optional diagnostics or I/O, keeping the pass itself pure.
pub fn eliminate_unreachable_functions_with_observer<F>(ast: &mut AST, mut observe: F)
where
    F: FnMut(CfgDumpStage, &Cfg),
{
    let mut cfg = cfg_for_ast(ast);
    observe(CfgDumpStage::BeforeDfe, &cfg);

    let removed_functions = remove_dead_functions(&mut cfg);

    if !removed_functions.is_empty() {
        let dead_node_ids: HashSet<usize> = removed_functions
            .into_iter()
            .flat_map(|f| f.node_ids)
            .collect();
        strip_dead_nodes(ast, &dead_node_ids);
    }

    assign_offsets(ast);

    let cfg = cfg_for_ast(ast);
    observe(CfgDumpStage::AfterDfe, &cfg);
}

/// Removes AST nodes belonging to dead functions, identified by their index in
/// `ast.nodes`. Non-label/instruction nodes (e.g. `GlobalDecl`) are always kept.
fn strip_dead_nodes(ast: &mut AST, dead_node_ids: &HashSet<usize>) {
    ast.nodes = std::mem::take(&mut ast.nodes)
        .into_iter()
        .enumerate()
        .filter(|(idx, node)| {
            !matches!(node, ASTNode::Label { .. } | ASTNode::Instruction { .. })
                || !dead_node_ids.contains(idx)
        })
        .map(|(_, node)| node)
        .collect();
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

fn cfg_for_ast(ast: &AST) -> Cfg {
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
    control_flow_graph(nodes, &function_entries, entry_label)
}

fn function_entries(ast: &AST) -> HashSet<String> {
    ast.function_entries().clone()
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::astnode::{GlobalDecl, Label},
        either::Either,
        sbpf_common::{inst_param::Register, instruction::Instruction, opcode::Opcode},
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
    fn test_strip_dead_nodes_and_assign_offsets_recomputes_cumulatively() {
        let mut ast = AST::new();
        // node indices: 0=entrypoint, 1=exit, 2=dead_a, 3=exit, 4=live, 5=exit, 6=dead_b, 7=exit, 8=target, 9=exit
        ast.nodes = vec![
            label_node("entrypoint", 0),
            instruction_node(Opcode::Exit, None, 0, None),
            label_node("dead_a", 8),
            instruction_node(Opcode::Exit, None, 8, None),
            label_node("live", 16),
            instruction_node(Opcode::Exit, None, 16, None),
            label_node("dead_b", 24),
            instruction_node(Opcode::Exit, None, 24, None),
            label_node("target", 32),
            instruction_node(Opcode::Exit, None, 32, None),
        ];
        ast.set_text_size(40);

        strip_dead_nodes(&mut ast, &HashSet::from([2usize, 3, 6, 7]));
        assign_offsets(&mut ast);

        assert!(matches!(
            &ast.nodes[2],
            ASTNode::Label { label, offset } if label.name == "live" && *offset == 8
        ));
        assert!(matches!(
            &ast.nodes[4],
            ASTNode::Label { label, offset } if label.name == "target" && *offset == 16
        ));
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
