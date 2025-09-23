.equ CONST_A, 0x1
.equ CONST_B, 2 * (3 + 4)

.globl e
e:
    lddw r1, 0x1
    add64 r1, CONST_A
    add64 r1, CONST_B
    exit


