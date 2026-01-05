use {
    crate::section::DebugSection,
    gimli::{
        DW_AT_comp_dir, DW_AT_decl_file, DW_AT_decl_line, DW_AT_high_pc, DW_AT_language,
        DW_AT_low_pc, DW_AT_name, DW_AT_producer, DW_AT_stmt_list, DW_LANG_Mips_Assembler,
        DW_TAG_label, Encoding, Format, LineEncoding, LittleEndian, SectionId,
        write::{Address, AttributeValue, DwarfUnit, EndianVec, LineProgram, LineString, Sections},
    },
};

const SBPF_INSTRUCTION_LENGTH: u8 = 8;

#[derive(Debug, Clone)]
pub struct DebugData {
    pub filename: String,
    pub directory: String,
    pub lines: Vec<(u64, u32)>,
    pub labels: Vec<(String, u64, u32)>,
    pub code_start: u64,
    pub code_end: u64,
}

/// Generate DebugSections from debug data
pub fn generate_debug_sections(
    data: &DebugData,
    text_offset: u64,
    section_names: &mut Vec<String>,
    current_offset: &mut u64,
) -> Vec<DebugSection> {
    let code_start = data.code_start + text_offset;
    let code_end = data.code_end + text_offset;
    let mut dwarf = generate_dwarf_sections(data, text_offset, code_start, code_end);

    let calc_name_offset = |names: &Vec<String>| -> u32 {
        (names.iter().map(|n| n.len() + 1).sum::<usize>() + 1) as u32
    };

    let mut sections = Vec::new();

    // .debug_abbrev section
    let mut abbrev = DebugSection::new(
        SectionId::DebugAbbrev.name(),
        calc_name_offset(section_names),
        dwarf.debug_abbrev.take(),
    );
    section_names.push(SectionId::DebugAbbrev.name().to_string());
    abbrev.set_offset(*current_offset);
    *current_offset += abbrev.size();
    sections.push(abbrev);

    // .debug_info section
    let mut info = DebugSection::new(
        SectionId::DebugInfo.name(),
        calc_name_offset(section_names),
        dwarf.debug_info.take(),
    );
    section_names.push(SectionId::DebugInfo.name().to_string());
    info.set_offset(*current_offset);
    *current_offset += info.size();
    sections.push(info);

    // .debug_line section
    let mut line = DebugSection::new(
        SectionId::DebugLine.name(),
        calc_name_offset(section_names),
        dwarf.debug_line.take(),
    );
    section_names.push(SectionId::DebugLine.name().to_string());
    line.set_offset(*current_offset);
    *current_offset += line.size();
    sections.push(line);

    // .debug_line_str section
    let mut line_str = DebugSection::new(
        SectionId::DebugLineStr.name(),
        calc_name_offset(section_names),
        dwarf.debug_line_str.take(),
    );
    section_names.push(SectionId::DebugLineStr.name().to_string());
    line_str.set_offset(*current_offset);
    *current_offset += line_str.size();
    sections.push(line_str);

    sections
}

// Generate DWARF sections using gimli
fn generate_dwarf_sections(
    data: &DebugData,
    text_offset: u64,
    code_start: u64,
    code_end: u64,
) -> Sections<EndianVec<LittleEndian>> {
    let encoding = Encoding {
        format: Format::Dwarf32,
        version: 5,
        address_size: 8,
    };

    let line_encoding = LineEncoding {
        minimum_instruction_length: SBPF_INSTRUCTION_LENGTH,
        ..LineEncoding::default()
    };

    let mut dwarf = DwarfUnit::new(encoding);

    // Add strings.
    let dir_string_id = dwarf.line_strings.add(data.directory.clone().into_bytes());
    let file_string_id = dwarf.line_strings.add(data.filename.clone().into_bytes());

    // Create line program.
    let mut line_program = LineProgram::new(
        encoding,
        line_encoding,
        LineString::LineStringRef(dir_string_id),
        None,
        LineString::LineStringRef(file_string_id),
        None,
    );

    let dir_id = line_program.default_directory();
    let file_id = line_program.add_file(LineString::LineStringRef(file_string_id), dir_id, None);

    // Add line entries.
    line_program.begin_sequence(Some(Address::Constant(code_start)));
    for (address, line) in &data.lines {
        let adjusted_addr = address + text_offset;
        line_program.row().file = file_id;
        line_program.row().address_offset = adjusted_addr - code_start;
        line_program.row().line = *line as u64;
        line_program.generate_row();
    }
    line_program.end_sequence(code_end);

    dwarf.unit.line_program = line_program;

    // Set compile unit attributes.
    let root_id = dwarf.unit.root();
    let root = dwarf.unit.get_mut(root_id);

    root.set(
        DW_AT_name,
        AttributeValue::String(data.filename.clone().into_bytes()),
    );
    root.set(
        DW_AT_comp_dir,
        AttributeValue::String(data.directory.clone().into_bytes()),
    );
    root.set(
        DW_AT_producer,
        AttributeValue::String(b"sbpf-assembler".to_vec()),
    );
    root.set(
        DW_AT_language,
        AttributeValue::Language(DW_LANG_Mips_Assembler),
    );
    root.set(
        DW_AT_low_pc,
        AttributeValue::Address(Address::Constant(code_start)),
    );
    root.set(
        DW_AT_high_pc,
        AttributeValue::Address(Address::Constant(code_end)),
    );
    root.set(DW_AT_stmt_list, AttributeValue::LineProgramRef);

    // Set labels.
    for (name, address, line) in &data.labels {
        let adjusted_addr = address + text_offset;
        let label_id = dwarf.unit.add(root_id, DW_TAG_label);
        let label_die = dwarf.unit.get_mut(label_id);

        label_die.set(
            DW_AT_name,
            AttributeValue::String(name.clone().into_bytes()),
        );
        label_die.set(DW_AT_decl_file, AttributeValue::Data4(0));
        label_die.set(DW_AT_decl_line, AttributeValue::Data4(*line));
        label_die.set(
            DW_AT_low_pc,
            AttributeValue::Address(Address::Constant(adjusted_addr)),
        );
    }

    // Write sections.
    let mut sections = Sections::new(EndianVec::new(LittleEndian));
    dwarf.write(&mut sections).expect("Failed to write DWARF");
    sections
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_debug_sections() {
        let mut section_names = vec![".text".to_string()];
        let mut offset = 100u64;

        let data = DebugData {
            filename: "test.s".to_string(),
            directory: "/tmp".to_string(),
            lines: vec![(0, 5), (8, 6)],
            labels: vec![("entrypoint".to_string(), 0, 4)],
            code_start: 0,
            code_end: 16,
        };

        let sections = generate_debug_sections(&data, 0x100, &mut section_names, &mut offset);

        assert_eq!(sections.len(), 4);
        assert_eq!(sections[0].name(), SectionId::DebugAbbrev.name());
        assert_eq!(sections[1].name(), SectionId::DebugInfo.name());
        assert_eq!(sections[2].name(), SectionId::DebugLine.name());
        assert_eq!(sections[3].name(), SectionId::DebugLineStr.name());

        for section in &sections {
            assert!(!section.bytecode().is_empty());
        }
    }
}
