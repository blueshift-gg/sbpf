use {
    crate::{InstId, InstructionNode, InstructionVisitor, graph_engine::DfsGraph},
    either::Either,
    sbpf_common::{instruction::Instruction, opcode::Opcode},
    smallvec::SmallVec,
    std::collections::{HashMap, HashSet},
};

pub type BlockId = usize;
pub type FunctionId = usize;

#[derive(Debug, Clone, Copy)]
pub enum InputNode<'a> {
    Label(&'a str),
    Instruction(&'a Instruction),
    Other,
}

/// A basic block owning its instruction nodes.
#[derive(Debug, Clone, Default)]
pub struct Block {
    pub node_ids: Vec<usize>,
    pub labels: Vec<(String, usize)>,
    pub instructions: Vec<InstructionNode>,
}

impl Block {
    pub fn node_ids(&self) -> &[usize] {
        &self.node_ids
    }

    pub fn labels(&self) -> &[(String, usize)] {
        &self.labels
    }

    pub fn instructions(&self) -> &[InstructionNode] {
        &self.instructions
    }
}

/// A function in the CFG owning its basic blocks. `block_ids` stores the global
/// `BlockId` of each owned block (parallel to `blocks`), so membership is explicit
/// rather than inferred from a contiguous range.
#[derive(Debug, Clone)]
pub struct CfgFunction {
    pub name: String,
    /// Global BlockIds for each block in `blocks`, in order.
    pub block_ids: Vec<BlockId>,
    pub blocks: Vec<Block>,
}

impl CfgFunction {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn block_ids(&self) -> &[BlockId] {
        &self.block_ids
    }

    pub fn blocks(&self) -> &[Block] {
        &self.blocks
    }

    /// The global BlockId of this function's entry (first) block, if any.
    pub fn entry_block_id(&self) -> Option<BlockId> {
        self.block_ids.first().copied()
    }
}

/// The control flow graph. Functions own their blocks, which own their instruction nodes.
/// Removing a function automatically drops all its blocks and instructions via Rust ownership.
#[derive(Debug, Clone, Default)]
pub struct Cfg {
    pub functions: Vec<CfgFunction>,
    pub successors: Vec<SmallVec<[BlockId; 3]>>,
    pub predecessors: Vec<SmallVec<[BlockId; 3]>>,
}

/// Builds a CFG in source order. If `entry_label` names a known function entry,
/// that function is placed first in `functions()`.
pub fn control_flow_graph<'a>(
    nodes: impl IntoIterator<Item = InputNode<'a>>,
    function_entries: &HashSet<String>,
    entry_label: Option<&str>,
) -> Cfg {
    let flat_blocks = collect_blocks(nodes);
    let n_blocks = flat_blocks.len();

    let functions = collect_functions(flat_blocks, function_entries, entry_label);

    let mut cfg = Cfg {
        functions,
        successors: vec![SmallVec::new(); n_blocks],
        predecessors: vec![SmallVec::new(); n_blocks],
    };

    for (from, to) in collect_edges(&cfg) {
        cfg.add_edge(from, to);
    }

    cfg
}

impl Cfg {
    pub fn functions(&self) -> &[CfgFunction] {
        &self.functions
    }

    /// Returns a block by its global BlockId.
    pub fn block(&self, id: BlockId) -> Option<&Block> {
        for func in &self.functions {
            if let Some(pos) = func.block_ids.iter().position(|&b| b == id) {
                return func.blocks.get(pos);
            }
        }
        None
    }

    /// Returns the index of the function that owns the given block, or `None` if the
    /// block is not found in any function.
    pub fn function_of_block(&self, block_id: BlockId) -> Option<FunctionId> {
        self.functions
            .iter()
            .position(|func| func.block_ids.contains(&block_id))
    }

    pub fn successors(&self, id: BlockId) -> &[BlockId] {
        self.successors
            .get(id)
            .map(SmallVec::as_slice)
            .unwrap_or_default()
    }

    pub fn predecessors(&self, id: BlockId) -> &[BlockId] {
        self.predecessors
            .get(id)
            .map(SmallVec::as_slice)
            .unwrap_or_default()
    }

