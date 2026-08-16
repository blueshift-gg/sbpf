<!-- This document is auto-generated. Do not edit manually. -->

# sBPF Opcode Reference

## Load Immediate Operation

Load a 64-bit immediate value into a register.

| Mnemonic | Opcode |  Usage  | Note |
|----------|--------|---------|------|
| lddw | `0x18` | `lddw dst, imm` | - |

## Load Memory Operations

Load a value from memory into a register.

| Mnemonic | Opcode |  Usage  | Note |
|----------|--------|---------|------|
| ldxb | `0x71` | `ldxb dst, [src + off]` | - |
| ldxh | `0x69` | `ldxh dst, [src + off]` | - |
| ldxw | `0x61` | `ldxw dst, [src + off]` | - |
| ldxdw | `0x79` | `ldxdw dst, [src + off]` | - |

## Store Immediate Operations

Store an immediate value to memory.

| Mnemonic | Opcode |  Usage  | Note |
|----------|--------|---------|------|
| stb | `0x72` | `stb [dst + off], imm` | - |
| sth | `0x6a` | `sth [dst + off], imm` | - |
| stw | `0x62` | `stw [dst + off], imm` | - |
| stdw | `0x7a` | `stdw [dst + off], imm` | - |

## Store Register Operations

Store a register value to memory.

| Mnemonic | Opcode |  Usage  | Note |
|----------|--------|---------|------|
| stxb | `0x73` | `stxb [dst + off], src` | - |
| stxh | `0x6b` | `stxh [dst + off], src` | - |
| stxw | `0x63` | `stxw [dst + off], src` | - |
| stxdw | `0x7b` | `stxdw [dst + off], src` | - |

## Binary Immediate Operations

Perform a binary ALU operation using an immediate operand.

| Mnemonic | Opcode |  Usage  | Note |
|----------|--------|---------|------|
| add32 | `0x04` | `add32 dst, imm` | - |
| sub32 | `0x14` | `sub32 dst, imm` | - |
| mul32 | `0x24` | `mul32 dst, imm` | - |
| div32 | `0x34` | `div32 dst, imm` | - |
| or32 | `0x44` | `or32 dst, imm` | - |
| and32 | `0x54` | `and32 dst, imm` | - |
| lsh32 | `0x64` | `lsh32 dst, imm` | - |
| rsh32 | `0x74` | `rsh32 dst, imm` | - |
| mod32 | `0x94` | `mod32 dst, imm` | - |
| xor32 | `0xa4` | `xor32 dst, imm` | - |
| mov32 | `0xb4` | `mov32 dst, imm` | - |
| arsh32 | `0xc4` | `arsh32 dst, imm` | - |
| lmul32 | `0x86` | `lmul32 dst, imm` | v2 only |
| udiv32 | `0x46` | `udiv32 dst, imm` | v2 only |
| urem32 | `0x66` | `urem32 dst, imm` | v2 only |
| sdiv32 | `0xc6` | `sdiv32 dst, imm` | v2 only |
| srem32 | `0xe6` | `srem32 dst, imm` | v2 only |
| add64 | `0x07` | `add64 dst, imm` | - |
| sub64 | `0x17` | `sub64 dst, imm` | - |
| mul64 | `0x27` | `mul64 dst, imm` | - |
| div64 | `0x37` | `div64 dst, imm` | - |
| or64 | `0x47` | `or64 dst, imm` | - |
| and64 | `0x57` | `and64 dst, imm` | - |
| lsh64 | `0x67` | `lsh64 dst, imm` | - |
| rsh64 | `0x77` | `rsh64 dst, imm` | - |
| mod64 | `0x97` | `mod64 dst, imm` | - |
| xor64 | `0xa7` | `xor64 dst, imm` | - |
| mov64 | `0xb7` | `mov64 dst, imm` | - |
| arsh64 | `0xc7` | `arsh64 dst, imm` | - |
| hor64 | `0xf7` | `hor64 dst, imm` | v2 only |
| lmul64 | `0x96` | `lmul64 dst, imm` | v2 only |
| uhmul64 | `0x36` | `uhmul64 dst, imm` | v2 only |
| udiv64 | `0x56` | `udiv64 dst, imm` | v2 only |
| urem64 | `0x76` | `urem64 dst, imm` | v2 only |
| shmul64 | `0xb6` | `shmul64 dst, imm` | v2 only |
| sdiv64 | `0xd6` | `sdiv64 dst, imm` | v2 only |
| srem64 | `0xf6` | `srem64 dst, imm` | v2 only |

