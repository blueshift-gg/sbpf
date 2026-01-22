use crate::{
    astnode::{ASTNode, ROData},
    dynsym::{DynamicSymbol, RelDyn},
    header::SectionHeader,
    parser::Token,
};

// Base Section trait
pub trait Section {
    fn name(&self) -> &str {
        ".unknown" // Default section name
    }

    fn bytecode(&self) -> Vec<u8> {
        Vec::new() // Default empty bytecode
    }

    // fn get_size(&self) -> u64
    fn size(&self) -> u64 {
        self.bytecode().len() as u64
    }

    // fn get_aligned_size(&self) -> u64

    // fn section_header_bytecode(&self) -> Vec<u8>
}

// Code Section implementation
#[derive(Debug)]
pub struct CodeSection {
    name: String,
    nodes: Vec<ASTNode>,
    size: u64,
    offset: u64,
}

impl CodeSection {
    pub fn new(nodes: Vec<ASTNode>, size: u64) -> Self {
        Self {
            name: String::from(".text"),
            nodes,
            size,
            offset: 0,
        }
    }

    pub fn get_nodes(&self) -> &Vec<ASTNode> {
        &self.nodes
    }

    pub fn get_size(&self) -> u64 {
        self.size
    }

    pub fn set_offset(&mut self, offset: u64) {
        self.offset = offset;
    }

    pub fn section_header_bytecode(&self) -> Vec<u8> {
        let flags = SectionHeader::SHF_ALLOC | SectionHeader::SHF_EXECINSTR;
        SectionHeader::new(
            1,
            SectionHeader::SHT_PROGBITS,
            flags,
            self.offset,
            self.offset,
            self.size,
            0,
            0,
            4,
            0,
        )
        .bytecode()
    }
}

impl Section for CodeSection {
    fn name(&self) -> &str {
        &self.name
    }

    fn bytecode(&self) -> Vec<u8> {
        let mut bytecode = Vec::new();
        for node in &self.nodes {
            if let Some(node_bytes) = node.bytecode() {
                bytecode.extend(node_bytes);
            }
        }
        bytecode
    }

    fn size(&self) -> u64 {
        self.size
    }
}

// Data Section implementation
#[derive(Debug)]
pub struct DataSection {
    name: String,
    nodes: Vec<ASTNode>,
    size: u64,
    offset: u64,
}

impl DataSection {
    pub fn new(nodes: Vec<ASTNode>, size: u64) -> Self {
        Self {
            name: String::from(".rodata"),
            nodes,
            size,
            offset: 0,
        }
    }

    pub fn get_nodes(&self) -> &Vec<ASTNode> {
        &self.nodes
    }

    pub fn get_size(&self) -> u64 {
        self.size
    }

    pub fn set_offset(&mut self, offset: u64) {
        self.offset = offset;
    }

    pub fn rodata(&self) -> Vec<(String, usize, String)> {
        let mut ro_data_labels = Vec::new();
        for node in &self.nodes {
            if let ASTNode::ROData {
                rodata: ROData { name, args, .. },
                offset,
            } = node
                && let Some(Token::StringLiteral(str_literal, _)) = args.get(1)
            {
                ro_data_labels.push((name.clone(), *offset as usize, str_literal.clone()));
            }
        }
        ro_data_labels
    }

    pub fn section_header_bytecode(&self) -> Vec<u8> {
        let flags = SectionHeader::SHF_ALLOC; // Read-only data
        SectionHeader::new(
            7,
            SectionHeader::SHT_PROGBITS,
            flags,
            self.offset,
            self.offset,
            self.size,
            0,
            0,
            1,
            0,
        )
        .bytecode()
    }
}

impl Section for DataSection {
    fn name(&self) -> &str {
        &self.name
    }

    fn size(&self) -> u64 {
        // Return 8-byte aligned size
        (self.size + 7) & !7
    }

    fn bytecode(&self) -> Vec<u8> {
        let mut bytecode = Vec::new();
        for node in &self.nodes {
            if let Some(node_bytes) = node.bytecode() {
                bytecode.extend(node_bytes);
            }
        }
        // Add padding to make size multiple of 8
        while bytecode.len() % 8 != 0 {
            bytecode.push(0);
        }

        bytecode
    }
}

