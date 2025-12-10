.globl entry_2

entry_0:
    lddw r1, 0x0
    call sol_log_64_
    exit

entry_1:
    lddw r1, 0x1
    call sol_log_64_
    jne r1, 0x2, entry_0

entry_2:
    lddw r1, 0x2
    call sol_log_64_
    ja entry_1