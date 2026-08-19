use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// A startup failure must exit while the desktop parent still holds stdin.
///
/// This pins the reason the tether reader lives on a detached OS thread. A
/// Tokio blocking-pool reader would keep runtime shutdown waiting for EOF and
/// turn the real supervisor error into a permanently wedged child.
#[test]
fn supervisor_startup_failure_is_not_held_open_by_the_stdin_tether() {
    let root = farhelm_teststate::tempdir().unwrap();
    let blocking_file = root.path().join("not-a-directory");
    std::fs::write(&blocking_file, b"fixture").unwrap();
    let state_dir = blocking_file.join("state");
    let mut child = Command::new(env!("CARGO_BIN_EXE_farhelm"))
        .args(["supervisor", "run", "--exit-on-stdin-close", "--state-dir"])
        .arg(&state_dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let _stdin_kept_open = child.stdin.take().unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);

    loop {
        if let Some(status) = child.try_wait().unwrap() {
            assert!(!status.success());
            break;
        }
        if Instant::now() >= deadline {
            child.kill().unwrap();
            let _ = child.wait();
            panic!("supervisor startup failure remained blocked on its open stdin tether");
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}
