/* @ts-self-types="./sbpf_assembler.d.ts" */

import * as wasm from "./sbpf_assembler_bg.wasm";
import { __wbg_set_wasm } from "./sbpf_assembler_bg.js";
__wbg_set_wasm(wasm);
wasm.__wbindgen_start();
export {
    assemble
} from "./sbpf_assembler_bg.js";
