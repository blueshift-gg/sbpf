// Test program that contains all supported opcodes.
.globl entrypoint

entrypoint:
  add64 r1, 0x10
  sub64 r1, 0x10
  mul64 r1, 0x10
  div64 r1, 0x10
  or64 r1, 0x10
  and64 r1, 0x10
  lsh64 r1, 0x4
  rsh64 r1, 0x4
  mod64 r1, 0x10
  xor64 r1, 0x10
  mov64 r1, 0x7
  arsh64 r1, 0x4

  add64 r1, r2
  sub64 r1, r2
  mul64 r1, r2
  div64 r1, r2
  or64 r1, r2
  and64 r1, r2
  lsh64 r1, r2
  rsh64 r1, r2
  mod64 r1, r2
  xor64 r1, r2
  mov64 r1, r2
  arsh64 r1, r2

  add32 r1, 0x10
  sub32 r1, 0x10
  mul32 r1, 0x10
  div32 r1, 0x10
  or32 r1, 0x10
  and32 r1, 0x10
  lsh32 r1, 0x4
  rsh32 r1, 0x4
  mod32 r1, 0x10
  xor32 r1, 0x10
  mov32 r1, 0x7
  arsh32 r1, 0x4

  add32 r1, r2
  sub32 r1, r2
  mul32 r1, r2
  div32 r1, r2
  or32 r1, r2
  and32 r1, r2
  lsh32 r1, r2
  rsh32 r1, r2
  mod32 r1, r2
  xor32 r1, r2
  mov32 r1, r2
  arsh32 r1, r2

  neg64 r1
  neg32 r1

  le16 r1
  le32 r1
  le64 r1
  be16 r1
  be32 r1
  be64 r1

  lddw r1, 0x123456789abcdef0
  lddw r1, str_0000

  ldxb r1, [r2+0x8]
  ldxh r1, [r2+0x8]
  ldxw r1, [r2+0x8]
  ldxdw r1, [r2+0x8]

  stb [r2+0x8], 0x7
  sth [r2+0x8], 0x7
  stw [r2+0x8], 0x7
  stdw [r2+0x8], 0x7
  stxb [r2+0x8], r1
  stxh [r2+0x8], r1
  stxw [r2+0x8], r1
  stxdw [r2+0x8], r1

  ja jmp_0248

jmp_0248:
  jeq r1, 0x1, jmp_02a0
  jne r1, 0x1, jmp_02a0
  jgt r1, 0x1, jmp_02a0
  jge r1, 0x1, jmp_02a0
  jlt r1, 0x1, jmp_02a0
  jle r1, 0x1, jmp_02a0
  jsgt r1, 0x1, jmp_02a0
  jsge r1, 0x1, jmp_02a0
  jslt r1, 0x1, jmp_02a0
  jsle r1, 0x1, jmp_02a0
  jset r1, 0x1, jmp_02a0

jmp_02a0:
  jeq r1, r2, jmp_0308
  jne r1, r2, jmp_0308
  jgt r1, r2, jmp_0308
  jge r1, r2, jmp_0308
  jlt r1, r2, jmp_0308
  jle r1, r2, jmp_0308
  jsgt r1, r2, jmp_0308
  jsge r1, r2, jmp_0308
  jslt r1, r2, jmp_0308
  jsle r1, r2, jmp_0308
  jset r1, r2, jmp_0308

  call sol_log_
  callx r1

jmp_0308:
  exit

.rodata
  str_0000: .ascii "hello"