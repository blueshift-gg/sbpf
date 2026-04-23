//! Integration tests for the `.include` directive.
//!
//! These tests exercise the end-to-end build path — they invoke the
//! `sbpf` binary against a real project on disk and check that the
//! resulting `.so` files (or error messages) match expectations.
//!
//! The `.include` directive is implemented inside the assembler parser
//! (not as a text-level preprocessor in `build.rs`). `.include "path"`
//! reads the referenced file and parses it into the same AST, so
//! labels are shared, duplicates are caught by the normal
//! `DuplicateLabel` check, and DWARF debug info tracks the source file
//! each instruction came from.
//!
//! Contract covered:
//!
//! * Basic `.include` works and produces a working program.
//! * Nested `.include` (included file includes another file) works.
//! * `.globl`/`.global` are allowed in included files.
//! * Paths are resolved relative to the *including* file, not the
//!   main file.
//! * A missing include path produces a readable error.
//! * Duplicate labels across files produce a `DuplicateLabel` error
//!   that names both files and points at the main-file `.include`
//!   directive that pulled in the second definition.
//! * When the duplicate is inside the main file, no `.include` chain is
//!   reported.

mod utils;

use {
    std::process::Command,
    utils::{
        TestEnv, init_project, run_build, update_assembly_file, verify_project_structure,
        verify_so_files, write_include_file,
    },
};

/// Main file that includes a helper and calls into it.
///
/// The helper file defines a `.rodata` entry and a code routine. Labels
/// are unique across files so this builds without any prefixing.
#[test]
fn test_include_directive() {
    let env = TestEnv::new("include_basic");

    init_project(&env, "include_basic");
    verify_project_structure(&env, "include_basic");

    write_include_file(
        &env,
        "include_basic",
        "log.s",
        r#".text
custom_log:
    lddw r1, include_msg
    lddw r2, 9
    call sol_log_
    exit

.rodata
    include_msg: .ascii "from log."
"#,
    );

    update_assembly_file(
        &env,
        "include_basic",
        r#".globl entrypoint
.include "log.s"
.text
entrypoint:
    call custom_log
    mov64 r0, 0
    exit
"#,
    );

    run_build(&env);
    verify_so_files(&env);

    env.cleanup();
}

/// Nested include: main → layer1.s → layer2.s. Proves that `.include`
/// paths inside an included file are resolved relative to that file's
/// own directory, not the main file's.
#[test]
fn test_include_nested() {
    let env = TestEnv::new("include_nested");

    init_project(&env, "include_nested");
    verify_project_structure(&env, "include_nested");

    write_include_file(
        &env,
        "include_nested",
        "layer2.s",
        r#".text
deep_log:
    lddw r1, deep_msg
    lddw r2, 10
    call sol_log_
    exit

.rodata
    deep_msg: .ascii "deep call."
"#,
    );

    write_include_file(
        &env,
        "include_nested",
        "layer1.s",
        r#".include "layer2.s"
"#,
    );

    update_assembly_file(
        &env,
        "include_nested",
        r#".globl entrypoint
.include "layer1.s"
.text
entrypoint:
    call deep_log
    mov64 r0, 0
    exit
"#,
    );

    run_build(&env);
    verify_so_files(&env);

    env.cleanup();
}

/// Paths inside a nested include are resolved relative to the including
/// file. Here `modules/a.s` includes `b.s`, which lives alongside it in
/// `modules/` — not in the main directory.
#[test]
fn test_include_nested_resolves_relative_to_including_file() {
    let env = TestEnv::new("include_rel_path");

    init_project(&env, "include_rel_path");
    verify_project_structure(&env, "include_rel_path");

    write_include_file(
        &env,
        "include_rel_path",
        "modules/b.s",
        r#".text
b_func:
    lddw r1, b_msg
    lddw r2, 6
    call sol_log_
    exit

.rodata
    b_msg: .ascii "from b"
"#,
    );

    write_include_file(
        &env,
        "include_rel_path",
        "modules/a.s",
        r#".include "b.s"
"#,
    );

    update_assembly_file(
        &env,
        "include_rel_path",
        r#".globl entrypoint
.include "modules/a.s"
.text
entrypoint:
    call b_func
    mov64 r0, 0
    exit
"#,
    );

    run_build(&env);
    verify_so_files(&env);

    env.cleanup();
}

/// `.globl` and `.global` are allowed anywhere, including inside
/// included files. This is per the agreed direction that `.include`
/// is pure textual inclusion, not import semantics.
#[test]
fn test_include_allows_globl_in_included_file() {
    let env = TestEnv::new("include_globl");

    init_project(&env, "include_globl");
    verify_project_structure(&env, "include_globl");

    write_include_file(
        &env,
        "include_globl",
        "log.s",
        r#".global custom_log
.text
custom_log:
    lddw r1, include_msg
    lddw r2, 9
    call sol_log_
    exit

.rodata
    include_msg: .ascii "from log."
"#,
    );

    update_assembly_file(
        &env,
        "include_globl",
        r#".globl entrypoint
.include "log.s"
.text
entrypoint:
    call custom_log
    mov64 r0, 0
    exit
"#,
    );

    run_build(&env);
    verify_so_files(&env);

    env.cleanup();
}

/// A missing include path produces a clear error message naming the
/// missing file.
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
        "Build should fail when include target is missing"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("nonexistent.s"),
        "Error should mention the missing file: {}",
        stderr
    );

    env.cleanup();
}

