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
