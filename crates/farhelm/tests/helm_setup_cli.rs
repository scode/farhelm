//! Process-level proof for the parts of `farhelm helm setup` that only a
//! child process can observe.
//!
//! Everything inside `setup.rs` is driven through an injected
//! `SetupContext`, deliberately: this repository's tests never mutate the
//! test runner's own environment. That leaves exactly one thing untested
//! by construction — the code in `main` that CAPTURES that environment,
//! including its refusal when `HOME` is missing. A child process is how
//! that gets exercised without touching this process's variables.

use std::process::{Command, Stdio};

/// The Linux entry point refuses when `HOME` is unset, and does so before
/// touching anything.
///
/// Every path setup writes hangs off `HOME` — the unit directory and, with
/// no `--state-dir`, the state directory too. Guessing one (the current
/// directory, `/`, the passwd entry) would install units that name a
/// directory the operator never chose, so the command stops and says why.
///
/// `HOME` is removed from the CHILD's environment only. The dry-run flag
/// keeps the test honest about side effects even if the refusal regressed:
/// nothing would be written either way.
#[cfg(target_os = "linux")]
#[farhelm_testtrace::test]
fn setup_refuses_to_run_without_home() {
    let output = Command::new(env!("CARGO_BIN_EXE_farhelm"))
        .args(["helm", "setup", "--dry-run"])
        .env_remove("HOME")
        .stdin(Stdio::null())
        .output()
        .expect("run the farhelm setup child");

    assert!(!output.status.success(), "{output:?}");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr
            .contains("HOME is not set, so farhelm helm setup cannot tell where your units belong"),
        "{stderr}"
    );
    // The refusal is the whole output: no unit was rendered, and no
    // systemctl command was announced.
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.is_empty(), "{stdout}");
}

/// An empty `HOME` is the same as no `HOME`.
///
/// Service managers and container images set it that way, and treating the
/// empty string as a directory would resolve every unit path against the
/// filesystem root.
#[cfg(target_os = "linux")]
#[farhelm_testtrace::test]
fn setup_treats_an_empty_home_as_missing() {
    let output = Command::new(env!("CARGO_BIN_EXE_farhelm"))
        .args(["helm", "setup", "--dry-run"])
        .env("HOME", "")
        .stdin(Stdio::null())
        .output()
        .expect("run the farhelm setup child");

    assert!(!output.status.success(), "{output:?}");
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("HOME is not set"),
        "{output:?}"
    );
}