#[derive(Debug, Default)]
pub struct NullSection {
    name: String,
    offset: u64,
}

impl NullSection {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn section_header_bytecode(&self) -> Vec<u8> {
        SectionHeader::new(0, SectionHeader::SHT_NULL, 0, 0, 0, 0, 0, 0, 0, 0).bytecode()
    }
}

impl Section for NullSection {
    // We can use all default implementations from the Section trait
}

#[derive(Debug)]
pub struct ShStrTabSection {
    name: String,
    name_offset: u32,
    section_names: Vec<String>,
    offset: u64,
}

impl ShStrTabSection {
    pub fn new(name_offset: u32, section_names: Vec<String>) -> Self {
        Self {
            name: String::from(".s"),
            name_offset,
            section_names: {
                let mut names = section_names;
                names.push(".s".to_string());
                names
            },
            offset: 0,
        }
    }

    pub fn set_offset(&mut self, offset: u64) {
        self.offset = offset;
    }

    pub fn section_header_bytecode(&self) -> Vec<u8> {
        SectionHeader::new(
            self.name_offset,
            SectionHeader::SHT_STRTAB,
            0,
            0,
            self.offset,
            self.size(),
            0,
            0,
            1,
            0,
        )
        .bytecode()
    }
}

impl Section for ShStrTabSection {
    fn name(&self) -> &str {
        &self.name
    }

    fn bytecode(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        // First byte is null
        bytes.push(0);

        // Add each non-empty section name with null terminator
        for name in &self.section_names {
            if !name.is_empty() {
                bytes.extend(name.as_bytes());
                bytes.push(0); // null terminator
            }
        }

        // Add padding to make size multiple of 8
        while bytes.len() % 8 != 0 {
            bytes.push(0);
        }

        bytes
    }

    fn size(&self) -> u64 {
        // Calculate section header offset
        let mut section_name_size = 0;

        for name in &self.section_names {
            if !name.is_empty() {
                section_name_size += 1 + name.len();
            }
        }

        section_name_size += 1; // null section

        section_name_size as u64 // Return the calculated size
    }
}

#[derive(Debug)]
pub struct DynamicSection {
    name: String,
    name_offset: u32,
    offset: u64,
    link: u32,
    rel_offset: u64,
    rel_size: u64,
    rel_count: u64,
    dynsym_offset: u64,
    dynstr_offset: u64,
    dynstr_size: u64,
}

impl DynamicSection {
    pub fn new(name_offset: u32) -> Self {
        Self {
            name: String::from(".dynamic"),
            name_offset,
            offset: 0,
            link: 0,
            rel_offset: 0,
            rel_size: 0,
            rel_count: 0,
            dynsym_offset: 0,
            dynstr_offset: 0,
            dynstr_size: 0,
        }
    }

    pub fn set_offset(&mut self, offset: u64) {
        self.offset = offset;
    }

    pub fn set_link(&mut self, link: u32) {
        self.link = link;
    }

    pub fn set_rel_offset(&mut self, offset: u64) {
        self.rel_offset = offset;
    }

    pub fn set_rel_size(&mut self, size: u64) {
        self.rel_size = size;
    }

    pub fn set_rel_count(&mut self, count: u64) {
        self.rel_count = count;
    }

    pub fn set_dynsym_offset(&mut self, offset: u64) {
        self.dynsym_offset = offset;
    }

    pub fn set_dynstr_offset(&mut self, offset: u64) {
        self.dynstr_offset = offset;
    }

    pub fn set_dynstr_size(&mut self, size: u64) {
        self.dynstr_size = size;
    }

    pub fn section_header_bytecode(&self) -> Vec<u8> {
        SectionHeader::new(
            self.name_offset,
            SectionHeader::SHT_DYNAMIC,
            SectionHeader::SHF_ALLOC | SectionHeader::SHF_WRITE,
            self.offset,
            self.offset,
            self.size(),
            self.link,
            0,
            8,
            16,
        )
        .bytecode()
    }
}

