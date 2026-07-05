.globl entrypoint

entrypoint:
  r1 = 0x10 ll
  r2 = 0x10 ll
  if w1 == w2 goto jmp_0080
  if w1 != w2 goto jmp_0080
  if w1 > w2 goto jmp_0080
  if w1 >= w2 goto jmp_0080
  if w1 < w2 goto jmp_0080
  if w1 <= w2 goto jmp_0080
  if w1 s> w2 goto jmp_0080
  if w1 s>= w2 goto jmp_0080
  if w1 s< w2 goto jmp_0080
  if w1 s<= w2 goto jmp_0080
  if w1 & w2 goto jmp_0080
  exit

jmp_0080:
  exit