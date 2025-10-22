mod utils;

use utils::{
    TestEnv, init_project, run_build, update_assembly_file, verify_project_structure,
    verify_so_files,
};

#[test]
fn test_memo_project_e2e() {
    let env = TestEnv::new("memo");

    // Step 1: Initialize the memo project
    init_project(&env, "memo");

    // Step 2: Verify project structure
    verify_project_structure(&env, "memo");

    // Step 3: Run build
    run_build(&env);

    // Step 4: Verify .so files were created
    verify_so_files(&env);

    // Step 5: Run tests
    // run_tests(&env);

    // Step 6: Clean up
    env.cleanup();
    println!("🎉 All tests passed! Memo project E2E test completed successfully.");
}

#[test]
fn test_memo_project_e2e_second() {
    let env = TestEnv::new("memo2");

    // Step 1: Initialize the memo2 project
    init_project(&env, "memo2");

    // Step 2: Verify project structure
    verify_project_structure(&env, "memo2");

    // Step 3: Update the memo2.s content to the specified content
    let new_memo_content = r#".equ NUM_ACCOUNTS, 0x00
.equ DATA_LEN, 0x08
.equ DATA, 0x10

.globl entrypoint
entrypoint:
  ldxdw r0, [r1+NUM_ACCOUNTS]
  ldxdw r2, [r1+DATA_LEN]
  add64 r1, DATA
  call sol_log_
  exit"#;
    update_assembly_file(&env, "memo2", new_memo_content);

    // Step 4: Run build
    run_build(&env);

    // Step 5: Verify .so files were created
    verify_so_files(&env);

    // Step 6: Run tests
    // run_tests(&env);

    // Step 7: Clean up
    env.cleanup();
    println!("🎉 Second test passed! Memo2 project E2E test completed successfully.");
}
