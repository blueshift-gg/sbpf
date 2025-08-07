# sbpf

A simple scaffold to bootstrap sBPF Assembly programs.

## Table of Contents

- [sbpf](#sbpf)
- [Dependencies](#dependencies)
- [Installation](#installation)
- [Usage](#usage)
- [Configuration System](#configuration-system)
- [Command Details](#command-details)
- [Environment Variables](#environment-variables)
- [Scripts System](#scripts-system)
- [CLI Overrides](#cli-overrides)
- [Security Features](#security-features)
- [Examples](#examples)
- [Contributing](#contributing)

### Dependencies

Please make sure you have the latest version of [Solana Command Line Tools](https://docs.solanalabs.com/cli/install) installed.

### Installation

```bash
cargo install --git https://github.com/blueshift-gg/sbpf.git
```

### Usage

Usage: `sbpf <COMMAND>`
To view all the commands you can run, type `sbpf help`. Here are the available commands:

- `init`: Create a new project scaffold with configuration
- `build`: Compile into a Solana program executable
- `deploy`: Build and deploy the program
- `test`: Test the deployed program
- `e2e`: Build, deploy, and test a program
- `clean`: Clean up build and deploy artifacts
- `config`: Manage project configuration files
- `script`: Run custom scripts defined in configuration
- `help`: Print this message or the help of the given subcommand(s)

Options:

- `-h`, --help     Print help
- `-V`, --version  Print version

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
sbpf config set build.mode light
sbpf config set deploy.cluster mainnet
sbpf config set scripts.test "cargo test --verbose"
```

### Configuration File Format

The `sbpf.toml` file supports the following sections:

```toml
[project]
name = "my-solana-program"
version = "0.1.0"

[build]
optimization = "debug"
target = "sbf"
flags = []
mode = "full"

[deploy]
cluster = "localhost"
wallet = "~/.config/solana/id.json"

[test]
validator_args = []

[scripts]
test = "cargo test"
# deploy-prod = "sbpf deploy --cluster mainnet" 
```

## Environment Variables

sbpf supports environment variables in configuration values for sensitive data:

```toml
[deploy]
cluster = "${SOLANA_RPC_URL:-localhost}"     # Use env var with fallback
program_id = "${PROGRAM_ID}"                 # Use env var
```

**Syntax:**

- `${VAR}` - Use environment variable VAR
- `${VAR:-default}` - Use VAR, or 'default' if not set

**Example usage:**

```bash
export SOLANA_RPC_URL="https://api.mainnet-beta.solana.com"
sbpf deploy  # Uses mainnet instead of localhost
```

## Scripts System

Define custom commands in your configuration that can be run with `sbpf script <name>`:

```toml
[scripts]
# Override built-in commands
test = "cargo test --verbose"
build = "echo 'Custom build' && sbpf build --mode light"

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

## CLI Overrides

Command-line arguments take precedence over configuration file settings:

### Build Overrides

```bash
sbpf build --mode light              # Override build.mode
sbpf build --optimization release    # Override build.optimization
```

### Deploy Overrides

```bash
sbpf deploy --cluster mainnet        # Override deploy.cluster
sbpf deploy --cluster https://custom-rpc.com  # Use custom RPC
```

### Test Overrides

```bash
sbpf test --command "yarn test"      # Override test command
```

## Security Features

sbpf includes built-in security warnings to help protect sensitive information:

- **Mainnet RPC Detection**: Warns when mainnet endpoints are in config
- **Custom RPC Warnings**: Alerts for potentially sensitive RPC URLs
- **Environment Variable Suggestions**: Recommends env vars for sensitive data

**Example warning:**

```bash
📄 Security Notes:
   Mainnet RPC detected: https://api.mainnet-beta.solana.com
   Consider using environment variables for production endpoints
```

### Command Details

#### Initialize a Project

To create a new project, use the `sbpf init` command. By default, it initializes a project with Rust tests using [Mollusk](https://github.com/buffalojoec/mollusk). You can also initialize a project with TypeScript tests using the `--ts-tests` option.

Create new projects with automatic configuration setup:

```bash
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

```bash
sbpf config --help
Initialize or manage configuration

Usage: sbpf config <COMMAND>

Commands:
  show      Show current configuration
  init      Initialize default configuration
  set       Set a configuration value
  manual    Show configuration manual
  help      Print this message or the help of the given subcommand(s)
```

## Examples

### Basic Project Setup

**Create a new Rust project:**

```bash
sbpf init my-program
cd my-program
sbpf build
sbpf deploy
sbpf test
```

**Create a TypeScript project:**

```bash
sbpf init my-program --ts-tests
cd my-program
sbpf build
sbpf deploy
sbpf test
```

### Advanced Configuration

**Environment-specific deployment:**

```bash
# Set up different environments
sbpf config set deploy.cluster devnet
export MAINNET_RPC="https://api.mainnet-beta.solana.com"

# Deploy to devnet 
sbpf deploy

# Override for mainnet
sbpf deploy --cluster $MAINNET_RPC
```

**Custom build pipeline:**

```bash
# Configure custom scripts
sbpf config set scripts.build "echo 'Building...' && sbpf build --mode light"
sbpf config set scripts.test "cargo test --verbose"
sbpf config set scripts.deploy-all "sbpf build && sbpf deploy"

# Use custom pipeline
sbpf script build
sbpf script test
sbpf script deploy-all
```

### Team Collaboration

Configuration files make team development seamless:

1. **Shared Settings**: Commit `sbpf.toml` to your repository
2. **Consistent Builds**: Every team member gets identical build behavior
3. **Environment Variables**: Keep sensitive data in env vars, not config
4. **Custom Scripts**: Share common development tasks

**Example team workflow:**

```bash
# Team member clones repo
git clone <your-repo>
cd <your-repo>

# Set up environment
export SOLANA_RPC_URL="https://our-team-rpc.com"

# Everything works immediately
sbpf build    # Uses team's build settings
sbpf deploy   # Uses team's deployment config
sbpf test     # Uses team's test configuration
```

## Migration Guide

**Existing projects work unchanged** - no breaking changes! To add configuration:

```bash
# In your existing project directory
sbpf config init

# Customize settings for your workflow
sbpf config set build.mode light
sbpf config set deploy.cluster devnet
sbpf config set scripts.test "cargo test --nocapture"

# Continue using sbpf with enhanced configuration
sbpf build
sbpf deploy
sbpf test
```

## Advanced Usage

### Build Modes

**Full Mode (Default):** Uses complete Solana toolchain (clang/ld)

```bash
sbpf config set build.mode full
sbpf build  # Uses Solana platform tools
```

**Light Mode:** Uses built-in sbpf-assembler

```bash
sbpf config set build.mode light
sbpf build  # Uses lightweight assembler
```

### Custom Linker Scripts

Specify custom linker scripts in configuration:

```toml
[build]
linker_script = "custom/linker.ld"
```

Or use per-program linker scripts by placing them in the src directory:

```bash
src/example/example.s
src/example/example.ld
```

### Environment-Specific Configuration

Handle different deployment environments:

```toml
# Development
[deploy]
cluster = "${DEV_RPC_URL:-localhost}"

# Production (using environment variables)
# export PROD_RPC_URL="https://api.mainnet-beta.solana.com"
# cluster = "${PROD_RPC_URL}"
```

### Contributing

PRs welcome!
