//! Process-level proof for `farhelm internal build-info`
//! (`.github/workflows/sign-sums.yml` runs it on the published binary).
//!
//! The marked answer's spelling is unit-tested in `main.rs`. What only a child
//! process shows is the wiring: that the built binary reports the mark its
//! linked helm was compiled with.

use std::process::{Command, Stdio};

/// A test build is not a release build, and the command must say so.
///
/// Why: the release workflow trusts this answer to stop a release whose helm
/// has no host payloads. If the command reported anything but the helm's own
/// compile-time value, an unmarked binary could pass that check.
///
/// Spec: run as a child, the command exits 0 and prints exactly
/// `version=<this crate's version>\nrelease-build=no\n`. The premise, an
/// unmarked test build, is asserted first: the release gate builds its tests
/// before the mark is exported (the last step of
/// `.github/dist-build-setup.yml`), and a build made with the mark set is not
/// what this test is about.
#[farhelm_testtrace::test]
fn build_info_reports_an_unmarked_test_build() {
    // An `if`, not `assert!`: the value is a compile-time constant, which
    // clippy refuses in an assertion.
    if farhelm_helm::RELEASE_BUILD {
        panic!(
            "this test build carries the release mark; the expectation below assumes an ordinary development build"
        );
    }
    let output = Command::new(env!("CARGO_BIN_EXE_farhelm"))
        .args(["internal", "build-info"])
        .stdin(Stdio::null())
        .output()
        .expect("run the farhelm build-info child");

    assert!(output.status.success(), "{output:?}");
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        format!("version={}\nrelease-build=no\n", env!("CARGO_PKG_VERSION"))
    );
}
