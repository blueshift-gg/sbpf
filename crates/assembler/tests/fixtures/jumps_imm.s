.globl entrypoint

entrypoint:
  lddw r1, 0x10
  jeq r1, 0x10, target
  jne r1, 0x20, target
  jgt r1, 0x5, target
  jge r1, 0x10, target
  jlt r1, 0x20, target
  jle r1, 0x10, target
  jsgt r1, 0x5, target
  jsge r1, 0x10, target
  jslt r1, 0x20, target
  jsle r1, 0x10, target
  jset r1, 0x10, target
  exit

target:
  exit
