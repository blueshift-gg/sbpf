.globl entrypoint

entrypoint:
  call my_fn
  exit

my_fn:
  ldxb r0, [r1+0]
  exit
