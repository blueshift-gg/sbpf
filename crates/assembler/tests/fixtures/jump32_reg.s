.globl entrypoint

entrypoint:
  lddw r1, 0x10
  lddw r2, 0x10
  jeq32 r1, r2, target
  jne32 r1, r2, target
  jgt32 r1, r2, target
  jge32 r1, r2, target
  jlt32 r1, r2, target
  jle32 r1, r2, target
  jsgt32 r1, r2, target
  jsge32 r1, r2, target
  jslt32 r1, r2, target
  jsle32 r1, r2, target
  jset32 r1, r2, target
  exit

target:
  exit