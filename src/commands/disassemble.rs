use {
    anyhow::{Error, Result},
    clap::Args,
    either::Either,
    sbpf_common::{inst_param::Number, instruction::AsmFormat, opcode::Opcode},
    sbpf_disassembler::program::Program,
    std::{collections::HashSet, fs::File, io::Read},
};

#[derive(Args)]
pub struct DisassembleArgs {
    #[arg(help = "Path to the ELF file (.so) to disassemble")]
    pub filename: String,
    #[arg(short, long, help = "Output full JSON debug information")]
    pub debug: bool,
    #[arg(
        short,
        long,
        default_value = "default",
        help = "Assembly format: 'default' or 'llvm'"
    )]
    pub format: String,
    #[arg(
        short,
        long,
        help = "Output raw instructions without labels or formatting"
    )]
    pub raw: bool,
}

pub fn disassemble(args: DisassembleArgs) -> Result<(), Error> {
    let mut file = File::open(&args.filename)?;
    let mut b = vec![];
    file.read_to_end(&mut b)?;
    let program = Program::from_bytes(b.as_ref())?;

    let format = match args.format.as_str() {
        "default" => AsmFormat::Default,
        "llvm" => AsmFormat::Llvm,
        other => anyhow::bail!("unknown format '{}', expected 'default' or 'llvm'", other),
    };
    let output = disassemble_program(program, args.debug, format, args.raw)?;
    print!("{}", output);
    Ok(())
}

