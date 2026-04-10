use sbpf_vm::{
    errors::{SbpfVmError, SbpfVmResult},
    memory::Memory,
};

pub fn abort() -> SbpfVmResult<u64> {
    Err(SbpfVmError::Abort)
}

pub fn sol_panic(registers: [u64; 5], memory: &mut Memory) -> SbpfVmResult<u64> {
    let file_ptr = registers[0];
    let file_len = registers[1];
    let line = registers[2];
    let column = registers[3];
    let file_bytes = memory.read_bytes(file_ptr, file_len as usize)?;
    let file = String::from_utf8_lossy(file_bytes);
    eprintln!("Program panicked at {}:{}:{}", file, line, column);
    Err(SbpfVmError::Abort)
}
