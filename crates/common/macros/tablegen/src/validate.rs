use {
    crate::{
        error::{combine_errors, err},
        ir::{OpcodeGroupDef, OpcodeTableDef},
    },
    std::collections::{HashMap, HashSet},
    syn::Error,
};

const VALID_SIZES: &[&str] = &["u8", "u16", "u32", "u64"];

/// Validate an opcode table.
pub fn validate_opcode_table(def: &OpcodeTableDef) -> Result<(), Error> {
    let mut errors = Vec::new();

    if def.opcodes.is_empty() {
        errors.push(err(
            def.enum_name.span(),
            "OpcodeTable requires at least one opcode variant",
        ));
    }

    // v3 can have overlapping codes with non-v3 so when checking for
    // duplicates codes, check v3 and non-v3 separately.
    let mut v3_codes: HashMap<u8, String> = HashMap::new();
    let mut non_v3_codes: HashMap<u8, String> = HashMap::new();

    for op in &def.opcodes {
        let name = op.variant_name();
        if op.is_arch_v3() {
            if let Some(prev) = v3_codes.insert(op.code, name.clone()) {
                errors.push(err(
                    op.span,
                    format!(
                        "duplicate code found for v3 opcodes `{prev}` and `{name}`: 0x{:02x} ",
                        op.code
                    ),
                ));
            }
        } else if let Some(prev) = non_v3_codes.insert(op.code, name.clone()) {
            errors.push(err(
                op.span,
                format!(
                    "duplicate code found for opcodes `{prev}` and `{name}`: 0x{:02x} ",
                    op.code
                ),
            ));
        }
    }

    for op in &def.opcodes {
        if op.mnemonic.is_empty() {
            errors.push(err(op.span, "mnemonic must not be empty"));
        }
        if op.doc.is_empty() {
            errors.push(err(op.span, "doc must not be empty"));
        }
        if let Some(size) = &op.size
            && !VALID_SIZES.contains(&size.as_str())
        {
            errors.push(err(
                op.span,
                format!(
                    "invalid size `{size}`, expected one of {}",
                    VALID_SIZES.join(", ")
                ),
            ));
        }
    }

    combine_errors(errors)
}

/// Validate an opcode group.
pub fn validate_opcode_group(def: &OpcodeGroupDef) -> Result<(), Error> {
    let mut errors = Vec::new();
    let mut seen = HashSet::new();

    if def.groups.is_empty() {
        errors.push(err(
            def.enum_name.span(),
            "OpcodeGroup requires at least one group variant",
        ));
    }

    for g in &def.groups {
        let name = g.variant_name();
        if !seen.insert(name.clone()) {
            errors.push(err(g.span, format!("duplicate group variant `{name}`")));
        }
        if g.title.is_empty() {
            errors.push(err(g.span, "title must not be empty"));
        }
        if g.description.is_empty() {
            errors.push(err(g.span, "description must not be empty"));
        }
    }

    combine_errors(errors)
}