    /// Returns an instruction by its global `InstId` (sequential position across all blocks).
    pub fn instruction(&self, inst_id: InstId) -> Option<&InstructionNode> {
        let mut base = 0usize;
        for (_, block) in self.all_blocks() {
            let n = block.instructions.len();
            if inst_id >= base && inst_id < base + n {
                return block.instructions.get(inst_id - base);
            }
            base += n;
        }
        None
    }

    /// Returns the first global `InstId` for the given block (sum of instruction counts
    /// of all preceding blocks in traversal order).
    pub fn block_inst_offset(&self, target_id: BlockId) -> usize {
        let mut base = 0usize;
        for (block_id, block) in self.all_blocks() {
            if block_id == target_id {
                return base;
            }
            base += block.instructions.len();
        }
        base
    }

    /// Iterates all (BlockId, &Block) pairs across all functions in order.
    pub fn all_blocks(&self) -> impl Iterator<Item = (BlockId, &Block)> {
        self.functions
            .iter()
            .flat_map(|f| f.block_ids.iter().copied().zip(f.blocks.iter()))
    }

    /// Iterates all (InstId, &InstructionNode) pairs across all blocks in order.
    pub fn all_instructions(&self) -> impl Iterator<Item = (InstId, &InstructionNode)> {
        self.all_blocks_with_inst_base()
            .flat_map(|(base, _, block)| {
                block
                    .instructions
                    .iter()
                    .enumerate()
                    .map(move |(i, node)| (base + i, node))
            })
    }

    /// Total number of blocks across all functions.
    pub fn total_blocks(&self) -> usize {
        self.functions.iter().map(|f| f.blocks.len()).sum()
    }

    /// Total number of instructions across all blocks.
    pub fn total_instructions(&self) -> usize {
        self.functions
            .iter()
            .flat_map(|f| f.blocks.iter())
            .map(|b| b.instructions.len())
            .sum()
    }

    /// Internal: iterator of (inst_base, BlockId, &Block) with pre-computed inst offset.
    fn all_blocks_with_inst_base(&self) -> impl Iterator<Item = (usize, BlockId, &Block)> {
        let mut base = 0usize;
        self.all_blocks().map(move |(id, block)| {
            let b = base;
            base += block.instructions.len();
            (b, id, block)
        })
    }

    fn add_edge(&mut self, from: BlockId, to: BlockId) {
        if let Some(successors) = self.successors.get_mut(from)
            && !successors.contains(&to)
        {
            successors.push(to);
        }

        if let Some(predecessors) = self.predecessors.get_mut(to)
            && !predecessors.contains(&from)
        {
            predecessors.push(from);
        }
    }
}

/// Groups flat blocks into CfgFunctions. Each block receives the global BlockId equal
/// to its position in the original flat list. A block starts a new function when one
/// of its labels appears in `function_entries`. If `entry_label` names a known function
/// entry, that function is moved to position 0 so `functions()[0]` is always the root.
fn collect_functions(
    blocks: Vec<Block>,
    function_entries: &HashSet<String>,
    entry_label: Option<&str>,
) -> Vec<CfgFunction> {
    if blocks.is_empty() {
        return Vec::new();
    }

    // No function metadata: wrap everything in a single implicit root function.
    if function_entries.is_empty() {
        let n = blocks.len();
        return vec![CfgFunction {
            name: String::new(),
            block_ids: (0..n).collect(),
            blocks,
        }];
    }

    let mut functions: Vec<CfgFunction> = Vec::new();

    for (block_id, block) in blocks.into_iter().enumerate() {
        // A block with a function-entry label starts a new function.
        let func_name = block
            .labels
            .iter()
            .find(|(label, _)| function_entries.contains(label.as_str()))
            .map(|(label, _)| label.clone());

        if let Some(name) = func_name {
            // Case 1: function-entry label → start a new function.
            // The entry block goes in first so entry_block_id() always returns the real entry.
            let mut function = CfgFunction {
                name,
                block_ids: Vec::new(),
                blocks: Vec::new(),
            };
            function.block_ids.push(block_id);
            function.blocks.push(block);
            functions.push(function);
        } else if let Some(func) = functions.last_mut() {
            // Case 2: continuation block — append to the current function.
            func.block_ids.push(block_id);
            func.blocks.push(block);
        } else {
            // Case 3: block before any function entry — not valid in the linker workflow.
            unreachable!("block {block_id} appears before any function-entry label");
        }
    }

    assert!(
        !functions.is_empty(),
        "no function-entry labels found in non-empty block list"
    );

    // Place the declared entry function first so functions()[0] is always the root.
    if let Some(entry_label) = entry_label
        && let Some(pos) = functions.iter().position(|f| f.name == entry_label)
    {
        functions[0..=pos].rotate_right(1);
    }

    functions
}

