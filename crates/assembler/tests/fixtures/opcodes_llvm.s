// Test program that contains all supported opcodes (written in LLVM dialect).
.globl entrypoint

entrypoint:
  r1 += 0x10
  r1 -= 0x10
  r1 *= 0x10
  r1 /= 0x10
  r1 |= 0x10
  r1 &= 0x10
  r1 <<= 0x4
  r1 >>= 0x4
  r1 %= 0x10
  r1 ^= 0x10
  r1 = 0x7
  r1 s>>= 0x4

  r1 += r2
  r1 -= r2
  r1 *= r2
  r1 /= r2
  r1 |= r2
  r1 &= r2
  r1 <<= r2
  r1 >>= r2
  r1 %= r2
  r1 ^= r2
  r1 = r2
  r1 s>>= r2

  w1 += 0x10
  w1 -= 0x10
  w1 *= 0x10
  w1 /= 0x10
  w1 |= 0x10
  w1 &= 0x10
  w1 <<= 0x4
  w1 >>= 0x4
  w1 %= 0x10
  w1 ^= 0x10
  w1 = 0x7
  w1 s>>= 0x4

  w1 += w2
  w1 -= w2
  w1 *= w2
  w1 /= w2
  w1 |= w2
  w1 &= w2
  w1 <<= w2
  w1 >>= w2
  w1 %= w2
  w1 ^= w2
  w1 = w2
  w1 s>>= w2

  r1 = -r1
  w1 = -w1

  r1 = le16 r1
  r1 = le32 r1
  r1 = le64 r1
  r1 = be16 r1
  r1 = be32 r1
  r1 = be64 r1

  r1 = 0x123456789abcdef0 ll
  r1 = str_0000 ll

  w1 = *(u8 *)(r2 + 0x8)
  w1 = *(u16 *)(r2 + 0x8)
  w1 = *(u32 *)(r2 + 0x8)
  r1 = *(u64 *)(r2 + 0x8)

  *(u8 *)(r2 + 0x8) = 0x7
  *(u16 *)(r2 + 0x8) = 0x7
  *(u32 *)(r2 + 0x8) = 0x7
  *(u64 *)(r2 + 0x8) = 0x7
  *(u8 *)(r2 + 0x8) = w1
  *(u16 *)(r2 + 0x8) = w1
  *(u32 *)(r2 + 0x8) = w1
  *(u64 *)(r2 + 0x8) = r1

  goto jmp_0248

jmp_0248:
  if r1 == 0x1 goto jmp_02a0
  if r1 != 0x1 goto jmp_02a0
  if r1 > 0x1 goto jmp_02a0
  if r1 >= 0x1 goto jmp_02a0
  if r1 < 0x1 goto jmp_02a0
  if r1 <= 0x1 goto jmp_02a0
  if r1 s> 0x1 goto jmp_02a0
  if r1 s>= 0x1 goto jmp_02a0
  if r1 s< 0x1 goto jmp_02a0
  if r1 s<= 0x1 goto jmp_02a0
  if r1 & 0x1 goto jmp_02a0

jmp_02a0:
  if r1 == r2 goto jmp_0308
  if r1 != r2 goto jmp_0308
  if r1 > r2 goto jmp_0308
  if r1 >= r2 goto jmp_0308
  if r1 < r2 goto jmp_0308
  if r1 <= r2 goto jmp_0308
  if r1 s> r2 goto jmp_0308
  if r1 s>= r2 goto jmp_0308
  if r1 s< r2 goto jmp_0308
  if r1 s<= r2 goto jmp_0308
  if r1 & r2 goto jmp_0308
  call sol_log_
  callx r1

jmp_0308:
  exit

.rodata
  str_0000: .ascii "hello"