impl Section for DynamicSection {
    fn name(&self) -> &str {
        &self.name
    }

    fn bytecode(&self) -> Vec<u8> {
        let mut bytes = Vec::new();

        // DT_FLAGS (DF_TEXTREL)
        bytes.extend_from_slice(&0x1e_u64.to_le_bytes());
        bytes.extend_from_slice(&0x04_u64.to_le_bytes());

        // DT_REL
        bytes.extend_from_slice(&0x11_u64.to_le_bytes());
        bytes.extend_from_slice(&self.rel_offset.to_le_bytes());

        // DT_RELSZ
        bytes.extend_from_slice(&0x12_u64.to_le_bytes());
        bytes.extend_from_slice(&self.rel_size.to_le_bytes());

        // DT_RELENT
        bytes.extend_from_slice(&0x13_u64.to_le_bytes());
        bytes.extend_from_slice(&0x10_u64.to_le_bytes()); // Constant: 16 bytes per entry

        // DT_RELCOUNT: number of relative relocation entries
        if self.rel_count > 0 {
            bytes.extend_from_slice(&0x6fff_fffa_u64.to_le_bytes());
            bytes.extend_from_slice(&self.rel_count.to_le_bytes());
        }

        // DT_SYMTAB
        bytes.extend_from_slice(&0x06_u64.to_le_bytes());
        bytes.extend_from_slice(&self.dynsym_offset.to_le_bytes());

        // DT_SYMENT
        bytes.extend_from_slice(&0x0b_u64.to_le_bytes());
        bytes.extend_from_slice(&0x18_u64.to_le_bytes()); // Constant: 24 bytes per symbol

        // DT_STRTAB
        bytes.extend_from_slice(&0x05_u64.to_le_bytes());
        bytes.extend_from_slice(&self.dynstr_offset.to_le_bytes());

        // DT_STRSZ
        bytes.extend_from_slice(&0x0a_u64.to_le_bytes());
        bytes.extend_from_slice(&self.dynstr_size.to_le_bytes());

        // DT_TEXTREL
        bytes.extend_from_slice(&0x16_u64.to_le_bytes());
        bytes.extend_from_slice(&0x00_u64.to_le_bytes());

        // DT_NULL
        bytes.extend_from_slice(&0x00_u64.to_le_bytes());
        bytes.extend_from_slice(&0x00_u64.to_le_bytes());

        bytes
    }

    fn size(&self) -> u64 {
        if self.rel_count > 0 { 11 * 16 } else { 10 * 16 }
    }
}

#[derive(Debug)]
pub struct DynStrSection {
    name: String,
    name_offset: u32,
    symbol_names: Vec<String>,
    offset: u64,
}

impl DynStrSection {
    pub fn new(name_offset: u32, symbol_names: Vec<String>) -> Self {
        Self {
            name: String::from(".dynstr"),
            name_offset,
            symbol_names,
            offset: 0,
        }
    }

    pub fn set_offset(&mut self, offset: u64) {
        self.offset = offset;
    }

    pub fn section_header_bytecode(&self) -> Vec<u8> {
        SectionHeader::new(
            self.name_offset,
            SectionHeader::SHT_STRTAB,
            SectionHeader::SHF_ALLOC, // Allocatable section
            self.offset,
            self.offset,
            self.size(),
            0,
            0,
            1,
            0,
        )
        .bytecode()
    }
}

impl Section for DynStrSection {
    fn name(&self) -> &str {
        &self.name
    }

    fn bytecode(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        // First byte is null
        bytes.push(0);

        // Add each symbol name with null terminator
        for name in &self.symbol_names {
            bytes.extend(name.as_bytes());
            bytes.push(0); // null terminator
        }
        // add padding to make size multiple of 8
        while bytes.len() % 8 != 0 {
            bytes.push(0);
        }
        bytes
    }

    fn size(&self) -> u64 {
        // Calculate total size: initial null byte + sum of (name lengths + null terminators)
        let mut size = 1 + self
            .symbol_names
            .iter()
            .map(|name| name.len() + 1)
            .sum::<usize>();
        // add padding to make size multiple of 8
        while size % 8 != 0 {
            size += 1;
        }
        size as u64
    }
}

