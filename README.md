## Table of Contents

- [sbpf](#sbpf)
    - [Installation](#installation)
    - [Usage](#usage)
    - [Command Details](#command-details)
      - [Initialize a Project](#initialize-a-project)
        - [Examples](#examples)
          - [Create a new project with Rust tests (default)](#create-a-new-project-with-rust-tests-default)
          - [Create a new project with TypeScript tests](#create-a-new-project-with-typescript-tests)
      - [Disassembler](#disassembler)
      - [Debugger](#debugger)
    - [Advanced Usage](#advanced-usage)
    - [License](#license)
    - [Contributing](#contributing)

# sbpf

![ci](https://github.com/blueshift-gg/sbpf/actions/workflows/ci.yml/badge.svg)
[![codecov](https://codecov.io/gh/blueshift-gg/sbpf/branch/master/graph/badge.svg)](https://codecov.io/gh/blueshift-gg/sbpf)

A simple scaffold to bootstrap sBPF Assembly programs.

### Installation

```sh
cargo install --git https://github.com/blueshift-gg/sbpf.git
```

### Usage

To view all the commands you can run, type `sbpf help`. Here are the available commands:

-   `init`: Create a new project scaffold.
-   `build`: Compile into a Solana program executable.
-   `deploy`: Build and deploy the program.
-   `test`: Test the deployed program.
-   `e2e`: Build, deploy, and test a program.
-   `clean`: Clean up build and deploy artifacts.
-   `disassemble`: Disassemble a Solana program executable.
-   `debug`: Debug an sBPF assembly program.
-   `help`: Print this message or the help of the given subcommand(s).

```
Usage: sbpf <COMMAND>

Commands:
  init         Create a new project scaffold
  build        Compile into a Solana program executable
  deploy       Build and deploy the program
  test         Test deployed program
  e2e          Build, deploy and test a program
  clean        Clean up build and deploy artifacts
  disassemble  Disassemble a Solana program executable
  debug        Debug an sBPF assembly program
  help         Print this message or the help of the given subcommand(s)

Options:
  -h, --help     Print help
  -V, --version  Print version
```

### Command Details

#### Initialize a Project

To create a new project, use the `sbpf init` command. By default, it initializes a project with Rust tests using [Mollusk](https://github.com/buffalojoec/mollusk). You can also initialize a project with TypeScript tests using the `--ts-tests` option.

```sh
sbpf init --help
Create a new project scaffold

Usage: sbpf init [OPTIONS] [NAME]

Arguments:
  [NAME]  The name of the project to create

Options:
  -t, --ts-tests  Initialize with TypeScript tests instead of Mollusk Rust tests
  -h, --help      Print help information
  -V, --version   Print version information
```

##### Examples

###### Create a new project with Rust tests (default)

```sh
sbpf init my-project
```

###### Create a new project with TypeScript tests

```sh
sbpf init my-project --ts-tests
```

After initializing the project, you can navigate into the project directory and use other commands to build, deploy, and test your program.

#### Disassembler

The disassembler converts a Solana program executable (ELF) into human-readable sBPF assembly.

```sh
sbpf disassemble <FILENAME>
```

#### Debugger

The debugger provides an interactive REPL for stepping through sBPF assembly programs.

*Debug an assembly file:*

```sh
sbpf debug --asm <FILENAME>
```

*Debug an ELF file:*

```sh
sbpf debug --elf <FILENAME>
```

*Input:*

To debug programs that require input, the debugger accepts a JSON file (or JSON string) containing the instruction being executed and the accounts involved. Pass it using the `--input` flag:

```sh
sbpf debug --asm src/my-program/my-program.s --input input.json
```

The JSON should contain the following information:

- **`instruction`**: The instruction to execute, including the program ID, account metas, and instruction data.
- **`accounts`**: The account states. The `data` field in each account and instruction should be base58 encoded.


*Example:*
```json
{
  "instruction": {
    "program_id": "78ycAjmvvq2Xjz6mBgGTsuHHNVADZ75NWgXKPY8wvF2s",
    "accounts": [
      {
        "pubkey": "3JF3sEqM796hk5WFqA6EtmEwJQ9quALszsfJyvXNQKy3",
        "is_signer": true,
        "is_writable": false
      },
      {
        "pubkey": "11157t3sqMV725NVRLrVQbAu98Jjfk1uCKehJnXXQs",
        "is_signer": false,
        "is_writable": true
      }
    ],
    "data": "8AQGAut7N95oMfV99bhRZ"
  },
  "accounts": [
    {
      "pubkey": "3JF3sEqM796hk5WFqA6EtmEwJQ9quALszsfJyvXNQKy3",
      "owner": "11111111111111111111111111111111",
      "lamports": 1000000000,
      "data": "",
      "executable": false
    },
    {
      "pubkey": "11157t3sqMV725NVRLrVQbAu98Jjfk1uCKehJnXXQs",
      "owner": "11111111111111111111111111111111",
      "lamports": 1000000000,
      "data": "",
      "executable": false
    }
  ]
}
```



### Advanced Usage

You can override the default linker with a [custom linker file](https://github.com/deanmlittle/sbpf-asm-noop/blob/master/src/noop/noop.ld) by including it in the src directory with the same name as your program. For example:

```
src/example/example.s
src/example/example.ld
```

### License

Licensed under either of

 * Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

### Contributing

PRs welcome!
