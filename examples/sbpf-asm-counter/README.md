# sbpf-asm-counter

A Solana program written in sBPF Assembly that allows users to create and increment an on-chain counter. 

Its main purpose is to demonstrate how to create an account and manage account data directly using sBPF.

It utilizes the following syscalls:

- `sol_create_program_address`
- `sol_memcmp_`
- `sol_get_rent_sysvar`
- `sol_memcpy_`
- `sol_invoke_signed_c`

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