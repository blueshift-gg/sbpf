.globl entrypoint

entrypoint:
  lddw r1, 0x10
  jeq32 r1, 0x10, target
  jne32 r1, 0x20, target
  jgt32 r1, 0x5, target
  jge32 r1, 0x10, target
  jlt32 r1, 0x20, target
  jle32 r1, 0x10, target
  jsgt32 r1, 0x5, target
  jsge32 r1, 0x10, target
  jslt32 r1, 0x20, target
  jsle32 r1, 0x10, target
  jset32 r1, 0x10, target
  exit

target:
  exit