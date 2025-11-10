.globl entrypoint

entrypoint:
    lddw r1, 0x4
    call sol_log_64_
    neg64 r1
    # should log 0xfffffffffffffffc
    call sol_log_64_
    exit