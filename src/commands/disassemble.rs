use {
    anyhow::{Error, Result},
    sbpf_disassembler::program::Program,
    std::{fs::File, io::Read},
};

pub fn disassemble(filename: String, asm: bool) -> Result<(), Error> {
    let mut file = File::open(filename)?;
    let mut b = vec![];
    file.read_to_end(&mut b)?;
    let program = Program::from_bytes(b.as_ref())?;

    if asm {
        println!(
            "{}",
            program
                .to_ixs()?
                .iter()
                .map(|ix| ix.to_asm().unwrap())
                .collect::<Vec<String>>()
                .join("\n")
        );
    } else {
        println!("{}", serde_json::to_string_pretty(&program)?);
    }

    Ok(())
}
