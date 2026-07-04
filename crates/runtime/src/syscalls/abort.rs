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

#[cfg(test)]
mod tests {
    use {super::*, sbpf_vm::memory::Memory};

    fn make_memory() -> Memory {
        Memory::new(vec![], vec![], 4096, 4096)
    }

    #[test]
    fn test_abort_returns_abort_error() {
        assert!(matches!(abort(), Err(SbpfVmError::Abort)));
    }

    #[test]
    fn test_sol_panic_returns_abort() {
        let mut memory = make_memory();
        let file = b"src/main.rs";
        memory.write_bytes(Memory::HEAP_START, file).unwrap();
        let registers = [Memory::HEAP_START, file.len() as u64, 42, 7, 0];
        assert!(matches!(
            sol_panic(registers, &mut memory),
            Err(SbpfVmError::Abort)
        ));
    }

    #[test]
    fn test_sol_panic_oob_file_ptr_returns_error() {
        let mut memory = make_memory();
        let registers = [0xDEAD_0000_0000, 10, 1, 1, 0];
        let result = sol_panic(registers, &mut memory);
        assert!(result.is_err());
        assert!(!matches!(result, Err(SbpfVmError::Abort)));
    }

    #[test]
    fn test_sol_panic_zero_length_file() {
        let mut memory = make_memory();
        let registers = [Memory::HEAP_START, 0, 0, 0, 0];
        assert!(matches!(
            sol_panic(registers, &mut memory),
            Err(SbpfVmError::Abort)
        ));
    }
}
