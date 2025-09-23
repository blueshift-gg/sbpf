.globl e

e:
    lddw r1, msg
    lddw r2, 12
    call sol_log_
    exit

.rodata
    msg: .ascii "Hello world."