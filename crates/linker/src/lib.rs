pub mod byteparser;

use byteparser::parse_bytecode;

use sbpf_assembler::Program;

use anyhow::Result;

pub fn link_program(source: &Vec<u8>) -> Result<Vec<u8>, String> {
    let parse_result = match parse_bytecode(source) {
        Ok(program) => program,
        Err(errors) => {
            return Err(errors);
        }
    };
    let program = Program::from_parse_result(parse_result);
    let bytecode = program.emit_bytecode();
    Ok(bytecode)
}
