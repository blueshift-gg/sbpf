.globl entrypoint

entrypoint:
  mov64 r3, r10
  sub64 r3, 8
  lddw r4, 0x7
  stxdw [r3+0], r4 // +ve offset
  add64 r3, 8
  ldxdw r2, [r3-8] // -ve offset
  call sol_log_64_ // r2 and r4 should both be 0x7
  exit