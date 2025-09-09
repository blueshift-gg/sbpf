import * as wasm from "./sbpf_assembler_bg.wasm";
export * from "./sbpf_assembler_bg.js";
import { __wbg_set_wasm } from "./sbpf_assembler_bg.js";
__wbg_set_wasm(wasm);
wasm.__wbindgen_start();
