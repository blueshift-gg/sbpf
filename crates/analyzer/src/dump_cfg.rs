use {
    sbpf_common::instruction::AsmFormat,
    sbpf_ir::{Cfg, graph_engine::DfsEngine},
    std::fmt::Write,
};

pub fn dump_cfg(cfg: &Cfg) -> String {
    let mut output = String::from("digraph cfg {\n  node [shape=box];\n");

    // Emit one DOT subgraph cluster per function, with each block declared inside
    // so Graphviz renders blocks visually grouped by their containing function.
    for (function_id, function) in cfg.functions().iter().enumerate() {
        writeln!(output, "  subgraph cluster_function_{function_id} {{")
            .expect("writing to a String cannot fail");
        writeln!(
            output,
            "    label=\"{}\";",
            escape_dot_label(function.name())
        )
        .expect("writing to a String cannot fail");

        for (&block_id, block) in function.block_ids().iter().zip(function.blocks().iter()) {
            let inst_base = cfg.block_inst_offset(block_id);
            writeln!(
                output,
                "    block_{block_id} [label=\"{}\"];",
                block_label(block_id, inst_base, block)
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
