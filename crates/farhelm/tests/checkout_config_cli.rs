//! Process-level proof for `farhelm helm checkout-config`'s storage rule
//! R1.5 and its CLI surface: the command opens an EXISTING, CURRENT-schema
//! helm.db and creates and migrates nothing, every fixture lives in a
//! private temp state dir passed by explicit `--state-dir` (this process's
//! environment is never touched), and `show` reflects each mutation the
//! other verbs make.

use std::path::Path;
use std::process::Command;

/// The fixture database stamped at schema version 26 — the version BEFORE
/// checkout configuration existed, committed as a file because the farhelm
/// crate has no raw sqlite dependency with which to build one at test time.
/// A 26→27 migration is purely additive DDL, so this file (registry table
/// plus the old version stamp) is a faithful "older helm.db".
const V26_FIXTURE: &str = "tests/fixtures/helm-v26.db";

/// Run the real binary with an explicit `--state-dir`, capturing all output.
/// The state-dir flag is appended AFTER the verb, matching the per-verb
/// grammar the token commands established (`farhelm helm token show
/// --state-dir ...`).
fn checkout_config(state_dir: &Path, args: &[&str]) -> (bool, String, String) {
    let mut argv = vec!["helm", "checkout-config"];
    argv.extend_from_slice(args);
    argv.push("--state-dir");
    argv.push(state_dir.to_str().expect("utf-8 state dir"));
    let output = Command::new(env!("CARGO_BIN_EXE_farhelm"))
        .args(&argv)
        .stdin(std::process::Stdio::null())
        .output()
        .expect("run the farhelm checkout-config child");
    (
        output.status.success(),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

/// A private state dir holding a CURRENT-schema helm.db built through the
/// helm's own store API — the only legitimate way a test mints one, since
/// the checkout-config CLI itself must never create.
fn fixture_db_path(state_dir: &Path) -> std::path::PathBuf {
    state_dir.join("helm.db")
}

/// Spec (R1.5): with the state dir absent, the CLI exits nonzero and the
/// directory STILL does not exist — the pure state-dir resolution plus the
/// never-create open mean a mistyped `--state-dir` configures nothing.
#[farhelm_testtrace::test]
fn checkout_config_absent_state_dir_fails_without_creating_it() {
    let dir = tempfile::tempdir().expect("tempdir");
    let state_dir = dir.path().join("does-not-exist");
    let (ok, stdout, stderr) = checkout_config(&state_dir, &["show"]);
    assert!(!ok, "an absent state dir must fail: {stdout}");
    assert!(
        stderr.contains("start (or update) the helm"),
        "the refusal must name the remedy: {stderr}"
    );
    assert!(
        !state_dir.exists(),
        "the state dir must still not exist after the refusal"
    );
}

/// Spec (R1.5): with a state dir but no helm.db in it, the CLI exits
/// nonzero and helm.db is still absent — the CREATE flag is omitted at the
/// sqlite level, not merely prechecked.
#[farhelm_testtrace::test]
fn checkout_config_absent_database_fails_without_creating_it() {
    let dir = tempfile::tempdir().expect("tempdir");
    let state_dir = dir.path().join("empty-state");
    std::fs::create_dir(&state_dir).expect("create the state dir");
    let (ok, _stdout, stderr) = checkout_config(&state_dir, &["show"]);
    assert!(!ok, "an absent helm.db must fail: {stderr}");
    assert!(
        stderr.contains("start (or update) the helm"),
        "the refusal must name the remedy: {stderr}"
    );
    assert!(
        !fixture_db_path(&state_dir).exists(),
        "helm.db must still not exist after the refusal"
    );
}

/// Spec (R1.5): an OLDER-schema helm.db is refused WITHOUT migrating — the
/// error names the version and the remedy, the file is byte-for-byte
/// identical afterwards, and the local row the helm's own `open` would
/// mint was not minted here either.
#[farhelm_testtrace::test]
fn checkout_config_refuses_older_schema_without_migrating() {
    let dir = tempfile::tempdir().expect("tempdir");
    let state_dir = dir.path().join("state");
    std::fs::create_dir(&state_dir).expect("create the state dir");
    let db_path = fixture_db_path(&state_dir);
    std::fs::copy(V26_FIXTURE, &db_path).expect("plant the v26 fixture");
    let before = std::fs::read(&db_path).expect("read the fixture back");

    let (ok, _stdout, stderr) = checkout_config(&state_dir, &["show"]);
    assert!(!ok, "a v26 database must be refused: {stderr}");
    assert!(
        stderr.contains("schema version 26") && stderr.contains("start (or update) the helm"),
        "the refusal must name the version and the remedy: {stderr}"
    );
    let after = std::fs::read(&db_path).expect("read the database back");
    assert_eq!(
        before, after,
        "the refused database must be byte-identical — no migration may run"
    );
}

/// Spec: the full CLI round trip against a CURRENT-schema database —
/// global set/clear for both fields, a per-host override and its
/// clear-to-inherit, and `show` reflecting every step including the
/// revision. Writes go through the real binary; the host row comes from
/// the helm's own store API, since this slice adds no host-management CLI.
#[tokio::test]
async fn checkout_config_set_show_clear_round_trip_through_the_binary() {
    let dir = tempfile::tempdir().expect("tempdir");
    let state_dir = dir.path().join("state");
    std::fs::create_dir(&state_dir).expect("create the state dir");
    let store = farhelm_helm::store::HelmStore::open(fixture_db_path(&state_dir).as_path())
        .await
        .expect("create fixture helm.db");
    let host = store
        .add_ssh_host("checkout.example", None, None)
        .await
        .expect("register the fixture host");
    drop(store);

    // Nothing is set yet: the global view is unset at revision 0.
    let (ok, stdout, stderr) = checkout_config(&state_dir, &["show"]);
    assert!(ok, "{stderr}");
    assert_eq!(
        stdout,
        "scope: global\nroot: unset\npost-clone: unset\nrevision: 0\n"
    );

    // Global settings, one mutation at a time, each visible in `show`.
    let (ok, stdout, stderr) = checkout_config(&state_dir, &["set-root", "~/worktrees"]);
    assert!(ok, "{stderr}");
    assert!(stdout.contains("revision is now 1"), "{stdout}");
    let (ok, _stdout, stderr) = checkout_config(&state_dir, &["set-post-clone", "make sync"]);
    assert!(ok, "{stderr}");
    let (ok, stdout, stderr) = checkout_config(&state_dir, &["show"]);
    assert!(ok, "{stderr}");
    assert_eq!(
        stdout,
        "scope: global\nroot: ~/worktrees\npost-clone: make sync\nrevision: 2\n"
    );

    // A host override wins per field; the other field inherits.
    let host_arg = host.to_string();
    let (ok, _stdout, stderr) =
        checkout_config(&state_dir, &["set-root", "/host/root", "--host", &host_arg]);
    assert!(ok, "{stderr}");
    let (ok, stdout, stderr) = checkout_config(&state_dir, &["show", "--host", &host_arg]);
    assert!(ok, "{stderr}");
    assert_eq!(
        stdout,
        format!(
            "scope: host {host}\nroot: /host/root (host override)\n\
             post-clone: make sync (inherited from global)\nrevision: 3\n"
        )
    );

    // Clearing the host override returns the field to inheritance.
    let (ok, _stdout, stderr) = checkout_config(&state_dir, &["clear-root", "--host", &host_arg]);
    assert!(ok, "{stderr}");
    let (ok, stdout, stderr) = checkout_config(&state_dir, &["show", "--host", &host_arg]);
    assert!(ok, "{stderr}");
    assert!(
        stdout.contains("root: ~/worktrees (inherited from global)"),
        "{stdout}"
    );

    // Global clear unsets for everyone.
    let (ok, _stdout, stderr) = checkout_config(&state_dir, &["clear-root"]);
    assert!(ok, "{stderr}");
    let (ok, stdout, stderr) = checkout_config(&state_dir, &["show"]);
    assert!(ok, "{stderr}");
    assert_eq!(
        stdout,
        "scope: global\nroot: unset\npost-clone: make sync\nrevision: 5\n"
    );

    // Clear the rest so the revision ledger above stays the whole story.
    let (ok, _stdout, stderr) = checkout_config(&state_dir, &["clear-post-clone"]);
    assert!(ok, "{stderr}");
    let (ok, stdout, stderr) = checkout_config(&state_dir, &["show"]);
    assert!(ok, "{stderr}");
    assert!(stdout.contains("revision: 6"), "{stdout}");
}
