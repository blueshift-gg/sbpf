use {
    sbpf_assembler::{Assembler, AssemblerOption},
    std::{env, fs, path::PathBuf},
};

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn assemble(filename: &str) -> Vec<u8> {
    let source = fs::read_to_string(fixtures_dir().join(filename))
        .unwrap_or_else(|e| panic!("failed to read {}: {}", filename, e));
    let assembler = Assembler::new(AssemblerOption::default());
    assembler
        .assemble(&source)
        .unwrap_or_else(|e| panic!("failed to assemble {}: {:?}", filename, e))
}

#[test]
fn test_default_and_llvm_produce_same_bytecode() {
    // opcodes.s and opcodes_llvm.s are both the same test program,
    // written in default sBPF assembly and the LLVM dialect
    // respectively. Hence,they should produce identical bytecode.
    let default_bytecode = assemble("opcodes.s");
    let llvm_bytecode = assemble("opcodes_llvm.s");
    assert_eq!(
        default_bytecode, llvm_bytecode,
        "should produce identical bytecode"
    );
}

#[test]
fn test_mixed_syntax_error() {
    // Test program that mixes default sBPF assembly and the LLVM dialect.
    let source = r#"
.globl entrypoint
entrypoint:
    add64 r1, r2
    r2 += r3
    exit
    "#;
    let assembler = Assembler::new(AssemblerOption::default());
    let err = assembler.assemble(source).unwrap_err();
    assert!(
        err.iter().any(|e| e.to_string().contains("Parse error")),
        "mixed syntax should produce a parse error"
    );
}
