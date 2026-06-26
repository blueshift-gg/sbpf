use {
    crate::{
        CompileError,
        ast::AST,
        astnode::{ASTNode, Label},
    },
    either::Either,
    sbpf_common::opcode::Opcode,
    std::collections::{HashMap, HashSet},
};

const CONTROL_FLOW_TARGET_PREFIX: &str = "temp_";

#[derive(Default)]
pub(crate) struct CanonicalizedTargets {
    pub(crate) labels_to_remove: HashSet<String>,
    pub(crate) errors: Vec<CompileError>,
}

enum ControlFlowTarget {
    Jump,
    Call,
}

pub(crate) fn canonicalize_control_flow_targets(nodes: &mut Vec<ASTNode>) -> CanonicalizedTargets {
    let mut label_at_offset = HashMap::new();
    let mut existing_labels = HashSet::new();
    let mut numeric_labels = Vec::new();
    let mut valid_target_offsets = HashSet::new();

    for (idx, node) in nodes.iter().enumerate() {
        match node {
            ASTNode::Label { label, offset } => {
                label_at_offset
                    .entry(*offset)
                    .or_insert_with(|| label.name.clone());
                existing_labels.insert(label.name.clone());
                numeric_labels.push((label.name.clone(), *offset, idx));
            }
            ASTNode::Instruction { offset, .. } => {
                valid_target_offsets.insert(*offset);
            }
            _ => {}
        }
    }

    let mut rewrites = Vec::new();
    let mut labels_to_insert_by_offset = HashMap::new();
    let mut labels_to_remove = HashSet::new();
    let mut errors = Vec::new();

    for (idx, node) in nodes.iter().enumerate() {
        let ASTNode::Instruction {
            instruction,
            offset,
        } = node
        else {
            continue;
        };

        if instruction.is_jump() {
            match &instruction.off {
                Some(Either::Left(label)) => {
                    if existing_labels.contains(label) {
                        continue;
                    }

                    if let Some(target_offset) =
                        AST::resolve_numeric_label(label, idx, &numeric_labels)
                    {
                        let canonical_label = canonical_label_for_target(
                            target_offset,
                            &mut label_at_offset,
                            &mut existing_labels,
                            &mut labels_to_insert_by_offset,
                            &mut labels_to_remove,
                        );
                        rewrites.push((idx, ControlFlowTarget::Jump, canonical_label));
                    }
                }
                Some(Either::Right(relative_offset)) => {
                    let Some(target_offset) =
                        relative_control_flow_target(*offset, *relative_offset as i64)
                    else {
                        errors.push(invalid_relative_target_error(
                            "jump",
                            *relative_offset as i64,
                            None,
                            &instruction.span,
                        ));
                        continue;
                    };

                    if !valid_target_offsets.contains(&target_offset) {
                        errors.push(invalid_relative_target_error(
                            "jump",
                            *relative_offset as i64,
                            Some(target_offset),
                            &instruction.span,
                        ));
                        continue;
                    }

                    let canonical_label = canonical_label_for_target(
                        target_offset,
                        &mut label_at_offset,
                        &mut existing_labels,
                        &mut labels_to_insert_by_offset,
                        &mut labels_to_remove,
                    );
                    rewrites.push((idx, ControlFlowTarget::Jump, canonical_label));
                }
                None => {}
            }
        } else if instruction.opcode == Opcode::Call
            && instruction.src.as_ref().is_some_and(|src| src.n == 1)
            && let Some(Either::Right(relative_offset)) = &instruction.imm
        {
            let relative_offset = relative_offset.to_i64();
            let Some(target_offset) = relative_control_flow_target(*offset, relative_offset) else {
                errors.push(invalid_relative_target_error(
                    "call",
                    relative_offset,
                    None,
                    &instruction.span,
                ));
                continue;
            };

            if !valid_target_offsets.contains(&target_offset) {
                errors.push(invalid_relative_target_error(
                    "call",
                    relative_offset,
                    Some(target_offset),
                    &instruction.span,
                ));
                continue;
            }

            let canonical_label = canonical_label_for_target(
                target_offset,
                &mut label_at_offset,
                &mut existing_labels,
                &mut labels_to_insert_by_offset,
                &mut labels_to_remove,
            );
            rewrites.push((idx, ControlFlowTarget::Call, canonical_label));
        }
    }

    // Canonicalization is an optimization prerequisite, not additional bytecode
    // validation. Leave the AST untouched when any target cannot be normalized.
    if !errors.is_empty() {
        return CanonicalizedTargets {
            labels_to_remove: HashSet::new(),
            errors,
        };
    }

    for (idx, target_kind, canonical_label) in rewrites {
        if let Some(ASTNode::Instruction { instruction, .. }) = nodes.get_mut(idx) {
            match target_kind {
                ControlFlowTarget::Jump => {
                    instruction.off = Some(Either::Left(canonical_label));
                }
                ControlFlowTarget::Call => {
                    instruction.imm = Some(Either::Left(canonical_label));
                }
            }
        }
    }

    if !labels_to_insert_by_offset.is_empty() {
        insert_temp_control_flow_target_labels(nodes, labels_to_insert_by_offset);
    }

    CanonicalizedTargets {
        labels_to_remove,
        errors,
    }
}

