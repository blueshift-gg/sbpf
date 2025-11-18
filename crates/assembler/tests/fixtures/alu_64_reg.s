.globl entrypoint

entrypoint:
  add64 r1, r2
  sub64 r2, r3
  mul64 r3, r1
  lddw r1, 0x1
  div64 r4, r1
  // sdiv64 r5, r6
  mod64 r6, r1
  smod64 r7, r8
  or64 r8, r9
  and64 r9, r1
  xor64 r1, r2
  mov64 r1, r2
  lsh64 r2, r1
  rsh64 r3, r1
  arsh64 r4, r5
  exit
