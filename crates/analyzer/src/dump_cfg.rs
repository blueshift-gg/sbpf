use {
    sbpf_common::instruction::AsmFormat,
    sbpf_ir::{BlockId, Cfg, graph_engine::DfsEngine},
    std::fmt::Write,
};

/// into a DOT graph without modifying [`dump_cfg`] itself.
pub trait CfgDumpOverlay {
    fn block_extra_label(&self, _block_id: BlockId) -> String {
        String::new()
    }

    fn block_attrs(&self, _block_id: BlockId) -> Vec<(&'static str, String)> {
        vec![]
    }

    fn function_extra_label(&self, _function_id: usize) -> String {
        String::new()
    }
}

struct NoOverlay;
impl CfgDumpOverlay for NoOverlay {}

pub fn dump_cfg(cfg: &Cfg) -> String {
    dump_cfg_with(cfg, &NoOverlay)
}

/// Like [`dump_cfg`] but accepts an [`CfgDumpOverlay`] to inject custom labels and
pub fn dump_cfg_with<O: CfgDumpOverlay>(cfg: &Cfg, overlay: &O) -> String {
    let mut output = String::from("digraph cfg {\n  node [shape=box];\n");

    // Emit one DOT subgraph cluster per function, with each block declared inside
    // so Graphviz renders blocks visually grouped by their containing function.
    for (function_id, function) in cfg.functions().iter().enumerate() {
        writeln!(output, "  subgraph cluster_function_{function_id} {{")
            .expect("writing to a String cannot fail");

        let func_extra = overlay.function_extra_label(function_id);
        if func_extra.is_empty() {
            writeln!(
                output,
                "    label=\"{}\";",
                escape_dot_label(function.name())
            )
            .expect("writing to a String cannot fail");
        } else {
            writeln!(
                output,
                "    label=\"{}\\n{}\";",
                escape_dot_label(function.name()),
                escape_dot_label(&func_extra)
            )
            .expect("writing to a String cannot fail");
        }

        for (&block_id, block) in function.block_ids().iter().zip(function.blocks().iter()) {
            let inst_base = cfg.block_inst_offset(block_id);
            let mut label = block_label(block_id, inst_base, block);
            let block_extra = overlay.block_extra_label(block_id);
            if !block_extra.is_empty() {
                write!(label, "{}\\l", escape_dot_label(&block_extra))
                    .expect("writing to a String cannot fail");
            }

            let attrs = overlay.block_attrs(block_id);
            let attrs_str: String = attrs
                .iter()
                .map(|(k, v)| format!(", {k}=\"{v}\""))
                .collect();

            writeln!(
                output,
                "    block_{block_id} [label=\"{label}\"{attrs_str}];",
            )
            .expect("writing to a String cannot fail");
        }

        output.push_str("  }\n");
    }

    // DFS from every function entry to emit control-flow edges between blocks.
    let entry_blocks = cfg
        .functions()
        .iter()
        .filter_map(|f| f.block_ids().first().copied());
    DfsEngine::new(cfg).visit_many(entry_blocks, &mut |block_id| {
        for successor in cfg.successors(block_id) {
            writeln!(output, "  block_{block_id} -> block_{successor};")
                .expect("writing to a String cannot fail");
        }
    });

    output.push_str("}\n");
    output
}

fn block_label(block_id: usize, inst_base: usize, block: &sbpf_ir::Block) -> String {
    let mut label = format!("block {block_id}\\l");

    if !block.labels().is_empty() {
        let labels = block
            .labels()
            .iter()
            .map(|(name, _)| escape_dot_label(name))
            .collect::<Vec<_>>()
            .join(", ");
        write!(label, "labels: {labels}\\l").expect("writing to a String cannot fail");
    }

    for (local_idx, node) in block.instructions().iter().enumerate() {
        let inst_id = inst_base + local_idx;
        let asm = node
            .instruction()
            .and_then(|inst| inst.to_asm(AsmFormat::Default).ok())
            .unwrap_or_else(|| node.opcode.to_string());
        write!(label, "{inst_id}: {asm}\\l").expect("writing to a String cannot fail");
    }

    label
}

