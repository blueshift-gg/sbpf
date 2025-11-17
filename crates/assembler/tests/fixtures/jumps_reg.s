.globl entrypoint

entrypoint:
  lddw r1, 0x10
  lddw r2, 0x10
  jeq r1, r2, target
  jne r1, r2, target
  jgt r1, r2, target
  jge r1, r2, target
  jlt r1, r2, target
  jle r1, r2, target
  jsgt r1, r2, target
  jsge r1, r2, target
  jslt r1, r2, target
  jsle r1, r2, target
  jset r1, r2, target
  exit

target:
  exit
