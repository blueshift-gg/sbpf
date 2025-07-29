## Table of Contents

-   [sbpf](#sbpf)
-   [Dependencies](#dependencies)
-   [Installation](#installation)
-   [Usage](#usage)
-   [Command Details](#command-details)
-   [Examples](#examples)
-   [Advanced Usage](#advanced-usage)
-   [Contributing](#contributing)

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

-   `init`: Create a new project scaffold.
-   `build`: Compile into a Solana program executable.
-   `deploy`: Build and deploy the program.
-   `test`: Test the deployed program.
-   `e2e`: Build, deploy, and test a program.
-   `clean`: Clean up build and deploy artifacts.
-   `config`: Manage project configuration files.
-   `help`: Print this message or the help of the given subcommand(s).

```
Usage: sbpf <COMMAND>

Commands:
  init    Create a new project scaffold
  build   Compile into a Solana program executable
  deploy  Build and deploy the program
  test    Test deployed program
  e2e     Build, deploy and test a program
  config  Initialize or manage configuration
  clean   Clean up build and deploy artifacts
  help    Print this message or the help of the given subcommand(s)

Options:
  -h, --help     Print help
  -V, --version  Print version
```

## Configuration System

sbpf now supports project-aware configuration through `sbpf.toml` files. This eliminates the need to repeatedly specify build settings, deployment targets, and other options.

### Quick Start with Configuration

```bash
# Create a new project (automatically includes sbpf.toml)
sbpf init my-solana-program

# Or add configuration to an existing project
sbpf config init

# View current configuration
sbpf config show

# Modify settings
sbpf config set build.optimization release
sbpf config set deploy.cluster testnet
```

### Configuration File Format

The `sbpf.toml` file supports the following sections:

```toml
[project]
name = "my-solana-program"
version = "0.1.0"
description = "My Solana program"

[build]
optimization = "debug"    # "debug" or "release"
target = "sbf"             # Target architecture
flags = ["--strip"]        # Additional compiler flags

[deploy]
cluster = "localhost"        # "localhost", "devnet", "testnet", "mainnet"
program_id = "SBPFxKXt..."    # Optional custom program ID

[test]
framework = "mollusk"      # "mollusk" or "typescript"
validator_args = ["--reset", "--quiet"]  # Additional test arguments
```

### Benefits of Configuration

**Before (Manual Every Time):**

```bash
sbpf build --optimization release
sbpf deploy my-program testnet
sbpf test --framework mollusk --validator-args --reset
```

**After (Project-Aware):**

```bash
sbpf build    # Uses optimization = "debug" from config
sbpf deploy   # Uses cluster = "localhost" from config  
sbpf test     # Uses framework = "mollusk" from config
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

### Configuration Management

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

#### Examples

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

**Modify build settings:**
```sh
sbpf config set build.optimization release
sbpf config set deploy.cluster mainnet
sbpf config set test.framework typescript
```

**Build with configuration:**
```sh
# Uses settings from sbpf.toml automatically
sbpf build
sbpf deploy
sbpf test
```

## Team Collaboration

Configuration files make team development easy:

1. **Shared Settings**: Commit `sbpf.toml` to your repository
2. **Consistent Builds**: Every team member gets identical build behavior
3. **No Documentation Drift**: Configuration is self-documenting
4. **Easy Onboarding**: New developers just run `sbpf build`

## Migration Guide

**Existing projects work unchanged** - no breaking changes! To add configuration:

```bash
# In your existing project directory
sbpf config init

# Customize settings
sbpf config set build.optimization release
sbpf config set deploy.cluster devnet

# Continue using sbpf as before, now with project awareness
sbpf build
```

## Advanced Usage

### Custom Linker Scripts

You can specify custom linker scripts in configuration:

```toml
[build]
linker_script = "custom/linker.ld"
```

Or use per-program linker scripts by placing them in the src directory with the same name as your program:

```
src/example/example.s
src/example/example.ld
```

### Environment-Specific Builds

Different projects can have different default settings:

```bash
# DeFi project
cd defi-protocol
sbpf config set build.optimization release
sbpf config set deploy.cluster mainnet

# Test NFT project  
cd nft-project
sbpf config set build.optimization debug
sbpf config set deploy.cluster localhost
```

Each project remembers its settings automatically.

### Contributing

PRs welcome!