/// A label defined in both the main file and an included file produces
/// a `DuplicateLabel` error. The error output should show both the
/// original (main file) and the redefinition (included file), and
/// should point at the `.include` directive that pulled in the second
/// definition.
#[test]
fn test_include_duplicate_label_main_then_included() {
    let env = TestEnv::new("include_dup_main_inc");

    init_project(&env, "include_dup_main_inc");
    verify_project_structure(&env, "include_dup_main_inc");

    write_include_file(
        &env,
        "include_dup_main_inc",
        "log.s",
        r#".text
shared:
    exit
"#,
    );

    update_assembly_file(
        &env,
        "include_dup_main_inc",
        r#".globl entrypoint
.text
shared:
    mov64 r0, 1
    exit
.include "log.s"
entrypoint:
    call shared
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
        "Build should fail on duplicate label across main + included file"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Duplicate label") && stderr.contains("shared"),
        "Error should mention DuplicateLabel for 'shared': {}",
        stderr
    );
    // Both files should be referenced somewhere in the rendered output.
    assert!(
        stderr.contains("log.s"),
        "Error output should reference the included file 'log.s': {}",
        stderr
    );
    assert!(
        stderr.contains("included from here"),
        "Error output should point at the .include directive: {}",
        stderr
    );

    env.cleanup();
}

/// Two included files each define the same label. The resulting error
/// should reference both files and show the `.include` directive chain.
#[test]
fn test_include_duplicate_label_between_two_includes() {
    let env = TestEnv::new("include_dup_two_inc");

    init_project(&env, "include_dup_two_inc");
    verify_project_structure(&env, "include_dup_two_inc");

    write_include_file(
        &env,
        "include_dup_two_inc",
        "first.s",
        r#".text
helper:
    mov64 r0, 1
    exit
"#,
    );

    write_include_file(
        &env,
        "include_dup_two_inc",
        "second.s",
        r#".text
helper:
    mov64 r0, 2
    exit
"#,
    );

    update_assembly_file(
        &env,
        "include_dup_two_inc",
        r#".globl entrypoint
.include "first.s"
.include "second.s"
entrypoint:
    call helper
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
        "Build should fail on duplicate label between two included files"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Duplicate label") && stderr.contains("helper"),
        "Error should mention DuplicateLabel for 'helper': {}",
        stderr
    );
    assert!(
        stderr.contains("first.s") && stderr.contains("second.s"),
        "Error output should reference both included files: {}",
        stderr
    );
    assert!(
        stderr.contains("included from here"),
        "Error output should point at the .include directive for the second file: {}",
        stderr
    );

    env.cleanup();
}

/// A duplicate label inside a single file still works as before — no
/// include chain is reported and the error only references the main
/// file.
#[test]
fn test_include_duplicate_label_same_file_no_chain() {
    let env = TestEnv::new("include_dup_same");

    init_project(&env, "include_dup_same");
    verify_project_structure(&env, "include_dup_same");

    update_assembly_file(
        &env,
        "include_dup_same",
        r#".globl entrypoint
entrypoint:
    mov64 r0, 1
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
        "Build should fail on duplicate label in the same file"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Duplicate label") && stderr.contains("entrypoint"),
        "Error should mention DuplicateLabel for 'entrypoint': {}",
        stderr
    );
    assert!(
        !stderr.contains("included from here"),
        "Same-file duplicates should not report an include chain: {}",
        stderr
    );

    env.cleanup();
}

/// Duplicate `.rodata` labels across files are detected the same way
/// as code labels. Regression for the old "prefix data labels"
/// behaviour: there should be no implicit renaming, just a normal
/// duplicate error.
#[test]
fn test_include_duplicate_rodata_label() {
    let env = TestEnv::new("include_dup_rodata");

    init_project(&env, "include_dup_rodata");
    verify_project_structure(&env, "include_dup_rodata");

    write_include_file(
        &env,
        "include_dup_rodata",
        "a.s",
        r#".rodata
    msg: .ascii "from a"
"#,
    );

    write_include_file(
        &env,
        "include_dup_rodata",
        "b.s",
        r#".rodata
    msg: .ascii "from b"
"#,
    );

    update_assembly_file(
        &env,
        "include_dup_rodata",
        r#".globl entrypoint
.include "a.s"
.include "b.s"
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
        "Build should fail on duplicate rodata label across includes"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Duplicate label") && stderr.contains("msg"),
        "Error should mention duplicate rodata label 'msg': {}",
        stderr
    );
    assert!(
        stderr.contains("a.s") && stderr.contains("b.s"),
        "Error should reference both include files: {}",
        stderr
    );

    env.cleanup();
}

/// Cyclic includes (A includes B, B includes A) should produce a clear
/// error instead of infinite recursion / stack overflow.
#[test]
fn test_include_cyclic_detection() {
    let env = TestEnv::new("include_cyclic");

    init_project(&env, "include_cyclic");
    verify_project_structure(&env, "include_cyclic");

    write_include_file(
        &env,
        "include_cyclic",
        "a.s",
        r#".include "b.s"
"#,
    );

    write_include_file(
        &env,
        "include_cyclic",
        "b.s",
        r#".include "a.s"
"#,
    );

    update_assembly_file(
        &env,
        "include_cyclic",
        r#".globl entrypoint
.include "a.s"
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
        "Build should fail on cyclic include"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Cyclic include") || stderr.contains("cyclic include"),
        "Error should mention cyclic include: {}",
        stderr
    );

    env.cleanup();
}