fn escape_dot_label(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        either::Either,
        sbpf_common::{instruction::Instruction, opcode::Opcode},
        sbpf_ir::{InputNode, control_flow_graph},
        std::collections::HashSet,
    };

    #[test]
    fn test_dump_cfg_writes_function_clusters_and_edges() {
        let jump = instruction(Opcode::Ja, Some(Either::Left("target".to_string())));
        let dead_exit = instruction(Opcode::Exit, None);
        let target_exit = instruction(Opcode::Exit, None);
        let nodes = [
            InputNode::Label("entrypoint"),
            InputNode::Instruction(&jump),
            InputNode::Instruction(&dead_exit),
            InputNode::Label("target"),
            InputNode::Instruction(&target_exit),
        ];
        let function_entries = HashSet::from(["entrypoint".to_string(), "target".to_string()]);
        let cfg = control_flow_graph(nodes, &function_entries, None);

        let dot = dump_cfg(&cfg);

        assert!(dot.starts_with("digraph cfg {\n"));
        assert!(dot.contains("subgraph cluster_function_0"));
        assert!(dot.contains("label=\"entrypoint\";"));
        assert!(dot.contains("subgraph cluster_function_1"));
        assert!(dot.contains("label=\"target\";"));
        assert!(dot.contains("0: ja target\\l"));
        assert!(dot.contains("block_0 -> block_2;"));
    }

    #[test]
    fn test_dump_cfg_with_overlay_applies_extra_label_and_attrs() {
        let exit = instruction(Opcode::Exit, None);
        let nodes = [InputNode::Label("entry"), InputNode::Instruction(&exit)];
        let cfg = control_flow_graph(nodes, &HashSet::from(["entry".to_string()]), None);

        struct HighlightAll;
        impl CfgDumpOverlay for HighlightAll {
            fn block_extra_label(&self, _block_id: BlockId) -> String {
                "CU: 99".to_string()
            }
            fn block_attrs(&self, _block_id: BlockId) -> Vec<(&'static str, String)> {
                vec![
                    ("style", "filled".to_string()),
                    ("fillcolor", "#e74c3c".to_string()),
                ]
            }
            fn function_extra_label(&self, _function_id: usize) -> String {
                "total CU: 99".to_string()
            }
        }

        let dot = dump_cfg_with(&cfg, &HighlightAll);

        assert!(dot.contains("CU: 99\\l"), "block extra label present");
        assert!(dot.contains(r#"style="filled""#), "style attr present");
        assert!(
            dot.contains(r##"fillcolor="#e74c3c""##),
            "fillcolor attr present"
        );
        assert!(dot.contains("total CU: 99"), "function extra label present");
    }

    #[test]
    fn test_dump_cfg_with_no_overlay_matches_dump_cfg() {
        let jump = instruction(Opcode::Ja, Some(Either::Left("target".to_string())));
        let dead_exit = instruction(Opcode::Exit, None);
        let target_exit = instruction(Opcode::Exit, None);
        let nodes = [
            InputNode::Label("entry"),
            InputNode::Instruction(&jump),
            InputNode::Instruction(&dead_exit),
            InputNode::Label("target"),
            InputNode::Instruction(&target_exit),
        ];
        let function_entries = HashSet::from(["entry".to_string(), "target".to_string()]);
        let cfg = control_flow_graph(nodes, &function_entries, None);

        assert_eq!(dump_cfg(&cfg), dump_cfg_with(&cfg, &NoOverlay));
    }

    fn instruction(opcode: Opcode, off: Option<Either<String, i16>>) -> Instruction {
        Instruction {
            opcode,
            dst: None,
            src: None,
            off,
            imm: None,
            span: 0..0,
        }
    }
}