#[derive(Debug)]
pub struct DynSymSection {
    name: String,
    name_offset: u32,
    offset: u64,
    link: u32,
    symbols: Vec<DynamicSymbol>,
}

impl DynSymSection {
    pub fn new(name_offset: u32, symbols: Vec<DynamicSymbol>) -> Self {
        Self {
            name: String::from(".dynsym"),
            name_offset,
            offset: 0,
            link: 0,
            symbols,
        }
    }

    pub fn set_offset(&mut self, offset: u64) {
        self.offset = offset;
    }

    pub fn set_link(&mut self, link: u32) {
        self.link = link;
    }

    pub fn section_header_bytecode(&self) -> Vec<u8> {
        let flags = SectionHeader::SHF_ALLOC;
        SectionHeader::new(
            self.name_offset,
            SectionHeader::SHT_DYNSYM,
            flags,
            self.offset,
            self.offset,
            self.size(),
            self.link,
            1,
            8,
            24,
        )
        .bytecode()
    }
}

impl Section for DynSymSection {
    fn name(&self) -> &str {
        &self.name
    }

    fn size(&self) -> u64 {
        // Each symbol entry is 24 bytes
        (self.symbols.len() as u64) * 24
    }

    fn bytecode(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        for symbol in &self.symbols {
            bytes.extend(symbol.bytecode());
        }
        bytes
    }
}

#[derive(Debug)]
pub struct RelDynSection {
    name: String,
    name_offset: u32,
    offset: u64,
    link: u32,
    entries: Vec<RelDyn>,
}

impl RelDynSection {
    pub fn new(name_offset: u32, entries: Vec<RelDyn>) -> Self {
        Self {
            name: String::from(".rel.dyn"),
            name_offset,
            offset: 0,
            link: 0,
            entries,
        }
    }

    pub fn set_offset(&mut self, offset: u64) {
        self.offset = offset;
    }

    pub fn set_link(&mut self, link: u32) {
        self.link = link;
    }

    pub fn size(&self) -> u64 {
        (self.entries.len() * 16) as u64 // Each RelDyn entry is 16 bytes
    }

    pub fn section_header_bytecode(&self) -> Vec<u8> {
        let flags = SectionHeader::SHF_ALLOC;
        SectionHeader::new(
            self.name_offset,
            SectionHeader::SHT_REL,
            flags,
            self.offset,
            self.offset,
            self.size(),
            self.link,
            0,
            8,
            16,
        )
        .bytecode()
    }
}

impl Section for RelDynSection {
    fn name(&self) -> &str {
        &self.name
    }

    fn size(&self) -> u64 {
        self.size()
    }

    fn bytecode(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        for entry in &self.entries {
            bytes.extend(entry.bytecode());
        }
        bytes
    }
}
#[derive(Debug, Clone)]
pub struct DebugSection {
    name: String,
    name_offset: u32,
    data: Vec<u8>,
    offset: u64,
}

