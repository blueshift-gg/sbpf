# sbpf-asm-vault

A Solana vault program written in sBPF Assembly that allows users to securely deposit and withdraw their lamports.

This program utilizes the following syscalls:

- sol_create_program_address
- sol_memcmp_
- sol_invoke_signed_c

## Build

To build the program, run the following command:

```bash
sbpf build
```

## Test

To test the program, run the following command:

```bash
sbpf test
```

---

Created with [sbpf](https://github.com/blueshift-gg/sbpf)