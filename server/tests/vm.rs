//! Tests that use disposable VMs for integration testing.
//!
//! Each test defined in this file runs the corresponding `vm-*.rs` test file in a new VM.
//! Due to high overhead, try to group tests into relatively few such files.

mod vm_test_runner;
use self::vm_test_runner::run_vm_test;

// RUST-WART: `libtest` has no means of defining subtests, or discovering tests at runtime.
// For now, we just duplicate the list of `tests/vm-*.rs` here.

#[test]
#[ignore]
fn smoke() {
    run_vm_test("vm-smoke");
}
