.globl entrypoint

entrypoint:
  stxb [r1+0x0], r2
  stxh [r1+0x1], r3
  stxw [r1+0x4], r4
  stxdw [r1+0x8], r5
  exit
