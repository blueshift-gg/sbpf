
.equ NUM_ACCOUNTS, 0x0000

.equ OWNER_HEADER, 0x0008
.equ OWNER_KEY, 0x0010
.equ OWNER_OWNER, 0x0030
.equ OWNER_LAMPORTS, 0x0050
.equ OWNER_DATA_LEN, 0x0058
.equ OWNER_DATA, 0x0060
.equ OWNER_RENT_EPOCH, 0x2860

.equ VAULT_HEADER, 0x2868
.equ VAULT_KEY, 0x2870
.equ VAULT_OWNER, 0x2890
.equ VAULT_LAMPORTS, 0x28b0
.equ VAULT_DATA_LEN, 0x28b8
.equ VAULT_DATA, 0x28c0
.equ VAULT_RENT_EPOCH, 0x50c0

.equ SYSTEM_PROGRAM_HEADER, 0x50c8
.equ SYSTEM_PROGRAM_KEY, 0x50d0
.equ SYSTEM_PROGRAM_OWNER, 0x50f0
.equ SYSTEM_PROGRAM_LAMPORTS, 0x5110
.equ SYSTEM_PROGRAM_DATA_LEN, 0x5118
.equ SYSTEM_PROGRAM_DATA, 0x5120
.equ SYSTEM_PROGRAM_RENT_EPOCH, 0x7930

.equ INSTRUCTION_DATA_LEN, 0x7938
.equ INSTRUCTION_DATA, 0x7940
.equ PROGRAM_ID, 0x794a

.equ VAULT_SEED, 0x746c756176


.globl entrypoint


entrypoint:

  ldxdw r4, [r1 + INSTRUCTION_DATA_LEN]
  jne r4, 10, error_invalid_instruction

  ##########################
  ##     Prepare seeds    ##
  ##########################

  mov64 r9, r10
  sub64 r9, 8
  lddw r2, VAULT_SEED
  stxdw [r9 + 0], r2

  mov64 r8, r9
  sub64 r8, 8
  ldxb r2, [r1 + INSTRUCTION_DATA + 1]
  stxdw [r8 + 0], r2

  mov64 r5, r8
  sub64 r5, 48

  # First seed ("vault")
  mov64 r2, r5                 
  stxdw [r2 + 0], r9          
  lddw r3, 5
  stxdw [r2 + 8], r3           

  # Second seed (owner key)
  add64 r2, 16
  mov64 r4, r1
  add64 r4, OWNER_KEY
  stxdw [r2 + 0], r4
  lddw r3, 32
  stxdw [r2 + 8], r3

  # bump
  add64 r2, 16
  stxdw [r2 + 0], r8
  lddw r3, 1
  stxdw [r2 + 8], r3

  ##########################
  ##      Validate PDA    ##
  ##########################

  mov64 r7, r1
  mov64 r1, r5 
  lddw r2, 3 
  mov64 r3, r7
  add64 r3, PROGRAM_ID 
  mov64 r4, r5 
  sub64 r4, 32
  call sol_create_program_address
  
  mov64 r1, r4
  mov64 r2, r7
  add64 r2, VAULT_KEY
  lddw r3, 32
  mov64 r4, r5 
  sub64 r4, 4
  call sol_memcmp_

  ldxw r1, [r4 + 0]
  jne r1, 0x0, error_invalid_pda


  # Branch based on instruction type.
  mov64 r1, r7
  ldxb r4, [r1 + INSTRUCTION_DATA + 0]
  jeq r4, 0x0, deposit
  jeq r4, 0x1, withdraw
  ja error_invalid_instruction


