use {
    crate::{
        debug::{self, DebugData},
        debuginfo::DebugInfo,
        dynsym::{DynamicSymbol, RelDyn, RelocationType},
        header::{ElfHeader, ProgramHeader},
        parser::ParseResult,
        section::{
            DynStrSection, DynSymSection, DynamicSection, NullSection, RelDynSection, Section,
            SectionType, ShStrTabSection,
        },
    },
    std::{collections::HashMap, fs::File, io::Write, path::Path},
};

#[derive(Debug)]
pub struct Program {
    pub elf_header: ElfHeader,
    pub program_headers: Option<Vec<ProgramHeader>>,
    pub sections: Vec<SectionType>,
}

impl Program {
    pub fn from_parse_result(
        ParseResult {
            code_section,
            data_section,
            dynamic_symbols,
            relocation_data,
            prog_is_static,
        }: ParseResult,
        debug_data: Option<DebugData>,
    ) -> Self {
        let mut elf_header = ElfHeader::new();
        let mut program_headers = None;

        // omit program headers if static
        let ph_count = if prog_is_static { 0 } else { 3 };
        elf_header.e_phnum = ph_count;

        // save read + execute size for program header before
        // ownership of code/data sections is transferred
        let text_size = code_section.size() + data_section.size();

        // Calculate base offset after ELF header and program headers
        let mut current_offset = 64 + (ph_count as u64 * 56); // 64 bytes ELF header, 56 bytes per program header

        let text_offset = current_offset;

        // Get the entry point offset from dynamic_symbols if available
        let entry_point_offset = dynamic_symbols
            .get_entry_points()
            .first()
            .map(|(_, offset)| *offset)
            .unwrap_or(0);

        elf_header.e_entry = text_offset + entry_point_offset;

        // Create a vector of sections
        let mut sections = Vec::new();
        sections.push(SectionType::Default(NullSection::new()));

        let mut section_names = Vec::new();

        // Code section
        let mut text_section = SectionType::Code(code_section);
        text_section.set_offset(current_offset);
        current_offset += text_section.size();
        section_names.push(text_section.name().to_string());
        sections.push(text_section);

        // Data section
        if data_section.size() > 0 {
            let mut rodata_section = SectionType::Data(data_section);
            rodata_section.set_offset(current_offset);
            current_offset += rodata_section.size();
            section_names.push(rodata_section.name().to_string());
            sections.push(rodata_section);
        }

        let padding = (8 - (current_offset % 8)) % 8;
        current_offset += padding;

        if !prog_is_static {
            let mut symbol_names = Vec::new();
            let mut dyn_syms = Vec::new();
            let mut dyn_str_offset = 1;

            dyn_syms.push(DynamicSymbol::new(0, 0, 0, 0, 0, 0));

            // all symbols handled right now are all global symbols
            for (name, _) in dynamic_symbols.get_entry_points() {
                symbol_names.push(name.clone());
                dyn_syms.push(DynamicSymbol::new(
                    dyn_str_offset as u32,
                    0x10,
                    0,
                    1,
                    elf_header.e_entry,
                    0,
                ));
                dyn_str_offset += name.len() + 1;
            }

            for (name, _) in dynamic_symbols.get_call_targets() {
                symbol_names.push(name.clone());
                dyn_syms.push(DynamicSymbol::new(dyn_str_offset as u32, 0x10, 0, 0, 0, 0));
                dyn_str_offset += name.len() + 1;
            }

            let mut rel_count = 0;
            let mut rel_dyns = Vec::new();
            for (offset, rel_type, name) in relocation_data.get_rel_dyns() {
                if rel_type == RelocationType::RSbfSyscall {
                    if let Some(index) = symbol_names.iter().position(|n| *n == name) {
                        rel_dyns.push(RelDyn::new(
                            offset + text_offset,
                            rel_type as u64,
                            index as u64 + 1,
                        ));
                    } else {
                        panic!("Symbol {} not found in symbol_names", name);
                    }
                } else if rel_type == RelocationType::RSbf64Relative {
                    rel_count += 1;
                    rel_dyns.push(RelDyn::new(offset + text_offset, rel_type as u64, 0));
                }
            }
            // create four dynamic related sections
            let mut dynamic_section = SectionType::Dynamic(DynamicSection::new(
                (section_names
                    .iter()
                    .map(|name| name.len() + 1)
                    .sum::<usize>()
                    + 1) as u32,
            ));
            section_names.push(dynamic_section.name().to_string());

            let mut dynsym_section = SectionType::DynSym(DynSymSection::new(
                (section_names
                    .iter()
                    .map(|name| name.len() + 1)
                    .sum::<usize>()
                    + 1) as u32,
                dyn_syms,
            ));
            section_names.push(dynsym_section.name().to_string());

            let mut dynstr_section = SectionType::DynStr(DynStrSection::new(
                (section_names
                    .iter()
                    .map(|name| name.len() + 1)
                    .sum::<usize>()
                    + 1) as u32,
                symbol_names,
            ));
            section_names.push(dynstr_section.name().to_string());

            let mut rel_dyn_section = SectionType::RelDyn(RelDynSection::new(
                (section_names
                    .iter()
                    .map(|name| name.len() + 1)
                    .sum::<usize>()
                    + 1) as u32,
                rel_dyns,
            ));
            section_names.push(rel_dyn_section.name().to_string());

            dynamic_section.set_offset(current_offset);
            if let SectionType::Dynamic(ref mut dynamic_section) = dynamic_section {
                // link to .dynstr
                dynamic_section.set_link(
                    section_names
                        .iter()
                        .position(|name| name == ".dynstr")
                        .expect("missing .dynstr section") as u32
                        + 1,
                );
                dynamic_section.set_rel_count(rel_count);
            }
            current_offset += dynamic_section.size();

            dynsym_section.set_offset(current_offset);
            if let SectionType::DynSym(ref mut dynsym_section) = dynsym_section {
                // link to .dynstr
                dynsym_section.set_link(
                    section_names
                        .iter()
                        .position(|name| name == ".dynstr")
                        .expect("missing .dynstr section") as u32
                        + 1,
                );
            }
            current_offset += dynsym_section.size();

            dynstr_section.set_offset(current_offset);
            current_offset += dynstr_section.size();

            rel_dyn_section.set_offset(current_offset);
            if let SectionType::RelDyn(ref mut rel_dyn_section) = rel_dyn_section {
                // link to .dynsym
                rel_dyn_section.set_link(
                    section_names
                        .iter()
                        .position(|name| name == ".dynsym")
                        .expect("missing .dynsym section") as u32
                        + 1,
                );
            }
            current_offset += rel_dyn_section.size();

            if let SectionType::Dynamic(ref mut dynamic_section) = dynamic_section {
                dynamic_section.set_rel_offset(rel_dyn_section.offset());
                dynamic_section.set_rel_size(rel_dyn_section.size());
                dynamic_section.set_dynsym_offset(dynsym_section.offset());
                dynamic_section.set_dynstr_offset(dynstr_section.offset());
                dynamic_section.set_dynstr_size(dynstr_section.size());
            }

            // Generate debug sections
            let debug_sections = Self::generate_debug_sections(
                &debug_data,
                text_offset,
                &mut section_names,
                &mut current_offset,
            );

            let mut shstrtab_section = SectionType::ShStrTab(ShStrTabSection::new(
                (section_names
                    .iter()
                    .map(|name| name.len() + 1)
                    .sum::<usize>()
                    + 1) as u32,
                section_names,
            ));
            shstrtab_section.set_offset(current_offset);
            current_offset += shstrtab_section.size();

            program_headers = Some(vec![
                ProgramHeader::new_load(
                    text_offset,
                    text_size,
                    true, // executable
                ),
                ProgramHeader::new_load(
                    dynsym_section.offset(),
                    dynsym_section.size() + dynstr_section.size() + rel_dyn_section.size(),
                    false,
                ),
                ProgramHeader::new_dynamic(dynamic_section.offset(), dynamic_section.size()),
            ]);

            sections.push(dynamic_section);
            sections.push(dynsym_section);
            sections.push(dynstr_section);
            sections.push(rel_dyn_section);

            for debug_section in debug_sections {
                sections.push(debug_section);
            }

            sections.push(shstrtab_section);
        } else {
            // Create a vector of section names
            let mut section_names = Vec::new();
            for section in &sections {
                section_names.push(section.name().to_string());
            }

            // Generate debug sections
            let debug_sections = Self::generate_debug_sections(
                &debug_data,
                text_offset,
                &mut section_names,
                &mut current_offset,
            );

            for debug_section in debug_sections {
                sections.push(debug_section);
            }

            let mut shstrtab_section = ShStrTabSection::new(
                section_names
                    .iter()
                    .map(|name| name.len() + 1)
                    .sum::<usize>() as u32,
                section_names,
            );
            shstrtab_section.set_offset(current_offset);
            current_offset += shstrtab_section.size();
            sections.push(SectionType::ShStrTab(shstrtab_section));
        }

        // Update section header offset in ELF header
        let padding = (8 - (current_offset % 8)) % 8;
        elf_header.e_shoff = current_offset + padding;
        elf_header.e_shnum = sections.len() as u16;
        elf_header.e_shstrndx = sections.len() as u16 - 1;

        Self {
            elf_header,
            program_headers,
            sections,
        }
    }

