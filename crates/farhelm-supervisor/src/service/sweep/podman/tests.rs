use super::*;
use std::os::unix::fs::PermissionsExt;

const ID: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
const OTHER_ID: &str = "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789";

/// Reproduce the literal argument boundary conmon uses for a stop-time
/// cleanup. A value after --exit-command-arg is not a conmon flag.
fn conmon(flags: &[&str], cleanup: &[&str]) -> Vec<Vec<u8>> {
    [
        "/usr/bin/conmon",
        "-c",
        ID,
        "-n",
        "owned",
        "--exit-command",
        "/ignored/podman",
    ]
    .into_iter()
    .chain(flags.iter().copied())
    .chain(cleanup.iter().flat_map(|arg| ["--exit-command-arg", *arg]))
    .map(|arg| arg.as_bytes().to_vec())
    .collect()
}

/// The storage namespace is part of the identity. Keep selectors as literal
/// arguments while leaving the runtime, hooks, and executable under Podman's
/// own configuration.
#[test]
fn takes_storage_selectors_from_a_main_monitors_cleanup() {
    let argv = conmon(
        &[],
        &[
            "--root",
            "/example/private root",
            "--runroot=/example/run",
            "--runtime",
            "--root",
            "--hooks-dir",
            "/ignored",
            "--module",
            "/ignored/module",
            "--network-config-dir",
            "",
            "--imagestore",
            "/example/images",
            "--storage-opt",
            "vfs.ignore_chown_errors=true",
            "--storage-opt=overlay.mount_program=/ignored/program",
            "--transient-store=false",
            "container",
            "cleanup",
            "--stopped-only",
            "--rm",
            "--rmi",
            ID,
        ],
    );
    let found = Container::from_argv(&argv).expect("main monitor with a matching cleanup");
    assert_eq!(found.id, ID);
    assert_eq!(found.name, "owned");
    assert!(found.auto_remove);
    assert_eq!(
        found.storage,
        [
            "--root=/example/private root",
            "--runroot=/example/run",
            "--imagestore=/example/images",
            "--storage-opt=vfs.ignore_chown_errors=true",
            "--transient-store=false",
        ]
        .map(OsString::from)
    );
}

/// An exec monitor neither started nor owns the container. A parse that
/// cannot tie the cleanup suffix to the main monitor supplies no stop target.
#[test]
fn ignores_exec_and_incomplete_or_mismatched_monitors() {
    for argv in [
        conmon(&["--exec"], &["container", "cleanup", ID]),
        conmon(&["-e"], &["container", "cleanup", ID]),
        conmon(&[], &["container", "cleanup", OTHER_ID]),
        conmon(&[], &["pod", "cleanup", ID]),
        conmon(&[], &["--root", "container", "cleanup", ID]),
    ] {
        assert!(Container::from_argv(&argv).is_none(), "{argv:?}");
    }
    let argv = conmon(&[], &["--root", "--exec", "container", "cleanup", ID]);
    assert_eq!(
        Container::from_argv(&argv).unwrap().storage,
        ["--root=--exec"]
    );
}

/// The fake writes the command path before returning so the assertion also
/// distinguishes a stop from its follow-up state reads.
fn fake_podman(
    dir: &Path,
    stop_body: &str,
    inspect_body: &str,
    exists_body: &str,
    rm_body: &str,
) -> std::path::PathBuf {
    let program = dir.join("podman");
    std::fs::write(
        &program,
        format!(
            "#!/bin/sh\n[ \"$1\" = --remote=false ] || exit 42\nshift\ncase \"$1 $2\" in\n\
             'stop --time') echo stop >> '{calls}'; {stop_body} ;;\n\
             'container inspect') echo inspect >> '{calls}'; {inspect_body} ;;\n\
             'container exists') echo exists >> '{calls}'; {exists_body} ;;\n\
             'rm --ignore') echo rm >> '{calls}'; {rm_body} ;;\n\
             *) exit 42 ;;\nesac\n",
            calls = dir.join("calls").display(),
        ),
    )
    .unwrap();
    std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o700)).unwrap();
    program
}

/// A failed command does not necessarily leave a running container behind.
/// Use its observed state, including positively confirmed absence, to choose
/// whether the caller needs a failure notice before continuing the sweep.
#[tokio::test]
async fn stop_checks_failed_state_and_finishes_auto_removal() {
    for (auto_remove, stop_body, inspect_body, exists_body, rm_body, success, calls) in [
        (
            false, "exit 0", "exit 42", "exit 42", "exit 42", true, "stop\n",
        ),
        (
            true,
            "exit 0",
            "exit 42",
            "exit 42",
            "exit 0",
            true,
            "stop\nrm\n",
        ),
        (
            true,
            "exit 0",
            "exit 42",
            "exit 42",
            "exit 125",
            false,
            "stop\nrm\n",
        ),
        (
            false,
            "exit 125",
            "echo exited",
            "exit 42",
            "exit 42",
            true,
            "stop\ninspect\n",
        ),
        (
            true,
            "exit 125",
            "echo exited",
            "exit 42",
            "exit 0",
            true,
            "stop\ninspect\nrm\n",
        ),
        (
            true,
            "exit 125",
            "echo exited",
            "exit 42",
            "exit 125",
            false,
            "stop\ninspect\nrm\n",
        ),
        (
            false,
            "exit 125",
            "echo running",
            "exit 42",
            "exit 42",
            false,
            "stop\ninspect\n",
        ),
        (
            true,
            "exit 125",
            "exit 125",
            "exit 1",
            "exit 42",
            true,
            "stop\ninspect\nexists\n",
        ),
        (
            false,
            "exit 125",
            "exit 125",
            "exit 125",
            "exit 42",
            false,
            "stop\ninspect\nexists\n",
        ),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let podman = fake_podman(dir.path(), stop_body, inspect_body, exists_body, rm_body);
        let cleanup = if auto_remove {
            ["container", "cleanup", "--rm", ID].as_slice()
        } else {
            ["container", "cleanup", ID].as_slice()
        };
        let container = Container::from_argv(&conmon(&[], cleanup)).unwrap();
        let now = Instant::now();
        assert_eq!(
            stop(&podman, &container, now + STOP_BUDGET, now + BUDGET).await,
            success
        );
        assert_eq!(
            std::fs::read_to_string(dir.path().join("calls")).unwrap(),
            calls
        );
    }
}

/// A stuck Podman child must leave part of the shared deadline for a state
/// read. Publish a call witness before sleeping so a missed launch cannot
/// pass as a timed-out command.
#[tokio::test]
async fn timed_out_stop_leaves_time_to_confirm_state() {
    let dir = tempfile::tempdir().unwrap();
    let podman = fake_podman(
        dir.path(),
        "exec sleep 90",
        "echo exited",
        "exit 42",
        "exit 42",
    );
    let container = Container::from_argv(&conmon(&[], &["container", "cleanup", ID])).unwrap();
    let now = Instant::now();
    assert!(
        stop(
            &podman,
            &container,
            now + Duration::from_secs(2),
            now + BUDGET
        )
        .await
    );
    assert_eq!(
        std::fs::read_to_string(dir.path().join("calls")).unwrap(),
        "stop\ninspect\n"
    );
}
