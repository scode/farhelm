//! Compile contracts for supported expansion and every rejected attribute shape.
//!
//! One temporary project reuses one isolated target directory across serial cases. It copies the
//! workspace lockfile, lets the first offline check prune it for this smaller graph, and rejects
//! any dependency identity change before the remaining cases run locked. The isolated target
//! avoids contending for the parent test process's build lock.

#![cfg(unix)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

#[path = "../../farhelm-testtrace/tests/support/process.rs"]
mod process;

const COMMAND_TIMEOUT: Duration = Duration::from_secs(120);
const OUTPUT_LIMIT: usize = 2 * 1024 * 1024;

/// One supported spelling or focused rejection with its stable diagnostic fragment.
struct CompileCase {
    /// Cargo integration-target name and diagnostic label.
    name: &'static str,
    /// Complete source compiled for this one case.
    source: &'static str,
    /// Stable compiler fragment for rejected cases; absence means compilation must succeed.
    expected_error: Option<&'static str>,
}

/// The whole matrix shares dependency artifacts but gives each case a distinct source target.
#[test]
fn attribute_compile_contract_matrix() {
    let project = tempfile::tempdir().expect("create isolated compile fixture");
    let workspace = workspace_root();
    write_project(project.path(), &workspace);

    for (index, case) in cases().into_iter().enumerate() {
        let source_path = project
            .path()
            .join("tests")
            .join(format!("{}.rs", case.name));
        fs::write(&source_path, case.source).expect("write compile-contract source");
        let result = run_cargo(project.path(), case.name, index != 0);
        if index == 0 {
            assert!(
                result.status.success(),
                "initial supported case failed:\n{}",
                result.output.display()
            );
            assert_locked_dependencies(project.path(), &workspace);
        }
        match case.expected_error {
            None => assert!(
                result.status.success(),
                "supported case {} failed:\n{}",
                case.name,
                result.output.display()
            ),
            Some(expected) => {
                assert!(
                    !result.status.success(),
                    "unsupported case {} unexpectedly compiled",
                    case.name
                );
                assert!(
                    result.output.display().contains(expected),
                    "case {} did not emit {expected:?}:\n{}",
                    case.name,
                    result.output.display()
                );
            }
        }
    }
}

/// Resolves the source and lockfile under test from the compile-test crate's fixed workspace path.
fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("macro crate is two levels below workspace root")
        .to_owned()
}

/// Builds an isolated child manifest whose only input is the support crate under test.
fn write_project(project: &Path, workspace: &Path) {
    fs::create_dir_all(project.join("tests")).expect("create fixture source directory");
    let support = workspace.join("crates/farhelm-testtrace");
    let manifest = format!(
        "[package]\nname = \"farhelm-testtrace-compile-contract\"\nversion = \"0.0.0\"\nedition = \"2024\"\n\n[dependencies]\nfarhelm-testtrace = {{ path = {:?} }}\n\n[workspace]\n",
        support
    );
    fs::write(project.join("Cargo.toml"), manifest).expect("write fixture manifest");

    let mut lock =
        fs::read_to_string(workspace.join("Cargo.lock")).expect("read workspace lockfile");
    lock.push_str(
        "\n[[package]]\nname = \"farhelm-testtrace-compile-contract\"\nversion = \"0.0.0\"\ndependencies = [\n \"farhelm-testtrace\",\n]\n",
    );
    fs::write(project.join("Cargo.lock"), lock).expect("write locked fixture graph");
}

