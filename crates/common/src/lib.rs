// RETEST-LIVE-TEST-INJECTED-FAILURE
compile_error!("RETEST-LIVE-TEST: deliberate incompatibility");

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
