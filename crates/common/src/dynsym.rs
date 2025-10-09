use std::collections::BTreeMap;

#[derive(Debug)]
pub struct DynamicSymbol {
    name: u32,  // index into .dynstr section
    info: u8,   // symbol binding and type
    other: u8,  // symbol visibility
    shndx: u16, // section index
    value: u64, // symbol value
    size: u64,  // symbol size
}

impl DynamicSymbol {
    pub fn new(name: u32, info: u8, other: u8, shndx: u16, value: u64, size: u64) -> Self {
        Self {
            name,
            info,
            other,
            shndx,
            value,
            size,
        }
    }

    pub fn bytecode(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend(self.name.to_le_bytes());
        bytes.push(self.info);
        bytes.push(self.other);
        bytes.extend(self.shndx.to_le_bytes());
        bytes.extend(self.value.to_le_bytes());
        bytes.extend(self.size.to_le_bytes());
        bytes
    }

    pub fn get_name(&self) -> u32 {
        self.name
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum SymbolKind {
    EntryPoint,
    CallTarget,
}

#[derive(Debug)]
pub struct DynamicSymbolMap {
    symbols: BTreeMap<String, Vec<(SymbolKind, u64)>>,
}

impl DynamicSymbolMap {
    pub fn new() -> Self {
        Self {
            symbols: BTreeMap::new(),
        }
    }

    pub fn copy(&self) -> Self {
        Self {
            symbols: self.symbols.clone(),
        }
    }

    pub fn add_symbol(&mut self, name: String, kind: SymbolKind, offset: u64) {
        self.symbols.entry(name).or_default().push((kind, offset));
    }

    pub fn add_entry_point(&mut self, name: String, offset: u64) {
        self.add_symbol(name, SymbolKind::EntryPoint, offset);
    }

    pub fn add_call_target(&mut self, name: String, offset: u64) {
        self.add_symbol(name, SymbolKind::CallTarget, offset);
    }

    pub fn get_entry_points(&self) -> Vec<(String, u64)> {
        self.get_symbols_by_kind(SymbolKind::EntryPoint)
    }

    pub fn get_call_targets(&self) -> Vec<(String, u64)> {
        self.get_symbols_by_kind(SymbolKind::CallTarget)
    }

    fn get_symbols_by_kind(&self, kind: SymbolKind) -> Vec<(String, u64)> {
        self.symbols
            .iter()
            .filter(|(_, symbols)| symbols.iter().any(|(k, _)| *k == kind))
            .map(|(name, symbols)| {
                (
                    name.clone(),
                    symbols.iter().find(|(k, _)| *k == kind).unwrap().1,
                )
            })
            .collect()
    }

    pub fn get_symbol(&self, name: &str) -> Option<&Vec<(SymbolKind, u64)>> {
        self.symbols.get(name)
    }

    pub fn get_symbols(&self) -> &BTreeMap<String, Vec<(SymbolKind, u64)>> {
        &self.symbols
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(u64)]
pub enum RelocationType {
    RSbf64Relative = 0x08,
    RSbfSyscall = 0x0a,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RelDyn {
    offset: u64,
    rel_type: u64,
    dynstr_offset: u64,
}

impl RelDyn {
    pub fn new(offset: u64, rel_type: u64, dynstr_offset: u64) -> Self {
        Self {
            offset,
            rel_type,
            dynstr_offset,
        }
    }

    pub fn bytecode(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend(self.offset.to_le_bytes());

        if self.rel_type == 0x08 {
            // 8 bytes rel_type
            bytes.extend(self.rel_type.to_le_bytes());
        } else if self.rel_type == 0x0a {
            // 4 bytes rel_type
            bytes.extend((self.rel_type as u32).to_le_bytes());
            // 4 bytes dynstr_offset
            bytes.extend((self.dynstr_offset as u32).to_le_bytes());
        }

        bytes
    }
}

#[derive(Debug)]
pub struct RelDynMap {
    rel_dyns: BTreeMap<u64, Vec<(RelocationType, String)>>,
}

impl RelDynMap {
    pub fn new() -> Self {
        Self {
            rel_dyns: BTreeMap::new(),
        }
    }

    pub fn add_rel_dyn(&mut self, offset: u64, rel_type: RelocationType, name: String) {
        self.rel_dyns
            .entry(offset)
            .or_default()
            .push((rel_type, name));
    }

    pub fn get_rel_dyns(&self) -> Vec<(u64, RelocationType, String)> {
        self.rel_dyns
            .iter()
            .flat_map(|(offset, rel_types)| {
                rel_types
                    .iter()
                    .map(move |(rel_type, name)| (*offset, *rel_type, name.clone()))
            })
            .collect()
    }

    pub fn copy(&self) -> Self {
        Self {
            rel_dyns: self.rel_dyns.clone(),
        }
    }
}
