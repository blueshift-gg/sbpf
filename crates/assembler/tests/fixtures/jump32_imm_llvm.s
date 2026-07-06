.globl entrypoint

entrypoint:
  r1 = 0x10 ll
  if w1 == 0x10 goto jmp_0070
  if w1 != 0x20 goto jmp_0070
  if w1 > 0x5 goto jmp_0070
  if w1 >= 0x10 goto jmp_0070
  if w1 < 0x20 goto jmp_0070
  if w1 <= 0x10 goto jmp_0070
  if w1 s> 0x5 goto jmp_0070
  if w1 s>= 0x10 goto jmp_0070
  if w1 s< 0x20 goto jmp_0070
  if w1 s<= 0x10 goto jmp_0070
  if w1 & 0x10 goto jmp_0070
  exit

jmp_0070:
  exit