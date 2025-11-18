.globl entrypoint

entrypoint:
  lddw r1, 0x1234
  be16 r1
  lddw r2, 0x12345678
  be32 r2
  lddw r3, 0x123456789abcdef0
  be64 r3
  lddw r4, 0x1234
  le16 r4
  lddw r5, 0x12345678
  le32 r5
  lddw r6, 0x123456789abcdef0
  le64 r6
  exit
