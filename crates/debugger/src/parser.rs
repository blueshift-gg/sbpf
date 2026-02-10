use {
    crate::error::{DebuggerError, DebuggerResult},
    gimli::{EndianSlice, RunTimeEndian, SectionId},
    object::{Object, ObjectSection},
    sbpf_vm::memory::Memory,
    std::{borrow::Cow, collections::HashMap, path::PathBuf},
};

#[derive(Debug, Clone)]
pub struct RODataSymbol {
    pub name: String,
    pub address: u64,
    pub content: String,
}

pub fn rodata_from_section(
    section: &sbpf_disassembler::rodata::RodataSection,
) -> Vec<RODataSymbol> {
    section
        .items
        .iter()
        .map(|item| RODataSymbol {
            name: item.label.clone(),
            address: Memory::RODATA_START + item.offset,
            content: item.data_type.to_asm(),
        })
        .collect()
}

#[derive(Debug, Clone)]
pub struct SourceLocation {
    pub file: String,
    pub line: u32,
    pub column: u32,
    _address: u64,
}

#[derive(Clone)]
pub struct LineMap {
    address_to_line: HashMap<u64, usize>,
    line_to_addresses: HashMap<usize, Vec<u64>>,
    source_locations: HashMap<u64, SourceLocation>,
    files: Vec<String>,
    text_offset: u64,
}

impl Default for LineMap {
    fn default() -> Self {
        Self::new()
    }
}

impl LineMap {
    pub fn new() -> Self {
        Self {
            address_to_line: HashMap::new(),
            line_to_addresses: HashMap::new(),
            source_locations: HashMap::new(),
            files: Vec::new(),
            text_offset: 0,
        }
    }

    pub fn from_elf_file(file_path: &str) -> DebuggerResult<Self> {
        let file_data = std::fs::read(file_path)?;
        Self::from_elf_data(&file_data)
    }

    pub fn from_elf_data(file_data: &[u8]) -> DebuggerResult<Self> {
        let object = object::File::parse(file_data)?;
        let mut line_map = Self::new();
        line_map.text_offset = object
            .section_by_name(".text")
            .map(|section| section.address())
            .unwrap_or(0);
        line_map.parse_debug_info_from_object(&object)?;
        Ok(line_map)
    }

    fn parse_debug_info_from_object(&mut self, obj_file: &object::File) -> DebuggerResult<()> {
        let endian = if obj_file.is_little_endian() {
            RunTimeEndian::Little
        } else {
            RunTimeEndian::Big
        };

        let load_section = |id: SectionId| -> Result<Cow<[u8]>, gimli::Error> {
            match obj_file.section_by_name(id.name()) {
                Some(section) => match section.uncompressed_data() {
                    Ok(data) => Ok(data),
                    Err(_) => Ok(Cow::Borrowed(&[])),
                },
                None => Ok(Cow::Borrowed(&[])),
            }
        };

        let borrow_section = |section| EndianSlice::new(Cow::as_ref(section), endian);

        let dwarf_sections =
            gimli::DwarfSections::load(&load_section).map_err(DebuggerError::Dwarf)?;
        let dwarf = dwarf_sections.borrow(borrow_section);

        let mut iter = dwarf.units();
        while let Some(header) = iter.next().map_err(DebuggerError::Dwarf)? {
            let unit = dwarf.unit(header).map_err(DebuggerError::Dwarf)?;
            let unit = unit.unit_ref(&dwarf);

            if let Some(program) = unit.line_program.clone() {
                let comp_dir = if let Some(ref dir) = unit.comp_dir {
                    PathBuf::from(dir.to_string_lossy().into_owned())
                } else {
                    PathBuf::new()
                };

                let mut rows = program.rows();
                while let Some((header, row)) = rows.next_row().map_err(DebuggerError::Dwarf)? {
                    if !row.end_sequence() {
                        let mut file_path = String::new();
                        if let Some(file) = row.file(header) {
                            let mut path = PathBuf::new();
                            path.clone_from(&comp_dir);

                            if file.directory_index() != 0
                                && let Some(dir) = file.directory(header)
                            {
                                path.push(
                                    unit.attr_string(dir)
                                        .map_err(DebuggerError::Dwarf)?
                                        .to_string_lossy()
                                        .as_ref(),
                                );
                            }

                            path.push(
                                unit.attr_string(file.path_name())
                                    .map_err(DebuggerError::Dwarf)?
                                    .to_string_lossy()
                                    .as_ref(),
                            );
                            file_path = path.to_string_lossy().to_string();
                        }

                        let line = match row.line() {
                            Some(line) => line.get() as u32,
                            None => 0,
                        };
                        let column = match row.column() {
                            gimli::ColumnType::LeftEdge => 0,
                            gimli::ColumnType::Column(column) => column.get() as u32,
                        };

                        let address = row.address();

                        self.address_to_line.insert(address, line as usize);
                        self.line_to_addresses
                            .entry(line as usize)
                            .or_default()
                            .push(address);

                        let source_loc = SourceLocation {
                            file: file_path.clone(),
                            line,
                            column,
                            _address: address,
                        };
                        self.source_locations.insert(address, source_loc);

                        if !file_path.is_empty() && !self.files.contains(&file_path) {
                            self.files.push(file_path);
                        }
                    }
                }
            }
        }

        Ok(())
    }

    pub fn get_line_for_address(&self, address: u64) -> Option<usize> {
        self.address_to_line.get(&address).copied()
    }

    pub fn get_addresses_for_line(&self, line: usize) -> Option<&[u64]> {
        self.line_to_addresses.get(&line).map(|v| v.as_slice())
    }

    pub fn get_line_for_pc(&self, pc: u64) -> Option<usize> {
        let address = pc.saturating_add(self.text_offset);
        self.get_line_for_address(address)
    }

    pub fn get_pcs_for_line(&self, line: usize) -> Vec<u64> {
        if let Some(addresses) = self.get_addresses_for_line(line) {
            addresses
                .iter()
                .map(|addr| addr.saturating_sub(self.text_offset))
                .collect()
        } else {
            Vec::new()
        }
    }

    pub fn get_source_location(&self, pc: u64) -> Option<&SourceLocation> {
        let address = pc.saturating_add(self.text_offset);
        self.source_locations.get(&address)
    }

    pub fn get_line_to_addresses(&self) -> &HashMap<usize, Vec<u64>> {
        &self.line_to_addresses
    }

    pub fn get_line_to_pcs(&self) -> HashMap<usize, Vec<u64>> {
        self.line_to_addresses
            .iter()
            .map(|(line, addrs)| {
                (
                    *line,
                    addrs
                        .iter()
                        .map(|addr| addr.saturating_sub(self.text_offset))
                        .collect(),
                )
            })
            .collect()
    }
}