impl DfsGraph for Cfg {
    type Node = BlockId;

    fn successors(&self, node: Self::Node) -> &[Self::Node] {
        self.successors(node)
    }
}

fn collect_blocks<'a>(nodes: impl IntoIterator<Item = InputNode<'a>>) -> Vec<Block> {
    let mut collector = BlockCollector::default();
    for (node_id, node) in nodes.into_iter().enumerate() {
        match node {
            InputNode::Label(label) => collector.on_label(node_id, label),
            InputNode::Instruction(instruction) => collector.on_instruction(node_id, instruction),
            InputNode::Other => {}
        }
    }
    collector.finish()
}

#[derive(Default)]
struct BlockCollector {
    blocks: Vec<Block>,
    current: Block,
}

impl BlockCollector {
    fn finish(mut self) -> Vec<Block> {
        assert!(
            self.current.labels.is_empty() || !self.current.instructions.is_empty(),
            "trailing label(s) {:?} have no instructions",
            self.current
                .labels
                .iter()
                .map(|(l, _)| l)
                .collect::<Vec<_>>()
        );
        if !self.current.instructions.is_empty() {
            self.push_current_block();
        }
        self.blocks
    }

    fn push_current_block(&mut self) {
        self.blocks.push(std::mem::take(&mut self.current));
    }
}

impl BlockCollector {
    fn on_label(&mut self, node_id: usize, label: &str) {
        if !self.current.instructions.is_empty() {
            self.push_current_block();
        }
        self.current.node_ids.push(node_id);
        self.current.labels.push((label.to_string(), node_id));
    }

    fn on_instruction(&mut self, node_id: usize, instruction: &Instruction) {
        self.current.node_ids.push(node_id);
        let node = InstructionNode::from_instruction(node_id, instruction.clone());
        self.current.instructions.push(node);

        if instruction.opcode == Opcode::Exit || instruction.is_jump() {
            self.push_current_block();
        }
    }
}

/// Collects CFG edges. Jump and call targets are always canonicalized to label names
/// by the assembler (via `canonicalize_control_flow_targets`) before CFG construction,
/// so only label-based lookups are needed.
fn collect_edges(cfg: &Cfg) -> Vec<(BlockId, BlockId)> {
    let label_to_block = label_to_block_map(cfg);
    let block_count = cfg.successors.len();

    let mut collector = EdgeCollector {
        cfg,
        label_to_block: &label_to_block,
        block_count,
        current_block: None,
        edges: Vec::new(),
    };

    for (block_id, block) in cfg.all_blocks() {
        collector.visit_block(block_id, block);
    }
    collector.edges
}

struct EdgeCollector<'a> {
    cfg: &'a Cfg,
    label_to_block: &'a HashMap<String, BlockId>,
    block_count: usize,
    current_block: Option<BlockId>,
    edges: Vec<(BlockId, BlockId)>,
}