impl DebugSection {
    pub fn new(name: &str, name_offset: u32, data: Vec<u8>) -> Self {
        Self {
            name: name.to_string(),
            name_offset,
            data,
            offset: 0,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn size(&self) -> u64 {
        let raw = self.data.len();
        let padding = (8 - (raw % 8)) % 8;
        (raw + padding) as u64
    }

    pub fn bytecode(&self) -> Vec<u8> {
        let mut bytes = self.data.clone();
        while !bytes.len().is_multiple_of(8) {
            bytes.push(0);
        }
        bytes
    }

    pub fn set_offset(&mut self, offset: u64) {
        self.offset = offset;
    }

    pub fn offset(&self) -> u64 {
        self.offset
    }

    pub fn section_header_bytecode(&self) -> Vec<u8> {
        SectionHeader::new(
            self.name_offset,
            SectionHeader::SHT_PROGBITS,
            0,
            0,
            self.offset,
            self.data.len() as u64, // size without padding
            0,
            0,
            1,
            0,
        )
        .bytecode()
    }
}

#[derive(Debug)]
pub enum SectionType {
    Code(CodeSection),
    Data(DataSection),
    ShStrTab(ShStrTabSection),
    Dynamic(DynamicSection),
    DynStr(DynStrSection),
    DynSym(DynSymSection),
    Default(NullSection),
    RelDyn(RelDynSection),
    DebugAbbrev(DebugSection),
    DebugInfo(DebugSection),
    DebugLine(DebugSection),
    DebugLineStr(DebugSection),
}

impl SectionType {
    pub fn name(&self) -> &str {
        match self {
            SectionType::Code(cs) => &cs.name,
            SectionType::Data(ds) => &ds.name,
            SectionType::ShStrTab(ss) => &ss.name,
            SectionType::Dynamic(ds) => &ds.name,
            SectionType::DynStr(ds) => &ds.name,
            SectionType::DynSym(ds) => &ds.name,
            SectionType::Default(ds) => &ds.name,
            SectionType::RelDyn(ds) => &ds.name,
            SectionType::DebugAbbrev(ds) => ds.name(),
            SectionType::DebugInfo(ds) => ds.name(),
            SectionType::DebugLine(ds) => ds.name(),
            SectionType::DebugLineStr(ds) => ds.name(),
        }
    }

    pub fn bytecode(&self) -> Vec<u8> {
        match self {
            SectionType::Code(cs) => cs.bytecode(),
            SectionType::Data(ds) => ds.bytecode(),
            SectionType::ShStrTab(ss) => ss.bytecode(),
            SectionType::Dynamic(ds) => ds.bytecode(),
            SectionType::DynStr(ds) => ds.bytecode(),
            SectionType::DynSym(ds) => ds.bytecode(),
            SectionType::Default(ds) => ds.bytecode(),
            SectionType::RelDyn(ds) => ds.bytecode(),
            SectionType::DebugAbbrev(ds) => ds.bytecode(),
            SectionType::DebugInfo(ds) => ds.bytecode(),
            SectionType::DebugLine(ds) => ds.bytecode(),
            SectionType::DebugLineStr(ds) => ds.bytecode(),
        }
    }

    pub fn size(&self) -> u64 {
        match self {
            SectionType::Code(cs) => cs.size(),
            SectionType::Data(ds) => ds.size(),
            SectionType::ShStrTab(ss) => ss.size(),
            SectionType::Dynamic(ds) => ds.size(),
            SectionType::DynStr(ds) => ds.size(),
            SectionType::DynSym(ds) => ds.size(),
            SectionType::Default(ds) => ds.size(),
            SectionType::RelDyn(ds) => ds.size(),
            SectionType::DebugAbbrev(ds) => ds.size(),
            SectionType::DebugInfo(ds) => ds.size(),
            SectionType::DebugLine(ds) => ds.size(),
            SectionType::DebugLineStr(ds) => ds.size(),
        }
    }

    pub fn section_header_bytecode(&self) -> Vec<u8> {
        match self {
            SectionType::Code(cs) => cs.section_header_bytecode(),
            SectionType::Data(ds) => ds.section_header_bytecode(),
            SectionType::ShStrTab(ss) => ss.section_header_bytecode(),
            SectionType::Dynamic(ds) => ds.section_header_bytecode(),
            SectionType::DynStr(ds) => ds.section_header_bytecode(),
            SectionType::DynSym(ds) => ds.section_header_bytecode(),
            SectionType::Default(ds) => ds.section_header_bytecode(),
            SectionType::RelDyn(ds) => ds.section_header_bytecode(),
            SectionType::DebugAbbrev(ds) => ds.section_header_bytecode(),
            SectionType::DebugInfo(ds) => ds.section_header_bytecode(),
            SectionType::DebugLine(ds) => ds.section_header_bytecode(),
            SectionType::DebugLineStr(ds) => ds.section_header_bytecode(),
        }
    }

    pub fn set_offset(&mut self, offset: u64) {
        match self {
            SectionType::Code(cs) => cs.set_offset(offset),
            SectionType::Data(ds) => ds.set_offset(offset),
            SectionType::ShStrTab(ss) => ss.set_offset(offset),
            SectionType::Dynamic(ds) => ds.set_offset(offset),
            SectionType::DynStr(ds) => ds.set_offset(offset),
            SectionType::DynSym(ds) => ds.set_offset(offset),
            SectionType::RelDyn(ds) => ds.set_offset(offset),
            SectionType::Default(_) => (), // NullSection doesn't need offset
            SectionType::DebugAbbrev(ds) => ds.set_offset(offset),
            SectionType::DebugInfo(ds) => ds.set_offset(offset),
            SectionType::DebugLine(ds) => ds.set_offset(offset),
            SectionType::DebugLineStr(ds) => ds.set_offset(offset),
        }
    }

    pub fn offset(&self) -> u64 {
        match self {
            SectionType::Code(cs) => cs.offset,
            SectionType::Data(ds) => ds.offset,
            SectionType::ShStrTab(ss) => ss.offset,
            SectionType::Dynamic(ds) => ds.offset,
            SectionType::DynStr(ds) => ds.offset,
            SectionType::DynSym(ds) => ds.offset,
            SectionType::Default(ns) => ns.offset,
            SectionType::RelDyn(rs) => rs.offset,
            SectionType::DebugAbbrev(ds) => ds.offset(),
            SectionType::DebugInfo(ds) => ds.offset(),
            SectionType::DebugLine(ds) => ds.offset(),
            SectionType::DebugLineStr(ds) => ds.offset(),
        }
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        sbpf_common::{instruction::Instruction, opcode::Opcode},
    };

    #[test]
    fn test_code_section_new() {
        let inst = Instruction {
            opcode: Opcode::Exit,
            dst: None,
            src: None,
            off: None,
            imm: None,
            span: 0..4,
        };
        let nodes = vec![ASTNode::Instruction {
            instruction: inst,
            offset: 0,
        }];

        let section = CodeSection::new(nodes, 8);
        assert_eq!(section.name(), ".text");
        assert_eq!(section.get_size(), 8);
    }

    #[test]
    fn test_code_section_bytecode() {
        let inst = Instruction {
            opcode: Opcode::Exit,
            dst: None,
            src: None,
            off: None,
            imm: None,
            span: 0..4,
        };
        let nodes = vec![ASTNode::Instruction {
            instruction: inst,
            offset: 0,
        }];

        let section = CodeSection::new(nodes, 8);
        let bytes = section.bytecode();
        assert_eq!(bytes.len(), 8);
    }

    #[test]
    fn test_data_section_new() {
        let rodata = ROData {
            name: "msg".to_string(),
            args: vec![
                Token::Directive("ascii".to_string(), 0..5),
                Token::StringLiteral("Hi".to_string(), 6..10),
            ],
            span: 0..10,
        };
        let nodes = vec![ASTNode::ROData { rodata, offset: 0 }];

        let section = DataSection::new(nodes, 2);
        assert_eq!(section.name(), ".rodata");
        assert_eq!(section.get_size(), 2);
    }

    #[test]
    fn test_data_section_rodata() {
        let rodata = ROData {
            name: "my_str".to_string(),
            args: vec![
                Token::Directive("ascii".to_string(), 0..5),
                Token::StringLiteral("test".to_string(), 6..12),
            ],
            span: 0..12,
        };
        let nodes = vec![ASTNode::ROData { rodata, offset: 0 }];

        let section = DataSection::new(nodes, 4);
        let rodata = section.rodata();
        assert_eq!(rodata.len(), 1);
        assert_eq!(rodata[0].0, "my_str");
        assert_eq!(rodata[0].2, "test");
    }

    #[test]
    fn test_null_section() {
        let section = NullSection::new();
        assert_eq!(section.name(), ".unknown");
        assert_eq!(section.bytecode().len(), 0);
        assert_eq!(section.size(), 0);
    }

    #[test]
    fn test_shstrtab_section() {
        let names = vec![".text".to_string(), ".data".to_string()];
        let mut section = ShStrTabSection::new(10, names);
        section.set_offset(100);

        assert_eq!(section.name(), ".s");
        assert!(section.size() > 0);

        let bytes = section.bytecode();
        assert!(bytes.len() > 0);
        assert_eq!(bytes[0], 0); // First byte is null
    }

    #[test]
    fn test_dynamic_section_setters() {
        let mut section = DynamicSection::new(5);
        section.set_offset(100);
        section.set_link(3);
        section.set_rel_offset(200);
        section.set_rel_size(48);
        section.set_rel_count(2);
        section.set_dynsym_offset(300);
        section.set_dynstr_offset(400);
        section.set_dynstr_size(50);

        assert_eq!(section.name(), ".dynamic");
        assert!(section.size() > 0);
    }

    #[test]
    fn test_dynamic_section_bytecode_with_rel_count() {
        let mut section = DynamicSection::new(0);
        section.set_rel_count(3);
        assert_eq!(section.size(), 11 * 16); // With rel_count
    }

    #[test]
    fn test_dynamic_section_bytecode_without_rel_count() {
        let section = DynamicSection::new(0);
        assert_eq!(section.size(), 10 * 16); // Without rel_count
    }

    #[test]
    fn test_dynstr_section() {
        let names = vec!["entrypoint".to_string(), "function".to_string()];
        let mut section = DynStrSection::new(8, names);
        section.set_offset(200);

        assert_eq!(section.name(), ".dynstr");

        let bytes = section.bytecode();
        assert_eq!(bytes[0], 0);
        assert_eq!(bytes.len() % 8, 0);
    }

    #[test]
    fn test_dynsym_section() {
        let symbols = vec![
            DynamicSymbol::new(0, 0, 0, 0, 0, 0),
            DynamicSymbol::new(1, 0x12, 0, 1, 0x120, 48),
        ];
        let mut section = DynSymSection::new(15, symbols);
        section.set_offset(300);
        section.set_link(4);

        assert_eq!(section.name(), ".dynsym");
        assert_eq!(section.size(), 2 * 24); // 2 symbols * 24 bytes each
    }

    #[test]
    fn test_rel_dyn_section() {
        let entries = vec![RelDyn::new(0x120, 0x08, 0), RelDyn::new(0x200, 0x0a, 1)];
        let mut section = RelDynSection::new(18, entries);
        section.set_offset(400);
        section.set_link(3);

        assert_eq!(section.name(), ".rel.dyn");
        assert_eq!(section.size(), 2 * 16); // 2 entries * 16 bytes each
    }

    #[test]
    fn test_section_type_enum_code() {
        let inst = Instruction {
            opcode: Opcode::Exit,
            dst: None,
            src: None,
            off: None,
            imm: None,
            span: 0..4,
        };
        let code_section = CodeSection::new(
            vec![ASTNode::Instruction {
                instruction: inst,
                offset: 0,
            }],
            8,
        );

        let section_type = SectionType::Code(code_section);
        assert_eq!(section_type.name(), ".text");
        assert_eq!(section_type.size(), 8);
    }

    #[test]
    fn test_section_type_set_offset() {
        let mut section = SectionType::Default(NullSection::new());
        section.set_offset(100);

        let mut dyn_section = SectionType::Dynamic(DynamicSection::new(0));
        dyn_section.set_offset(200);
        assert_eq!(dyn_section.offset(), 200);
    }

    #[test]
    fn test_debug_section_types() {
        let sections = vec![
            SectionType::DebugAbbrev(DebugSection::new(".debug_abbrev", 0, vec![1, 2, 3])),
            SectionType::DebugInfo(DebugSection::new(".debug_info", 0, vec![4, 5, 6])),
            SectionType::DebugLine(DebugSection::new(".debug_line", 0, vec![7, 8, 9])),
            SectionType::DebugLineStr(DebugSection::new(".debug_line_str", 0, vec![10, 11, 12])),
        ];

        let expected_names = [
            ".debug_abbrev",
            ".debug_info",
            ".debug_line",
            ".debug_line_str",
        ];
        for (i, section) in sections.iter().enumerate() {
            assert_eq!(section.name(), expected_names[i]);
            assert_eq!(section.size(), 8);
            assert_eq!(section.bytecode().len(), 8);
            assert!(!section.section_header_bytecode().is_empty());
        }
    }
}
