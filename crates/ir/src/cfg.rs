use {
    crate::{
        InstId, InstructionNode, InstructionVisitor,
        dataflow::{Def, DefId, DefSite, InstKey, MAX_WRITABLE_REG, Reg, Use, UseRef, operands},
        graph_engine::{DfsGraph, WorklistEngine},
    },
    either::Either,
    sbpf_common::{instruction::Instruction, opcode::Opcode},
    smallvec::SmallVec,
    std::collections::{HashMap, HashSet},
};

pub type BlockId = usize;
pub type FunctionId = usize;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CfgRodata {
    pub name: String,
    pub section_offset: u64,
    pub size: u64,
    pub referenced_blocks: Vec<BlockId>,
}

impl CfgRodata {
    pub fn new(name: impl Into<String>, section_offset: u64, size: u64) -> Self {
        Self {
            name: name.into(),
            section_offset,
            size,
            referenced_blocks: Vec::new(),
        }
    }
}

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
    /// Def table for this function's def-use chains, indexed by `DefId`.
    defs: Vec<Def>,
    /// `InstKey` -> (index into `blocks`, index into that block's instructions).
    key_index: Vec<(u32, u32)>,
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

    pub fn def(&self, d: DefId) -> &Def {
        &self.defs[d.index()]
    }

    /// All definitions, starting with one `Entry` def per register r0–r9.
    pub fn defs(&self) -> &[Def] {
        &self.defs
    }

    pub fn node(&self, k: InstKey) -> &InstructionNode {
        let (block, idx) = self.key_index[k.index()];
        &self.blocks[block as usize].instructions[idx as usize]
    }

    pub fn block_of(&self, k: InstKey) -> BlockId {
        self.block_ids[self.key_index[k.index()].0 as usize]
    }

    pub fn use_at(&self, u: UseRef) -> &Use {
        &self.node(u.inst).uses()[u.operand as usize]
    }

    fn node_mut(&mut self, k: InstKey) -> &mut InstructionNode {
        let (block, idx) = self.key_index[k.index()];
        &mut self.blocks[block as usize].instructions[idx as usize]
    }

    /// Swaps the opcode of `k` for one with the same operand registers and kinds
    /// (e.g. `add32` -> `add64`). The def-use chains are unchanged.
    pub fn replace_opcode(&mut self, k: InstKey, opcode: Opcode) {
        let node = self.node_mut(k);
        debug_assert!(
            node.instruction().is_some_and(|inst| {
                let mut replaced = inst.clone();
                replaced.opcode = opcode;
                operands(inst) == operands(&replaced)
            }),
            "replace_opcode must keep the operand shape"
        );
        node.set_opcode(opcode);
    }

    /// Inserts `instruction` right after the instruction defining `d`, and returns
    /// the new definition. `instruction` must read and write only `d`'s register
    /// (e.g. `mov32 rd, rd`). The chains are patched in place: the new use is
    /// reached by `d` alone, and the new def takes over all of `d`'s old users.
    /// This is exact because every path from `d` passes through the new node.
    pub fn insert_redef_after(&mut self, d: DefId, instruction: Instruction) -> DefId {
        let DefSite::Inst(at) = self.defs[d.index()].site else {
            panic!("insert_redef_after needs an instruction def");
        };
        let reg = self.defs[d.index()].reg;
        let ops = operands(&instruction);
        assert!(
            ops.defs.as_slice() == [reg]
                && ops.clobbers.is_empty()
                && ops.uses.iter().all(|&(r, _)| r == reg),
            "inserted instruction must only read and write r{reg}"
        );

        let key = InstKey(self.key_index.len() as u32);
        let new_def = DefId(self.defs.len() as u32);
        let mut node = InstructionNode::synthesized(instruction);
        node.set_key(key);
        node.defs_mut().push(new_def);
        for used in node.uses_mut() {
            used.reaching = SmallVec::from_slice(&[d]);
        }

        // Hand d's users over to the new def.
        let old_users = std::mem::take(&mut self.defs[d.index()].users);
        for &user in &old_users {
            let reaching = &mut self.node_mut(user.inst).uses_mut()[user.operand as usize].reaching;
            for def in reaching.iter_mut() {
                if *def == d {
                    *def = new_def;
                }
            }
            reaching.sort_unstable();
            reaching.dedup();
        }
        self.defs[d.index()].users = (0..node.uses().len())
            .map(|operand| UseRef {
                inst: key,
                operand: operand as u8,
            })
            .collect();
        self.defs.push(Def {
            site: DefSite::Inst(key),
            reg,
            users: old_users,
        });

        // Place the node and shift the positions of everything after it.
        let (block, idx) = self.key_index[at.index()];
        let insert_at = idx + 1;
        let instructions = &mut self.blocks[block as usize].instructions;
        instructions.insert(insert_at as usize, node);
        for later in &instructions[insert_at as usize + 1..] {
            self.key_index[later.key().index()].1 += 1;
        }
        self.key_index.push((block, insert_at));

        new_def
    }
}

