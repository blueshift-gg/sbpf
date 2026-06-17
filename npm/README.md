# @blueshift-gg/sbpf-assembler

Assemble and link sBPF assembly to bytecode, powered by WebAssembly.

## Install

```sh
npm install @blueshift-gg/sbpf-assembler
# or
yarn add @blueshift-gg/sbpf-assembler
# or
pnpm add @blueshift-gg/sbpf-assembler
# or
bun add @blueshift-gg/sbpf-assembler
```

## Usage

**Node.js**

```js
import { assemble } from "@blueshift-gg/sbpf-assembler";
// or: const { assemble } = require("@blueshift-gg/sbpf-assembler");

const program = `
.globl e
e:
  mov64 r0, 1
  exit
`;

const bytecode = assemble(program, 0 /* arch version */);
```

**Browser (via bundler)**

```js
import init, { assemble } from "@blueshift-gg/sbpf-assembler";

await init();    // WASM is loaded asynchronously in browsers

const program = `
.globl e
e:
  mov64 r0, 1
  exit
`;

const bytecode = assemble(program, 0 /* arch version */);
```

**Explicit subpath (if your bundler's condition resolution is unusual)**

```js
import { assemble } from "@blueshift-gg/sbpf-assembler/node";
import { assemble } from "@blueshift-gg/sbpf-assembler/bundler";
import init, { assemble } from "@blueshift-gg/sbpf-assembler/web";
```

## API

### `assemble(source: string, arch: number): Uint8Array`

Compiles an sBPF assembly program to ELF bytecode.

| Parameter | Type | Description |
|---|---|---|
| `source` | `string` | sBPF assembly source |
| `arch` | `number` | Architecture version (e.g. `0`) |

## License

MIT OR Apache-2.0
