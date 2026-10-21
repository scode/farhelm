//! Exercise the actual internal guard/helper exec boundary, without a helm,
//! network, user rc files or process-global environment mutation. Every process
//! and directory here belongs to the fixture; no installed service is involved.

use farhelm_supervisor::bounded_command::{Ended, GroupChild, OutputCaps};
use serde_json::json;
use std::fs;
use std::os::fd::AsRawFd as _;
use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};
use std::os::unix::process::ExitStatusExt as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

const CAPS: OutputCaps = OutputCaps {
    stdout: 65_536,
    stderr: Some(65_536),
};
const BINARY: &str = env!("CARGO_BIN_EXE_farhelm");

/// The public CLI consumes its private on-disk format; the fixture records real
/// inode/device evidence with no birth-time assumption about the test filesystem.
struct Fixture {
    _temporary: tempfile::TempDir,
    root: PathBuf,
    spec: PathBuf,
    target: PathBuf,
    cache: PathBuf,
}

impl Fixture {
    /// A fake login shell executes the supplied payload after an owned rc body.
    /// Git is real and URL-rewritten to a private bare upstream; no credential
    /// fixture can accidentally reach GitHub.
    fn new(rc: &str, deadline: u64) -> Self {
        let temporary = tempfile::tempdir().unwrap();
        let root = temporary.path().canonicalize().unwrap();
        let namespace = root.join("checkouts/.farhelm-spares/acme");
        fs::create_dir_all(&namespace).unwrap();
        let target = namespace.join(format!(".bar:building-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&target).unwrap();
        let cache = root.join("state/repo-cache/acme/bar.git");
        fs::create_dir_all(cache.parent().unwrap()).unwrap();
        let git = Command::new("sh")
            .args(["-c", "command -v git"])
            .output()
            .unwrap();
        assert!(git.status.success());
        let git = String::from_utf8(git.stdout).unwrap().trim().to_owned();
        fs::create_dir(root.join("bin")).unwrap();
        fs::create_dir(root.join("empty-templates")).unwrap();
        let wrapper = format!(
            "#!/bin/sh\nprintf '%s\\n' \"$*\" >> \"$BUILD_ROOT/git-argv\"\n\
            printf '%s:%s:%s:%s:%s:%s:%s\\n' \"$GIT_TERMINAL_PROMPT\" \"$GIT_ASKPASS\" \"$SSH_ASKPASS\" \"$SSH_ASKPASS_REQUIRE\" \
            \"${{GIT_DIR-unset}}\" \"${{FARHELM_SESSION_TOKEN-unset}}\" \"${{FARHELM_SESSION_ID-unset}}\" >> \"$BUILD_ROOT/git-env\"\n\
            exec {} \"$@\"\n",
            farhelm_proto::text::shell_quote(&git)
        );
        script(&root.join("bin/git"), &wrapper);
        let shell = root.join("login-shell");
        script(
            &shell,
            &format!(
                "#!/bin/sh\ntest \"$1\" = -l && test \"$2\" = -i && test \"$3\" = -c || exit 90\n{rc}\nexec /bin/sh -c \"$4\"\n"
            ),
        );
        fs::write(root.join("gitconfig"), format!(
            "[protocol]\n\tallow = never\n[protocol \"file\"]\n\tallow = always\n[url \"file://{}/upstream.git\"]\n\tinsteadOf = https://github.com/acme/bar.git\n[credential]\n\thelper = fixture-credential-helper\n",
            root.display())).unwrap();
        let spec = root.join(format!("{}.json", uuid::Uuid::new_v4()));
        let evidence = |path: &Path| {
            let m = fs::metadata(path).unwrap();
            json!({"device":m.dev(),"inode":m.ino(),"birth_ns":null})
        };
        fs::write(&spec, serde_json::to_vec(&json!({
            "cache":cache,"repo":{"owner":"acme","name":"bar"},
            "target":{"namespace":namespace,"namespace_identity":evidence(&namespace),
                "name":target.file_name().unwrap().to_str().unwrap(),"identity":evidence(&target),
                "repo":{"owner":"acme","name":"bar"}},
            "shell":shell,"farhelm_exe":BINARY,"farhelm_bin_dir":root.join("bin"),
            "deadline_millis":deadline
        })).unwrap()).unwrap();
        Self {
            _temporary: temporary,
            root,
            spec,
            target,
            cache,
        }
    }

    /// Child-only environment injection is the supported way to prove startup
    /// and helper scrubs. The test runner's environment remains untouched.
    fn command(&self, verb: &str) -> Command {
        let mut command = Command::new(BINARY);
        self.environment(&mut command);
        command.args(["internal", verb]).arg(&self.spec);
        command
    }

    /// Owner-death tests also exec a disposable libtest owner. Give that child
    /// exactly the same isolated Git/config inputs as a direct guard invocation.
    fn environment(&self, command: &mut Command) {
        // A lost rewrite must fail locally, never contact GitHub. Clear config
        // injection as well as repository redirects before deliberate hostile
        // inputs below; the test runner's process environment stays unchanged.
        for (key, _) in
            std::env::vars_os().filter(|(key, _)| key.to_string_lossy().starts_with("GIT_"))
        {
            command.env_remove(key);
        }
        command
            .env("BUILD_ROOT", &self.root)
            .env("GIT_CONFIG_GLOBAL", self.root.join("gitconfig"))
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("HOME", &self.root)
            .env("GIT_ALLOW_PROTOCOL", "file")
            .env("GIT_TEMPLATE_DIR", self.root.join("empty-templates"))
            .env("GIT_DIR", self.root.join("must-not-be-used"))
            .env("FARHELM_SESSION_TOKEN", "fixture-only-secret")
            .env("FARHELM_SESSION_ID", "unrelated-session")
            .env("FARHELM_AGENT_ID", "unrelated-agent")
            .env("FARHELM_SUPERVISOR_SOCK", "/unrelated/socket")
            .env("FARHELM_TAB_ID", "unrelated-tab");
    }

    /// Create a real committed upstream. The result's commit and missing
    /// alternates distinguish dissociation from a merely successful fake clone.
    fn upstream(&self) -> String {
        let work = self.root.join("work");
        git(&self.root, &["init", "-q", work.to_str().unwrap()]);
        git(&work, &["symbolic-ref", "HEAD", "refs/heads/main"]);
        fs::write(work.join("tracked"), "complete fixture\n").unwrap();
        git(&work, &["add", "tracked"]);
        git(
            &work,
            &[
                "-c",
                "user.name=fixture",
                "-c",
                "user.email=fixture@example.invalid",
                "commit",
                "-qm",
                "fixture",
            ],
        );
        git(
            &self.root,
            &[
                "clone",
                "-q",
                "--bare",
                work.to_str().unwrap(),
                self.root.join("upstream.git").to_str().unwrap(),
            ],
        );
        git(&work, &["rev-parse", "HEAD"])
    }
}

/// Fixture executables are private and explicit, never installed on PATH for
/// the test runner or written into another user's startup files.
fn script(path: &Path, content: &str) {
    fs::write(path, content).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
}

/// Setup/inspection Git uses only fixture configuration and empty templates.
/// Protocol denial also covers a missing rewrite; no failure can reach GitHub.
fn git(cwd: &Path, args: &[&str]) -> String {
    let templates = tempfile::tempdir().unwrap();
    let output = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .env_clear()
        .env("PATH", std::env::var_os("PATH").unwrap())
        .env("HOME", cwd)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_TEMPLATE_DIR", templates.path())
        .env("GIT_ALLOW_PROTOCOL", "file")
        .env("GIT_TERMINAL_PROMPT", "0")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "setup git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}

/// All process/file polling names a positive oracle and has one shared budget.
/// Diagnostics remain inside the owning temporary fixture's lifetime.
async fn until(label: &str, mut ready: impl FnMut() -> bool) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    while !ready() {
        assert!(tokio::time::Instant::now() < deadline, "timed out: {label}");
        // sleep-ok: shared bounded polling of an explicit fixture readiness oracle.
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

/// Orphan descendants are not our waitable children; a zombie is terminated too.
fn terminated(pid: i32) -> bool {
    let output = Command::new("ps")
        .args(["-o", "stat=", "-p", &pid.to_string()])
        .output()
        .unwrap();
    let state = String::from_utf8_lossy(&output.stdout);
    state.trim().is_empty() || state.trim().starts_with('Z')
}

/// Own direct fixture processes through panic as well as success.
struct Owned(std::process::Child);
impl Drop for Owned {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// A recorded indirect child is minted by our owner fixture, not discovered by
/// process-name matching. Cleanup never signals a process group it does not own.
struct Indirect(Option<i32>);
impl Indirect {
    /// Once termination is observed, the numeric PID is no longer authority
    /// to signal anything: another process could reuse it before teardown.
    fn disarm(&mut self) {
        self.0 = None;
    }
}
impl Drop for Indirect {
    fn drop(&mut self) {
        if let Some(pid) = self.0 {
            unsafe {
                libc::kill(pid, libc::SIGKILL);
            }
        }
    }
}

/// A loopback-only Git endpoint which always requires a password. Counting real
/// requests distinguishes prompt refusal from a broken rewrite or blocked URL.
/// Drop wakes the owned accept thread and joins it before fixture teardown.
struct CredentialServer {
    address: std::net::SocketAddr,
    requests: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl CredentialServer {
    /// Bind before returning so the actual Git fetch cannot outrun readiness.
    fn new() -> Self {
        use std::io::{Read as _, Write as _};
        use std::sync::{
            Arc,
            atomic::{AtomicBool, AtomicUsize, Ordering},
        };
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let requests = Arc::new(AtomicUsize::new(0));
        let stop = Arc::new(AtomicBool::new(false));
        let count = requests.clone();
        let stopping = stop.clone();
        let thread = std::thread::spawn(move || {
            while let Ok((mut stream, _)) = listener.accept() {
                if stopping.load(Ordering::SeqCst) {
                    break;
                }
                stream
                    .set_read_timeout(Some(Duration::from_secs(1)))
                    .unwrap();
                stream
                    .set_write_timeout(Some(Duration::from_secs(1)))
                    .unwrap();
                let mut request = [0; 4096];
                if stream.read(&mut request).is_ok_and(|bytes| bytes > 0) {
                    stream.write_all(b"HTTP/1.1 401 Unauthorized\r\nWWW-Authenticate: Basic realm=\"fixture\"\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").unwrap();
                    count.fetch_add(1, Ordering::SeqCst);
                }
            }
        });
        Self {
            address,
            requests,
            stop,
            thread: Some(thread),
        }
    }
}

impl Drop for CredentialServer {
    fn drop(&mut self) {
        self.stop.store(true, std::sync::atomic::Ordering::SeqCst);
        let _ = std::net::TcpStream::connect_timeout(&self.address, Duration::from_secs(1));
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// Actual CLI success requires real clone completion, a guard promotion and
/// bounded natural SIGKILL. Startup and Git receive no session authority, Git
/// gets the post-rc prompt policy, and cache borrowing has been dissociated.
#[farhelm_testtrace::test]
async fn spare_builder_cli_real_clone_scrubs_after_login_and_dissociates() {
    let fixture = Fixture::new(
        "\
        test -z \"${FARHELM_SESSION_TOKEN-}${FARHELM_SESSION_ID-}${FARHELM_AGENT_ID-}${FARHELM_TAB_ID-}${FARHELM_SUPERVISOR_SOCK-}\" || exit 91\n\
        (exec </dev/tty) 2>/dev/null && exit 92\n\
        test -z \"$(cat)\" || exit 93\n\
        ps -o ni= -p $$ > \"$BUILD_ROOT/nice\"\n\
        export GIT_TERMINAL_PROMPT=1 GIT_ASKPASS=bad SSH_ASKPASS_REQUIRE=force",
        60_000,
    );
    let expected = fixture.upstream();
    let mut child =
        GroupChild::spawn_detached_tethered(&mut fixture.command("build-spare-guard").into(), CAPS)
            .unwrap();
    let ended = child
        .finish(tokio::time::Instant::now() + Duration::from_secs(60), CAPS)
        .await
        .unwrap();
    assert!(
        matches!(ended, Ended::Exited(ref output) if output.status.signal() == Some(libc::SIGKILL)),
        "{ended:?}"
    );
    assert_eq!(
        fs::read(fixture.spec.with_extension("result")).unwrap(),
        b"true"
    );
    assert_eq!(git(&fixture.target, &["rev-parse", "HEAD"]), expected);
    assert!(!fixture.target.join(".git/objects/info/alternates").exists());
    assert!(fixture.cache.is_dir());
    let env = fs::read_to_string(fixture.root.join("git-env")).unwrap();
    assert_eq!(env.lines().count(), 3);
    assert!(
        env.lines()
            .all(|line| line == "0:/bin/false:/bin/false:never:unset:unset:unset"),
        "{env}"
    );
    let argv = fs::read_to_string(fixture.root.join("git-argv")).unwrap();
    let lines: Vec<_> = argv.lines().collect();
    assert_eq!(lines.len(), 3);
    assert!(lines[0].starts_with("init --bare -q "));
    assert!(lines[1].starts_with(
        "-c gc.autoDetach=false -c maintenance.autoDetach=false -c gc.autoPackLimit=10 --git-dir "
    ));
    assert!(lines[2].starts_with("-c core.commitGraph=false clone --reference "));
    assert!(lines[2].contains(" --dissociate -- https://github.com/acme/bar.git "));
    assert!(
        fs::read_to_string(fixture.root.join("nice"))
            .unwrap()
            .trim()
            .parse::<i32>()
            .unwrap()
            >= 10
    );
}

/// A real Git fetch requiring a typed password must fail without waiting for
/// input or certifying a clone, even when rc files re-enable terminal prompts.
/// Only HTTP to the owned loopback endpoint is allowed; a lost rewrite refuses
/// GitHub's HTTPS locally rather than leaking a request outside this fixture.
#[farhelm_testtrace::test]
async fn spare_builder_password_required_fails_without_prompting_or_completion() {
    let server = CredentialServer::new();
    let fixture = Fixture::new("export GIT_TERMINAL_PROMPT=1 GIT_ASKPASS=bad", 30_000);
    fs::write(fixture.root.join("gitconfig"), format!(
        "[protocol]\n\tallow = never\n[protocol \"http\"]\n\tallow = always\n[url \"http://{}/repository.git\"]\n\tinsteadOf = https://github.com/acme/bar.git\n[credential]\n\thelper =\n\tusername = fixture\n",
        server.address)).unwrap();
    let mut command = fixture.command("build-spare-guard");
    command
        .env("GIT_ALLOW_PROTOCOL", "http")
        .env("NO_PROXY", "*")
        .env("no_proxy", "*")
        // The refusal assertion below names Git's diagnostic, not its
        // translation under the operator's inherited locale/gettext override.
        .env("LC_ALL", "C")
        .env("LANGUAGE", "C");
    let mut child = GroupChild::spawn_detached_tethered(&mut command.into(), CAPS).unwrap();
    let ended = child
        .finish(tokio::time::Instant::now() + Duration::from_secs(10), CAPS)
        .await
        .unwrap();
    let Ended::Exited(output) = ended else {
        panic!("credential refusal must finish before timeout: {ended:?}")
    };
    assert_eq!(output.status.signal(), Some(libc::SIGKILL));
    assert!(
        server.requests.load(std::sync::atomic::Ordering::SeqCst) > 0,
        "premise: the actual owned HTTP server demanded credentials"
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("terminal prompts disabled"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!fixture.spec.with_extension("receipt").exists());
    assert!(!fixture.spec.with_extension("result").exists());
    assert!(
        fixture.cache.is_dir(),
        "cache init ran before the refused fetch"
    );
    assert!(!fixture.target.join(".git").exists());
    let argv = fs::read_to_string(fixture.root.join("git-argv")).unwrap();
    assert_eq!(
        argv.lines().count(),
        2,
        "fetch failure cannot proceed to clone"
    );
    assert!(
        fs::read_to_string(fixture.root.join("git-env"))
            .unwrap()
            .lines()
            .all(|line| line.starts_with("0:/bin/false:/bin/false:never:"))
    );
}

/// Shell success without reaching the helper must not produce accepted clone
/// completion. A directory or rc-created .git is not evidence of a finished clone.
#[farhelm_testtrace::test]
async fn spare_builder_rc_exit_zero_has_no_helper_receipt() {
    let fixture = Fixture::new(
        "mkdir -p \"$BUILD_ROOT/checkouts/.farhelm-spares/acme/fake.git\"; exit 0",
        10_000,
    );
    let mut child =
        GroupChild::spawn_detached_tethered(&mut fixture.command("build-spare-guard").into(), CAPS)
            .unwrap();
    let ended = child
        .finish(tokio::time::Instant::now() + Duration::from_secs(10), CAPS)
        .await
        .unwrap();
    assert!(
        matches!(ended, Ended::Exited(ref output) if output.status.signal() == Some(libc::SIGKILL))
    );
    assert!(!fixture.spec.with_extension("result").exists());
    assert!(!fixture.spec.with_extension("receipt").exists());
    assert!(!fixture.root.join("git-argv").exists());
}

/// A manual invocation in the caller's existing session must refuse before
/// spawning user code or sending a signal to the caller's process group.
#[farhelm_testtrace::test]
fn spare_builder_guard_refuses_undetached_invocation() {
    let fixture = Fixture::new("touch \"$BUILD_ROOT/rc-ran\"", 10_000);
    let output = fixture
        .command("build-spare-guard")
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("own detached session"));
    assert!(!fixture.root.join("rc-ran").exists());
}

/// A helper still alive while the cache is held must refuse a target that was
/// removed or replaced before release. This proves no Git/no recreation, not
/// that this process had already reached flock; the clone-core unit pins order.
#[farhelm_testtrace::test]
async fn spare_builder_late_helper_refuses_removed_or_replaced_target() {
    for replace in [false, true] {
        let fixture = Fixture::new("", 10_000);
        let lock = fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(fixture.cache.with_extension("lock"))
            .unwrap();
        assert_eq!(unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX) }, 0);
        let mut command = fixture.command("build-spare");
        command.stdout(Stdio::null()).stderr(Stdio::null());
        let mut child = Owned(command.spawn().unwrap());
        assert!(
            child.0.try_wait().unwrap().is_none(),
            "premise: helper is still alive"
        );
        let old = fixture.root.join("old-building");
        fs::rename(&fixture.target, &old).unwrap();
        if replace {
            fs::create_dir(&fixture.target).unwrap();
            fs::write(fixture.target.join("foreign"), "keep").unwrap();
        }
        drop(lock);
        let mut status = None;
        until("late helper exits after cache release", || {
            status = child.0.try_wait().unwrap();
            status.is_some()
        })
        .await;
        let status = status.unwrap();
        assert!(!status.success());
        assert!(!fixture.root.join("git-argv").exists());
        assert_eq!(fixture.target.exists(), replace);
        if replace {
            assert_eq!(fs::read(fixture.target.join("foreign")).unwrap(), b"keep");
        }
        assert!(!fixture.spec.with_extension("receipt").exists());
    }
}

/// The independent guard deadline starts before user rc code and terminates its
/// ordinary descendants. No helper ran, so that retirement cannot publish.
#[farhelm_testtrace::test]
async fn spare_builder_guard_deadline_precedes_clone_and_kills_rc_descendants() {
    let fixture = Fixture::new(
        "sleep 300 & echo \"$$ $PPID $!\" > \"$BUILD_ROOT/pids\"; wait",
        5_000,
    );
    let mut child =
        GroupChild::spawn_detached_tethered(&mut fixture.command("build-spare-guard").into(), CAPS)
            .unwrap();
    until("rc publishes ordinary same-group descendant", || {
        fs::read_to_string(fixture.root.join("pids"))
            .is_ok_and(|text| text.split_whitespace().count() == 3)
    })
    .await;
    let pids: Vec<i32> = fs::read_to_string(fixture.root.join("pids"))
        .unwrap()
        .split_whitespace()
        .map(|p| p.parse().unwrap())
        .collect();
    assert_eq!(pids.len(), 3);
    let mut descendant = Indirect(Some(pids[2]));
    assert!(
        !terminated(pids[2]),
        "premise: rc descendant is alive before deadline"
    );
    let ended = child
        .finish(tokio::time::Instant::now() + Duration::from_secs(15), CAPS)
        .await
        .unwrap();
    assert!(
        matches!(ended, Ended::Exited(ref output) if output.status.signal() == Some(libc::SIGKILL))
    );
    until("deadline signals rc child", || terminated(pids[2])).await;
    descendant.disarm();
    assert!(!fixture.spec.with_extension("result").exists());
    assert!(!fixture.root.join("git-argv").exists());
}

/// Run as a separately killable owner so the test exercises stdin EOF after
/// SIGKILL, not GroupChild's Rust Drop. An unrelated exec must not keep the writer.
#[test]
fn spare_builder_owner_fixture_process() {
    let Some(path) = std::env::var_os("SPARE_OWNER_SPEC") else {
        return;
    };
    let root = PathBuf::from(std::env::var_os("BUILD_ROOT").unwrap());
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        let mut command = tokio::process::Command::new(BINARY);
        command.args(["internal", "build-spare-guard"]).arg(path);
        let mut child = GroupChild::spawn_detached_tethered(&mut command, CAPS).unwrap();
        // SIGKILL deliberately bypasses this owner's cleanup. If the fixture
        // returns normally instead, retain the same kill/wait ownership as its
        // caller rather than dropping an unwaited Child.
        let unrelated = Owned(
            Command::new("sleep")
                .arg("300")
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .unwrap(),
        );
        fs::write(root.join("unrelated"), unrelated.0.id().to_string()).unwrap();
        let _ = child
            .finish(tokio::time::Instant::now() + Duration::from_secs(300), CAPS)
            .await;
    });
}

/// EOF must terminate the ordinary rc group even when its owner skipped every
/// destructor, and another exec from that owner remains alive. finish is already
/// waiting, proving Child::wait cannot accidentally close the stored tether.
#[farhelm_testtrace::test]
async fn spare_builder_owner_sigkill_retires_guard_without_writer_inheritance() {
    let fixture = Fixture::new(
        "sleep 300 & echo \"$$ $PPID $!\" > \"$BUILD_ROOT/pids\"; wait",
        60_000,
    );
    let mut owner_command = Command::new(std::env::current_exe().unwrap());
    fixture.environment(&mut owner_command);
    owner_command
        .args([
            "--exact",
            "spare_builder_owner_fixture_process",
            "--nocapture",
        ])
        .env("SPARE_OWNER_SPEC", &fixture.spec)
        .env("BUILD_ROOT", &fixture.root)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .stdin(Stdio::null());
    let mut owner = Owned(owner_command.spawn().unwrap());
    until("owner and guarded rc announce exact child ids", || {
        fs::read_to_string(fixture.root.join("unrelated"))
            .is_ok_and(|text| text.parse::<i32>().is_ok())
            && fs::read_to_string(fixture.root.join("pids"))
                .is_ok_and(|text| text.split_whitespace().count() == 3)
    })
    .await;
    let pids: Vec<i32> = fs::read_to_string(fixture.root.join("pids"))
        .unwrap()
        .split_whitespace()
        .map(|p| p.parse().unwrap())
        .collect();
    let unrelated: i32 = fs::read_to_string(fixture.root.join("unrelated"))
        .unwrap()
        .parse()
        .unwrap();
    let _unrelated = Indirect(Some(unrelated));
    let mut descendant = Indirect(Some(pids[2]));
    assert!(!terminated(pids[1]) && !terminated(pids[2]) && !terminated(unrelated));
    assert!(
        owner.0.try_wait().unwrap().is_none(),
        "premise: owner alive while finish waits"
    );
    owner.0.kill().unwrap();
    owner.0.wait().unwrap();
    until("owner EOF signals guard and ordinary rc descendant", || {
        terminated(pids[1]) && terminated(pids[2])
    })
    .await;
    descendant.disarm();
    assert!(
        !terminated(unrelated),
        "unrelated exec stayed alive without keeping tether writer"
    );
    assert!(!fixture.spec.with_extension("result").exists());
}

/// The owner tether still controls a real helper after rc files finished and
/// Git reached its cache fetch. A held fixture wrapper supplies the positive
/// boundary; killing the owner must terminate both helper and ordinary Git child.
#[farhelm_testtrace::test]
async fn spare_builder_owner_sigkill_after_rc_retires_cache_fetch() {
    let fixture = Fixture::new("echo \"$PPID\" > \"$BUILD_ROOT/guard\"", 60_000);
    fixture.upstream();
    let wrapper_path = fixture.root.join("bin/git");
    let wrapper = fs::read_to_string(&wrapper_path).unwrap();
    let held = "#!/bin/sh\nif test \"$1\" = -c && test \"$2\" = gc.autoDetach=false; then\n\
        echo \"$$ $PPID\" > \"$BUILD_ROOT/fetch-pids\"\nread signal < \"$BUILD_ROOT/fetch-continue\"\nfi\n";
    script(
        &wrapper_path,
        &format!("{held}{}", wrapper.strip_prefix("#!/bin/sh\n").unwrap()),
    );
    let fifo = std::ffi::CString::new(
        fixture
            .root
            .join("fetch-continue")
            .as_os_str()
            .as_encoded_bytes(),
    )
    .unwrap();
    assert_eq!(unsafe { libc::mkfifo(fifo.as_ptr(), 0o600) }, 0);
    let mut owner_command = Command::new(std::env::current_exe().unwrap());
    fixture.environment(&mut owner_command);
    owner_command
        .args([
            "--exact",
            "spare_builder_owner_fixture_process",
            "--nocapture",
        ])
        .env("SPARE_OWNER_SPEC", &fixture.spec)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .stdin(Stdio::null());
    let mut owner = Owned(owner_command.spawn().unwrap());
    until("actual helper reaches held cache fetch after rc", || {
        fs::read_to_string(fixture.root.join("fetch-pids"))
            .is_ok_and(|text| text.split_whitespace().count() == 2)
            && fs::read_to_string(fixture.root.join("guard"))
                .is_ok_and(|text| text.trim().parse::<i32>().is_ok())
            && fs::read_to_string(fixture.root.join("unrelated"))
                .is_ok_and(|text| text.parse::<i32>().is_ok())
    })
    .await;
    let pids: Vec<i32> = fs::read_to_string(fixture.root.join("fetch-pids"))
        .unwrap()
        .split_whitespace()
        .map(|p| p.parse().unwrap())
        .collect();
    let guard = fs::read_to_string(fixture.root.join("guard"))
        .unwrap()
        .trim()
        .parse::<i32>()
        .unwrap();
    let unrelated = fs::read_to_string(fixture.root.join("unrelated"))
        .unwrap()
        .parse::<i32>()
        .unwrap();
    let _unrelated = Indirect(Some(unrelated));
    let mut fetch = Indirect(Some(pids[0]));
    let mut helper = Indirect(Some(pids[1]));
    assert!(
        fixture.cache.is_dir(),
        "premise: init completed before held fetch"
    );
    assert!(!terminated(guard) && !terminated(pids[0]) && !terminated(pids[1]));
    assert!(
        owner.0.try_wait().unwrap().is_none(),
        "premise: owner still holds tether"
    );
    owner.0.kill().unwrap();
    owner.0.wait().unwrap();
    until("owner EOF terminates post-rc helper and fetch", || {
        terminated(guard) && terminated(pids[0]) && terminated(pids[1])
    })
    .await;
    fetch.disarm();
    helper.disarm();
    assert!(!fixture.spec.with_extension("receipt").exists());
    assert!(!fixture.spec.with_extension("result").exists());
}

/// Completion must request termination even for ordinary rc descendants which
/// outlive their direct shell, both with inherited pipes and with pipes closed.
#[farhelm_testtrace::test]
async fn spare_builder_completed_clone_retires_held_and_closed_pipe_descendants() {
    for close in [false, true] {
        let redirect = if close { ">/dev/null 2>&1" } else { "" };
        let fixture = Fixture::new(
            &format!(
                "sleep 300 {redirect} & echo \"$!\" > \"$BUILD_ROOT/descendant\"; read signal < \"$BUILD_ROOT/continue\""
            ),
            60_000,
        );
        fixture.upstream();
        let fifo =
            std::ffi::CString::new(fixture.root.join("continue").as_os_str().as_encoded_bytes())
                .unwrap();
        assert_eq!(unsafe { libc::mkfifo(fifo.as_ptr(), 0o600) }, 0);
        let mut child = GroupChild::spawn_detached_tethered(
            &mut fixture.command("build-spare-guard").into(),
            CAPS,
        )
        .unwrap();
        until("rc background descendant before helper", || {
            fs::read_to_string(fixture.root.join("descendant"))
                .is_ok_and(|text| text.trim().parse::<i32>().is_ok())
        })
        .await;
        let pid = fs::read_to_string(fixture.root.join("descendant"))
            .unwrap()
            .trim()
            .parse::<i32>()
            .unwrap();
        let mut descendant = Indirect(Some(pid));
        assert!(
            !terminated(pid),
            "premise: ordinary rc child is live before clone"
        );
        assert!(
            !fixture.root.join("git-argv").exists(),
            "premise: helper has not begun"
        );
        tokio::fs::write(fixture.root.join("continue"), b"continue\n")
            .await
            .unwrap();
        let ended = child
            .finish(tokio::time::Instant::now() + Duration::from_secs(60), CAPS)
            .await
            .unwrap();
        assert!(
            matches!(ended, Ended::Exited(ref output) if output.status.signal() == Some(libc::SIGKILL)),
            "{ended:?}"
        );
        assert_eq!(
            fs::read(fixture.spec.with_extension("result")).unwrap(),
            b"true"
        );
        until("completion signals ordinary rc descendant", || {
            terminated(pid)
        })
        .await;
        descendant.disarm();
    }
}

/// Once both output streams close, finish reaches Child::wait. Its automatic
/// stdin close must not drop the separately owned tether and cause early EOF.
#[farhelm_testtrace::test]
async fn spare_builder_tether_survives_wait_after_output_closes() {
    let fixture = Fixture::new("", 10_000);
    let ready = fixture.root.join("ready");
    let eof = fixture.root.join("unexpected-eof");
    let quote = farhelm_proto::text::shell_quote;
    let mut command = tokio::process::Command::new("/bin/sh");
    command.arg("-c").arg(format!(
        "printf ready > {}; exec 1>&- 2>&-; read line; printf eof > {}",
        quote(ready.to_str().unwrap()),
        quote(eof.to_str().unwrap())
    ));
    let mut child = GroupChild::spawn_detached_tethered(&mut command, CAPS).unwrap();
    until("child about to close output then read tether", || {
        fs::read(&ready).is_ok_and(|bytes| bytes == b"ready")
    })
    .await;
    let ended = child
        .finish(
            tokio::time::Instant::now() + Duration::from_millis(500),
            CAPS,
        )
        .await
        .unwrap();
    assert!(
        matches!(ended, Ended::TimedOut),
        "wait closed the tether early: {ended:?}"
    );
    assert!(!eof.exists());
}