/// The control flow graph. Functions own their blocks, which own their instruction nodes.
/// Removing a function automatically drops all its blocks and instructions via Rust ownership.
#[derive(Debug, Clone, Default)]
pub struct Cfg {
    pub functions: Vec<CfgFunction>,
    pub successors: Vec<SmallVec<[BlockId; 3]>>,
    pub predecessors: Vec<SmallVec<[BlockId; 3]>>,
    pub rodata: Vec<CfgRodata>,
    /// Successors within the same function: jumps and fall-through, no call edges.
    intra_successors: Vec<SmallVec<[BlockId; 2]>>,
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
        rodata: Vec::new(),
        intra_successors: vec![SmallVec::new(); n_blocks],
    };

    for (from, to, kind) in collect_edges(&cfg) {
        cfg.add_edge(from, to);
        if kind == EdgeKind::Intra && cfg.function_of_block(from) == cfg.function_of_block(to) {
            cfg.add_intra_edge(from, to);
        }
    }

    cfg.link_def_use();
    cfg
}

impl Cfg {
    pub fn functions(&self) -> &[CfgFunction] {
        &self.functions
    }

    pub fn rodata(&self) -> &[CfgRodata] {
        &self.rodata
    }

    /// Store rodata objects and resolve their references to blocks.
    pub fn set_rodata<'a>(
        &mut self,
        rodata: impl IntoIterator<Item = CfgRodata>,
        references: impl IntoIterator<Item = (u64, &'a str)>,
    ) {
        self.rodata = rodata.into_iter().collect();

        let label_to_block = label_to_block_map(self);
        for (offset, target) in references {
            let Some(target) = label_to_block.get(target).copied() else {
                continue;
            };
            let Some(data) = self.rodata.iter_mut().find(|data| {
                offset >= data.section_offset
                    && offset
                        .checked_add(8)
                        .is_some_and(|end| end <= data.section_offset.saturating_add(data.size))
            }) else {
                continue;
            };
            data.referenced_blocks.push(target);
        }
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

    /// Successors within the same function (jumps and fall-through, no call edges).
    pub fn intra_successors(&self, id: BlockId) -> &[BlockId] {
        self.intra_successors
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

    fn add_intra_edge(&mut self, from: BlockId, to: BlockId) {
        if let Some(successors) = self.intra_successors.get_mut(from)
            && !successors.contains(&to)
        {
            successors.push(to);
        }
    }

    /// Builds every function's def-use chains with reaching definitions over the
    /// intra-function edges.
    fn link_def_use(&mut self) {
        let Cfg {
            functions,
            intra_successors,
            ..
        } = self;
        for function in functions {
            link_function_def_use(function, intra_successors);
        }
    }
}

/// Reaching definitions per register slot, each sorted by `DefId`.
type ReachingState = [SmallVec<[DefId; 2]>; MAX_WRITABLE_REG as usize + 1];

fn link_function_def_use(function: &mut CfgFunction, intra_successors: &[SmallVec<[BlockId; 2]>]) {
    // Number instructions and definitions. DefIds 0..=9 are the Entry defs of r0..r9.
    function.defs = (0..=MAX_WRITABLE_REG)
        .map(|reg| Def {
            site: DefSite::Entry,
            reg,
            users: SmallVec::new(),
        })
        .collect();
    function.key_index.clear();
    for (block_idx, block) in function.blocks.iter_mut().enumerate() {
        for (idx, node) in block.instructions.iter_mut().enumerate() {
            let key = InstKey(function.key_index.len() as u32);
            function.key_index.push((block_idx as u32, idx as u32));
            node.set_key(key);
            node.defs_mut().clear();
            let Some(ops) = node.instruction().map(operands) else {
                continue;
            };
            let sites = ops.defs.iter().map(|&reg| (reg, DefSite::Inst(key))).chain(
                ops.clobbers
                    .iter()
                    .map(|&reg| (reg, DefSite::CallClobber(key))),
            );
            for (reg, site) in sites {
                node.defs_mut().push(DefId(function.defs.len() as u32));
                function.defs.push(Def {
                    site,
                    reg,
                    users: SmallVec::new(),
                });
            }
        }
    }

    // Reaching definitions at each block entry, to a fixed point.
    let local_of: HashMap<BlockId, usize> = function
        .block_ids
        .iter()
        .enumerate()
        .map(|(local, &id)| (id, local))
        .collect();
    let entry_state: ReachingState =
        std::array::from_fn(|reg| SmallVec::from_slice(&[DefId(reg as u32)]));
    let mut block_entry: Vec<Option<ReachingState>> = vec![None; function.blocks.len()];
    let mut worklist = WorklistEngine::new();
    if !function.blocks.is_empty() {
        block_entry[0] = Some(entry_state.clone());
        worklist.push(0usize);
    }
    while let Some(local) = worklist.pop() {
        let mut state = block_entry[local]
            .clone()
            .expect("queued blocks have a state");
        apply_block_defs(function, local, &mut state, |_, _| {});
        for succ in &intra_successors[function.block_ids[local]] {
            let Some(&succ_local) = local_of.get(succ) else {
                continue;
            };
            let changed = match &mut block_entry[succ_local] {
                Some(existing) => join_reaching(existing, &state),
                slot @ None => {
                    *slot = Some(state.clone());
                    true
                }
            };
            if changed {
                worklist.push(succ_local);
            }
        }
    }

    // Link each use to its reaching defs. Blocks unreachable within the function
    // conservatively start from the Entry defs.
    for (local, entry) in block_entry.into_iter().enumerate() {
        let mut state = entry.unwrap_or_else(|| entry_state.clone());
        let mut links = Vec::new();
        apply_block_defs(function, local, &mut state, |use_ref, reaching| {
            links.push((use_ref, reaching.clone()));
        });
        for (use_ref, reaching) in links {
            for &def in &reaching {
                function.defs[def.index()].users.push(use_ref);
            }
            function.node_mut(use_ref.inst).uses_mut()[use_ref.operand as usize].reaching =
                reaching;
        }
    }
}

/// Walks a block's instructions, reporting each use with the defs reaching it
/// and then applying the instruction's own defs to `state`.
fn apply_block_defs(
    function: &CfgFunction,
    local: usize,
    state: &mut ReachingState,
    mut on_use: impl FnMut(UseRef, &SmallVec<[DefId; 2]>),
) {
    for node in &function.blocks[local].instructions {
        for (operand, used) in node.uses().iter().enumerate() {
            let use_ref = UseRef {
                inst: node.key(),
                operand: operand as u8,
            };
            on_use(use_ref, &state[used.reg as usize]);
        }
        for &def in node.defs() {
            let reg: Reg = function.defs[def.index()].reg;
            state[reg as usize] = SmallVec::from_slice(&[def]);
        }
    }
}

/// Unions `incoming` into `existing`; returns whether `existing` grew.
fn join_reaching(existing: &mut ReachingState, incoming: &ReachingState) -> bool {
    let mut changed = false;
    for (slot, add) in existing.iter_mut().zip(incoming) {
        for &def in add {
            if let Err(pos) = slot.binary_search(&def) {
                slot.insert(pos, def);
                changed = true;
            }
        }
    }
    changed
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
            defs: Vec::new(),
            key_index: Vec::new(),
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
                defs: Vec::new(),
                key_index: Vec::new(),
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EdgeKind {
    /// Jump or fall-through within a function.
    Intra,
    /// Call edge into a callee's entry.
    Call,
}

fn collect_edges(cfg: &Cfg) -> Vec<(BlockId, BlockId, EdgeKind)> {
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
    edges: Vec<(BlockId, BlockId, EdgeKind)>,
}

impl EdgeCollector<'_> {
    fn add_edge(&mut self, to: BlockId, kind: EdgeKind) {
        if let Some(from) = self.current_block {
            self.edges.push((from, to, kind));
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
            self.edges.push((block_id, next, EdgeKind::Intra));
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
            self.add_edge(target, EdgeKind::Call);
        }
    }

    fn visit_jump(&mut self, _node: &InstructionNode, instruction: &Instruction) {
        if let Some(Either::Left(label)) = &instruction.off
            && let Some(&target) = self.label_to_block.get(label.as_str())
        {
            self.add_edge(target, EdgeKind::Intra);
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
        crate::graph_engine::{BfsEngine, DfsEngine},
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
    fn test_cfg_bfs_visits_blocks() {
        let cfg = test_cfg();
        let mut visited = Vec::new();

        BfsEngine::new(&cfg)
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

#[cfg(test)]
mod def_use_tests {
    use {
        super::*,
        crate::dataflow::{OperandKind, Use},
        either::Either,
        sbpf_common::inst_param::{Number, Register},
    };

    enum Src<'a> {
        Label(&'a str),
        Inst(Instruction),
    }

    fn build(program: &[Src]) -> Cfg {
        let nodes = program.iter().map(|node| match node {
            Src::Label(label) => InputNode::Label(label),
            Src::Inst(inst) => InputNode::Instruction(inst),
        });
        control_flow_graph(nodes, &HashSet::new(), None)
    }

    fn inst(opcode: Opcode, dst: Option<u8>, src: Option<u8>, imm: Option<i64>) -> Src<'static> {
        Src::Inst(Instruction {
            opcode,
            dst: dst.map(|n| Register { n }),
            src: src.map(|n| Register { n }),
            off: None,
            imm: imm.map(|v| Either::Right(Number::Int(v))),
            span: 0..0,
        })
    }

    fn jump(opcode: Opcode, dst: u8, imm: i64, target: &str) -> Src<'static> {
        Src::Inst(Instruction {
            opcode,
            dst: Some(Register { n: dst }),
            src: None,
            off: Some(Either::Left(target.to_string())),
            imm: Some(Either::Right(Number::Int(imm))),
            span: 0..0,
        })
    }

    fn call(target: &str) -> Src<'static> {
        Src::Inst(Instruction {
            opcode: Opcode::Call,
            dst: None,
            src: None,
            off: None,
            imm: Some(Either::Left(target.to_string())),
            span: 0..0,
        })
    }

    /// All instruction nodes of the single function, in block order.
    fn nodes(cfg: &Cfg) -> Vec<&InstructionNode> {
        cfg.functions()[0]
            .blocks()
            .iter()
            .flat_map(|b| b.instructions())
            .collect()
    }

    /// The reaching defs of `nodes[i].uses()[operand]`, described by where each def comes from.
    fn reaching(cfg: &Cfg, i: usize, operand: usize) -> Vec<String> {
        let func = &cfg.functions()[0];
        let order: Vec<InstKey> = nodes(cfg).iter().map(|n| n.key()).collect();
        let describe = |d: &DefId| {
            let def = func.def(*d);
            match def.site {
                DefSite::Entry => format!("entry:r{}", def.reg),
                DefSite::Inst(k) => format!("inst{}:r{}", pos(&order, k), def.reg),
                DefSite::CallClobber(k) => format!("clobber{}:r{}", pos(&order, k), def.reg),
            }
        };
        nodes(cfg)[i].uses()[operand]
            .reaching
            .iter()
            .map(describe)
            .collect()
    }

    fn pos(order: &[InstKey], k: InstKey) -> usize {
        order.iter().position(|&o| o == k).unwrap()
    }

    /// A position-based description of every use's reaching defs, independent of
    /// how keys and DefIds were numbered.
    fn canonical_chains(cfg: &Cfg) -> Vec<(usize, usize, Vec<String>)> {
        let n = nodes(cfg);
        (0..n.len())
            .flat_map(|i| (0..n[i].uses().len()).map(move |op| (i, op)))
            .map(|(i, op)| (i, op, reaching(cfg, i, op)))
            .collect()
    }

    /// Every def's `users` must match exactly the uses that list it in `reaching`,
    /// and every key must resolve to its own node.
    fn assert_consistent(cfg: &Cfg) {
        let func = &cfg.functions()[0];
        for node in nodes(cfg) {
            assert_eq!(func.node(node.key()).key(), node.key());
        }
        for (d, def) in func.defs().iter().enumerate() {
            let d = DefId(d as u32);
            for &user in &def.users {
                assert!(func.use_at(user).reaching.contains(&d));
            }
        }
        for node in nodes(cfg) {
            for (op, used) in node.uses().iter().enumerate() {
                for &d in &used.reaching {
                    let user = UseRef {
                        inst: node.key(),
                        operand: op as u8,
                    };
                    assert_eq!(func.def(d).users.iter().filter(|&&u| u == user).count(), 1);
                }
            }
        }
    }

    #[test]
    fn test_def_use_straight_line_links_latest_def() {
        let cfg = build(&[
            Src::Label("entrypoint"),
            inst(Opcode::Mov64Imm, Some(1), None, Some(1)),
            inst(Opcode::Mov64Imm, Some(1), None, Some(2)),
            inst(Opcode::Add64Reg, Some(2), Some(1), None),
            inst(Opcode::Exit, None, None, None),
        ]);

        // add64 r2, r1: r2 (Dst) comes from entry, r1 (Src) from the second mov.
        let add = nodes(&cfg)[2];
        assert_eq!(add.uses()[0].kind, OperandKind::Dst);
        assert_eq!(reaching(&cfg, 2, 0), ["entry:r2"]);
        assert_eq!(reaching(&cfg, 2, 1), ["inst1:r1"]);
        // The first mov is dead: no users.
        let func = &cfg.functions()[0];
        assert!(func.def(nodes(&cfg)[0].defs()[0]).users.is_empty());
        // exit reads r0 implicitly.
        assert_eq!(nodes(&cfg)[3].uses()[0].kind, OperandKind::Implicit);
        assert_eq!(reaching(&cfg, 3, 0), ["entry:r0"]);
        assert_consistent(&cfg);
    }

    #[test]
    fn test_def_use_read_modify_write_links_previous_def() {
        let cfg = build(&[
            Src::Label("entrypoint"),
            inst(Opcode::Mov64Imm, Some(1), None, Some(1)),
            inst(Opcode::Add64Reg, Some(1), Some(2), None),
            inst(Opcode::Mov64Reg, Some(0), Some(1), None),
            inst(Opcode::Exit, None, None, None),
        ]);

        assert_eq!(reaching(&cfg, 1, 0), ["inst0:r1"]);
        assert_eq!(reaching(&cfg, 2, 0), ["inst1:r1"]);
        assert_consistent(&cfg);
    }

    #[test]
    fn test_def_use_diamond_merges_two_defs() {
        let cfg = build(&[
            Src::Label("entrypoint"),
            inst(Opcode::Mov64Imm, Some(1), None, Some(1)),
            jump(Opcode::JeqImm, 2, 0, "join"),
            inst(Opcode::Mov64Imm, Some(1), None, Some(2)),
            Src::Label("join"),
            inst(Opcode::Mov64Reg, Some(3), Some(1), None),
            inst(Opcode::Exit, None, None, None),
        ]);

        let mut merged = reaching(&cfg, 3, 0);
        merged.sort();
        assert_eq!(merged, ["inst0:r1", "inst2:r1"]);
        assert_consistent(&cfg);
    }

    #[test]
    fn test_def_use_loop_carried_def_reaches_itself() {
        let cfg = build(&[
            Src::Label("entrypoint"),
            inst(Opcode::Mov64Imm, Some(1), None, Some(0)),
            Src::Label("loop"),
            inst(Opcode::Add64Imm, Some(1), None, Some(1)),
            jump(Opcode::JneImm, 1, 10, "loop"),
            inst(Opcode::Exit, None, None, None),
        ]);

        let mut add_input = reaching(&cfg, 1, 0);
        add_input.sort();
        assert_eq!(add_input, ["inst0:r1", "inst1:r1"]);
        assert_consistent(&cfg);
    }

    #[test]
    fn test_def_use_call_clobbers_args_but_not_callee_saved() {
        let cfg = build(&[
            Src::Label("entrypoint"),
            inst(Opcode::Mov64Imm, Some(1), None, Some(1)),
            inst(Opcode::Mov64Imm, Some(6), None, Some(2)),
            call("helper"),
            inst(Opcode::Mov64Reg, Some(7), Some(1), None),
            inst(Opcode::Mov64Reg, Some(8), Some(6), None),
            inst(Opcode::Mov64Reg, Some(9), Some(0), None),
            inst(Opcode::Exit, None, None, None),
            Src::Label("helper"),
            inst(Opcode::Exit, None, None, None),
        ]);

        // The call reads r1 (argument) from the first mov.
        let call_uses: Vec<_> = nodes(&cfg)[2].uses().iter().map(|u| u.reg).collect();
        assert_eq!(call_uses, [1, 2, 3, 4, 5]);
        assert_eq!(reaching(&cfg, 2, 0), ["inst0:r1"]);
        assert_eq!(reaching(&cfg, 3, 0), ["clobber2:r1"]);
        assert_eq!(reaching(&cfg, 4, 0), ["inst1:r6"]);
        assert_eq!(reaching(&cfg, 5, 0), ["inst2:r0"]);
        // The call edge into `helper` is not an intra-function edge.
        assert!(
            !cfg.intra_successors(0)
                .contains(&cfg.functions()[0].block_ids()[1])
        );
        assert_consistent(&cfg);
    }

    #[test]
    fn test_def_use_ignores_frame_pointer() {
        let cfg = build(&[
            Src::Label("entrypoint"),
            inst(Opcode::Ldxdw, Some(1), Some(10), None),
            inst(Opcode::Exit, None, None, None),
        ]);

        assert!(nodes(&cfg)[0].uses().is_empty());
        assert_consistent(&cfg);
    }

    fn fixup(reg: u8) -> Instruction {
        Instruction {
            opcode: Opcode::Mov32Reg,
            dst: Some(Register { n: reg }),
            src: Some(Register { n: reg }),
            off: None,
            imm: None,
            span: 0..0,
        }
    }

    /// Applies `insert_redef_after` to the def made by node `at`, then checks the
    /// patched chains against a Cfg built from scratch with the instruction inserted.
    fn assert_insert_matches_rebuild(mut program: Vec<Src>, at: usize, reg: u8) {
        let mut cfg = build(&program);
        let d = nodes(&cfg)[at].defs()[0];
        cfg.functions[0].insert_redef_after(d, fixup(reg));
        assert_consistent(&cfg);

        // Find where node `at` sits in `program` (skipping labels) and insert after it.
        let program_pos = program
            .iter()
            .enumerate()
            .filter(|(_, n)| matches!(n, Src::Inst(_)))
            .nth(at)
            .unwrap()
            .0;
        program.insert(program_pos + 1, Src::Inst(fixup(reg)));
        let rebuilt = build(&program);

        assert_eq!(canonical_chains(&cfg), canonical_chains(&rebuilt));
        let ops = |c: &Cfg| nodes(c).iter().map(|n| n.opcode).collect::<Vec<_>>();
        assert_eq!(ops(&cfg), ops(&rebuilt));
    }

    #[test]
    fn test_insert_redef_after_matches_rebuild_straight_line() {
        assert_insert_matches_rebuild(
            vec![
                Src::Label("entrypoint"),
                inst(Opcode::Add32Imm, Some(1), None, Some(5)),
                inst(Opcode::Mov64Reg, Some(2), Some(1), None),
                inst(Opcode::Add64Reg, Some(2), Some(1), None),
                inst(Opcode::Exit, None, None, None),
            ],
            0,
            1,
        );
    }

    #[test]
    fn test_insert_redef_after_matches_rebuild_diamond() {
        assert_insert_matches_rebuild(
            vec![
                Src::Label("entrypoint"),
                inst(Opcode::Mov64Imm, Some(1), None, Some(1)),
                jump(Opcode::JeqImm, 2, 0, "join"),
                inst(Opcode::Sub32Imm, Some(1), None, Some(3)),
                Src::Label("join"),
                inst(Opcode::Mov64Reg, Some(0), Some(1), None),
                inst(Opcode::Exit, None, None, None),
            ],
            2,
            1,
        );
    }

    #[test]
    fn test_insert_redef_after_matches_rebuild_loop() {
        assert_insert_matches_rebuild(
            vec![
                Src::Label("entrypoint"),
                inst(Opcode::Mov64Imm, Some(1), None, Some(0)),
                Src::Label("loop"),
                inst(Opcode::Add32Imm, Some(1), None, Some(1)),
                jump(Opcode::JneImm, 1, 10, "loop"),
                inst(Opcode::Mov64Reg, Some(0), Some(1), None),
                inst(Opcode::Exit, None, None, None),
            ],
            1,
            1,
        );
    }

    #[test]
    fn test_replace_opcode_keeps_chains() {
        let program = [
            Src::Label("entrypoint"),
            inst(Opcode::Add32Reg, Some(1), Some(2), None),
            inst(Opcode::Mov64Reg, Some(0), Some(1), None),
            inst(Opcode::Exit, None, None, None),
        ];
        let mut cfg = build(&program);
        let before = canonical_chains(&cfg);
        let key = nodes(&cfg)[0].key();

        cfg.functions[0].replace_opcode(key, Opcode::Add64Reg);

        assert_eq!(nodes(&cfg)[0].opcode, Opcode::Add64Reg);
        assert_eq!(
            nodes(&cfg)[0].instruction().unwrap().opcode,
            Opcode::Add64Reg
        );
        assert_eq!(canonical_chains(&cfg), before);
        assert_consistent(&cfg);
    }

    #[test]
    fn test_inserted_node_has_no_source() {
        let mut cfg = build(&[
            Src::Label("entrypoint"),
            inst(Opcode::Add32Imm, Some(1), None, Some(5)),
            inst(Opcode::Exit, None, None, None),
        ]);
        let d = nodes(&cfg)[0].defs()[0];
        let d2 = cfg.functions[0].insert_redef_after(d, fixup(1));

        let func = &cfg.functions()[0];
        let DefSite::Inst(k) = func.def(d2).site else {
            panic!("expected an instruction def");
        };
        assert_eq!(func.node(k).source_node_id(), None);
        assert_eq!(func.blocks()[0].node_ids().len(), 3); // label + 2 source instructions
        let Use { reaching, .. } = &func.node(k).uses()[0];
        assert_eq!(reaching.as_slice(), [d]);
    }
}