fn disassemble_program(
    program: Program,
    debug: bool,
    format: AsmFormat,
    raw: bool,
) -> Result<String, Error> {
    let mut output = String::new();

    if debug {
        output = serde_json::to_string_pretty(&program)?;
    } else if raw {
        let (ixs, _, _) = program.to_ixs_raw()?;
        for ix in &ixs {
            output.push_str(&format!("{}\n", ix.to_asm(format)?));
        }
    } else {
        let entrypoint_offset = program.get_entrypoint_offset();

        let (mut ixs, rodata, _) = program.to_ixs()?;

        // Build position map
        let positions: Vec<u64> = ixs
            .iter()
            .scan(0u64, |pos, ix| {
                let current = *pos;
                *pos += ix.get_size();
                Some(current)
            })
            .collect();

        // Collect all target positions
        let mut jmp_targets: HashSet<u64> = HashSet::new();
        let mut fn_targets: HashSet<u64> = HashSet::new();
        for (idx, ix) in ixs.iter().enumerate() {
            if ix.is_jump()
                && let Some(Either::Right(off)) = &ix.off
            {
                let target_idx = (idx as i64 + 1 + *off as i64) as usize;
                if let Some(&target_pos) = positions.get(target_idx) {
                    jmp_targets.insert(target_pos);
                }
            }

            if ix.opcode == Opcode::Call
                && let Some(Either::Right(Number::Int(imm))) = &ix.imm
            {
                let target_idx = (idx as i64 + 1 + *imm) as usize;
                if let Some(&target_pos) = positions.get(target_idx) {
                    fn_targets.insert(target_pos);
                }
            }
        }

        // Output .globl entrypoint directive at the top
        output.push_str(".globl entrypoint\n");

        let mut in_labeled_block = false;
        for (idx, ix) in ixs.iter_mut().enumerate() {
            let pos = positions[idx];
            let is_fn_target = fn_targets.contains(&pos);
            let is_jmp_target = jmp_targets.contains(&pos);
            let is_entrypoint = entrypoint_offset == Some(pos);

            // Output labels if this position is a target or entrypoint
            if is_fn_target || is_jmp_target || is_entrypoint {
                output.push('\n');
                if is_entrypoint {
                    output.push_str("entrypoint:\n");
                }
                if is_fn_target && !is_entrypoint {
                    output.push_str(&format!("fn_{:04x}:\n", pos));
                }
                if is_jmp_target {
                    output.push_str(&format!("jmp_{:04x}:\n", pos));
                }
                in_labeled_block = true;
            }

            // Replace numeric values with labels for display.
            if ix.is_jump()
                && let Some(Either::Right(off)) = &ix.off
            {
                let target_idx = (idx as i64 + 1 + *off as i64) as usize;
                if let Some(&target_pos) = positions.get(target_idx) {
                    ix.off = Some(Either::Left(format!("jmp_{:04x}", target_pos)));
                }
            }

            if ix.opcode == Opcode::Call
                && let Some(Either::Right(Number::Int(imm))) = &ix.imm
            {
                let target_idx = (idx as i64 + 1 + *imm) as usize;
                if let Some(&target_pos) = positions.get(target_idx) {
                    ix.imm = Some(Either::Left(format!("fn_{:04x}", target_pos)));
                }
            }

            if ix.opcode == Opcode::Lddw
                && let Some(Either::Right(Number::Int(imm))) = &ix.imm
                && let Some(rodata) = &rodata
                && let Some(label) = rodata.get_label(*imm as u64)
            {
                ix.imm = Some(Either::Left(label.to_string()));
            }

            // Indent instructions under labels
            let indent = if in_labeled_block { "  " } else { "" };
            output.push_str(&format!("{}{}\n", indent, ix.to_asm(format)?));
        }

        // Output rodata section if present
        if let Some(rodata) = rodata
            && rodata.has_items()
        {
            output.push('\n');
            output.push_str(&rodata.to_asm());
        }
    }

    Ok(output)
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        hex_literal::hex,
        sbpf_assembler::{Assembler, AssemblerOption},
    };

    #[test]
    fn test_disassemble_with_labels() {
        // Test program with multiple jumps and internal calls:
        //
        // .globl entrypoint
        // entrypoint:
        //   call test_1
        //   ja jump_here
        //
        // jump_back:
        //   lddw r1, 0x1
        //   call sol_log_64_
        //   call test_1
        //   exit
        //
        // jump_here:
        //   lddw r1, 0x3
        //   call sol_log_64_
        //   call test_2
        //   ja jump_back
        //   exit
        //
        // test_1:
        //   lddw r1, 0x2
        //   call sol_log_64_
        //   exit
        //
        // test_2:
        //   lddw r1, 0x4
        //   call sol_log_64_
        //   exit
        let elf_bytes = hex!(
            "7f454c460201010000000000000000000300f70001000000e8000000000000004000000000000000"
            "0003000000000000000000004000380003004000070006000100000005000000e800000000000000"
            "e800000000000000e800000000000000a800000000000000a8000000000000000010000000000000"
            "0100000004000000300200000000000030020000000000003002000000000000a000000000000000"
            "a0000000000000000010000000000000020000000600000090010000000000009001000000000000"
            "9001000000000000a000000000000000a0000000000000000800000000000000851000000c000000"
            "05000500000000001801000001000000000000000000000085100000ffffffff8510000007000000"
            "95000000000000001801000003000000000000000000000085100000ffffffff8510000006000000"
            "0500f6ff0000000095000000000000001801000002000000000000000000000085100000ffffffff"
            "95000000000000001801000004000000000000000000000085100000ffffffff9500000000000000"
            "1e000000000000000400000000000000110000000000000090020000000000001200000000000000"
            "40000000000000001300000000000000100000000000000006000000000000003002000000000000"
            "0b000000000000001800000000000000050000000000000078020000000000000a00000000000000"
            "18000000000000001600000000000000000000000000000000000000000000000000000000000000"
            "0000000000000000000000000000000000000000000000000100000010000100e800000000000000"
            "00000000000000000c000000100000000000000000000000000000000000000000656e747279706f"
            "696e7400736f6c5f6c6f675f36345f0008010000000000000a000000020000003001000000000000"
            "0a0000000200000060010000000000000a0000000200000080010000000000000a00000002000000"
            "002e74657874002e64796e616d6963002e64796e73796d002e64796e737472002e72656c2e64796e"
            "002e7300000000000000000000000000000000000000000000000000000000000000000000000000"
            "00000000000000000000000000000000000000000000000000000000000000000100000001000000"
            "0600000000000000e800000000000000e800000000000000a8000000000000000000000000000000"
            "04000000000000000000000000000000070000000600000003000000000000009001000000000000"
            "9001000000000000a000000000000000040000000000000008000000000000001000000000000000"
            "100000000b0000000200000000000000300200000000000030020000000000004800000000000000"
            "04000000010000000800000000000000180000000000000018000000030000000200000000000000"
            "78020000000000007802000000000000180000000000000000000000000000000100000000000000"
            "00000000000000002000000009000000020000000000000090020000000000009002000000000000"
            "40000000000000000300000000000000080000000000000010000000000000002900000003000000"
            "00000000000000000000000000000000d0020000000000002c000000000000000000000000000000"
            "01000000000000000000000000000000"
        );

        assert_eq!(
            disassemble_program(
                Program::from_bytes(&elf_bytes).unwrap(),
                false,
                AsmFormat::Default,
                false,
            )
            .unwrap(),
            r#".globl entrypoint

entrypoint:
  call fn_0068
  ja jmp_0038

jmp_0010:
  lddw r1, 0x1
  call sol_log_64_
  call fn_0068
  exit

jmp_0038:
  lddw r1, 0x3
  call sol_log_64_
  call fn_0088
  ja jmp_0010
  exit

fn_0068:
  lddw r1, 0x2
  call sol_log_64_
  exit

fn_0088:
  lddw r1, 0x4
  call sol_log_64_
  exit
"#
        );

        assert_eq!(
            disassemble_program(
                Program::from_bytes(&elf_bytes).unwrap(),
                false,
                AsmFormat::Llvm,
                false,
            )
            .unwrap(),
            r#".globl entrypoint

entrypoint:
  call fn_0068
  goto jmp_0038

jmp_0010:
  r1 = 0x1 ll
  call sol_log_64_
  call fn_0068
  exit

jmp_0038:
  r1 = 0x3 ll
  call sol_log_64_
  call fn_0088
  goto jmp_0010
  exit

fn_0068:
  r1 = 0x2 ll
  call sol_log_64_
  exit

fn_0088:
  r1 = 0x4 ll
  call sol_log_64_
  exit
"#
        );
    }

    #[test]
    fn test_disassemble_with_same_target() {
        // Test program where both call and jump target the same position:
        //
        // .globl entrypoint
        // entrypoint:
        //   call my_func
        //   ja my_func
        //
        // my_func:
        //   lddw r1, 0x1
        //   call sol_log_64_
        //   exit
        let elf_bytes = hex!(
            "7f454c460201010000000000000000000300f70001000000e8000000000000004000000000000000"
            "5802000000000000000000004000380003004000070006000100000005000000e800000000000000"
            "e800000000000000e800000000000000300000000000000030000000000000000010000000000000"
            "0100000004000000b801000000000000b801000000000000b8010000000000007000000000000000"
            "70000000000000000010000000000000020000000600000018010000000000001801000000000000"
            "1801000000000000a000000000000000a00000000000000008000000000000008510000001000000"
            "05000000000000001801000001000000000000000000000085100000ffffffff9500000000000000"
            "1e000000000000000400000000000000110000000000000018020000000000001200000000000000"
            "1000000000000000130000000000000010000000000000000600000000000000b801000000000000"
            "0b000000000000001800000000000000050000000000000000020000000000000a00000000000000"
            "18000000000000001600000000000000000000000000000000000000000000000000000000000000"
            "0000000000000000000000000000000000000000000000000100000010000100e800000000000000"
            "00000000000000000c000000100000000000000000000000000000000000000000656e747279706f"
            "696e7400736f6c5f6c6f675f36345f0008010000000000000a00000002000000002e74657874002e"
            "64796e616d6963002e64796e73796d002e64796e737472002e72656c2e64796e002e730000000000"
            "00000000000000000000000000000000000000000000000000000000000000000000000000000000"
            "00000000000000000000000000000000000000000000000001000000010000000600000000000000"
            "e800000000000000e800000000000000300000000000000000000000000000000400000000000000"
            "00000000000000000700000006000000030000000000000018010000000000001801000000000000"
            "a000000000000000040000000000000008000000000000001000000000000000100000000b000000"
            "0200000000000000b801000000000000b80100000000000048000000000000000400000001000000"
            "08000000000000001800000000000000180000000300000002000000000000000002000000000000"
            "00020000000000001800000000000000000000000000000001000000000000000000000000000000"
            "20000000090000000200000000000000180200000000000018020000000000001000000000000000"
            "03000000000000000800000000000000100000000000000029000000030000000000000000000000"
            "000000000000000028020000000000002c0000000000000000000000000000000100000000000000"
            "0000000000000000"
        );

        // Both fn and jmp labels should be present
        assert_eq!(
            disassemble_program(
                Program::from_bytes(&elf_bytes).unwrap(),
                false,
                AsmFormat::Default,
                false,
            )
            .unwrap(),
            r#".globl entrypoint

entrypoint:
  call fn_0010
  ja jmp_0010

fn_0010:
jmp_0010:
  lddw r1, 0x1
  call sol_log_64_
  exit
"#
        );

        assert_eq!(
            disassemble_program(
                Program::from_bytes(&elf_bytes).unwrap(),
                false,
                AsmFormat::Llvm,
                false,
            )
            .unwrap(),
            r#".globl entrypoint

entrypoint:
  call fn_0010
  goto jmp_0010

fn_0010:
jmp_0010:
  r1 = 0x1 ll
  call sol_log_64_
  exit
"#
        );
    }

    #[test]
    fn test_disassemble_with_rodata() {
        // Test program with .rodata section
        //
        // .globl entrypoint
        // entrypoint:
        //   lddw r1, my_byte
        //   lddw r2, my_word
        //   lddw r3, my_long
        //   lddw r4, my_quad
        //   call sol_log_64_
        //
        //   lddw r1, my_string
        //   lddw r2, 12
        //   call sol_log_
        //   exit
        //
        // .rodata
        //   my_byte: .byte 0x1, 0x2, 0x3
        //   my_word: .short 0x1234
        //   my_long: .long 0x12345678
        //   my_quad: .quad 0x123456789ABCDEF0
        //   my_string: .ascii "Hello World!"
        let elf_bytes = hex!(
            "7f454c460201010000000000000000000300f70001000000e8000000000000004000000000000000"
            "6003000000000000000000004000380003004000080007000100000005000000e800000000000000"
            "e800000000000000e800000000000000950000000000000095000000000000000010000000000000"
            "0100000004000000300200000000000030020000000000003002000000000000f800000000000000"
            "f8000000000000000010000000000000020000000600000080010000000000008001000000000000"
            "8001000000000000b000000000000000b00000000000000008000000000000001801000060010000"
            "00000000000000001802000063010000000000000000000018030000650100000000000000000000"
            "1804000069010000000000000000000085100000ffffffff18010000710100000000000000000000"
            "180200000c000000000000000000000085100000ffffffff95000000000000000102033412785634"
            "12f0debc9a7856341248656c6c6f20576f726c64210000001e000000000000000400000000000000"
            "1100000000000000b802000000000000120000000000000070000000000000001300000000000000"
            "1000000000000000faffff6f00000000050000000000000006000000000000003002000000000000"
            "0b000000000000001800000000000000050000000000000090020000000000000a00000000000000"
            "28000000000000001600000000000000000000000000000000000000000000000000000000000000"
            "0000000000000000000000000000000000000000000000000100000010000100e800000000000000"
            "00000000000000000c00000010000000000000000000000000000000000000001500000010000000"
            "0000000000000000000000000000000000656e747279706f696e7400736f6c5f6c6f675f00736f6c"
            "5f6c6f675f36345f0000000000000000e8000000000000000800000000000000f800000000000000"
            "08000000000000000801000000000000080000000000000018010000000000000800000000000000"
            "28010000000000000a00000003000000300100000000000008000000000000005001000000000000"
            "0a00000002000000002e74657874002e726f64617461002e64796e616d6963002e64796e73796d00"
            "2e64796e737472002e72656c2e64796e002e73000000000000000000000000000000000000000000"
            "00000000000000000000000000000000000000000000000000000000000000000000000000000000"
            "000000000000000001000000010000000600000000000000e800000000000000e800000000000000"
            "78000000000000000000000000000000040000000000000000000000000000000700000001000000"
            "0200000000000000600100000000000060010000000000001d000000000000000000000000000000"
            "010000000000000000000000000000000f0000000600000003000000000000008001000000000000"
            "8001000000000000b000000000000000050000000000000008000000000000001000000000000000"
            "180000000b0000000200000000000000300200000000000030020000000000006000000000000000"
            "05000000010000000800000000000000180000000000000020000000030000000200000000000000"
            "90020000000000009002000000000000280000000000000000000000000000000100000000000000"
            "000000000000000028000000090000000200000000000000b802000000000000b802000000000000"
            "70000000000000000400000000000000080000000000000010000000000000003100000003000000"
            "00000000000000000000000000000000280300000000000034000000000000000000000000000000"
            "01000000000000000000000000000000"
        );

        assert_eq!(
            disassemble_program(
                Program::from_bytes(&elf_bytes).unwrap(),
                false,
                AsmFormat::Default,
                false,
            )
            .unwrap(),
            r#".globl entrypoint

entrypoint:
  lddw r1, data_0000
  lddw r2, data_0003
  lddw r3, data_0005
  lddw r4, data_0009
  call sol_log_64_
  lddw r1, str_0011
  lddw r2, 0xc
  call sol_log_
  exit

.rodata
  data_0000: .byte 0x01, 0x02, 0x03
  data_0003: .word 0x1234
  data_0005: .long 0x12345678
  data_0009: .quad 0x123456789abcdef0
  str_0011: .ascii "Hello World!"
"#
        );

        assert_eq!(
            disassemble_program(
                Program::from_bytes(&elf_bytes).unwrap(),
                false,
                AsmFormat::Llvm,
                false,
            )
            .unwrap(),
            r#".globl entrypoint

entrypoint:
  r1 = data_0000 ll
  r2 = data_0003 ll
  r3 = data_0005 ll
  r4 = data_0009 ll
  call sol_log_64_
  r1 = str_0011 ll
  r2 = 0xc ll
  call sol_log_
  exit

.rodata
  data_0000: .byte 0x01, 0x02, 0x03
  data_0003: .word 0x1234
  data_0005: .long 0x12345678
  data_0009: .quad 0x123456789abcdef0
  str_0011: .ascii "Hello World!"
"#
        );
    }

    #[test]
    fn test_disassemble_v3() {
        // Test v3 program with static syscall
        //
        // .globl e
        // e:
        //     lddw r1, 0x1
        //     call sol_log_64_
        //     exit
        let elf_bytes = hex!(
            "7f454c460201010000000000000000000300f7000100000000000000010000004000000000000000"
            "a8000000000000000300000040003800010040000300020001000000010000007800000000000000"
            "00000000010000000000000001000000200000000000000020000000000000000000000000000000"
            "180100000100000000000000000000008500000078312a5c9500000000000000002e74657874002e"
            "73000000000000000000000000000000000000000000000000000000000000000000000000000000"
            "00000000000000000000000000000000000000000000000000000000000000000100000001000000"
            "06000000000000007800000000000000780000000000000020000000000000000000000000000000"
            "04000000000000000000000000000000060000000300000000000000000000000000000000000000"
            "98000000000000000a00000000000000000000000000000001000000000000000000000000000000"
        );

        assert_eq!(
            disassemble_program(
                Program::from_bytes(&elf_bytes).unwrap(),
                false,
                AsmFormat::Default,
                false,
            )
            .unwrap(),
            r#".globl entrypoint

entrypoint:
  lddw r1, 0x1
  call sol_log_64_
  exit
"#
        );

        assert_eq!(
            disassemble_program(
                Program::from_bytes(&elf_bytes).unwrap(),
                false,
                AsmFormat::Llvm,
                false,
            )
            .unwrap(),
            r#".globl entrypoint

entrypoint:
  r1 = 0x1 ll
  call sol_log_64_
  exit
"#
        );
    }

    #[test]
    fn test_disassemble_v3_with_rodata() {
        // Test v3 program with rodata
        //
        // .globl e
        // e:
        //   lddw r1, msg
        //   lddw r2, 5
        //   call sol_log_
        //   exit
        //
        // .rodata
        //   msg: .ascii "hello"
        let elf_bytes = hex!(
            "7f454c460201010000000000000000000300f7000100000000000000010000004000000000000000"
            "0001000000000000030000004000380002004000040003000100000004000000b000000000000000"
            "00000000000000000000000000000000080000000000000008000000000000000000000000000000"
            "0100000001000000b800000000000000000000000100000000000000010000003000000000000000"
            "3000000000000000000000000000000068656c6c6f00000018010000000000000000000000000000"
            "1802000005000000000000000000000085000000bd5975209500000000000000002e74657874002e"
            "726f64617461002e7300000000000000000000000000000000000000000000000000000000000000"
            "00000000000000000000000000000000000000000000000000000000000000000000000000000000"
            "07000000010000000200000000000000b000000000000000b0000000000000000500000000000000"
            "00000000000000000100000000000000000000000000000001000000010000000600000000000000"
            "b800000000000000b800000000000000300000000000000000000000000000000400000000000000"
            "00000000000000000e0000000300000000000000000000000000000000000000e800000000000000"
            "1200000000000000000000000000000001000000000000000000000000000000"
        );

        assert_eq!(
            disassemble_program(
                Program::from_bytes(&elf_bytes).unwrap(),
                false,
                AsmFormat::Default,
                false,
            )
            .unwrap(),
            r#".globl entrypoint

entrypoint:
  lddw r1, str_0000
  lddw r2, 0x5
  call sol_log_
  exit

.rodata
  str_0000: .ascii "hello"
"#
        );

        assert_eq!(
            disassemble_program(
                Program::from_bytes(&elf_bytes).unwrap(),
                false,
                AsmFormat::Llvm,
                false,
            )
            .unwrap(),
            r#".globl entrypoint

entrypoint:
  r1 = str_0000 ll
  r2 = 0x5 ll
  call sol_log_
  exit

.rodata
  str_0000: .ascii "hello"
"#
        );
    }

    #[test]
    fn test_disassemble_raw() {
        let elf_bytes = hex!(
            "7f454c460201010000000000000000000300f70001000000e8000000000000004000000000000000"
            "0003000000000000000000004000380003004000070006000100000005000000e800000000000000"
            "e800000000000000e800000000000000a800000000000000a8000000000000000010000000000000"
            "0100000004000000300200000000000030020000000000003002000000000000a000000000000000"
            "a0000000000000000010000000000000020000000600000090010000000000009001000000000000"
            "9001000000000000a000000000000000a0000000000000000800000000000000851000000c000000"
            "05000500000000001801000001000000000000000000000085100000ffffffff8510000007000000"
            "95000000000000001801000003000000000000000000000085100000ffffffff8510000006000000"
            "0500f6ff0000000095000000000000001801000002000000000000000000000085100000ffffffff"
            "95000000000000001801000004000000000000000000000085100000ffffffff9500000000000000"
            "1e000000000000000400000000000000110000000000000090020000000000001200000000000000"
            "40000000000000001300000000000000100000000000000006000000000000003002000000000000"
            "0b000000000000001800000000000000050000000000000078020000000000000a00000000000000"
            "18000000000000001600000000000000000000000000000000000000000000000000000000000000"
            "0000000000000000000000000000000000000000000000000100000010000100e800000000000000"
            "00000000000000000c000000100000000000000000000000000000000000000000656e747279706f"
            "696e7400736f6c5f6c6f675f36345f0008010000000000000a000000020000003001000000000000"
            "0a0000000200000060010000000000000a0000000200000080010000000000000a00000002000000"
            "002e74657874002e64796e616d6963002e64796e73796d002e64796e737472002e72656c2e64796e"
            "002e7300000000000000000000000000000000000000000000000000000000000000000000000000"
            "00000000000000000000000000000000000000000000000000000000000000000100000001000000"
            "0600000000000000e800000000000000e800000000000000a8000000000000000000000000000000"
            "04000000000000000000000000000000070000000600000003000000000000009001000000000000"
            "9001000000000000a000000000000000040000000000000008000000000000001000000000000000"
            "100000000b0000000200000000000000300200000000000030020000000000004800000000000000"
            "04000000010000000800000000000000180000000000000018000000030000000200000000000000"
            "78020000000000007802000000000000180000000000000000000000000000000100000000000000"
            "00000000000000002000000009000000020000000000000090020000000000009002000000000000"
            "40000000000000000300000000000000080000000000000010000000000000002900000003000000"
            "00000000000000000000000000000000d0020000000000002c000000000000000000000000000000"
            "01000000000000000000000000000000"
        );

        // Disassembled program should have no labels.
        assert_eq!(
            disassemble_program(
                Program::from_bytes(&elf_bytes).unwrap(),
                false,
                AsmFormat::Default,
                true,
            )
            .unwrap(),
            r#"call 0xc
ja +0x5
lddw r1, 0x1
call sol_log_64_
call 0x7
exit
lddw r1, 0x3
call sol_log_64_
call 0x6
ja -0xa
exit
lddw r1, 0x2
call sol_log_64_
exit
lddw r1, 0x4
call sol_log_64_
exit
"#
        );

        // Disassembled program should have no labels.
        assert_eq!(
            disassemble_program(
                Program::from_bytes(&elf_bytes).unwrap(),
                false,
                AsmFormat::Llvm,
                true,
            )
            .unwrap(),
            r#"call 0xc
goto +0x5
r1 = 0x1 ll
call sol_log_64_
call 0x7
exit
r1 = 0x3 ll
call sol_log_64_
call 0x6
goto -0xa
exit
r1 = 0x2 ll
call sol_log_64_
exit
r1 = 0x4 ll
call sol_log_64_
exit
"#
        );
    }

    #[test]
    fn test_llvm_roundtrip() {
        let source = r#"
.globl entrypoint
.rodata
message: .ascii "Hello!"
.text
entrypoint:
  lddw r1, message
  call helper
  ja skip

loop:
  mov64 r1, 0x1
  ja done

skip:
  mov64 r1, 0x2
  ja loop

helper:
  mov64 r0, 0x7
  exit

done:
  exit
"#;
        let assembler = Assembler::new(AssemblerOption::default());
        let original = assembler.assemble(source).expect("failed to assemble source");

        let llvm_disassembly = disassemble_program(
            Program::from_bytes(&original).expect("failed to parse original bytes"),
            false,
            AsmFormat::Llvm,
            false,
        )
        .expect("failed to disassemble original bytes");

        let roundtrip = assembler
            .assemble(&llvm_disassembly)
            .expect("failed to reassemble llvm disassembly");

        assert_eq!(
            original, roundtrip,
            "llvm disassembly should roundtrip back to identical bytecode"
        );
    }
}