## Binary Register Operations

Perform a binary ALU operation using a register operand.

| Mnemonic | Opcode |  Usage  | Note |
|----------|--------|---------|------|
| add32 | `0x0c` | `add32 dst, src` | - |
| sub32 | `0x1c` | `sub32 dst, src` | - |
| mul32 | `0x2c` | `mul32 dst, src` | - |
| div32 | `0x3c` | `div32 dst, src` | - |
| or32 | `0x4c` | `or32 dst, src` | - |
| and32 | `0x5c` | `and32 dst, src` | - |
| lsh32 | `0x6c` | `lsh32 dst, src` | - |
| rsh32 | `0x7c` | `rsh32 dst, src` | - |
| mod32 | `0x9c` | `mod32 dst, src` | - |
| xor32 | `0xac` | `xor32 dst, src` | - |
| mov32 | `0xbc` | `mov32 dst, src` | - |
| arsh32 | `0xcc` | `arsh32 dst, src` | - |
| lmul32 | `0x8e` | `lmul32 dst, src` | v2 only |
| udiv32 | `0x4e` | `udiv32 dst, src` | v2 only |
| urem32 | `0x6e` | `urem32 dst, src` | v2 only |
| sdiv32 | `0xce` | `sdiv32 dst, src` | v2 only |
| srem32 | `0xee` | `srem32 dst, src` | v2 only |
| add64 | `0x0f` | `add64 dst, src` | - |
| sub64 | `0x1f` | `sub64 dst, src` | - |
| mul64 | `0x2f` | `mul64 dst, src` | - |
| div64 | `0x3f` | `div64 dst, src` | - |
| or64 | `0x4f` | `or64 dst, src` | - |
| and64 | `0x5f` | `and64 dst, src` | - |
| lsh64 | `0x6f` | `lsh64 dst, src` | - |
| rsh64 | `0x7f` | `rsh64 dst, src` | - |
| mod64 | `0x9f` | `mod64 dst, src` | - |
| xor64 | `0xaf` | `xor64 dst, src` | - |
| mov64 | `0xbf` | `mov64 dst, src` | - |
| arsh64 | `0xcf` | `arsh64 dst, src` | - |
| lmul64 | `0x9e` | `lmul64 dst, src` | v2 only |
| uhmul64 | `0x3e` | `uhmul64 dst, src` | v2 only |
| udiv64 | `0x5e` | `udiv64 dst, src` | v2 only |
| urem64 | `0x7e` | `urem64 dst, src` | v2 only |
| shmul64 | `0xbe` | `shmul64 dst, src` | v2 only |
| sdiv64 | `0xde` | `sdiv64 dst, src` | v2 only |
| srem64 | `0xfe` | `srem64 dst, src` | v2 only |

## Unary Operations

Perform a unary ALU operation on a register operand.

| Mnemonic | Opcode |  Usage  | Note |
|----------|--------|---------|------|
| neg32 | `0x84` | `neg32 dst` | - |
| neg64 | `0x87` | `neg64 dst` | - |

## Endian Operations

Perform byte-order conversion on a register value.

| Mnemonic | Opcode |  Usage  | Note |
|----------|--------|---------|------|
| le | `0xd4` | `le16 dst / le32 dst / le64 dst` | - |
| be | `0xdc` | `be16 dst / be32 dst / be64 dst` | - |

## Jump Operation

Perform an unconditional jump to a target address.

| Mnemonic | Opcode |  Usage  | Note |
|----------|--------|---------|------|
| ja | `0x05` | `ja off` | - |

## Jump Immediate Operations

Perform a conditional jump by comparing a register with an immediate.

