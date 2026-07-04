use {
    crate::config::ExecutionCost,
    sbpf_vm::{
        compute::ComputeMeter,
        errors::{SbpfVmError, SbpfVmResult},
        memory::Memory,
    },
};

fn mem_op_consume(n: u64, compute: &ComputeMeter, costs: &ExecutionCost) -> SbpfVmResult<()> {
    let cost = costs
        .mem_op_base_cost
        .max(n.checked_div(costs.cpi_bytes_per_unit).unwrap_or(u64::MAX));
    compute.consume(cost)
}

fn is_nonoverlapping(src: u64, src_len: u64, dst: u64, dst_len: u64) -> bool {
    if src > dst {
        src.saturating_sub(dst) >= dst_len
    } else {
        dst.saturating_sub(src) >= src_len
    }
}

pub fn sol_memcpy(
    registers: [u64; 5],
    memory: &mut Memory,
    compute: &ComputeMeter,
    costs: &ExecutionCost,
) -> SbpfVmResult<u64> {
    let dst = registers[0];
    let src = registers[1];
    let n = registers[2];

    mem_op_consume(n, compute, costs)?;

    if !is_nonoverlapping(src, n, dst, n) {
        return Err(SbpfVmError::OverlappingMemoryRegions);
    }

    let data = memory.read_bytes(src, n as usize)?.to_vec();
    memory.write_bytes(dst, &data)?;
    Ok(0)
}

pub fn sol_memmove(
    registers: [u64; 5],
    memory: &mut Memory,
    compute: &ComputeMeter,
    costs: &ExecutionCost,
) -> SbpfVmResult<u64> {
    let dst = registers[0];
    let src = registers[1];
    let n = registers[2];

    mem_op_consume(n, compute, costs)?;

    let data = memory.read_bytes(src, n as usize)?.to_vec();
    memory.write_bytes(dst, &data)?;
    Ok(0)
}

pub fn sol_memset(
    registers: [u64; 5],
    memory: &mut Memory,
    compute: &ComputeMeter,
    costs: &ExecutionCost,
) -> SbpfVmResult<u64> {
    let dst = registers[0];
    let c = registers[1] as u8;
    let n = registers[2];

    mem_op_consume(n, compute, costs)?;

    let data = vec![c; n as usize];
    memory.write_bytes(dst, &data)?;
    Ok(0)
}

