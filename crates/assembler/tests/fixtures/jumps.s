.globl e

e:
    lddw r1, 0x1
    jeq r1, 0x1, target_1
    jeq r1, 0x2, target_2
    exit
    
target_1:
    exit

target_2:
    exit