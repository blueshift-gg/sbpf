## Table of Contents

- [sbpf](#sbpf)
- [Dependencies](#dependencies)
- [Installation](#installation)
- [Usage](#usage)
- [Configuration-System](#configuration-system)
- [Command Details](#command-details)
- [Examples](#examples)
- [Advanced Usage](#advanced-usage)
- [Contributing](#contributing)

# sbpf

A simple scaffold to bootstrap sBPF Assembly programs.

### Dependencies

Please make sure you have the latest version of [Solana Command Line Tools](https://docs.solanalabs.com/cli/install) installed.

### Installation

```sh
cargo install --git https://github.com/blueshift-gg/sbpf.git
```

### Usage

To view all the commands you can run, type `sbpf help`. Here are the available commands:

- `init`: Create a new project scaffold.
- `build`: Compile into a Solana program executable.
- `deploy`: Build and deploy the program.
- `test`: Test the deployed program.
- `e2e`: Build, deploy, and test a program.
- `clean`: Clean up build and deploy artifacts.
- `config`: Manage project configuration files.
- `help`: Print this message or the help of the given subcommand(s).

Options:

- `-h`, --help     Print help
- `-V`, --version  Print version

Usage: sbpf `<COMMAND>`

### Configuration System

sbpf now supports project-aware configuration through `sbpf.toml` files. This eliminates the need to repeatedly specify build settings, deployment targets, and other options.

#### Quick Start with Configuration

```bash
# Create a new project (automatically includes sbpf.toml)
sbpf init my-solana-program

# Or add configuration to an existing project
sbpf config init

# View current configuration
sbpf config show

# Modify settings
sbpf config set deploy.cluster mainnet
sbpf config set scripts.test "cargo test --verbose"
```

#### Configuration File Format

The `sbpf.toml` file supports the following sections:

```toml
[project]
name = "sbpf"
version = "0.1.0"

[scripts]
test = "cargo test"

[deploy]
cluster = "localhost"
wallet = "~/.config/solana/id.json"
```

#### Scripts System

Define custom commands in your configuration that can be run with `sbpf script <name>`:

```toml
[scripts]
# Override built-in commands
test = "cargo test --verbose"
build = "echo 'Custom build' && sbpf build"

# Custom scripts
lint = "cargo clippy -- -D warnings"
format = "cargo fmt"
deploy-staging = "sbpf deploy --cluster devnet"
audit = "cargo audit"
```

**Usage:**

```bash
sbpf script lint           # Run custom lint script
sbpf script deploy-staging # Run staging deployment
sbpf script audit          # Run security audit
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

#### Configuration Management

Manage project configuration without manually editing TOML files:

```sh
sbpf config --help
Initialize or manage configuration

Usage: sbpf config <COMMAND>

Commands:
  show  Show current configuration
  init  Initialize default configuration
  set   Set a configuration value
  help  Print this message or the help of the given subcommand(s)
```

### Examples

**Create a new project with Rust tests (includes sbpf.toml):**

```sh
sbpf init my-project
```

**Create a new project with TypeScript tests:**

```sh
sbpf init my-project --ts-tests
```

**Add configuration to existing project:**

```sh
sbpf config init
```

**View current settings:**

```sh
sbpf config show
```

**Modify settings:**

```sh
sbpf config set deploy.cluster mainnet
```

**Build with configuration:**

```sh
# Uses settings from sbpf.toml automatically
sbpf build
sbpf deploy
sbpf test
```

**Custom build pipeline:**

```bash
# Configure custom scripts
sbpf config set scripts.build "echo 'Building...' && sbpf build"
sbpf config set scripts.test "cargo test --verbose"
sbpf config set scripts.deploy-all "sbpf build && sbpf deploy"

# Use custom pipeline
sbpf script build
sbpf script test
sbpf script deploy-all
```

### Advanced Usage

You can override the default linker with a [custom linker file](https://github.com/deanmlittle/sbpf-asm-noop/blob/master/src/noop/noop.ld) by including it in the src directory with the same name as your program. For example:

```bash
src/example/example.s
src/example/example.ld
```

### Contributing

PRs welcome!
