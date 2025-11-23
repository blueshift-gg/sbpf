.globl entrypoint

entrypoint:
  lddw r4, 64 // decimal number
  lddw r3, 0x0040 // hex number
  add64 r4, -32 // -ve decimal number
  add64 r4, -0x20 // -ve hex number
  call sol_log_64_
  exit