    pub fn emit_bytecode(&self) -> Vec<u8> {
        let mut bytes = Vec::new();

        // Emit ELF Header bytes
        bytes.extend(self.elf_header.bytecode());

        // Emit program headers
        if self.program_headers.is_some() {
            for ph in self.program_headers.as_ref().unwrap() {
                bytes.extend(ph.bytecode());
            }
        }

        // Emit sections
        for section in &self.sections {
            bytes.extend(section.bytecode());
        }

        // Emit section headers
        for section in &self.sections {
            bytes.extend(section.section_header_bytecode());
        }

        bytes
    }

    fn generate_debug_sections(
        debug_data: &Option<DebugData>,
        text_offset: u64,
        section_names: &mut Vec<String>,
        current_offset: &mut u64,
    ) -> Vec<SectionType> {
        if let Some(data) = debug_data {
            debug::generate_debug_sections(data, text_offset, section_names, current_offset)
                .into_iter()
                .enumerate()
                .map(|(i, s)| match i {
                    0 => SectionType::DebugAbbrev(s),
                    1 => SectionType::DebugInfo(s),
                    2 => SectionType::DebugLine(s),
                    3 => SectionType::DebugLineStr(s),
                    _ => unreachable!(),
                })
                .collect()
        } else {
            Vec::new()
        }
    }

