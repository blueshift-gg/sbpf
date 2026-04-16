use {
    crate::section::{DebugSection, SectionType},
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
    /// Multi-file line entries: `(offset, filename, directory, line)`.
    /// When non-empty, used instead of `lines` for DWARF emission so the
    /// step debugger can map each instruction back to the `.s` file that
    /// contained it (including included files).
    pub lines_multi: Vec<(u64, String, String, u32)>,
    /// Multi-file label entries: `(name, offset, filename, directory, line)`.
    /// When non-empty, used instead of `labels` for DWARF emission.
    pub labels_multi: Vec<(String, u64, String, String, u32)>,
    pub code_start: u64,
    pub code_end: u64,
}

fn calc_name_offset(names: &[String]) -> u32 {
    (names
        .iter()
        .filter(|n| !n.is_empty())
        .map(|n| n.len() + 1)
        .sum::<usize>()
        + 1) as u32
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

// Generate DWARF sections using gimli.
//
// If `data.lines_multi` / `data.labels_multi` are non-empty, the line
// program is built with one DWARF file entry per distinct source file,
// so the step debugger can resolve each instruction to the correct
// included `.s` file. Otherwise the single-file path is used.
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

    let use_multi = !data.lines_multi.is_empty();

    // Build the line program. In multi-file mode we register every
    // `(directory, filename)` referenced by `lines_multi` and emit rows
    // that point at the right file id.
    let main_dir_string_id = dwarf.line_strings.add(data.directory.clone().into_bytes());
    let main_file_string_id = dwarf.line_strings.add(data.filename.clone().into_bytes());
    let mut line_program = LineProgram::new(
        encoding,
        line_encoding,
        LineString::LineStringRef(main_dir_string_id),
        None,
        LineString::LineStringRef(main_file_string_id),
        None,
    );

    // Map from (filename, directory) to the raw DWARF file index.
    // Built during line program registration and reused for
    // DW_AT_decl_file on label DIEs. The main file is always index 0
    // in DWARF v5; files added via `add_file` get indices 1, 2, ...
    let mut file_index_map: std::collections::HashMap<(String, String), u32> =
        std::collections::HashMap::new();

    if use_multi {
        use std::collections::HashMap;

        // Pre-register the main file at index 0 — the LineProgram
        // constructor already set it as the primary source file.
        file_index_map.insert((data.filename.clone(), data.directory.clone()), 0);

        let mut dir_ids: HashMap<String, gimli::write::DirectoryId> = HashMap::new();
        let mut file_ids: HashMap<(String, String), gimli::write::FileId> = HashMap::new();
        let mut next_file_idx = 1u32;

        // Register all (dir, file) entries from both lines_multi and
        // labels_multi so that label-only files get a DWARF file index.
        let all_file_refs = data
            .lines_multi
            .iter()
            .map(|(_, f, d, _)| (f, d))
            .chain(data.labels_multi.iter().map(|(_, _, f, d, _)| (f, d)));

        for (filename, directory) in all_file_refs {
            let key = (filename.clone(), directory.clone());
            let dir_id = *dir_ids.entry(directory.clone()).or_insert_with(|| {
                let id = dwarf.line_strings.add(directory.clone().into_bytes());
                line_program.add_directory(LineString::LineStringRef(id))
            });
            file_ids.entry(key.clone()).or_insert_with(|| {
                let id = dwarf.line_strings.add(filename.clone().into_bytes());
                file_index_map.entry(key).or_insert_with(|| {
                    let idx = next_file_idx;
                    next_file_idx += 1;
                    idx
                });
                line_program.add_file(LineString::LineStringRef(id), dir_id, None)
            });
        }

        line_program.begin_sequence(Some(Address::Constant(code_start)));
        for (address, filename, directory, line) in &data.lines_multi {
            let adjusted_addr = address + text_offset;
            if let Some(file_id) = file_ids.get(&(filename.clone(), directory.clone())) {
                line_program.row().file = *file_id;
            }
            line_program.row().address_offset = adjusted_addr - code_start;
            line_program.row().line = *line as u64;
            line_program.generate_row();
        }
        line_program.end_sequence(code_end);
    } else {
        let dir_id = line_program.default_directory();
        let file_id =
            line_program.add_file(LineString::LineStringRef(main_file_string_id), dir_id, None);

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
    }

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

    // Add label DIEs.
    if use_multi {
        for (name, address, filename, directory, line) in &data.labels_multi {
            let adjusted_addr = address + text_offset;
            let label_id = dwarf.unit.add(root_id, DW_TAG_label);
            let label_die = dwarf.unit.get_mut(label_id);

            label_die.set(
                DW_AT_name,
                AttributeValue::String(name.clone().into_bytes()),
            );
            let decl_file = file_index_map
                .get(&(filename.clone(), directory.clone()))
                .copied()
                .unwrap_or(0);
            label_die.set(DW_AT_decl_file, AttributeValue::Data4(decl_file));
            label_die.set(DW_AT_decl_line, AttributeValue::Data4(*line));
            label_die.set(
                DW_AT_low_pc,
                AttributeValue::Address(Address::Constant(adjusted_addr)),
            );
        }
    } else {
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
    }

    // Write sections.
    let mut sections = Sections::new(EndianVec::new(LittleEndian));
    dwarf.write(&mut sections).expect("Failed to write DWARF");
    sections
}

/// Reuse debug sections we came across while byteparsing
pub fn reuse_debug_sections(
    parsed_debug_sections: Vec<DebugSection>,
    section_names: &mut Vec<String>,
    current_offset: &mut u64,
) -> Vec<SectionType> {
    // reuse debug sections that came from byteparsing
    let mut sections = Vec::default();
    for mut debug_section in parsed_debug_sections.into_iter() {
        debug_section.set_name_offset(calc_name_offset(section_names));
        section_names.push(debug_section.name().to_string());
        debug_section.set_offset(*current_offset);
        *current_offset += debug_section.size();
        if debug_section.name() == SectionId::DebugAbbrev.name() {
            sections.push(SectionType::DebugAbbrev(debug_section));
        } else if debug_section.name() == SectionId::DebugInfo.name() {
            sections.push(SectionType::DebugInfo(debug_section));
        } else if debug_section.name() == SectionId::DebugLine.name() {
            sections.push(SectionType::DebugLine(debug_section));
        } else if debug_section.name() == SectionId::DebugLineStr.name() {
            sections.push(SectionType::DebugLineStr(debug_section));
        } else if debug_section.name() == SectionId::DebugStr.name() {
            sections.push(SectionType::DebugStr(debug_section));
        } else if debug_section.name() == SectionId::DebugFrame.name() {
            sections.push(SectionType::DebugFrame(debug_section));
        } else if debug_section.name() == SectionId::DebugLoc.name() {
            sections.push(SectionType::DebugLoc(debug_section));
        } else if debug_section.name() == SectionId::DebugRanges.name() {
            sections.push(SectionType::DebugRanges(debug_section));
        } else {
            eprintln!(
                "Unimplemented debug section: {}, consider adding it",
                debug_section.name()
            );
            continue;
        }
    }
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
            lines_multi: Vec::new(),
            labels_multi: Vec::new(),
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
