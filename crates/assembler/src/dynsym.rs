use {
    either::Either,
    sbpf_common::{instruction::Instruction, opcode::Opcode},
    std::collections::BTreeMap,
};

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

#[derive(Debug, Default)]
pub struct DynamicSymbolMap {
    symbols: BTreeMap<String, Vec<(SymbolKind, u64)>>,
}

impl DynamicSymbolMap {
    pub fn new() -> Self {
        Self::default()
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

pub fn get_relocation_info(inst: &Instruction) -> (RelocationType, String) {
    match inst.opcode {
        Opcode::Lddw => match &inst.imm {
            Some(Either::Left(identifier)) => (RelocationType::RSbf64Relative, identifier.clone()),
            _ => panic!("Expected label operand"),
        },
        _ => {
            if let Some(Either::Left(identifier)) = &inst.imm {
                (RelocationType::RSbfSyscall, identifier.clone())
            } else {
                panic!("Expected label operand")
            }
        }
    }
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

#[derive(Debug, Default)]
pub struct RelDynMap {
    rel_dyns: BTreeMap<u64, Vec<(RelocationType, String)>>,
}

impl RelDynMap {
    pub fn new() -> Self {
        Self::default()
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

#[cfg(test)]
mod tests {
    use super::*;
    use sbpf_common::inst_param::Register;

    #[test]
    fn test_dynamic_symbol_get_name() {
        let sym = DynamicSymbol::new(42, 0x12, 0, 1, 0, 0);
        assert_eq!(sym.get_name(), 42);
    }

    #[test]
    fn test_dynamic_symbol_map_new() {
        let map = DynamicSymbolMap::new();
        assert!(map.symbols.is_empty());
    }

    #[test]
    fn test_dynamic_symbol_map_add_entry_point() {
        let mut map = DynamicSymbolMap::new();
        map.add_entry_point("entrypoint".to_string(), 0x120);

        let entry_points = map.get_entry_points();
        assert_eq!(entry_points.len(), 1);
        assert_eq!(entry_points[0].0, "entrypoint");
        assert_eq!(entry_points[0].1, 0x120);
    }

    #[test]
    fn test_dynamic_symbol_map_add_call_target() {
        let mut map = DynamicSymbolMap::new();
        map.add_call_target("function".to_string(), 0x200);

        let call_targets = map.get_call_targets();
        assert_eq!(call_targets.len(), 1);
        assert_eq!(call_targets[0].0, "function");
        assert_eq!(call_targets[0].1, 0x200);
    }

    #[test]
    fn test_dynamic_symbol_map_get_symbol() {
        let mut map = DynamicSymbolMap::new();
        map.add_symbol("test".to_string(), SymbolKind::CallTarget, 100);

        let sym = map.get_symbol("test");
        assert!(sym.is_some());
        assert_eq!(sym.unwrap().len(), 1);

        let not_found = map.get_symbol("nonexistent");
        assert!(not_found.is_none());
    }

    #[test]
    fn test_dynamic_symbol_map_get_symbols() {
        let mut map = DynamicSymbolMap::new();
        map.add_symbol("func1".to_string(), SymbolKind::CallTarget, 100);
        map.add_symbol("func2".to_string(), SymbolKind::EntryPoint, 200);

        let symbols = map.get_symbols();
        assert_eq!(symbols.len(), 2);
        assert!(symbols.contains_key("func1"));
        assert!(symbols.contains_key("func2"));
    }

    #[test]
    fn test_dynamic_symbol_map_copy() {
        let mut map = DynamicSymbolMap::new();
        map.add_entry_point("main".to_string(), 0);

        let copy = map.copy();
        assert_eq!(copy.symbols.len(), 1);
        assert!(copy.get_symbol("main").is_some());
    }

    #[test]
    fn test_get_relocation_info_lddw() {
        let inst = Instruction {
            opcode: Opcode::Lddw,
            dst: Some(Register { n: 1 }),
            src: None,
            off: None,
            imm: Some(Either::Left("my_data".to_string())),
            span: 0..10,
        };

        let (rel_type, name) = get_relocation_info(&inst);
        assert_eq!(rel_type, RelocationType::RSbf64Relative);
        assert_eq!(name, "my_data");
    }

    #[test]
    fn test_get_relocation_info_call() {
        let inst = Instruction {
            opcode: Opcode::Call,
            dst: None,
            src: Some(Register { n: 1 }),
            off: None,
            imm: Some(Either::Left("my_function".to_string())),
            span: 0..10,
        };

        let (rel_type, name) = get_relocation_info(&inst);
        assert_eq!(rel_type, RelocationType::RSbfSyscall);
        assert_eq!(name, "my_function");
    }

    #[test]
    fn test_rel_dyn_map_add_and_get() {
        let mut map = RelDynMap::new();
        map.add_rel_dyn(0x100, RelocationType::RSbf64Relative, "data".to_string());
        map.add_rel_dyn(0x200, RelocationType::RSbfSyscall, "func".to_string());

        let rel_dyns = map.get_rel_dyns();
        assert_eq!(rel_dyns.len(), 2);
    }

    #[test]
    fn test_rel_dyn_map_copy() {
        let mut map = RelDynMap::new();
        map.add_rel_dyn(0x100, RelocationType::RSbf64Relative, "test".to_string());

        let copy = map.copy();
        assert_eq!(copy.rel_dyns.len(), 1);
    }
}