    pub fn has_rodata(&self) -> bool {
        self.sections.iter().any(|s| s.name() == ".rodata")
    }

    pub fn parse_rodata(&self) -> Vec<(String, usize, String)> {
        let rodata = self
            .sections
            .iter()
            .find(|s| s.name() == ".rodata")
            .unwrap();
        if let SectionType::Data(data_section) = rodata {
            data_section.rodata()
        } else {
            panic!("ROData section not found");
        }
    }

    pub fn get_debug_map(&self) -> HashMap<u64, DebugInfo> {
        let code = self.sections.iter().find(|s| s.name() == ".text").unwrap();
        if let SectionType::Code(code_section) = code {
            code_section.get_debug_map().clone()
        } else {
            panic!("Code section not found");
        }
    }

    pub fn save_to_file(&self, input_path: &str) -> std::io::Result<()> {
        // Get the file stem (name without extension) from input path
        let path = Path::new(input_path);
        let file_stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("output");

        // Create the output file name with .so extension
        let output_path = format!("{}.so", file_stem);

        // Get the bytecode
        let bytes = self.emit_bytecode();

        // Write bytes to file
        let mut file = File::create(output_path)?;
        file.write_all(&bytes)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use {super::*, crate::parser::parse};

    #[test]
    fn test_program_from_simple_source() {
        let source = "exit";
        let parse_result = parse(source).unwrap();
        let program = Program::from_parse_result(parse_result, None);

        // Verify basic structure
        assert!(!program.sections.is_empty());
        assert!(program.sections.len() >= 2);
    }

    #[test]
    fn test_program_without_rodata() {
        let source = "exit";
        let parse_result = parse(source).unwrap();
        let program = Program::from_parse_result(parse_result, None);

        assert!(!program.has_rodata());
    }

    #[test]
    fn test_program_emit_bytecode() {
        let source = "exit";
        let parse_result = parse(source).unwrap();
        let program = Program::from_parse_result(parse_result, None);

        let bytecode = program.emit_bytecode();
        assert!(!bytecode.is_empty());
        // Should start with ELF magic
        assert_eq!(&bytecode[0..4], b"\x7fELF");
    }

    #[test]
    fn test_program_get_debug_map() {
        let source = "exit";
        let parse_result = parse(source).unwrap();
        let program = Program::from_parse_result(parse_result, None);

        let debug_map = program.get_debug_map();
        assert!(!debug_map.is_empty());
    }

    #[test]
    fn test_program_static_no_program_headers() {
        // Create a static program (no dynamic symbols)
        let source = "exit";
        let mut parse_result = parse(source).unwrap();
        parse_result.prog_is_static = true;

        let program = Program::from_parse_result(parse_result, None);
        assert!(program.program_headers.is_none());
        assert_eq!(program.elf_header.e_phnum, 0);
    }

    #[test]
    fn test_program_sections_ordering() {
        let source = "exit";
        let parse_result = parse(source).unwrap();
        let program = Program::from_parse_result(parse_result, None);

        // First section should be null
        assert_eq!(program.sections[0].name(), "");
        // Second should be .text
        assert_eq!(program.sections[1].name(), ".text");
    }
}
