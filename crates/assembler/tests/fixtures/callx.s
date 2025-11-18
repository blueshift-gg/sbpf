.globl entrypoint

entrypoint:
  lddw r8, helper_function
  callx r8
  exit

helper_function:
  exit
