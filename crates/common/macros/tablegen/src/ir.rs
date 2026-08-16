use {
    proc_macro2::Span,
    std::collections::BTreeMap,
    syn::{Ident, Path},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OpcodeArch {
    V2,
    V3,
}

impl OpcodeArch {
    pub fn as_str(self) -> &'static str {
        match self {
            OpcodeArch::V2 => "v2",
            OpcodeArch::V3 => "v3",
        }
    }
}

/// An opcode variant after parsing `#[opcode(...)]`.
#[derive(Debug, Clone)]
pub struct OpcodeDef {
    pub variant: Ident,
    pub mnemonic: String,
    pub code: u8,
    /// The group the opcode belongs to
    pub group: Path,
    pub doc: String,
    pub operator: Option<String>,
    /// Memory access width for load/store ops
    pub size: Option<String>,
    /// If the opcode is only available in a specific arch (v2 or v3)
    pub arch: Option<OpcodeArch>,
    pub span: Span,
}

impl OpcodeDef {
    pub fn group_variant_name(&self) -> Option<String> {
        self.group.segments.last().map(|s| s.ident.to_string())
    }

    pub fn variant_name(&self) -> String {
        self.variant.to_string()
    }

    pub fn is_arch_v3(&self) -> bool {
        self.arch == Some(OpcodeArch::V3)
    }

    /// Check if this variant is the FromStr target for its mnemonic.
    pub fn is_from_str_target(&self, all: &[OpcodeDef]) -> bool {
        let variants: Vec<_> = all.iter().filter(|o| o.mnemonic == self.mnemonic).collect();
        // If the mnemonic has a single variant, then it is the target.
        if variants.len() <= 1 {
            return true;
        }
        // If the mnemonic has more than one variants, then prefer the Imm variant as the target.
        // For example - add64 is the mnemonic for both Add64Imm and Add64Reg variants.
        let imm = variants.iter().find(|o| o.variant_name().ends_with("Imm"));
        match imm {
            Some(target) => target.variant_name() == self.variant_name(),
            None => {
                // If there's no Imm variant, then the first variant in the list is the target.
                variants
                    .first()
                    .map(|o| o.variant_name() == self.variant_name())
                    .unwrap_or(true)
            }
        }
    }
}

/// A group variant after parsing `#[group(...)]`.
#[derive(Debug, Clone)]
pub struct GroupDef {
    pub variant: Ident,
    pub title: String,
    pub description: String,
    pub decode: Path,
    pub validate: Path,
    pub execute: Path,
    pub span: Span,
}

/// Handler function type paths after parsing `#[handlers(...)]`.
#[derive(Debug, Clone)]
pub struct HandlerTypePaths {
    pub decode: Path,
    pub validate: Path,
    pub execute: Path,
    pub span: Span,
}

impl GroupDef {
    pub fn variant_name(&self) -> String {
        self.variant.to_string()
    }
}

/// Full opcode table.
#[derive(Debug, Clone)]
pub struct OpcodeTableDef {
    pub enum_name: Ident,
    pub opcodes: Vec<OpcodeDef>,
}

impl OpcodeTableDef {
    /// Get a mapping of all group variants and their opcodes.
    pub fn opcodes_by_group(&self) -> BTreeMap<String, Vec<&OpcodeDef>> {
        let mut map: BTreeMap<String, Vec<&OpcodeDef>> = BTreeMap::new();
        for op in &self.opcodes {
            if let Some(g) = op.group_variant_name() {
                map.entry(g).or_default().push(op);
            }
        }
        map
    }
}

/// Full opcode group.
#[derive(Debug, Clone)]
pub struct OpcodeGroupDef {
    pub enum_name: Ident,
    pub handlers: HandlerTypePaths,
    pub groups: Vec<GroupDef>,
}

impl OpcodeGroupDef {
    pub fn get(&self, variant: &str) -> Option<&GroupDef> {
        self.groups.iter().find(|g| g.variant_name() == variant)
    }
}
