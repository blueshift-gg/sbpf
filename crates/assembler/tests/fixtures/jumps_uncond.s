.globl entrypoint

entrypoint:
  ja target1

1:
  ja 2f

target1:
  ja 1b

2:
  ja target2

target2:
  exit
