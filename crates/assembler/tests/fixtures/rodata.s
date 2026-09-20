.rodata
  x: .byte 0, 0x01, 2, 0x10

.rodata
  msg: .ascii "Hello world."

.rodata
  a: .quad 0x3
  b: .quad a          # points to a in .rodata
  c: .quad func       # points to func in .text

.text
.globl entrypoint
entrypoint:
  lddw r6, x
  ldxb r1, [r6 + 0]   # Load x[0] into r1
  ldxb r2, [r6 + 1]   # Load x[1] into r2
  ldxb r3, [r6 + 2]   # Load x[2] into r3
  ldxb r4, [r6 + 3]   # Load x[3] into r4
  call sol_log_64_

  lddw r1, msg
  lddw r2, 12
  call sol_log_

  lddw r4, b          # Load b into r4
  ldxdw r4, [r4+0x0]  # Load address of a from b
  ldxdw r1, [r4+0x0]  # Load value of a (0x3) into r1
  lddw r8, c          # Load c into r8
  ldxdw r8, [r8+0x0]  # Load func address into r8
  callx r8            # Call func

  exit

func:
  call sol_log_64_
  exit

