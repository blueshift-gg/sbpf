.globl entrypoint

entrypoint:
  lddw r4, 0x7
  mov64 r3, r10
  sub64 r3, 8
  stxdw [r3+0], r4 // stxdw with +ve offset
  add64 r3, 8
  ldxdw r2, [r3-8] // ldxdw with -ve offset
  call sol_log_64_ // r2 and r4 should both be 0x7

  lddw r4, 0x8
  mov64 r3, r10
  stxdw [r3-8], r4 // stxdw with -ve offset
  sub64 r3, 8
  ldxdw r2, [r3+0] // ldxdw with +ve offset
  call sol_log_64_ // r2 and r4 should both be 0x8
  exit