# Contributing to SBPF
sBPF is the first fully independent Solana BPF assembler. It enables users to quickly get started developing sBPF Assembly projects, skipping the heavy build requirements of solana-rust and solana-llvm and translating assembly code directly into SVM-compatible bytecode, handling just the assembler-linker pipeline without the heavy burden of code generation from higher level languages. We aim to provide comprehensive tooling and support for sBPF assembly projects, as well as to improve upon it with additional features to make it composable and user-friendly. Any [Pull Request](https://github.com/blueshift-gg/sbpf/pulls) or [Issue](https://github.com/blueshift-gg/sbpf/issues) that does not align with our roadmap will be closed as unplanned.

## Vibe Coding
We are humans who write our own tooling because machines have inferior reasoning capabilities and make stupid decisions resulting in inefficient bytecode. We encourage the use of LLMs to help you understand issues, catch bugs and author high quality PRs, however we do not accept PRs that are purely or excessively AI-generated. Trust me when I say that if an LLM could have done it, we wouldn't have wasted our precious time waiting for a community PR. 

## Communication
As a community-owned project, we encourage open dialog on this repo. Please be respectful and understand that this project has a roadmap that may or may not agree with your own. If you wish to communicate your concerns or thoughts about a part of the project openly, feel free to [open a git issue](https://github.com/blueshift-gg/sbpf/issues/new). If you need more instant feedback from a team member, or want to discuss things in more detail, feel free to [join our Discord](https://discord.blueshift.gg).

## Feature Requests
Please [open an issue](https://github.com/blueshift-gg/sbpf/issues/new) requesting a feature and/or discuss in the [Discord](https://discord.blueshift.gg).

## Features
If you would like to introduce new features to SBPF, Please [open an issue](https://github.com/blueshift-gg/sbpf/issues/new) first, or join the Discord to discuss. We would hate for you to put in a bunch of work only to have it rejected for being out of line with our current development roadmap!

## Bug Reports
To report a bug, please [open an issue](https://github.com/blueshift-gg/sbpf/issues/new). Also ensure to separate individual bugs into individual issues. This helps us to track, triage and solve bugs in a more efficient manner.

## Bug Fixes
For minor bugs that are solvable in a single-issue PR, feel free to immediately [open a PR](https://github.com/blueshift-gg/sbpf/compare), referencing an open issue if one already exists. For larger bugs that affect major portions of code or implementation details, it is recommended to [open an issue](https://github.com/blueshift-gg/sbpf/issues/new) first and ping the team on [Discord](https://discord.blueshift.gg) to discuss.

## Nits/Typos
Feel free to directly [open a PR](https://github.com/blueshift-gg/sbpf/compare).

## Compatibility with sbpf-linker

[sbpf-linker](https://github.com/blueshift-gg/sbpf-linker) is a downstream consumer of the sbpf API. It relinks BPF binaries into its sbpf-compatible form, so changes to this repository must account for compatibility with the linker.

The [downstream CI job](https://github.com/blueshift-gg/sbpf/blob/master/.github/workflows/downstream-sbpf-linker.yml) runs on every PR against sbpf-linker. If your changes break compatibility, submit a PR to sbpf-linker that ports the changes, following these steps:

1. Clone sbpf-linker and create a feature branch based on the [downstream gate branch](https://github.com/blueshift-gg/sbpf-linker/tree/sbpf-linker-next).
2. Update both [sbpf dependencies](https://github.com/blueshift-gg/sbpf-linker/blob/8edec876120bcc3e0679d5636b724ddd994df1cc/Cargo.toml#L18-L19) to point to your sbpf crates version (you can also specify a commit with rev tag).
3. Adapt the linker to your changes and verify that its tests pass.
4. Open a companion PR and link it from your sbpf PR so maintainers can coordinate merging both.

### Other Possible Failures: 

1. If the checkout or setup step fails, your change isn't the cause. Re-run the job, and ping a maintainer if it keeps failing.
2. Bug in your PR that your own tests missed. Fix the PR, and add a test to sbpf.
3. Your change exposed an old sbpf bug. Fix sbpf, possibly as a separate PR.
4. Your change exposed an old sbpf-linker bug. Fix sbpf-linker.

*For test failures: Does the failing test pass against sbpf master? If yes, your change caused or exposed it.*