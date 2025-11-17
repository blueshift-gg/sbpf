.globl entrypoint

entrypoint:
  add32 r1, r2
  sub32 r2, r3
  mul32 r3, r1
  lddw r1, 0x1
  div32 r4, r1
  // sdiv32 r5, r6
  mod32 r6, r1
  smod32 r7, r8
  or32 r8, r9
  and32 r9, r1
  xor32 r1, r2
  mov32 r1, r2
  lsh32 r2, r1
  rsh32 r3, r1
  arsh32 r4, r5
  exit