deposit:

  ##########################
  ## Set up account metas ##
  ##########################

  mov64 r9, r5
  sub64 r9, 32

  # Owner
  mov64 r2, r9
  mov64 r3, r1
  add64 r3, OWNER_KEY
  stxdw [r2 + 0], r3                                              # pubkey
  ldxb r3, [r1 + OWNER_HEADER + 2]
  stxb [r2 + 8], r3                                               # is_writable
  ldxb r3, [r1 + OWNER_HEADER + 1]
  stxb [r2 + 9], r3                                               # is_signer

  # Vault
  add64 r2, 16
  mov64 r3, r1
  add64 r3, VAULT_KEY
  stxdw [r2 + 0], r3                                              # pubkey
  ldxb r3, [r1 + VAULT_HEADER + 2]
  stxb [r2 + 8], r3                                               # is_writable
  ldxb r3, [r1 + VAULT_HEADER + 1]
  stxb [r2 + 9], r3                                               # is_signer

  #############################
  ## Set up instruction data ##
  #############################

  mov64 r8, r9
  sub64 r8, 16

  mov64 r2, r8
  lddw r3, 2                                                      # Instruction discriminator (2 = Transfer)
  stxw [r2 + 0], r3
  ldxdw r3, [r1 + INSTRUCTION_DATA + 2]                     
  stxdw [r2 + 4], r3                                              # Lamports to transfer

  ############################
  ## Set up the instruction ##
  ############################

  mov64 r7, r8
  sub64 r7, 40

  mov64 r2, r7
  mov64 r3, r1
  add64 r3, SYSTEM_PROGRAM_KEY
  stxdw [r2 + 0], r3                                              # program_id
  mov64 r3, r9
  stxdw [r2 + 8], r3                                              # accounts      
  lddw r3, 2
  stxdw [r2 + 16], r3                                             # account_len
  mov64 r3, r8
  stxdw [r2 + 24], r3                                             # data                   
  lddw r3, 12
  stxdw [r2 + 32], r3                                             # data_len

  ##########################
  ## Set up account infos ##
  ##########################

  mov64 r6, r7
  sub64 r6, 112

  # Owner
  mov64 r2, r6
  mov64 r3, r1 
  add64 r3, OWNER_KEY
  stxdw [r2 + 0], r3                                              # key
  mov64 r3, r1 
  add64 r3, OWNER_LAMPORTS
  stxdw [r2 + 8], r3                                              # lamports
  ldxdw r3, [r1 + OWNER_DATA_LEN]
  stxdw [r2 + 16], r3                                             # data_len
  mov64 r3, r1 
  add64 r3, OWNER_DATA
  stxdw [r2 + 24], r3                                             # data
  mov64 r3, r1 
  add64 r3, OWNER_OWNER
  stxdw [r2 + 32], r3                                             # owner
  ldxdw r3, [r1 + OWNER_RENT_EPOCH]
  stxdw [r2 + 40], r3                                             # rent_epoch
  ldxb r3, [r1 + OWNER_HEADER + 1]
  stxb [r2 + 48], r3                                              # is_signer
  ldxb r3, [r1 + OWNER_HEADER + 2]
  stxb [r2 + 49], r3                                              # is_writable
  ldxb r3, [r1 + OWNER_HEADER + 3]
  stxb [r2 + 50], r3                                              # is_executable

  # Vault
  add64 r2, 56
  mov64 r3, r1
  add64 r3, VAULT_KEY
  stxdw [r2 + 0], r3
  mov64 r3, r1                                                    # key
  add64 r3, VAULT_LAMPORTS
  stxdw [r2 + 8], r3                                              # lamports
  ldxdw r3, [r1 + VAULT_DATA_LEN]
  stxdw [r2 + 16], r3                                             # data_len
  mov64 r3, r1
  add64 r3, VAULT_DATA
  stxdw [r2 + 24], r3                                             # data
  mov64 r3, r1
  add64 r3, VAULT_OWNER
  stxdw [r2 + 32], r3                                             # owner
  ldxdw r3, [r1 + VAULT_RENT_EPOCH]
  stxdw [r2 + 40], r3                                             # rent_epoch
  ldxb r3, [r1 + VAULT_HEADER + 1]
  stxb [r2 + 48], r3                                              # is_signer
  ldxb r3, [r1 + VAULT_HEADER + 2]
  stxb [r2 + 49], r3                                              # is_writable
  ldxb r3, [r1 + VAULT_HEADER + 3]
  stxb [r2 + 50], r3                                              # is_executable

  ####################
  ## Invoke the CPI ##
  ####################
  
  mov64 r1, r7                                                    # Instruction
  mov64 r2, r6                                                    # Account infos
  lddw r3, 2                                                      # Number of account infos
  lddw r4, 0                                                      # Seeds (none required)
  lddw r5, 0                                                      # Seeds count
  call sol_invoke_signed_c

  exit

