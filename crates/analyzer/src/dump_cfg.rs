use {
    crate::critical_path,
    sbpf_common::instruction::AsmFormat,
    sbpf_ir::{BlockId, Cfg, graph_engine::DfsEngine},
    std::{collections::{HashMap, HashSet}, fmt::Write},
};

pub fn dump_cfg(cfg: &Cfg) -> String {
    dump_cfg_impl(cfg, &HashSet::new(), &HashSet::new(), &HashMap::new())
}

/// DOT dump highlighting critical-path blocks and edges in red.
/// Compact block labels; functions with loops stay unhighlighted (DP requires a DAG).
pub fn dump_cfg_with_critical_path(cfg: &Cfg) -> String {
    let cp_results = critical_path(cfg);

    let critical_blocks: HashSet<BlockId> = cp_results
        .iter()
        .filter_map(|r| r.as_ref().ok())
        .flat_map(|r| r.path.iter().copied())
        .collect();

    let critical_edges: HashSet<(BlockId, BlockId)> = cp_results
        .iter()
        .filter_map(|r| r.as_ref().ok())
        .flat_map(|r| r.path.windows(2).map(|w| (w[0], w[1])))
        .collect();

    // Merge block_cu maps from all functions so every block has a CU count.
    let block_cu: HashMap<BlockId, u64> = cp_results
        .iter()
        .filter_map(|r| r.as_ref().ok())
        .flat_map(|r| r.block_cu.iter().map(|(&id, &cu)| (id, cu)))
        .collect();

    dump_cfg_impl(cfg, &critical_blocks, &critical_edges, &block_cu)
}

fn dump_cfg_impl(
    cfg: &Cfg,
    critical_blocks: &HashSet<BlockId>,
    critical_edges: &HashSet<(BlockId, BlockId)>,
    block_cu: &HashMap<BlockId, u64>,
) -> String {
    let compact = !block_cu.is_empty();

    let mut output = String::from("digraph cfg {\n");
    if compact {
        // Keep nodes small and lay them out top-to-bottom.
        output.push_str("  graph [rankdir=TB];\n");
        output.push_str("  node [shape=box fontsize=10];\n");
    } else {
        output.push_str("  node [shape=box];\n");
    }

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
            let on_path = critical_blocks.contains(&block_id);
            let attrs = if on_path {
                r##" fillcolor="#e74c3c" style=filled fontcolor=white"##
            } else {
                ""
            };
            let label = if compact {
                compact_block_label(block_id, block, block_cu.get(&block_id).copied())
            } else {
                let inst_base = cfg.block_inst_offset(block_id);
                full_block_label(block_id, inst_base, block)
            };
            writeln!(output, "    block_{block_id} [label=\"{label}\"{attrs}];")
                .expect("writing to a String cannot fail");
        }

        output.push_str("  }\n");
    }

    let entry_blocks = cfg
        .functions()
        .iter()
        .filter_map(|f| f.block_ids().first().copied());
    DfsEngine::new(cfg).visit_many(entry_blocks, &mut |block_id| {
        for &successor in cfg.successors(block_id) {
            let edge_attrs = if critical_edges.contains(&(block_id, successor)) {
                r##" [color="#e74c3c" penwidth=2]"##
            } else {
                ""
            };
            writeln!(output, "  block_{block_id} -> block_{successor}{edge_attrs};")
                .expect("writing to a String cannot fail");
        }
    });

    output.push_str("}\n");
    output
}

/// Compact label: block ID + first label name + CU count. No instruction text.
fn compact_block_label(block_id: usize, block: &sbpf_ir::Block, cu: Option<u64>) -> String {
    let mut label = format!("block {block_id}");
    if let Some((name, _)) = block.labels().first() {
        write!(label, "\\n{}", escape_dot_label(name)).expect("writing to a String cannot fail");
    }
    if let Some(cu) = cu {
        write!(label, "\\n{cu} CU").expect("writing to a String cannot fail");
    }
    label
}

/// Full label: block ID + all label names + every instruction.
fn full_block_label(block_id: usize, inst_base: usize, block: &sbpf_ir::Block) -> String {
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
