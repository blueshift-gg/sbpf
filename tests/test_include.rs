mod utils;

use {
    std::process::Command,
    utils::{
        TestEnv, init_project, run_build, update_assembly_file, verify_project_structure,
        verify_so_files, write_include_file,
    },
};

#[test]
fn test_include_directive() {
    let env = TestEnv::new("include_test");

    init_project(&env, "include_test");
    verify_project_structure(&env, "include_test");

    // Write included file with custom_log logic
    write_include_file(
        &env,
        "include_test",
        "log.s",
        r#".global custom_log
custom_log:
    lddw r1, message
    lddw r2, 14
    call sol_log_
    exit

.rodata
    message: .ascii "Hello, Solana!"
"#,
    );

    // Main file uses .include
    update_assembly_file(
        &env,
        "include_test",
        r#".globl entrypoint
.include "log.s"
.text
entrypoint:
  call custom_log
  exit
"#,
    );

    run_build(&env);
    verify_so_files(&env);

    env.cleanup();
}

#[test]
fn test_include_nested() {
    let env = TestEnv::new("include_nested");

    init_project(&env, "include_nested");
    verify_project_structure(&env, "include_nested");

    // Innermost: just the log logic
    write_include_file(
        &env,
        "include_nested",
        "log_impl.s",
        r#".global custom_log
custom_log:
    lddw r1, message
    lddw r2, 14
    call sol_log_
    exit

.rodata
    message: .ascii "Nested!"
"#,
    );

    // Middle: includes log_impl
    write_include_file(
        &env,
        "include_nested",
        "log.s",
        r#".include "log_impl.s"
"#,
    );

    // Main: includes log.s
    update_assembly_file(
        &env,
        "include_nested",
        r#".globl entrypoint
.include "log.s"
.text
entrypoint:
  call custom_log
  exit
"#,
    );

    run_build(&env);
    verify_so_files(&env);

    env.cleanup();
}

#[test]
fn test_include_missing_file_fails() {
    let env = TestEnv::new("include_missing");

    init_project(&env, "include_missing");
    verify_project_structure(&env, "include_missing");

    // Main file references non-existent include
    update_assembly_file(
        &env,
        "include_missing",
        r#".globl entrypoint
.include "nonexistent.s"
.text
entrypoint:
  exit
"#,
    );

    let output = Command::new(&env.sbpf_bin)
        .current_dir(&env.project_dir)
        .arg("build")
        .output()
        .expect("Failed to run sbpf build");

    assert!(
        !output.status.success(),
        "Build should fail when include file is missing"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("nonexistent") || stderr.contains("Failed to read"),
        "Error message should mention the missing file: {}",
        stderr
    );

    env.cleanup();
}
