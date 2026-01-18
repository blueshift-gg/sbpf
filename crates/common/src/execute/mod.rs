mod alu32;
mod alu64;
mod call;
mod endian;
mod helpers;
mod jump;
mod load;
mod store;

use crate::{errors::ExecutionError, instruction::Instruction, opcode::Opcode};
use alu32::{execute_alu32_imm, execute_alu32_reg, execute_neg32};
use alu64::{execute_alu64_imm, execute_alu64_reg, execute_neg64};
pub use call::{execute_call_immediate, execute_call_register, execute_exit};
use endian::execute_endian;
pub use jump::{execute_jump, execute_jump_immediate, execute_jump_register};
use load::{execute_lddw, execute_ldxb, execute_ldxdw, execute_ldxh, execute_ldxw};
use store::{
    execute_stb, execute_stdw, execute_sth, execute_stw, execute_stxb, execute_stxdw, execute_stxh,
    execute_stxw,
};

pub type ExecutionResult<T> = Result<T, ExecutionError>;

pub trait SbpfVm {
    fn get_register(&self, reg: usize) -> u64;
    fn set_register(&mut self, reg: usize, value: u64);

    fn get_pc(&self) -> usize;
    fn set_pc(&mut self, pc: usize);
    fn advance_pc(&mut self) {
        self.set_pc(self.get_pc() + 1);
    }

    fn read_u8(&self, addr: u64) -> ExecutionResult<u8>;
    fn read_u16(&self, addr: u64) -> ExecutionResult<u16>;
    fn read_u32(&self, addr: u64) -> ExecutionResult<u32>;
    fn read_u64(&self, addr: u64) -> ExecutionResult<u64>;

    fn write_u8(&mut self, addr: u64, value: u8) -> ExecutionResult<()>;
    fn write_u16(&mut self, addr: u64, value: u16) -> ExecutionResult<()>;
    fn write_u32(&mut self, addr: u64, value: u32) -> ExecutionResult<()>;
    fn write_u64(&mut self, addr: u64, value: u64) -> ExecutionResult<()>;

    fn get_call_depth(&self) -> usize;
    fn max_call_depth(&self) -> usize;
    fn push_frame(
        &mut self,
        return_pc: usize,
        saved_regs: [u64; 4],
        saved_fp: u64,
    ) -> ExecutionResult<()>;
    fn pop_frame(&mut self) -> Option<(usize, [u64; 4], u64)>;

    fn halt(&mut self, exit_code: u64);
    fn is_halted(&self) -> bool;

    fn stack_frame_size(&self) -> u64;

    fn handle_syscall(&mut self, name: &str) -> ExecutionResult<u64>;
}

pub fn execute_binary_immediate(vm: &mut dyn SbpfVm, inst: &Instruction) -> ExecutionResult<()> {
    match inst.opcode {
        Opcode::Add64Imm
        | Opcode::Sub64Imm
        | Opcode::Mul64Imm
        | Opcode::Div64Imm
        | Opcode::Or64Imm
        | Opcode::And64Imm
        | Opcode::Lsh64Imm
        | Opcode::Rsh64Imm
        | Opcode::Mod64Imm
        | Opcode::Xor64Imm
        | Opcode::Mov64Imm
        | Opcode::Arsh64Imm => execute_alu64_imm(vm, inst),
        Opcode::Add32Imm
        | Opcode::Sub32Imm
        | Opcode::Mul32Imm
        | Opcode::Div32Imm
        | Opcode::Or32Imm
        | Opcode::And32Imm
        | Opcode::Lsh32Imm
        | Opcode::Rsh32Imm
        | Opcode::Mod32Imm
        | Opcode::Xor32Imm
        | Opcode::Mov32Imm
        | Opcode::Arsh32Imm => execute_alu32_imm(vm, inst),
        _ => Err(ExecutionError::InvalidInstruction),
    }
}

pub fn execute_binary_register(vm: &mut dyn SbpfVm, inst: &Instruction) -> ExecutionResult<()> {
    match inst.opcode {
        Opcode::Add64Reg
        | Opcode::Sub64Reg
        | Opcode::Mul64Reg
        | Opcode::Div64Reg
        | Opcode::Or64Reg
        | Opcode::And64Reg
        | Opcode::Lsh64Reg
        | Opcode::Rsh64Reg
        | Opcode::Mod64Reg
        | Opcode::Xor64Reg
        | Opcode::Mov64Reg
        | Opcode::Arsh64Reg => execute_alu64_reg(vm, inst),
        Opcode::Add32Reg
        | Opcode::Sub32Reg
        | Opcode::Mul32Reg
        | Opcode::Div32Reg
        | Opcode::Or32Reg
        | Opcode::And32Reg
        | Opcode::Lsh32Reg
        | Opcode::Rsh32Reg
        | Opcode::Mod32Reg
        | Opcode::Xor32Reg
        | Opcode::Mov32Reg
        | Opcode::Arsh32Reg => execute_alu32_reg(vm, inst),
        _ => Err(ExecutionError::InvalidInstruction),
    }
}

pub fn execute_unary(vm: &mut dyn SbpfVm, inst: &Instruction) -> ExecutionResult<()> {
    match inst.opcode {
        Opcode::Neg64 => execute_neg64(vm, inst),
        Opcode::Neg32 => execute_neg32(vm, inst),
        Opcode::Le | Opcode::Be => execute_endian(vm, inst),
        _ => Err(ExecutionError::InvalidInstruction),
    }
}

pub fn execute_load_immediate(vm: &mut dyn SbpfVm, inst: &Instruction) -> ExecutionResult<()> {
    execute_lddw(vm, inst)
}

pub fn execute_load_memory(vm: &mut dyn SbpfVm, inst: &Instruction) -> ExecutionResult<()> {
    match inst.opcode {
        Opcode::Ldxb => execute_ldxb(vm, inst),
        Opcode::Ldxh => execute_ldxh(vm, inst),
        Opcode::Ldxw => execute_ldxw(vm, inst),
        Opcode::Ldxdw => execute_ldxdw(vm, inst),
        _ => Err(ExecutionError::InvalidInstruction),
    }
}

pub fn execute_store_immediate(vm: &mut dyn SbpfVm, inst: &Instruction) -> ExecutionResult<()> {
    match inst.opcode {
        Opcode::Stb => execute_stb(vm, inst),
        Opcode::Sth => execute_sth(vm, inst),
        Opcode::Stw => execute_stw(vm, inst),
        Opcode::Stdw => execute_stdw(vm, inst),
        _ => Err(ExecutionError::InvalidInstruction),
    }
}

pub fn execute_store_register(vm: &mut dyn SbpfVm, inst: &Instruction) -> ExecutionResult<()> {
    match inst.opcode {
        Opcode::Stxb => execute_stxb(vm, inst),
        Opcode::Stxh => execute_stxh(vm, inst),
        Opcode::Stxw => execute_stxw(vm, inst),
        Opcode::Stxdw => execute_stxdw(vm, inst),
        _ => Err(ExecutionError::InvalidInstruction),
    }
}