/// Requires the temporary graph to use only dependency identities pinned in the workspace.
///
/// Cargo must remove irrelevant workspace entries and rewrite path-package dependency lists.
/// Those structural edits are allowed; changing a version, source or checksum is not. This
/// catches accidental offline resolution drift without depending on Cargo's TOML formatting.
fn assert_locked_dependencies(project: &Path, workspace: &Path) {
    let read_packages = |root: &Path| {
        let text = fs::read_to_string(root.join("Cargo.lock")).expect("read compile lockfile");
        let lock: toml::Value = toml::from_str(&text).expect("parse compile lockfile");
        lock["package"]
            .as_array()
            .expect("lockfile package array")
            .clone()
    };
    let original = read_packages(workspace);
    for package in read_packages(project) {
        if package["name"].as_str() == Some("farhelm-testtrace-compile-contract") {
            assert_eq!(package["version"].as_str(), Some("0.0.0"));
            assert!(package.get("source").is_none());
            continue;
        }
        assert!(
            original.iter().any(|pinned| {
                ["name", "version", "source", "checksum"]
                    .into_iter()
                    .all(|field| pinned.get(field) == package.get(field))
            }),
            "temporary compile graph changed a pinned dependency: {package}",
        );
    }
}

/// Checks one case under a bounded process group and explicit Cargo environment.
/// Only the initial positive case may prune the copied graph; all later cases run locked.
fn run_cargo(project: &Path, binary: &str, locked: bool) -> process::CommandResult {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let mut command = Command::new(cargo);
    command
        .current_dir(project)
        .args([
            "check",
            "--offline",
            "--manifest-path",
            project
                .join("Cargo.toml")
                .to_str()
                .expect("UTF-8 fixture path"),
            "--test",
            binary,
        ])
        .env_clear()
        .env("CARGO_INCREMENTAL", "0")
        .env("CARGO_NET_OFFLINE", "true")
        .env("CARGO_TARGET_DIR", project.join("target"))
        .env("CARGO_TERM_COLOR", "never");
    if locked {
        command.arg("--locked");
    }
    // Keep rustup's selected toolchain: dropping it makes nested compiler shims rediscover
    // overrides and can trigger component installation concurrently with compilation.
    for name in [
        "PATH",
        "HOME",
        "CARGO_HOME",
        "RUSTUP_HOME",
        "RUSTUP_TOOLCHAIN",
        "RUSTC",
        "RUSTDOC",
    ] {
        if let Some(value) = std::env::var_os(name) {
            command.env(name, value);
        }
    }
    process::run_bounded(command, COMMAND_TIMEOUT, OUTPUT_LIMIT)
        .unwrap_or_else(|failure| panic!("compile case {binary}: {failure}"))
}

/// The compiler harness shares the same bounded timeout, overflow, and reaping behavior.
#[test]
fn bounded_cargo_supervision_kills_and_reaps() {
    process::assert_supervision_contracts();
}

