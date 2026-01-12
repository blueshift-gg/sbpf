use {
    gimli::{EndianSlice, RunTimeEndian, SectionId},
    object::{Object, ObjectSection},
    std::{borrow::Cow, collections::HashMap},
};

#[derive(Debug)]
struct LabelInfo {
    name: String,
    line: Option<u32>,
}

fn parse_dwarf_info(file_data: &[u8]) -> (HashMap<u64, u32>, Vec<LabelInfo>) {
    let object = object::File::parse(file_data).expect("Failed to parse ELF");

    let endian = RunTimeEndian::Little;

    let load_section = |id: SectionId| -> Result<Cow<[u8]>, gimli::Error> {
        match object.section_by_name(id.name()) {
            Some(section) => match section.uncompressed_data() {
                Ok(data) => Ok(data),
                Err(_) => Ok(Cow::Borrowed(&[])),
            },
            None => Ok(Cow::Borrowed(&[])),
        }
    };

    let dwarf_sections =
        gimli::DwarfSections::load(&load_section).expect("Failed to load DWARF sections");
    let dwarf = dwarf_sections.borrow(|section| EndianSlice::new(section, endian));

    let mut address_to_line: HashMap<u64, u32> = HashMap::new();
    let mut labels: Vec<LabelInfo> = Vec::new();

    let mut iter = dwarf.units();
    while let Ok(Some(header)) = iter.next() {
        let unit = dwarf.unit(header).expect("Failed to parse unit");
        let unit_ref = unit.unit_ref(&dwarf);

        // Extract labels
        let mut entries = unit_ref.entries();
        while let Ok(Some((_delta, entry))) = entries.next_dfs() {
            if entry.tag() == gimli::DW_TAG_subprogram || entry.tag() == gimli::DW_TAG_label {
                let mut name = None;
                let mut line = None;

                let mut attrs = entry.attrs();
                while let Ok(Some(attr)) = attrs.next() {
                    match attr.name() {
                        gimli::DW_AT_name => {
                            if let Ok(s) = dwarf.attr_string(&unit, attr.value()) {
                                name = Some(s.to_string_lossy().to_string());
                            }
                        }
                        gimli::DW_AT_decl_line => {
                            if let gimli::AttributeValue::Udata(l) = attr.value() {
                                line = Some(l as u32);
                            }
                        }
                        _ => {}
                    }
                }

                if let Some(name) = name {
                    labels.push(LabelInfo { name, line });
                }
            }
        }

        // Extract line numbers
        if let Some(program) = unit.line_program.clone() {
            let mut rows = program.rows();
            while let Ok(Some((_header, row))) = rows.next_row() {
                if !row.end_sequence() {
                    let address = row.address();
                    let line = row.line().map(|l| l.get()).unwrap_or(0) as u32;
                    address_to_line.insert(address, line);
                }
            }
        }
    }

    (address_to_line, labels)
}

#[test]
fn test_debug_data_e2e() {
    const TEST_SOURCE: &str = r#".globl entrypoint
entrypoint:          // line 2
  call test_1        // line 3
  ja jump_here       // line 4

jump_here:           // line 6
  lddw r1, 0x3       // line 7
  call sol_log_64_   // line 8
  call test_2        // line 9
  exit               // line 10

test_1:              // line 12
  lddw r1, 0x2       // line 13
  call sol_log_64_   // line 14
  exit               // line 15

test_2:              // line 17
  lddw r1, 0x4       // line 18
  call sol_log_64_   // line 19
  exit               // line 20
"#;

    // Assemble with debug info
    let bytecode = sbpf_assembler::assemble_with_debug_data(TEST_SOURCE, "test.s", "/test", false)
        .expect("Failed to assemble with debug data");

    // Parse DWARF info
    let (address_to_line, labels) = parse_dwarf_info(&bytecode);

    // Verify expected line numbers are present.
    let expected_lines: Vec<u32> = vec![3, 4, 7, 8, 9, 10, 13, 14, 15, 18, 19, 20];
    let actual_lines: std::collections::HashSet<u32> = address_to_line.values().copied().collect();

    for expected_line in expected_lines {
        assert!(
            actual_lines.contains(&expected_line),
            "Expected line {:?} not found in debug info",
            expected_line,
        );
    }

    // Verify labels exist at expected line numbers
    let expected_labels: Vec<(&str, u32)> = vec![
        ("entrypoint", 2),
        ("jump_here", 6),
        ("test_1", 12),
        ("test_2", 17),
    ];
    for (name, expected_line) in expected_labels {
        let label = labels.iter().find(|l| l.name == name);
        assert!(
            label.is_some(),
            "Expected label `{:?}` not found in debug info",
            name,
        );
        let label = label.unwrap();
        assert_eq!(
            label.line,
            Some(expected_line),
            "Expected label `{:?}` att line {:?}, found at {:?}",
            name,
            expected_line,
            label.line
        );
    }
}
