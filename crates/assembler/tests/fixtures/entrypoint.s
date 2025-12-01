.globl entry_2

entry_1:
    lddw r1, 0x1
    call sol_log_64_
    exit

entry_2:
    lddw r1, 0x2
    call sol_log_64_
    exit