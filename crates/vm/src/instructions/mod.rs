pub mod alu32;
pub mod alu64;
pub mod call;
pub mod endian;
pub mod jump;
pub mod load;
pub mod store;

use {
    crate::{errors::VmResult, syscalls::SyscallHandler, vm::Vm},
    sbpf_common::{instruction::Instruction, opcode::Opcode},
};

/// Execute an instruction
pub fn execute_instruction(
    vm: &mut Vm,
    inst: &Instruction,
    syscall_handler: &mut dyn SyscallHandler,
) -> VmResult<()> {
    match inst.opcode {
        // Load instructions
        Opcode::Lddw => load::execute_lddw(vm, inst),
        Opcode::Ldxb => load::execute_ldxb(vm, inst),
        Opcode::Ldxh => load::execute_ldxh(vm, inst),
        Opcode::Ldxw => load::execute_ldxw(vm, inst),
        Opcode::Ldxdw => load::execute_ldxdw(vm, inst),

        // Store immediate instructions
        Opcode::Stb => store::execute_stb(vm, inst),
        Opcode::Sth => store::execute_sth(vm, inst),
        Opcode::Stw => store::execute_stw(vm, inst),
        Opcode::Stdw => store::execute_stdw(vm, inst),

        // Store register instructions
        Opcode::Stxb => store::execute_stxb(vm, inst),
        Opcode::Stxh => store::execute_stxh(vm, inst),
        Opcode::Stxw => store::execute_stxw(vm, inst),
        Opcode::Stxdw => store::execute_stxdw(vm, inst),

        // ALU 64-bit operations
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
        | Opcode::Arsh64Imm => alu64::execute_alu64_imm(vm, inst),

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
        | Opcode::Arsh64Reg => alu64::execute_alu64_reg(vm, inst),

        // ALU 32-bit operations
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
        | Opcode::Arsh32Imm => alu32::execute_alu32_imm(vm, inst),

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
        | Opcode::Arsh32Reg => alu32::execute_alu32_reg(vm, inst),

        // Unary operations
        Opcode::Neg32 => alu32::execute_neg32(vm, inst),
        Opcode::Neg64 => alu64::execute_neg64(vm, inst),

        // Endian operations
        Opcode::Le | Opcode::Be => endian::execute_endian(vm, inst),

        // Jump operations
        Opcode::Ja
        | Opcode::JeqImm
        | Opcode::JeqReg
        | Opcode::JgtImm
        | Opcode::JgtReg
        | Opcode::JgeImm
        | Opcode::JgeReg
        | Opcode::JltImm
        | Opcode::JltReg
        | Opcode::JleImm
        | Opcode::JleReg
        | Opcode::JsetImm
        | Opcode::JsetReg
        | Opcode::JneImm
        | Opcode::JneReg
        | Opcode::JsgtImm
        | Opcode::JsgtReg
        | Opcode::JsgeImm
        | Opcode::JsgeReg
        | Opcode::JsltImm
        | Opcode::JsltReg
        | Opcode::JsleImm
        | Opcode::JsleReg => jump::execute_jump(vm, inst),

        // Call and exit operations
        Opcode::Call => call::execute_call(vm, inst, syscall_handler),
        Opcode::Callx => call::execute_callx(vm, inst),
        Opcode::Exit => call::execute_exit(vm),

        _ => Err(crate::errors::VmError::InvalidInstruction),
    }
}
