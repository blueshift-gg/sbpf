.globl entrypoint
entrypoint:
  lddw r1, message
  lddw r2, 14
  call sol_log_
  exit
.rodata
  message: .ascii "Hello, Solana!"