fn canonical_label_for_target(
    target_offset: u64,
    label_at_offset: &mut HashMap<u64, String>,
    existing_labels: &mut HashSet<String>,
    labels_to_insert_by_offset: &mut HashMap<u64, Label>,
    labels_to_remove: &mut HashSet<String>,
) -> String {
    if let Some(label) = label_at_offset.get(&target_offset) {
        return label.clone();
    }

    labels_to_insert_by_offset
        .entry(target_offset)
        .or_insert_with(|| {
            let label = Label {
                name: next_temp_control_flow_target_label_name(target_offset, existing_labels),
                span: 0..0,
            };
            existing_labels.insert(label.name.clone());
            label_at_offset.insert(target_offset, label.name.clone());
            labels_to_remove.insert(label.name.clone());
            label
        })
        .name
        .clone()
}

fn next_temp_control_flow_target_label_name(
    offset: u64,
    existing_labels: &HashSet<String>,
) -> String {
    let base = format!("{CONTROL_FLOW_TARGET_PREFIX}{offset}");
    if !existing_labels.contains(&base) {
        return base;
    }

    let mut suffix = 0;
    loop {
        let candidate = format!("{base}_{suffix}");
        if !existing_labels.contains(&candidate) {
            return candidate;
        }
        suffix += 1;
    }
}

fn relative_control_flow_target(offset: u64, relative_offset: i64) -> Option<u64> {
    let offset = i64::try_from(offset).ok()?;
    let displacement = relative_offset.checked_add(1)?.checked_mul(8)?;
    let target_offset = offset.checked_add(displacement)?;
    u64::try_from(target_offset).ok()
}

fn invalid_relative_target_error(
    target_kind: &str,
    relative_offset: i64,
    target_offset: Option<u64>,
    span: &std::ops::Range<usize>,
) -> CompileError {
    let error = if let Some(target_offset) = target_offset {
        format!(
            "Relative {target_kind} target {relative_offset:+} resolves to byte offset \
             {target_offset}, which is not an instruction"
        )
    } else {
        format!("Relative {target_kind} target {relative_offset:+} resolves outside the program")
    };

    CompileError::BytecodeError {
        error,
        span: span.clone(),
        custom_label: None,
    }
}

fn insert_temp_control_flow_target_labels(
    nodes: &mut Vec<ASTNode>,
    labels_by_offset: HashMap<u64, Label>,
) {
    let mut rewritten_nodes = Vec::with_capacity(nodes.len() + labels_by_offset.len());

    for node in std::mem::take(nodes) {
        if let ASTNode::Instruction { offset, .. } = &node
            && let Some(label) = labels_by_offset.get(offset)
        {
            rewritten_nodes.push(ASTNode::Label {
                label: label.clone(),
                offset: *offset,
            });
        }
        rewritten_nodes.push(node);
    }

    *nodes = rewritten_nodes;
}

pub(crate) fn remove_temp_control_flow_target_labels(
    nodes: &mut Vec<ASTNode>,
    labels_to_remove: &HashSet<String>,
) {
    if labels_to_remove.is_empty() {
        return;
    }

    nodes.retain(|node| {
        !matches!(node, ASTNode::Label { label, .. } if labels_to_remove.contains(&label.name))
    });
}
