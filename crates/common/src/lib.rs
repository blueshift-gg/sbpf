// CASE-2-INJECTED-FAILURE
// Deliberate, temporary: makes any crate that depends on sbpf-common fail to build.
// Reverted by the final commit of this branch. Do not merge.
compile_error!("CASE-2-INJECTED-FAILURE: deliberate incompatibility for CI testing");

pub mod decode;
pub mod errors;
pub mod execute;
pub mod inst_param;
pub mod instruction;
pub mod opcode;
pub mod optype;
pub mod syscalls;
pub mod syscalls_map;
pub mod validate;

pub use sbpf_common_tablegen::{OpcodeError, OpcodeGroup, OpcodeTable};