impl EdgeCollector<'_> {
    fn add_edge(&mut self, to: BlockId) {
        if let Some(from) = self.current_block {
            self.edges.push((from, to));
        }
    }

    fn add_fallthrough_edge(&mut self) {
        let Some(block_id) = self.current_block else {
            return;
        };
        let next = block_id + 1;
        if next >= self.block_count {
            return;
        }
        // Suppress fall-through across function boundaries: the only valid way
        // to enter a function is via an explicit `call imm` instruction.
        let same_function = match (
            self.cfg.function_of_block(block_id),
            self.cfg.function_of_block(next),
        ) {
            (Some(f1), Some(f2)) => f1 == f2,
            _ => true,
        };
        if same_function {
            self.edges.push((block_id, next));
        }
    }
}

impl EdgeCollector<'_> {
    fn visit_block(&mut self, block_id: BlockId, block: &Block) {
        self.current_block = Some(block_id);

        for node in block.instructions() {
            self.visit_instruction_node(node);
        }

        let Some(last_node) = block.instructions().last() else {
            return;
        };
        let Some(last_instruction) = last_node.instruction() else {
            return;
        };

        if last_instruction.opcode == Opcode::Exit {
            return;
        }

        if !last_instruction.is_jump() {
            self.add_fallthrough_edge();
        }
    }
}

impl InstructionVisitor for EdgeCollector<'_> {
    fn visit_call(&mut self, _node: &InstructionNode, instruction: &Instruction) {
        if let Some(Either::Left(label)) = &instruction.imm
            && let Some(&target) = self.label_to_block.get(label.as_str())
        {
            self.add_edge(target);
        }
    }

    fn visit_jump(&mut self, _node: &InstructionNode, instruction: &Instruction) {
        if let Some(Either::Left(label)) = &instruction.off
            && let Some(&target) = self.label_to_block.get(label.as_str())
        {
            self.add_edge(target);
        }
        if instruction.opcode != Opcode::Ja {
            self.add_fallthrough_edge();
        }
    }
}

