.globl entrypoint

entrypoint:
  lddw r1, 0x4
  neg64 r1
  lddw r2, 0x10
  neg32 r2
  exit
