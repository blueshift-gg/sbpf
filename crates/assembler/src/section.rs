use {
    crate::{
        astnode::{ASTNode, ROData},
        debuginfo::DebugInfo,
        dynsym::{DynamicSymbol, RelDyn},
        header::SectionHeader,
        lexer::Token,
    },
    sbpf_common::platform::BpfPlatform,
    std::collections::HashMap,
};

// Base Section trait
pub trait Section {
    fn name(&self) -> &str {
        ".unknown" // Default section name
    }

    fn bytecode<Platform: BpfPlatform>(&self) -> Vec<u8> {
        Vec::new() // Default empty bytecode
    }

    // fn get_size(&self) -> u64
    fn size(&self) -> u64 {
        // Note: size cannot call bytecode() anymore since it's generic
        // Implementations must override size() directly
        0
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
    debug_map: HashMap<u64, DebugInfo>,
}

impl CodeSection {
    pub fn new<Platform: BpfPlatform>(nodes: Vec<ASTNode>, size: u64) -> Self {
        let mut debug_map = HashMap::new();
        for node in &nodes {
            if let Some((_, node_debug_map)) = node.bytecode_with_debug_map::<Platform>() {
                debug_map.extend(node_debug_map);
            }
        }
        Self {
            name: String::from(".text"),
            nodes,
            size,
            offset: 0,
            debug_map,
        }
    }

    pub fn get_nodes(&self) -> &Vec<ASTNode> {
        &self.nodes
    }

    pub fn get_size(&self) -> u64 {
        self.size
    }

    pub fn get_debug_map(&self) -> &HashMap<u64, DebugInfo> {
        &self.debug_map
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

    fn bytecode<Platform: BpfPlatform>(&self) -> Vec<u8> {
        let mut bytecode = Vec::new();
        for node in &self.nodes {
            if let Some(node_bytes) = node.bytecode::<Platform>() {
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
        self.size
    }

    fn bytecode<Platform: BpfPlatform>(&self) -> Vec<u8> {
        let mut bytecode = Vec::new();
        for node in &self.nodes {
            if let Some(node_bytes) = node.bytecode::<Platform>() {
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

    fn bytecode<Platform: BpfPlatform>(&self) -> Vec<u8> {
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
            5,
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

    fn bytecode<Platform: BpfPlatform>(&self) -> Vec<u8> {
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

    fn bytecode<Platform: BpfPlatform>(&self) -> Vec<u8> {
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
    symbols: Vec<DynamicSymbol>,
}

impl DynSymSection {
    pub fn new(name_offset: u32, symbols: Vec<DynamicSymbol>) -> Self {
        Self {
            name: String::from(".dynsym"),
            name_offset,
            offset: 0,
            symbols,
        }
    }

    pub fn set_offset(&mut self, offset: u64) {
        self.offset = offset;
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
            5,
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

    fn bytecode<Platform: BpfPlatform>(&self) -> Vec<u8> {
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
    entries: Vec<RelDyn>,
}

impl RelDynSection {
    pub fn new(name_offset: u32, entries: Vec<RelDyn>) -> Self {
        Self {
            name: String::from(".rel.dyn"),
            name_offset,
            offset: 0,
            entries,
        }
    }

    pub fn set_offset(&mut self, offset: u64) {
        self.offset = offset;
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
            4,
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

    fn bytecode<Platform: BpfPlatform>(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        for entry in &self.entries {
            bytes.extend(entry.bytecode());
        }
        bytes
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
        }
    }

    pub fn bytecode<Platform: BpfPlatform>(&self) -> Vec<u8> {
        match self {
            SectionType::Code(cs) => cs.bytecode::<Platform>(),
            SectionType::Data(ds) => ds.bytecode::<Platform>(),
            SectionType::ShStrTab(ss) => ss.bytecode::<Platform>(),
            SectionType::Dynamic(ds) => ds.bytecode::<Platform>(),
            SectionType::DynStr(ds) => ds.bytecode::<Platform>(),
            SectionType::DynSym(ds) => ds.bytecode::<Platform>(),
            SectionType::Default(ds) => ds.bytecode::<Platform>(),
            SectionType::RelDyn(ds) => ds.bytecode::<Platform>(),
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
        }
    }
}
