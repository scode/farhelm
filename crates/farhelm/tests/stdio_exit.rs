//! Process-level proof for the one exit status that unlocks provisioning.

use std::process::{Command, Stdio};

/// Run the shipped stdio proxy as a child so exit-code policy is tested past
/// `main`, including `process::exit` paths that a unit test cannot observe.
fn run_stdio(state_dir: &std::path::Path) -> std::process::ExitStatus {
    Command::new(env!("CARGO_BIN_EXE_farhelm"))
        .args(["internal", "stdio", "--state-dir"])
        .arg(state_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("run the farhelm stdio child")
}

/// Missing and refused sockets are positive absence; a different connection
/// error must not grant the helm permission to offer provisioning.
#[test]
fn stdio_exit_75_is_reserved_for_positive_absence() {
    let root = tempfile::tempdir().expect("isolated stdio state roots");
    assert_eq!(run_stdio(&root.path().join("missing")).code(), Some(75));

    let refused = root.path().join("refused");
    std::fs::create_dir(&refused).expect("create refused state directory");
    let socket = refused.join("supervisor.sock");
    let listener = std::os::unix::net::UnixListener::bind(&socket).expect("bind stale socket");
    drop(listener);
    assert_eq!(run_stdio(&refused).code(), Some(75));

    let not_directory = root.path().join("not-a-directory");
    std::fs::write(&not_directory, b"not a directory").expect("create ENOTDIR fixture");
    assert_eq!(run_stdio(&not_directory).code(), Some(1));
}
