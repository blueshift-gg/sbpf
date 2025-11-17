.globl entrypoint

entrypoint:
  ldxb r2, [r1+0x0]
  ldxh r3, [r1+0x1]
  ldxw r4, [r1+0x2]
  ldxdw r5, [r1+0x4]
  lddw r6, 0x1234
  exit