pub fn sol_memcmp(
    registers: [u64; 5],
    memory: &mut Memory,
    compute: &ComputeMeter,
    costs: &ExecutionCost,
) -> SbpfVmResult<u64> {
    let s1 = registers[0];
    let s2 = registers[1];
    let n = registers[2];
    let result_ptr = registers[3];

    mem_op_consume(n, compute, costs)?;

    let s1_bytes = memory.read_bytes(s1, n as usize)?;
    let s2_bytes = memory.read_bytes(s2, n as usize)?;

    let mut result: i32 = 0;
    for i in 0..n as usize {
        if s1_bytes[i] != s2_bytes[i] {
            result = (s1_bytes[i] as i32).saturating_sub(s2_bytes[i] as i32);
            break;
        }
    }

    memory.write_u32(result_ptr, result as u32)?;
    Ok(0)
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        sbpf_vm::{compute::ComputeMeter, errors::SbpfVmError, memory::Memory},
    };

    fn make_memory() -> Memory {
        Memory::new(vec![], vec![], 4096, 64 * 1024)
    }

    fn costs() -> ExecutionCost {
        ExecutionCost::default()
    }

    fn meter(limit: u64) -> ComputeMeter {
        ComputeMeter::new(limit)
    }

    #[test]
    fn test_memcpy_basic() {
        let mut memory = make_memory();
        let src = Memory::HEAP_START;
        let dst = Memory::HEAP_START + 64;
        memory.write_bytes(src, &[1, 2, 3, 4, 5]).unwrap();

        let registers = [dst, src, 5, 0, 0];
        sol_memcpy(registers, &mut memory, &meter(1_000_000), &costs()).unwrap();

        assert_eq!(memory.read_bytes(dst, 5).unwrap(), &[1, 2, 3, 4, 5]);
    }

    #[test]
    fn test_memcpy_overlapping_returns_error() {
        let mut memory = make_memory();
        // src at HEAP_START, dst 4 bytes into src, n=8 → overlapping
        let src = Memory::HEAP_START;
        let dst = Memory::HEAP_START + 4;
        memory.write_bytes(src, &[0u8; 16]).unwrap();

        let registers = [dst, src, 8, 0, 0];
        assert!(matches!(
            sol_memcpy(registers, &mut memory, &meter(1_000_000), &costs()),
            Err(SbpfVmError::OverlappingMemoryRegions)
        ));
    }

    #[test]
    fn test_memcpy_adjacent_not_overlapping() {
        let mut memory = make_memory();
        let src = Memory::HEAP_START;
        let dst = Memory::HEAP_START + 5;
        memory.write_bytes(src, &[10, 20, 30, 40, 50]).unwrap();

        let registers = [dst, src, 5, 0, 0];
        sol_memcpy(registers, &mut memory, &meter(1_000_000), &costs()).unwrap();
        assert_eq!(memory.read_bytes(dst, 5).unwrap(), &[10, 20, 30, 40, 50]);
    }

    #[test]
    fn test_memcpy_zero_length() {
        let mut memory = make_memory();
        let src = Memory::HEAP_START;
        let dst = Memory::HEAP_START + 32;
        memory.write_bytes(src, &[0xAA]).unwrap();

        let registers = [dst, src, 0, 0, 0];
        sol_memcpy(registers, &mut memory, &meter(1_000_000), &costs()).unwrap();
        assert_eq!(memory.read_u8(dst).unwrap(), 0);
    }

    #[test]
    fn test_memcpy_compute_exhausted() {
        let mut memory = make_memory();
        memory
            .write_bytes(Memory::HEAP_START, &[1, 2, 3, 4, 5])
            .unwrap();

        let registers = [Memory::HEAP_START + 64, Memory::HEAP_START, 5, 0, 0];
        // mem_op_base_cost = 10; budget of 9 is not enough
        assert!(matches!(
            sol_memcpy(registers, &mut memory, &meter(9), &costs()),
            Err(SbpfVmError::ComputeBudgetExceeded { .. })
        ));
    }

    #[test]
    fn test_memcpy_oob_dst() {
        let mut memory = make_memory();
        let src = Memory::HEAP_START;
        let heap_size = 64 * 1024u64;
        let dst = Memory::HEAP_START + heap_size - 3;
        memory.write_bytes(src, &[1, 2, 3, 4, 5]).unwrap();

        let registers = [dst, src, 5, 0, 0];
        let result = sol_memcpy(registers, &mut memory, &meter(1_000_000), &costs());
        assert!(result.is_err());
    }

    #[test]
    fn test_memcpy_oob_src() {
        let mut memory = make_memory();
        let heap_size = 64 * 1024u64;
        let src = Memory::HEAP_START + heap_size - 2;
        let dst = Memory::HEAP_START;

        let registers = [dst, src, 5, 0, 0];
        let result = sol_memcpy(registers, &mut memory, &meter(1_000_000), &costs());
        assert!(result.is_err());
    }

    #[test]
    fn test_memmove_basic() {
        let mut memory = make_memory();
        let src = Memory::HEAP_START;
        let dst = Memory::HEAP_START + 64;
        memory.write_bytes(src, &[5, 4, 3, 2, 1]).unwrap();

        let registers = [dst, src, 5, 0, 0];
        sol_memmove(registers, &mut memory, &meter(1_000_000), &costs()).unwrap();
        assert_eq!(memory.read_bytes(dst, 5).unwrap(), &[5, 4, 3, 2, 1]);
    }

    #[test]
    fn test_memmove_overlapping_allowed() {
        let mut memory = make_memory();
        let src = Memory::HEAP_START;
        let dst = Memory::HEAP_START + 4;
        memory.write_bytes(src, &[1, 2, 3, 4, 5, 6, 7, 8]).unwrap();

        let registers = [dst, src, 8, 0, 0];
        sol_memmove(registers, &mut memory, &meter(1_000_000), &costs()).unwrap();
    }

    #[test]
    fn test_memmove_compute_exhausted() {
        let mut memory = make_memory();
        memory.write_bytes(Memory::HEAP_START, &[1]).unwrap();
        let registers = [Memory::HEAP_START + 64, Memory::HEAP_START, 1, 0, 0];
        assert!(matches!(
            sol_memmove(registers, &mut memory, &meter(9), &costs()),
            Err(SbpfVmError::ComputeBudgetExceeded { .. })
        ));
    }

    #[test]
    fn test_memset_basic() {
        let mut memory = make_memory();
        let dst = Memory::HEAP_START;

        let registers = [dst, 0xAB, 8, 0, 0];
        sol_memset(registers, &mut memory, &meter(1_000_000), &costs()).unwrap();

        assert_eq!(memory.read_bytes(dst, 8).unwrap(), &[0xAB; 8]);
    }

    #[test]
    fn test_memset_zero_fill() {
        let mut memory = make_memory();
        let dst = Memory::HEAP_START;
        memory.write_bytes(dst, &[0xFF; 4]).unwrap();

        let registers = [dst, 0x00, 4, 0, 0];
        sol_memset(registers, &mut memory, &meter(1_000_000), &costs()).unwrap();
        assert_eq!(memory.read_bytes(dst, 4).unwrap(), &[0u8; 4]);
    }

    #[test]
    fn test_memset_compute_exhausted() {
        let mut memory = make_memory();
        let registers = [Memory::HEAP_START, 0xFF, 5, 0, 0];
        assert!(matches!(
            sol_memset(registers, &mut memory, &meter(9), &costs()),
            Err(SbpfVmError::ComputeBudgetExceeded { .. })
        ));
    }

    #[test]
    fn test_memcmp_equal() {
        let mut memory = make_memory();
        let s1 = Memory::HEAP_START;
        let s2 = Memory::HEAP_START + 16;
        let result_ptr = Memory::HEAP_START + 32;
        memory.write_bytes(s1, &[1, 2, 3, 4, 5]).unwrap();
        memory.write_bytes(s2, &[1, 2, 3, 4, 5]).unwrap();

        let registers = [s1, s2, 5, result_ptr, 0];
        sol_memcmp(registers, &mut memory, &meter(1_000_000), &costs()).unwrap();

        let cmp = memory.read_u32(result_ptr).unwrap() as i32;
        assert_eq!(cmp, 0);
    }

    #[test]
    fn test_memcmp_less_than() {
        let mut memory = make_memory();
        let s1 = Memory::HEAP_START;
        let s2 = Memory::HEAP_START + 16;
        let result_ptr = Memory::HEAP_START + 32;
        // s1[2]=3, s2[2]=4 → 3 - 4 = -1
        memory.write_bytes(s1, &[1, 2, 3]).unwrap();
        memory.write_bytes(s2, &[1, 2, 4]).unwrap();

        let registers = [s1, s2, 3, result_ptr, 0];
        sol_memcmp(registers, &mut memory, &meter(1_000_000), &costs()).unwrap();

        let cmp = memory.read_u32(result_ptr).unwrap() as i32;
        assert!(cmp < 0);
    }

    #[test]
    fn test_memcmp_greater_than() {
        let mut memory = make_memory();
        let s1 = Memory::HEAP_START;
        let s2 = Memory::HEAP_START + 16;
        let result_ptr = Memory::HEAP_START + 32;
        memory.write_bytes(s1, &[1, 2, 5]).unwrap();
        memory.write_bytes(s2, &[1, 2, 4]).unwrap();

        let registers = [s1, s2, 3, result_ptr, 0];
        sol_memcmp(registers, &mut memory, &meter(1_000_000), &costs()).unwrap();

        let cmp = memory.read_u32(result_ptr).unwrap() as i32;
        assert!(cmp > 0);
    }

    #[test]
    fn test_memcmp_compute_exhausted() {
        let mut memory = make_memory();
        let s1 = Memory::HEAP_START;
        let s2 = Memory::HEAP_START + 16;
        let result_ptr = Memory::HEAP_START + 32;
        let registers = [s1, s2, 5, result_ptr, 0];
        assert!(matches!(
            sol_memcmp(registers, &mut memory, &meter(9), &costs()),
            Err(SbpfVmError::ComputeBudgetExceeded { .. })
        ));
    }
}