fn label_to_block_map(cfg: &Cfg) -> HashMap<String, BlockId> {
    cfg.all_blocks()
        .flat_map(|(block_id, block)| {
            block
                .labels
                .iter()
                .map(move |(label, _)| (label.clone(), block_id))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::graph_engine::{DfsEngine, WorklistEngine},
        either::Either,
        sbpf_common::{inst_param::Register, instruction::Instruction},
    };

    #[test]
    fn test_cfg_dfs_visits_blocks() {
        let cfg = test_cfg();
        let mut visited = Vec::new();

        DfsEngine::new(&cfg).visit(0, &mut |block| visited.push(block));

        assert_eq!(visited, vec![0, 2]);
        assert_eq!(cfg.total_blocks(), 3);
        assert_eq!(cfg.block(2).unwrap().labels()[0].0, "target");
    }

    #[test]
    fn test_cfg_worklist_visits_blocks() {
        let cfg = test_cfg();
        let mut visited = Vec::new();

        WorklistEngine::new(&cfg)
            .initialize([0])
            .run(&mut |block| visited.push(block));

        assert_eq!(visited, vec![0, 2]);
    }

    #[test]
    fn test_cfg_groups_blocks_by_function_entries() {
        let entry_jump = instruction(Opcode::Ja, Some(Either::Left("internal".to_string())));
        let internal_exit = instruction(Opcode::Exit, None);
        let helper_exit = instruction(Opcode::Exit, None);
        let nodes = [
            InputNode::Label("entrypoint"),
            InputNode::Instruction(&entry_jump),
            InputNode::Label("internal"),
            InputNode::Instruction(&internal_exit),
            InputNode::Label("helper"),
            InputNode::Instruction(&helper_exit),
        ];
        let function_entries = HashSet::from(["entrypoint".to_string(), "helper".to_string()]);

        let cfg = control_flow_graph(nodes, &function_entries, None);

        assert_eq!(cfg.functions().len(), 2);
        assert_eq!(cfg.functions()[0].name(), "entrypoint");
        assert_eq!(cfg.functions()[0].blocks().len(), 2);
        assert_eq!(cfg.functions()[0].block_ids(), &[0, 1]);
        assert_eq!(cfg.functions()[1].name(), "helper");
        assert_eq!(cfg.functions()[1].blocks().len(), 1);
        assert_eq!(cfg.functions()[1].block_ids(), &[2]);
    }

    #[test]
    fn test_cfg_places_declared_entry_function_first() {
        // Source order: helper (block 0) then entrypoint (block 1).
        // The declared entry should appear first in functions() despite coming second in source.
        let helper_exit = instruction(Opcode::Exit, None);
        let call_helper = Instruction {
            opcode: Opcode::Call,
            dst: None,
            src: Some(Register { n: 1 }),
            off: None,
            imm: Some(Either::Left("helper".to_string())),
            span: 0..0,
        };
        let entry_exit = instruction(Opcode::Exit, None);
        let nodes = [
            InputNode::Label("helper"),
            InputNode::Instruction(&helper_exit),
            InputNode::Label("entrypoint"),
            InputNode::Instruction(&call_helper),
            InputNode::Instruction(&entry_exit),
        ];
        let function_entries = HashSet::from(["helper".to_string(), "entrypoint".to_string()]);

        let cfg = control_flow_graph(nodes, &function_entries, Some("entrypoint"));

        assert_eq!(cfg.functions()[0].name(), "entrypoint");
        assert_eq!(cfg.functions()[0].entry_block_id(), Some(1)); // entrypoint is block 1 in source order
        assert_eq!(cfg.block(1).unwrap().labels()[0].0, "entrypoint");
        assert_eq!(cfg.functions()[1].name(), "helper");
        assert_eq!(cfg.successors(1), &[0]); // entrypoint calls helper (block 0)
    }

    #[test]
    fn test_cfg_groups_labels_with_following_instructions() {
        let call = Instruction {
            opcode: Opcode::Call,
            dst: None,
            src: Some(Register { n: 1 }),
            off: None,
            imm: Some(Either::Left("panic".to_string())),
            span: 0..0,
        };
        let dead_exit = instruction(Opcode::Exit, None);
        let panic_exit = instruction(Opcode::Exit, None);
        // Source order: each label immediately precedes its instructions.
        let nodes = [
            InputNode::Label("entrypoint"),
            InputNode::Instruction(&call),
            InputNode::Label("dead_function"),
            InputNode::Instruction(&dead_exit),
            InputNode::Label("panic"),
            InputNode::Instruction(&panic_exit),
        ];
        let cfg = control_flow_graph(nodes, &HashSet::new(), None);

        assert_eq!(cfg.total_blocks(), 3);
        assert_eq!(cfg.block(0).unwrap().node_ids(), &[0, 1]);
        assert_eq!(cfg.block(1).unwrap().node_ids(), &[2, 3]);
        assert_eq!(cfg.block(2).unwrap().node_ids(), &[4, 5]);
        assert_eq!(cfg.successors(0), &[2, 1]);
        assert!(cfg.successors(1).is_empty());
        assert!(cfg.successors(2).is_empty());
    }

    #[test]
    fn test_cfg_fallthrough_does_not_cross_function_boundary() {
        let nop = instruction_with_registers(Opcode::Mov64Imm, Some(0), None, None);
        let exit = instruction(Opcode::Exit, None);
        let nodes = [
            InputNode::Label("func_a"),
            InputNode::Instruction(&nop),
            InputNode::Label("func_b"),
            InputNode::Instruction(&exit),
        ];
        let function_entries = HashSet::from(["func_a".to_string(), "func_b".to_string()]);
        let cfg = control_flow_graph(nodes, &function_entries, None);

        assert!(cfg.successors(0).is_empty());
    }

    fn test_cfg() -> Cfg {
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
        control_flow_graph(nodes, &HashSet::new(), None)
    }

    fn instruction(opcode: Opcode, off: Option<Either<String, i16>>) -> Instruction {
        instruction_with_registers(opcode, None, None, off)
    }

    fn instruction_with_registers(
        opcode: Opcode,
        dst: Option<u8>,
        src: Option<u8>,
        off: Option<Either<String, i16>>,
    ) -> Instruction {
        Instruction {
            opcode,
            dst: dst.map(|n| Register { n }),
            src: src.map(|n| Register { n }),
            off,
            imm: None,
            span: 0..0,
        }
    }
}