withdraw:

  ##########################
  ## Set up account metas ##
  ##########################

  mov64 r9, r5
  sub64 r9, 32

  # Vault
  mov64 r2, r9
  mov64 r3, r1
  add64 r3, VAULT_KEY
  stxdw [r2 + 0], r3                                              # pubkey
  ldxb r3, [r1 + VAULT_HEADER + 2]
  stxb [r2 + 8], r3                                               # is_writable
  ldxb r3, [r1 + VAULT_HEADER + 1]
  lddw r3, 1
  stxb [r2 + 9], r3                                               # is_signer

  # Owner
  add64 r2, 16
  mov64 r3, r1
  add64 r3, OWNER_KEY
  stxdw [r2 + 0], r3                                              # pubkey
  ldxb r3, [r1 + OWNER_HEADER + 2]
  stxb [r2 + 8], r3                                               # is_writable
  ldxb r3, [r1 + OWNER_HEADER + 1]
  stxb [r2 + 9], r3                                               # is_signer

  #############################
  ## Set up instruction data ##
  #############################

  mov64 r8, r9
  sub64 r8, 16

  mov64 r2, r8
  lddw r3, 2                                                      # Instruction discriminator (2 = Transfer)
  stxw [r2 + 0], r3
  ldxdw r3, [r1 + INSTRUCTION_DATA + 2]                                     
  stxdw [r2 + 4], r3                                              # Lamports to transfer

  ############################
  ## Set up the instruction ##
  ############################

  mov64 r7, r8
  sub64 r7, 40

  mov64 r2, r7
  mov64 r3, r1
  add64 r3, SYSTEM_PROGRAM_KEY
  stxdw [r2 + 0], r3                                              # program_id
  mov64 r3, r9
  stxdw [r2 + 8], r3                                              # accounts      
  lddw r3, 2
  stxdw [r2 + 16], r3                                             # account_len
  mov64 r3, r8
  stxdw [r2 + 24], r3                                             # data                   
  lddw r3, 12
  stxdw [r2 + 32], r3                                             # data_len

  ##########################
  ## Set up account infos ##
  ##########################

  mov64 r6, r7
  sub64 r6, 112

  # Vault
  mov64 r2, r6
  mov64 r3, r1 
  add64 r3, VAULT_KEY
  stxdw [r2 + 0], r3                                              # key
  mov64 r3, r1 
  add64 r3, VAULT_LAMPORTS
  stxdw [r2 + 8], r3                                              # lamports
  ldxdw r3, [r1 + VAULT_DATA_LEN]
  stxdw [r2 + 16], r3                                             # data_len
  mov64 r3, r1 
  add64 r3, VAULT_DATA
  stxdw [r2 + 24], r3                                             # data
  mov64 r3, r1 
  add64 r3, VAULT_OWNER
  stxdw [r2 + 32], r3                                             # owner
  ldxdw r3, [r1 + VAULT_RENT_EPOCH]
  stxdw [r2 + 40], r3                                             # rent_epoch
  ldxb r3, [r1 + VAULT_HEADER + 1]
  lddw r3, 1
  stxb [r2 + 48], r3                                              # is_signer
  ldxb r3, [r1 + VAULT_HEADER + 2]
  stxb [r2 + 49], r3                                              # is_writable
  ldxb r3, [r1 + VAULT_HEADER + 3]
  stxb [r2 + 50], r3                                              # is_executable

  # Owner
  add64 r2, 56
  mov64 r3, r1
  add64 r3, OWNER_KEY
  stxdw [r2 + 0], r3
  mov64 r3, r1                                                    # key
  add64 r3, OWNER_LAMPORTS
  stxdw [r2 + 8], r3                                              # lamports
  ldxdw r3, [r1 + OWNER_DATA_LEN]
  stxdw [r2 + 16], r3                                             # data_len
  mov64 r3, r1
  add64 r3, OWNER_DATA
  stxdw [r2 + 24], r3                                             # data
  mov64 r3, r1
  add64 r3, OWNER_OWNER
  stxdw [r2 + 32], r3                                             # owner
  ldxdw r3, [r1 + OWNER_RENT_EPOCH]
  stxdw [r2 + 40], r3                                             # rent_epoch
  ldxb r3, [r1 + OWNER_HEADER + 1]
  stxb [r2 + 48], r3                                              # is_signer
  ldxb r3, [r1 + OWNER_HEADER + 2]
  stxb [r2 + 49], r3                                              # is_writable
  ldxb r3, [r1 + OWNER_HEADER + 3]
  stxb [r2 + 50], r3                                              # is_executable

  ##########################
  ##  Set up signer seeds ##
  ##########################

  mov64 r9, r6
  sub64 r9, 16

  mov64 r2, r9
  stxdw [r2 + 0], r5
  lddw r3, 3                                                      
  stxdw [r2 + 8], r3                                              


  ####################
  ## Invoke the CPI ##
  ####################
  
  mov64 r1, r7                                                    # Instruction
  mov64 r2, r6                                                    # Account infos
  lddw r3, 2                                                      # Number of account infos
  mov64 r4, r9                                                    # Seeds
  lddw r5, 1                                                      # Seeds count
  call sol_invoke_signed_c

  exit

error_invalid_instruction:
  lddw r0, 0xb
  exit

error_invalid_pda:
  lddw r0, 0xc
  exit
