## Table of Contents

- [sbpf](#sbpf)
    - [Installation](#installation)
    - [Usage](#usage)
    - [Command Details](#command-details)
      - [Initialize a Project](#initialize-a-project)
        - [Examples](#examples)
          - [Create a new project with Rust tests (default)](#create-a-new-project-with-rust-tests-default)
          - [Create a new project with TypeScript tests](#create-a-new-project-with-typescript-tests)
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
-   `help`: Print this message or the help of the given subcommand(s).

```
Usage: sbpf <COMMAND>

Commands:
  init    Create a new project scaffold
  build   Compile into a Solana program executable
  deploy  Build and deploy the program
  test    Test deployed program
  e2e     Build, deploy and test a program
  clean   Clean up build and deploy artifacts
  help    Print this message or the help of the given subcommand(s)

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
