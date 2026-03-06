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

    write_include_file(
        &env,
        "include_test",
        "log.s",
        r#"custom_log:
    lddw r1, message
    lddw r2, 14
    call sol_log_
    exit

.rodata
    message: .ascii "Hello, Solana!"
"#,
    );

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

    write_include_file(
        &env,
        "include_nested",
        "log_impl.s",
        r#"custom_log:
    lddw r1, message
    lddw r2, 14
    call sol_log_
    exit

.rodata
    message: .ascii "Nested!"
"#,
    );

    write_include_file(
        &env,
        "include_nested",
        "log.s",
        r#".include "log_impl.s"
"#,
    );

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

#[test]
fn test_include_rejects_globl_in_included_file() {
    let env = TestEnv::new("include_globl");

    init_project(&env, "include_globl");
    verify_project_structure(&env, "include_globl");

    write_include_file(
        &env,
        "include_globl",
        "helper.s",
        r#".globl helper_fn
helper_fn:
    mov64 r0, 0
    exit
"#,
    );

    update_assembly_file(
        &env,
        "include_globl",
        r#".globl entrypoint
.include "helper.s"
.text
entrypoint:
  call helper_fn
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
        "Build should fail when .globl is used in an included file"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains(".globl") && stderr.contains("not allowed"),
        "Error should mention .globl is not allowed in included files: {}",
        stderr
    );

    env.cleanup();
}

#[test]
fn test_include_rejects_global_in_included_file() {
    let env = TestEnv::new("include_global");

    init_project(&env, "include_global");
    verify_project_structure(&env, "include_global");

    write_include_file(
        &env,
        "include_global",
        "helper.s",
        r#".global helper_fn
helper_fn:
    mov64 r0, 0
    exit
"#,
    );

    update_assembly_file(
        &env,
        "include_global",
        r#".globl entrypoint
.include "helper.s"
.text
entrypoint:
  call helper_fn
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
        "Build should fail when .global is used in an included file"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains(".globl") && stderr.contains("not allowed"),
        "Error should mention .globl is not allowed in included files: {}",
        stderr
    );

    env.cleanup();
}

#[test]
fn test_include_auto_prefixes_data_labels() {
    let env = TestEnv::new("include_prefix");

    init_project(&env, "include_prefix");
    verify_project_structure(&env, "include_prefix");

    write_include_file(
        &env,
        "include_prefix",
        "log.s",
        r#".text
log_msg:
    lddw r1, msg
    lddw r2, 13
    call sol_log_
    exit

.rodata
    msg: .ascii "from log"
"#,
    );

    write_include_file(
        &env,
        "include_prefix",
        "math.s",
        r#".text
add_one:
    lddw r1, msg
    lddw r2, 9
    call sol_log_
    exit

.rodata
    msg: .ascii "from math"
"#,
    );

    update_assembly_file(
        &env,
        "include_prefix",
        r#".globl entrypoint
.include "log.s"
.include "math.s"
.text
entrypoint:
  call log_msg
  call add_one
  mov64 r0, 0
  exit
"#,
    );

    run_build(&env);
    verify_so_files(&env);

    env.cleanup();
}

#[test]
fn test_include_prefix_uses_path_for_same_filename_in_different_dirs() {
    let env = TestEnv::new("include_path_prefix");

    init_project(&env, "include_path_prefix");
    verify_project_structure(&env, "include_path_prefix");

    // Two format.s files in different instruction modules - both define msg
    write_include_file(
        &env,
        "include_path_prefix",
        "instructions/transfer/format.s",
        r#".text
log_transfer:
    lddw r1, msg
    lddw r2, 12
    call sol_log_
    exit

.rodata
    msg: .ascii "Transfer ok"
"#,
    );

    write_include_file(
        &env,
        "include_path_prefix",
        "instructions/swap/format.s",
        r#".text
log_swap:
    lddw r1, msg
    lddw r2, 8
    call sol_log_
    exit

.rodata
    msg: .ascii "Swap ok"
"#,
    );

    update_assembly_file(
        &env,
        "include_path_prefix",
        r#".globl entrypoint
.include "instructions/transfer/format.s"
.include "instructions/swap/format.s"
.text
entrypoint:
  call log_transfer
  call log_swap
  mov64 r0, 0
  exit
"#,
    );

    run_build(&env);
    verify_so_files(&env);

    env.cleanup();
}
