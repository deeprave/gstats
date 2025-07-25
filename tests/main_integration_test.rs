// Integration test for main binary functionality
// NOTE: This test is TEMPORARY and will be removed when main.rs
// is updated with actual gstats functionality
//
// This test validates our testing infrastructure by following TDD methodology:
// 1. RED: Created failing test expecting different output
// 2. GREEN: Updated main.rs to make test pass  
// 3. REFACTOR: Added documentation and verified test patterns

use std::process::Command;

/// Test that validates the main binary produces expected output
/// 
/// This integration test runs the compiled binary and checks its stdout output.
/// It demonstrates our testing infrastructure works correctly and establishes
/// patterns for future testing.
#[test]
fn test_main_binary_output() {
    // Run the main binary and capture output
    let output = Command::new("cargo")
        .args(&["run", "--quiet"])
        .output()
        .expect("Failed to execute cargo run");

    let stdout = String::from_utf8(output.stdout).expect("Invalid UTF-8 output");
    
    // Verify the output matches our expected testing message
    // This validates that both the binary compilation and execution work correctly
    assert_eq!(stdout.trim(), "gstats testing infrastructure validated!");
}
