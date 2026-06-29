use {
    anyhow::{Error, Result},
    clap::Args,
    either::Either,
    sbpf_common::{inst_param::Number, instruction::Instruction, opcode::Opcode},
    sbpf_disassembler::program::Program,
    sbpf_ir::{InputNode, control_flow_graph},
    sbpf_transform::{dump_cfg_with_critical_path, dump_critical_path},
    std::{
        collections::{HashMap, HashSet},
        fs::File,
        io::Read,
    },
};

#[derive(Args)]
pub struct AnalyzeArgs {
    #[arg(help = "Path to the ELF file (.so) to analyze")]
    pub filename: String,
    #[arg(
        long,
        help = "Output a DOT graph with critical-path blocks highlighted in red"
    )]
    pub dot: bool,
}

pub fn analyze(args: AnalyzeArgs) -> Result<(), Error> {
    let mut file = File::open(&args.filename)?;
    let mut bytes = vec![];
    file.read_to_end(&mut bytes)?;

    let program = Program::from_bytes(bytes.as_ref())?;
    let entrypoint_offset = program.get_entrypoint_offset();
    let (ixs, _, _) = program.to_ixs()?;

    // Build per-instruction byte positions (Lddw is 16 bytes, everything else 8).
    let positions: Vec<u64> = ixs
        .iter()
        .scan(0u64, |pos, ix| {
            let current = *pos;
            *pos += ix.get_size();
            Some(current)
        })
        .collect();

    // Scan for jump targets and internal call targets.
    let mut jmp_targets: HashSet<u64> = HashSet::new();
    let mut fn_targets: HashSet<u64> = HashSet::new();

    for (idx, ix) in ixs.iter().enumerate() {
        if ix.is_jump()
            && let Some(Either::Right(off)) = &ix.off
        {
            let target_idx = (idx as i64 + 1 + *off as i64) as usize;
            if let Some(&pos) = positions.get(target_idx) {
                jmp_targets.insert(pos);
            }
        }
        if ix.opcode == Opcode::Call
            && let Some(Either::Right(Number::Int(imm))) = &ix.imm
        {
            let target_idx = (idx as i64 + 1 + *imm) as usize;
            if let Some(&pos) = positions.get(target_idx) {
                fn_targets.insert(pos);
            }
        }
    }

    // Assign a canonical label name to every labelled position.
    // Entrypoint wins if it coincides with a fn target.
    let ep = entrypoint_offset.unwrap_or(0);
    let mut label_at: HashMap<u64, String> = HashMap::new();
    label_at.insert(ep, "entrypoint".to_string());
    for &pos in &fn_targets {
        label_at
            .entry(pos)
            .or_insert_with(|| format!("fn_{:04x}", pos));
    }
    for &pos in &jmp_targets {
        label_at
            .entry(pos)
            .or_insert_with(|| format!("jmp_{:04x}", pos));
    }
    // Ensure position 0 always has a function-entry label so no blocks are
    // orphaned before the first function. In ELFs where the entrypoint is
    // mid-file, position 0 would otherwise be unlabeled.
    label_at.entry(0).or_insert_with(|| "fn_0000".to_string());
    fn_targets.insert(0);

    // Build owned interleaved node list (labels + instructions).
    enum Node {
        Label(String),
        Instr(Instruction),
    }
    let mut nodes: Vec<Node> = Vec::with_capacity(ixs.len() * 2);
    for (idx, ix) in ixs.into_iter().enumerate() {
        let pos = positions[idx];
        if let Some(label) = label_at.get(&pos) {
            nodes.push(Node::Label(label.clone()));
        }
        nodes.push(Node::Instr(ix));
    }

    // Function entries = entrypoint + every internal call target.
    let function_entries: HashSet<String> = fn_targets
        .iter()
        .map(|&pos| label_at[&pos].clone())
        .chain(std::iter::once("entrypoint".to_string()))
        .collect();

    let cfg_nodes = nodes.iter().map(|n| match n {
        Node::Label(s) => InputNode::Label(s.as_str()),
        Node::Instr(ix) => InputNode::Instruction(ix),
    });

    let cfg = control_flow_graph(cfg_nodes, &function_entries, Some("entrypoint"));

    if args.dot {
        print!("{}", dump_cfg_with_critical_path(&cfg));
    } else {
        print!("{}", dump_critical_path(&cfg));
    }
    Ok(())
}
