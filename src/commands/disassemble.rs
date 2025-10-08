use std::fs::File;
use std::io::Read;

use anyhow::{Error, Result};
use sbpf_disassembler::program::Program;

pub fn disassemble(filename: String, asm: bool) -> Result<(), Error> {
    let mut file = File::open(filename)?;
    let mut b = vec![];
    file.read_to_end(&mut b)?;
    let program = Program::from_bytes(b.as_ref())?;

    if asm {
        println!(
            "{}",
            program
                .section_header_entries
                .iter()
                .map(|h| h.ixs.clone())
                .filter(|ixs| !ixs.is_empty())
                .map(|ixs| ixs
                    .iter()
                    .map(|i| i.to_asm().unwrap())
                    .collect::<Vec<String>>()
                    .join("\n"))
                .collect::<Vec<String>>()
                .join("\n")
        );
    } else {
        println!("{}", serde_json::to_string_pretty(&program)?);
    }

    Ok(())
}