/// Defines positive spellings and focused failures without brittle full-output snapshots.
fn cases() -> Vec<CompileCase> {
    vec![
        CompileCase {
            name: "supported_spellings",
            source: r#"
use farhelm_testtrace::test;
use std::process::{ExitCode, Termination};

#[test]
fn imported_name_still_emits_builtin_test() {}

#[farhelm_testtrace::test]
fn sync_result() -> Result<(), &'static str> { Ok(()) }

#[farhelm_testtrace::test]
fn exit_code() -> ExitCode { ExitCode::SUCCESS }

#[farhelm_testtrace::test]
async fn default_async() {}

#[farhelm_testtrace::test(flavor = "current_thread", start_paused = false)]
async fn explicit_current() {}

#[farhelm_testtrace::test(start_paused = true)]
async fn paused_current() {}

#[farhelm_testtrace::test(flavor = "multi_thread")]
async fn multi() {}

#[farhelm_testtrace::test(flavor = "multi_thread", worker_threads = 2, start_paused = false)]
async fn workers() {}

struct Custom;
impl farhelm_testtrace::TestOutcome for Custom {
    fn observed_success(&self) -> bool { true }
}
impl Termination for Custom {
    fn report(self) -> ExitCode { ExitCode::SUCCESS }
}
#[farhelm_testtrace::test]
fn custom() -> Custom { Custom }

fn main() {}
"#,
            expected_error: None,
        },
        CompileCase {
            name: "unsupported_option",
            source: r#"#[farhelm_testtrace::test(unhandled = true)] async fn case() {} fn main() {}"#,
            expected_error: Some("unsupported farhelm_testtrace::test option"),
        },
        CompileCase {
            name: "duplicate_option",
            source: r#"#[farhelm_testtrace::test(flavor = "current_thread", flavor = "multi_thread")] async fn case() {} fn main() {}"#,
            expected_error: Some("duplicate flavor option"),
        },
        CompileCase {
            name: "duplicate_workers",
            source: r#"#[farhelm_testtrace::test(flavor = "multi_thread", worker_threads = 1, worker_threads = 2)] async fn case() {} fn main() {}"#,
            expected_error: Some("duplicate worker_threads option"),
        },
        CompileCase {
            name: "duplicate_paused",
            source: r#"#[farhelm_testtrace::test(start_paused = false, start_paused = true)] async fn case() {} fn main() {}"#,
            expected_error: Some("duplicate start_paused option"),
        },
        CompileCase {
            name: "zero_workers",
            source: r#"#[farhelm_testtrace::test(flavor = "multi_thread", worker_threads = 0)] async fn case() {} fn main() {}"#,
            expected_error: Some("worker_threads must be at least one"),
        },
        CompileCase {
            name: "invalid_flavor",
            source: r#"#[farhelm_testtrace::test(flavor = "basic_scheduler")] async fn case() {} fn main() {}"#,
            expected_error: Some("supports only current_thread or multi_thread flavor"),
        },
        CompileCase {
            name: "sync_options",
            source: r#"#[farhelm_testtrace::test(start_paused = false)] fn case() {} fn main() {}"#,
            expected_error: Some(
                "synchronous farhelm_testtrace::test functions do not accept runtime options",
            ),
        },
        CompileCase {
            name: "workers_on_current",
            source: r#"#[farhelm_testtrace::test(flavor = "current_thread", worker_threads = 2)] async fn case() {} fn main() {}"#,
            expected_error: Some("worker_threads requires flavor = \"multi_thread\""),
        },
        CompileCase {
            name: "paused_multi",
            source: r#"#[farhelm_testtrace::test(flavor = "multi_thread", start_paused = true)] async fn case() {} fn main() {}"#,
            expected_error: Some("start_paused = true requires the current_thread flavor"),
        },
        CompileCase {
            name: "parameters",
            source: r#"#[farhelm_testtrace::test] fn case(value: usize) {} fn main() {}"#,
            expected_error: Some("functions cannot take parameters"),
        },
        CompileCase {
            name: "generic",
            source: r#"#[farhelm_testtrace::test] fn case<T>() {} fn main() {}"#,
            expected_error: Some("functions cannot be generic"),
        },
        CompileCase {
            name: "unsafe_shape",
            source: r#"#[farhelm_testtrace::test] unsafe fn case() {} fn main() {}"#,
            expected_error: Some("requires an ordinary safe function"),
        },
        CompileCase {
            name: "conflicting_test",
            source: r#"#[farhelm_testtrace::test] #[test] fn case() {} fn main() {}"#,
            expected_error: Some("do not stack test attributes"),
        },
        CompileCase {
            name: "conflicting_cfg_attr",
            source: r#"#[farhelm_testtrace::test] #[cfg_attr(unix, test)] fn case() {} fn main() {}"#,
            expected_error: Some("do not stack test attributes"),
        },
        CompileCase {
            name: "conditional_should_panic",
            source: r#"#[farhelm_testtrace::test] #[cfg_attr(unix, should_panic(expected = "conditional"))] fn case() {} fn main() {}"#,
            expected_error: None,
        },
        CompileCase {
            name: "conflicting_qualified_test",
            source: r#"#[farhelm_testtrace::test] #[tokio::test] async fn case() {} fn main() {}"#,
            expected_error: Some("do not stack test attributes"),
        },
        CompileCase {
            name: "custom_outcome_requires_observation",
            source: r#"
use std::process::{ExitCode, Termination};
struct Custom;
impl Termination for Custom { fn report(self) -> ExitCode { ExitCode::SUCCESS } }
#[farhelm_testtrace::test]
fn case() -> Custom { Custom }
fn main() {}
"#,
            expected_error: Some("TestOutcome"),
        },
    ]
}
