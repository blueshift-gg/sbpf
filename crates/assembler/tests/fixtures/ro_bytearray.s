.rodata
  x: .byte 0, 0x01, 2, 0x10

.text
.globl entrypoint
entrypoint:
  lddw r6, x
  ldxb r1, [r6 + 0]   # Load x[0] into r1
  ldxb r2, [r6 + 1]   # Load x[1] into r2
  ldxb r3, [r6 + 2]   # Load x[2] into r3
  ldxb r4, [r6 + 3]   # Load x[3] into r4
  call sol_log_64_
  exit