| Mnemonic | Opcode |  Usage  | Note |
|----------|--------|---------|------|
| jeq | `0x15` | `jeq dst, imm, off` | - |
| jgt | `0x25` | `jgt dst, imm, off` | - |
| jge | `0x35` | `jge dst, imm, off` | - |
| jlt | `0xa5` | `jlt dst, imm, off` | - |
| jle | `0xb5` | `jle dst, imm, off` | - |
| jset | `0x45` | `jset dst, imm, off` | - |
| jne | `0x55` | `jne dst, imm, off` | - |
| jsgt | `0x65` | `jsgt dst, imm, off` | - |
| jsge | `0x75` | `jsge dst, imm, off` | - |
| jslt | `0xc5` | `jslt dst, imm, off` | - |
| jsle | `0xd5` | `jsle dst, imm, off` | - |

## Jump Register Operations

Perform a conditional jump by comparing two registers.

| Mnemonic | Opcode |  Usage  | Note |
|----------|--------|---------|------|
| jeq | `0x1d` | `jeq dst, src, off` | - |
| jgt | `0x2d` | `jgt dst, src, off` | - |
| jge | `0x3d` | `jge dst, src, off` | - |
| jlt | `0xad` | `jlt dst, src, off` | - |
| jle | `0xbd` | `jle dst, src, off` | - |
| jset | `0x4d` | `jset dst, src, off` | - |
| jne | `0x5d` | `jne dst, src, off` | - |
| jsgt | `0x6d` | `jsgt dst, src, off` | - |
| jsge | `0x7d` | `jsge dst, src, off` | - |
| jslt | `0xcd` | `jslt dst, src, off` | - |
| jsle | `0xdd` | `jsle dst, src, off` | - |

## Jump32 Immediate Operations

Perform a 32-bit conditional jump by comparing a register with an immediate.

| Mnemonic | Opcode |  Usage  | Note |
|----------|--------|---------|------|
| jeq32 | `0x16` | `jeq32 dst, imm, off` | v3 only |
| jgt32 | `0x26` | `jgt32 dst, imm, off` | v3 only |
| jge32 | `0x36` | `jge32 dst, imm, off` | v3 only |
| jlt32 | `0xa6` | `jlt32 dst, imm, off` | v3 only |
| jle32 | `0xb6` | `jle32 dst, imm, off` | v3 only |
| jset32 | `0x46` | `jset32 dst, imm, off` | v3 only |
| jne32 | `0x56` | `jne32 dst, imm, off` | v3 only |
| jsgt32 | `0x66` | `jsgt32 dst, imm, off` | v3 only |
| jsge32 | `0x76` | `jsge32 dst, imm, off` | v3 only |
| jslt32 | `0xc6` | `jslt32 dst, imm, off` | v3 only |
| jsle32 | `0xd6` | `jsle32 dst, imm, off` | v3 only |

## Jump32 Register Operations

Perform a 32-bit conditional jump by comparing two registers.

| Mnemonic | Opcode |  Usage  | Note |
|----------|--------|---------|------|
| jeq32 | `0x1e` | `jeq32 dst, src, off` | v3 only |
| jgt32 | `0x2e` | `jgt32 dst, src, off` | v3 only |
| jge32 | `0x3e` | `jge32 dst, src, off` | v3 only |
| jlt32 | `0xae` | `jlt32 dst, src, off` | v3 only |
| jle32 | `0xbe` | `jle32 dst, src, off` | v3 only |
| jset32 | `0x4e` | `jset32 dst, src, off` | v3 only |
| jne32 | `0x5e` | `jne32 dst, src, off` | v3 only |
| jsgt32 | `0x6e` | `jsgt32 dst, src, off` | v3 only |
| jsge32 | `0x7e` | `jsge32 dst, src, off` | v3 only |
| jslt32 | `0xce` | `jslt32 dst, src, off` | v3 only |
| jsle32 | `0xde` | `jsle32 dst, src, off` | v3 only |

## Call Immediate Operation

Call a function using an immediate target address.

| Mnemonic | Opcode |  Usage  | Note |
|----------|--------|---------|------|
| call | `0x85` | `call imm` | - |

## Call Register Operation

Call a function using a target address stored in a register.

| Mnemonic | Opcode |  Usage  | Note |
|----------|--------|---------|------|
| callx | `0x8d` | `callx dst` | - |

## Exit Operation

Return from function or exit program.

| Mnemonic | Opcode |  Usage  | Note |
|----------|--------|---------|------|
| exit | `0x95` | `exit` | - |
