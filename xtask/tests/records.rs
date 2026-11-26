//! End-to-end tests of `cargo xtask records`: the built tool runs as a child process against local bare repos.
//!
//! Each test builds its own world in a fresh temporary directory: a bare repo as the remote (no credentials, no
//! network), fake checkouts (a directory holding a `.jj` or `.git` marker, which is all slot detection looks at), a
//! `HOME`, and a config file passed with `--config`. `HOME`, `PATH` and the working directory are set only on the
//! child commands; the test process's own environment is never modified.
//!
//! Two techniques make races deterministic instead of timing-dependent:
//!
//! - A `git` wrapper script placed first on the child's `PATH` logs every git argv and environment the tool uses, and
//!   can run a hook just before the tool's first `git push`. The hook is how a test lands another slot's push, or a
//!   late edit, at exactly the moment between the tool's snapshot and its push.
//! - Backup's token minting talks to a small fake HTTP server on 127.0.0.1 that records each request and answers with
//!   canned Cloudflare envelopes.
//!
//! The `Host:` trailer comes from `tailscale status` and the child's `USER`, and the machine running the tests may well
//! have a real tailscale. So every child gets a stand-in `tailscale` first on its `PATH` (by default one that fails,
//! as a logged-out tailscale does) and a fixed `USER`, and no test depends on, or reveals, the real machine's name.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpListener};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const TOOL: &str = env!("CARGO_BIN_EXE_xtask");
const TOKEN: &str = "art_v1_testsecret0123456789?expires=1760000000";

// ---------------------------------------------------------------------------------------------------------------------
// Fixture: temporary directories, the remote, checkouts, config
// ---------------------------------------------------------------------------------------------------------------------

/// A temporary directory removed on drop (also when the test panics, so failures do not accumulate in `/tmp`).
struct TempDir(PathBuf);

impl TempDir {
    fn new(label: &str) -> Self {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let path = std::env::temp_dir().join(format!(
            "xtask-records-{label}-{}-{n}-{nanos}",
            std::process::id()
        ));
        fs::create_dir_all(&path).unwrap();
        // Canonical, so paths the tool prints (built from canonical checkouts) compare equal to ours.
        Self(fs::canonicalize(&path).unwrap())
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// One test's world: a bare remote, a `HOME`, a clones directory, fake checkouts for the named slots, and a config.
///
/// The config also lists `exec-10` to `exec-16` with checkouts that do not exist, as the real host config pre-seeds
/// unused executor slots; every test therefore also checks that absent checkouts are harmless.
struct World {
    tmp: TempDir,
    remote: PathBuf,
    home: PathBuf,
    clones: PathBuf,
    config: PathBuf,
    checkouts: BTreeMap<String, PathBuf>,
}

impl World {
    /// A world whose config lists `slots` (each with a fake checkout) and no token.
    fn new(slots: &[&str]) -> Self {
        Self::with_config_extra(slots, "")
    }

    /// Like `new`, with `extra` TOML appended to the config's top level (before the slot tables).
    fn with_config_extra(slots: &[&str], extra: &str) -> Self {
        let tmp = TempDir::new("world");
        let root = tmp.0.clone();
        let remote = root.join("remote.git");
        git(
            &root,
            &[
                "init",
                "--bare",
                "-q",
                "--initial-branch=main",
                "remote.git",
            ],
        );
        let home = root.join("home");
        fs::create_dir_all(&home).unwrap();
        let clones = root.join("clones");
        let mut checkouts = BTreeMap::new();
        for (i, slot) in slots.iter().enumerate() {
            let checkout = root.join("checkouts").join(format!("farhelm-{slot}"));
            // Alternate the markers so both jj workspaces and git checkouts are recognized.
            let marker = if i % 2 == 0 { ".jj" } else { ".git" };
            fs::create_dir_all(checkout.join(marker)).unwrap();
            checkouts.insert(slot.to_string(), checkout);
        }
        let config = root.join("config.toml");
        fs::create_dir_all(root.join("stub-bin")).unwrap();
        let world = Self {
            tmp,
            remote,
            home,
            clones,
            config,
            checkouts,
        };
        world.write_config(extra);
        world.fake_tailscale("echo 'Logged out.' >&2\nexit 1");
        world
    }

    /// (Re)writes the config: remote, clones_dir, `extra`, the slots, and the absent pre-seeded slots.
    fn write_config(&self, extra: &str) {
        let mut text = format!(
            "remote = {:?}\nclones_dir = {:?}\n{extra}\n",
            self.remote.to_str().unwrap(),
            self.clones.to_str().unwrap()
        );
        for (slot, checkout) in &self.checkouts {
            text.push_str(&format!(
                "[[slot]]\nname = {slot:?}\ncheckout = {:?}\n\n",
                checkout.to_str().unwrap()
            ));
        }
        for n in 10..=16 {
            let absent = self.tmp.0.join("absent").join(format!("exec-{n}"));
            text.push_str(&format!(
                "[[slot]]\nname = \"exec-{n}\"\ncheckout = {:?}\n\n",
                absent.to_str().unwrap()
            ));
        }
        fs::write(&self.config, text).unwrap();
    }

    fn checkout(&self, slot: &str) -> &Path {
        &self.checkouts[slot]
    }

    fn clone_of(&self, slot: &str) -> PathBuf {
        self.clones.join(slot)
    }

    /// The tool, run from `slot`'s checkout.
    fn tool(&self, slot: &str, args: &[&str]) -> Command {
        self.tool_in(self.checkout(slot), args)
    }

    /// The tool, run from `cwd`, with this world's `HOME` and config, the stand-in `tailscale` first on `PATH`, `USER`
    /// set to `agent` (and no `LOGNAME`), and no proxy settings (the backup tests talk to 127.0.0.1 and must not be
    /// routed elsewhere).
    fn tool_in(&self, cwd: &Path, args: &[&str]) -> Command {
        let mut command = Command::new(TOOL);
        command
            .arg("records")
            .arg("--config")
            .arg(&self.config)
            .args(args)
            .current_dir(cwd)
            .env("HOME", &self.home)
            .env("PATH", self.search_path(&[]))
            .env("USER", "agent")
            .env_remove("LOGNAME");
        for proxy in [
            "http_proxy",
            "https_proxy",
            "HTTP_PROXY",
            "HTTPS_PROXY",
            "ALL_PROXY",
            "all_proxy",
        ] {
            command.env_remove(proxy);
        }
        command
    }

    /// Runs the tool from `slot`'s checkout and returns its output.
    fn run(&self, slot: &str, args: &[&str]) -> Output {
        self.tool(slot, args).output().unwrap()
    }

    /// Runs `path <name>` from `slot` and returns the printed directory, asserting success.
    fn path(&self, slot: &str, name: &str) -> PathBuf {
        let output = self.run(slot, &["path", name]);
        assert_success(&output);
        PathBuf::from(stdout(&output).trim_end())
    }

    /// Runs `push` from `slot` and returns the pushed commit, asserting success.
    fn push(&self, slot: &str, names: &[&str], message: &str) -> String {
        let mut args = vec!["push"];
        args.extend_from_slice(names);
        args.extend_from_slice(&["-m", message]);
        let output = self.run(slot, &args);
        assert_success(&output);
        let out = stdout(&output);
        out.trim_end()
            .strip_prefix("pushed ")
            .unwrap_or_else(|| panic!("expected `pushed <id>`, got {out:?}"))
            .to_string()
    }

    /// Writes a file in `slot`'s clone, creating parent directories.
    fn write(&self, slot: &str, relative: &str, content: &str) {
        let path = self.clone_of(slot).join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
    }

    fn read(&self, slot: &str, relative: &str) -> Option<String> {
        fs::read_to_string(self.clone_of(slot).join(relative)).ok()
    }

    /// The remote's `main`, or `None` while it is empty.
    fn remote_main(&self) -> Option<String> {
        let output = git_output(
            &self.remote,
            &["rev-parse", "--verify", "--quiet", "refs/heads/main"],
        );
        output
            .status
            .success()
            .then(|| stdout(&output).trim().to_string())
    }

    /// A file's content at the remote's `main`, or `None` when absent.
    fn remote_file(&self, relative: &str) -> Option<String> {
        let output = git_output(&self.remote, &["show", &format!("main:{relative}")]);
        output.status.success().then(|| stdout(&output))
    }

    /// The clone's `HEAD`, or `None` while unborn.
    fn clone_head(&self, slot: &str) -> Option<String> {
        let output = git_output(
            &self.clone_of(slot),
            &["rev-parse", "--verify", "--quiet", "HEAD"],
        );
        output
            .status
            .success()
            .then(|| stdout(&output).trim().to_string())
    }

    /// `git status --porcelain` of the clone, as a sorted list of entries.
    fn clone_status(&self, slot: &str) -> Vec<String> {
        let out = git(
            &self.clone_of(slot),
            &["status", "--porcelain=v1", "--untracked-files=all"],
        );
        let mut entries: Vec<String> = out.lines().map(str::to_string).collect();
        entries.sort();
        entries
    }

    /// Replaces the remote's `main` with an unrelated commit, as a force push by someone else would.
    fn rewrite_remote(&self) {
        let empty = git_stdin(&self.remote, &["hash-object", "-t", "tree", "--stdin"], "");
        let commit = git_stdin(
            &self.remote,
            &["commit-tree", empty.trim(), "-F", "-"],
            "rewritten\n",
        );
        git(
            &self.remote,
            &["update-ref", "refs/heads/main", commit.trim()],
        );
    }

    /// Writes a `git` wrapper into this world and returns a `PATH` that puts it first. The wrapper appends each argv
    /// to `git-argv.log` and the `GIT_CONFIG_*` environment to `git-env.log`, and runs `hook` (shell text) once,
    /// just before the tool's first `git push`.
    fn git_wrapper(&self, hook: &str) -> OsString {
        let bin = self.tmp.0.join("wrapper-bin");
        fs::create_dir_all(&bin).unwrap();
        let script = format!(
            "#!/bin/sh\n\
             printf '%s\\n' \"$*\" >> '{argv}'\n\
             env | grep '^GIT_CONFIG_' >> '{env}'\n\
             case \" $* \" in\n\
             *\" push --porcelain \"*)\n\
             if [ ! -e '{marker}' ]; then\n\
             : > '{marker}'\n\
             {hook}\n\
             fi\n\
             ;;\n\
             esac\n\
             exec '{git}' \"$@\"\n",
            argv = self.argv_log().display(),
            env = self.env_log().display(),
            marker = self.tmp.0.join("hook-ran").display(),
            git = real_git().display(),
        );
        let path = bin.join("git");
        fs::write(&path, script).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
        self.search_path(&[&bin])
    }

    /// A child `PATH`: `first`, then the stand-in `tailscale`'s directory, then the test process's own `PATH` (for
    /// git and the shell tools the stand-ins use).
    fn search_path(&self, first: &[&Path]) -> OsString {
        let mut dirs: Vec<PathBuf> = first.iter().map(|d| d.to_path_buf()).collect();
        dirs.push(self.stub_bin());
        dirs.extend(std::env::split_paths(
            &std::env::var_os("PATH").unwrap_or_default(),
        ));
        std::env::join_paths(dirs).unwrap()
    }

    /// The directory holding the stand-in `tailscale`.
    fn stub_bin(&self) -> PathBuf {
        self.tmp.0.join("stub-bin")
    }

    /// Replaces the stand-in `tailscale` with a shell script running `body`, after it appends its working directory
    /// and arguments to `tailscale.log` (one line per run). The tool runs it from the slot's checkout, so the log says
    /// which slot looked its host up, and how often.
    fn fake_tailscale(&self, body: &str) {
        let script = format!(
            "#!/bin/sh\necho \"`pwd` $*\" >> '{log}'\n{body}\n",
            log = self.tailscale_log().display()
        );
        let path = self.stub_bin().join("tailscale");
        fs::write(&path, script).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    }

    /// A stand-in `tailscale` that answers `status --json` for a machine whose `Self.DNSName` is `dns_name` (given in
    /// its fully qualified form, with the trailing dot, as tailscale prints it).
    fn fake_tailscale_named(&self, dns_name: &str) {
        self.fake_tailscale(&format!(
            "cat <<'JSON'\n{{\"BackendState\":\"Running\",\"Self\":{{\"HostName\":\"host-a\",\"DNSName\":\"{dns_name}\"}}}}\nJSON"
        ));
    }

    /// Every run of the stand-in `tailscale` so far, as `<working directory> <arguments>` lines.
    fn tailscale_runs(&self) -> Vec<String> {
        fs::read_to_string(self.tailscale_log())
            .map(|t| t.lines().map(str::to_string).collect())
            .unwrap_or_default()
    }

    fn tailscale_log(&self) -> PathBuf {
        self.tmp.0.join("tailscale.log")
    }

    fn argv_log(&self) -> PathBuf {
        self.tmp.0.join("git-argv.log")
    }

    fn env_log(&self) -> PathBuf {
        self.tmp.0.join("git-env.log")
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// Process helpers
// ---------------------------------------------------------------------------------------------------------------------

/// The `git` the test itself uses (and the wrapper execs): the first one on the test process's `PATH`.
fn real_git() -> PathBuf {
    let path = std::env::var_os("PATH").unwrap_or_default();
    std::env::split_paths(&path)
        .map(|dir| dir.join("git"))
        .find(|candidate| candidate.is_file())
        .expect("git on PATH")
}

/// A test-side git command, isolated from the user's git config the same way the tool isolates its own.
fn git_command(dir: &Path, args: &[&str]) -> Command {
    let mut command = Command::new(real_git());
    command.arg("-C").arg(dir).args(args);
    for (key, _) in std::env::vars_os() {
        if key.to_string_lossy().starts_with("GIT_") {
            command.env_remove(key);
        }
    }
    command
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_AUTHOR_NAME", "test")
        .env("GIT_AUTHOR_EMAIL", "test@example.invalid")
        .env("GIT_COMMITTER_NAME", "test")
        .env("GIT_COMMITTER_EMAIL", "test@example.invalid");
    command
}

fn git_output(dir: &Path, args: &[&str]) -> Output {
    git_command(dir, args).output().unwrap()
}

/// Runs test-side git and returns stdout, panicking with stderr on failure.
fn git(dir: &Path, args: &[&str]) -> String {
    let output = git_output(dir, args);
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        stderr(&output)
    );
    stdout(&output)
}

fn git_stdin(dir: &Path, args: &[&str], input: &str) -> String {
    let mut child = git_command(dir, args)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success(), "git {args:?} failed");
    stdout(&output)
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "tool failed ({}):\nstdout: {}\nstderr: {}",
        output.status,
        stdout(output),
        stderr(output)
    );
}

/// Asserts the exit status and that stderr mentions `needle`.
fn assert_exit(output: &Output, code: i32, needle: &str) {
    assert_eq!(
        output.status.code(),
        Some(code),
        "stdout: {}\nstderr: {}",
        stdout(output),
        stderr(output)
    );
    assert!(
        stderr(output).contains(needle),
        "stderr should mention {needle:?}: {}",
        stderr(output)
    );
}

/// The commit's parents, author, committer and full message, for checking what a push wrote.
fn commit_info(repo: &Path, commit: &str) -> (Vec<String>, String, String) {
    let parents = git(repo, &["rev-list", "--parents", "-n", "1", commit]);
    let parents = parents
        .split_whitespace()
        .skip(1)
        .map(str::to_string)
        .collect();
    let people = git(
        repo,
        &["log", "-1", "--format=%an <%ae> / %cn <%ce>", commit],
    );
    let message = git(repo, &["log", "-1", "--format=%B", commit]);
    (parents, people.trim().to_string(), message)
}

// ---------------------------------------------------------------------------------------------------------------------
// path: cloning, slot detection, config and name errors
// ---------------------------------------------------------------------------------------------------------------------

/// The first `path` from a slot creates its clone from an empty remote and prints the record's directory under it,
/// creating that directory. This is every executor's first contact with a fresh records repo.
#[test]
fn first_path_clones_an_empty_remote_and_creates_the_directory() {
    let world = World::new(&["exec-1"]);
    assert!(
        world.remote_main().is_none(),
        "premise: the remote is empty"
    );
    assert!(!world.clone_of("exec-1").exists(), "premise: no clone yet");

    let printed = world.path("exec-1", "plan/alpha");

    assert_eq!(
        printed,
        world.clone_of("exec-1").join("farhelm/plans/alpha")
    );
    assert!(printed.is_dir());
    assert!(world.clone_of("exec-1").join(".git").is_dir());
    assert_eq!(world.clone_head("exec-1"), None, "nothing to check out yet");
    assert_eq!(world.remote_main(), None, "reading never writes the remote");
    // Only the calling slot's clone exists; the absent pre-seeded slots got nothing.
    let entries: Vec<_> = fs::read_dir(&world.clones)
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    assert_eq!(entries, vec![OsString::from("exec-1")]);
}

/// Every record name maps to its directory, and `executor` to the calling slot's own: the instructions name records
/// without slot names and rely on this mapping.
#[test]
fn path_prints_each_record_directory() {
    let world = World::new(&["exec-2", "landing"]);
    let exec = world.clone_of("exec-2");
    let landing = world.clone_of("landing");
    assert_eq!(
        world.path("exec-2", "executor"),
        exec.join("farhelm/executors/exec-2")
    );
    assert_eq!(
        world.path("exec-2", "plan/p-1"),
        exec.join("farhelm/plans/p-1")
    );
    assert_eq!(
        world.path("landing", "landing"),
        landing.join("farhelm/landing")
    );
    assert_eq!(
        world.path("landing", "round/3"),
        landing.join("farhelm/rounds/3")
    );
    // Reading is not restricted: the landing slot may read a plan's record.
    assert_eq!(
        world.path("landing", "plan/p-1"),
        landing.join("farhelm/plans/p-1")
    );
}

/// The slot is found from any subdirectory of its checkout, because agents run commands from wherever they are.
#[test]
fn slot_is_detected_from_a_subdirectory() {
    let world = World::new(&["exec-1", "exec-2"]);
    let deep = world.checkout("exec-2").join("crates/some/deep/dir");
    fs::create_dir_all(&deep).unwrap();
    let output = world
        .tool_in(&deep, &["path", "executor"])
        .output()
        .unwrap();
    assert_success(&output);
    assert_eq!(
        PathBuf::from(stdout(&output).trim_end()),
        world.clone_of("exec-2").join("farhelm/executors/exec-2")
    );
}

/// A checkout the config does not list, or a directory in no checkout at all, is refused: guessing a slot would let
/// one checkout write another's records.
#[test]
fn unknown_checkouts_are_refused() {
    let world = World::new(&["exec-1"]);
    let stranger = world.tmp.0.join("checkouts/unlisted");
    fs::create_dir_all(stranger.join(".git")).unwrap();
    let output = world
        .tool_in(&stranger, &["path", "executor"])
        .output()
        .unwrap();
    assert_exit(&output, 1, "no [[slot]]");

    let nowhere = world.tmp.0.join("not-a-checkout");
    fs::create_dir_all(&nowhere).unwrap();
    let output = world
        .tool_in(&nowhere, &["path", "executor"])
        .output()
        .unwrap();
    assert_exit(&output, 1, "not inside a checkout");
    assert!(!world.clones.exists(), "a refused call creates no clone");
}

/// A config error (here a slot listed twice) fails every command with exit 1 and names the problem, and a missing
/// config file is reported as such.
#[test]
fn config_errors_exit_1() {
    let world = World::new(&["exec-1"]);
    let extra = format!(
        "[[slot]]\nname = \"exec-1\"\ncheckout = \"/elsewhere\"\n\n[[slot]]\nname = \"exec-1\"\ncheckout = {:?}\n",
        world.checkout("exec-1").to_str().unwrap()
    );
    fs::write(
        &world.config,
        format!("remote = {:?}\n{extra}", world.remote.to_str().unwrap()),
    )
    .unwrap();
    assert_exit(
        &world.run("exec-1", &["path", "executor"]),
        1,
        "listed twice",
    );

    fs::remove_file(&world.config).unwrap();
    assert_exit(
        &world.run("exec-1", &["path", "executor"]),
        1,
        "cannot read the records config",
    );
}

/// Malformed record names and command lines are usage errors (exit 2), found before the config is read or any clone
/// is created.
#[test]
fn malformed_names_are_usage_errors() {
    let world = World::new(&["exec-1"]);
    for args in [
        &["path", "plan/Bad"][..],
        &["path", "round/01"],
        &["push", "plan/ok", "round/0", "-m", "x"],
        &["push", "plan/ok"],
        &["path"],
        &["bogus"],
    ] {
        let output = world.run("exec-1", args);
        assert_eq!(
            output.status.code(),
            Some(2),
            "{args:?}: {}",
            stderr(&output)
        );
    }
    assert!(!world.clones.exists());
}

/// `--help` prints usage and succeeds, so an agent can discover the commands without a config.
#[test]
fn help_prints_usage() {
    let world = World::new(&[]);
    let output = Command::new(TOOL)
        .args(["records", "--help"])
        .current_dir(&world.tmp.0)
        .output()
        .unwrap();
    assert_success(&output);
    assert!(stdout(&output).contains("push <name>... -m <message>"));
}

// ---------------------------------------------------------------------------------------------------------------------
// push: permissions, scope, author, nothing to push
// ---------------------------------------------------------------------------------------------------------------------

/// A slot may push only what it owns, and a refused push happens before anything else: not even the slot's clone
/// is created. `executor` from the landing slot is refused for reading too.
#[test]
fn pushes_outside_the_slot_are_refused_before_anything_happens() {
    let world = World::new(&["exec-1", "landing"]);
    for (slot, args) in [
        ("exec-1", &["push", "landing", "-m", "x"][..]),
        ("exec-1", &["push", "plan/ok", "round/1", "-m", "x"]),
        ("landing", &["push", "plan/x", "-m", "x"]),
        ("landing", &["push", "executor", "-m", "x"]),
    ] {
        assert_exit(&world.run(slot, args), 1, "may not push");
    }
    assert!(
        !world.clones.exists(),
        "a refused push must not create a clone"
    );
    assert_exit(
        &world.run("landing", &["path", "executor"]),
        1,
        "landing slot",
    );
    assert_eq!(world.remote_main(), None);
}

/// A push carries exactly the named records' directories: another record's new file and an edit to another pushed
/// record both stay local edits. This is what lets an executor push its log without committing a half-written note
/// in another record.
#[test]
fn push_commits_only_the_named_directories() {
    let world = World::new(&["exec-1"]);
    world.path("exec-1", "plan/a");
    world.write("exec-1", "farhelm/plans/a/log.md", "a1\n");
    world.write("exec-1", "farhelm/plans/b/log.md", "b1\n");
    world.write("exec-1", "farhelm/executors/exec-1/log.md", "e1\n");
    world.push("exec-1", &["plan/a", "plan/b"], "first");

    world.write("exec-1", "farhelm/plans/a/log.md", "a2\n");
    world.write("exec-1", "farhelm/plans/a/notes/new.md", "n\n");
    world.write("exec-1", "farhelm/plans/b/log.md", "b2 unpushed\n");
    let commit = world.push("exec-1", &["plan/a", "executor"], "second");

    assert_eq!(world.remote_main().as_deref(), Some(commit.as_str()));
    assert_eq!(
        world.remote_file("farhelm/plans/a/log.md").as_deref(),
        Some("a2\n")
    );
    assert_eq!(
        world.remote_file("farhelm/plans/a/notes/new.md").as_deref(),
        Some("n\n")
    );
    assert_eq!(
        world
            .remote_file("farhelm/executors/exec-1/log.md")
            .as_deref(),
        Some("e1\n")
    );
    assert_eq!(
        world.remote_file("farhelm/plans/b/log.md").as_deref(),
        Some("b1\n")
    );
    assert_eq!(world.clone_head("exec-1").as_deref(), Some(commit.as_str()));
    assert_eq!(
        world.clone_status("exec-1"),
        vec![" M farhelm/plans/b/log.md".to_string()],
        "only the unpushed record shows as a local edit"
    );
    assert_eq!(
        world.read("exec-1", "farhelm/plans/b/log.md").as_deref(),
        Some("b2 unpushed\n"),
        "the working tree is untouched"
    );
}

/// Deleting a file inside a named record is pushed as a deletion, and a record directory removed entirely is pushed
/// as removed: the commit mirrors the directory exactly, not just its additions.
#[test]
fn push_carries_deletions() {
    let world = World::new(&["exec-1"]);
    world.path("exec-1", "plan/a");
    world.write("exec-1", "farhelm/plans/a/log.md", "a\n");
    world.write("exec-1", "farhelm/plans/a/notes/old.md", "old\n");
    world.write("exec-1", "farhelm/plans/gone/log.md", "g\n");
    world.push("exec-1", &["plan/a", "plan/gone"], "add");

    fs::remove_file(
        world
            .clone_of("exec-1")
            .join("farhelm/plans/a/notes/old.md"),
    )
    .unwrap();
    fs::remove_dir_all(world.clone_of("exec-1").join("farhelm/plans/gone")).unwrap();
    world.push("exec-1", &["plan/a", "plan/gone"], "remove");

    assert_eq!(world.remote_file("farhelm/plans/a/notes/old.md"), None);
    assert_eq!(world.remote_file("farhelm/plans/gone/log.md"), None);
    assert_eq!(
        world.remote_file("farhelm/plans/a/log.md").as_deref(),
        Some("a\n")
    );
    assert!(world.clone_status("exec-1").is_empty());
}

/// When the named directories already match the remote, push prints `nothing to push`, exits 0 and sends nothing,
/// including on an empty remote with an empty record directory. Callers push at fixed points whether or not they wrote
/// anything, so this must be cheap and silent.
#[test]
fn nothing_to_push() {
    let world = World::new(&["exec-1"]);
    world.path("exec-1", "plan/a");
    let output = world.run("exec-1", &["push", "plan/a", "-m", "empty"]);
    assert_success(&output);
    assert_eq!(stdout(&output), "nothing to push\n");
    assert_eq!(world.remote_main(), None);

    world.write("exec-1", "farhelm/plans/a/log.md", "a\n");
    let commit = world.push("exec-1", &["plan/a"], "one");
    // An unrelated local edit does not count as something to push for plan/a.
    world.write("exec-1", "farhelm/plans/b/log.md", "b\n");
    let output = world.run("exec-1", &["push", "plan/a", "-m", "again"]);
    assert_success(&output);
    assert_eq!(stdout(&output), "nothing to push\n");
    assert_eq!(world.remote_main().as_deref(), Some(commit.as_str()));
}

/// Commits are stamped with the slot, as author, committer and `Slot:` trailer, with a placeholder email: the records
/// repo's history says which slot wrote what, and no address of whoever ran the tool. With tailscale failing (the
/// world's default stand-in), the message ends at `Slot:`.
#[test]
fn commits_carry_the_slot() {
    let world = World::new(&["exec-4"]);
    world.path("exec-4", "executor");
    world.write("exec-4", "farhelm/executors/exec-4/log.md", "x\n");
    let commit = world.push("exec-4", &["executor"], "log: started\n");
    let (parents, people, message) = commit_info(&world.remote, &commit);
    assert!(parents.is_empty(), "the first commit on an empty remote");
    assert_eq!(
        people,
        "records exec-4 <records@farhelm.invalid> / records exec-4 <records@farhelm.invalid>"
    );
    assert_eq!(message, "log: started\n\nSlot: exec-4\n\n");
    assert_eq!(
        world.tailscale_runs().len(),
        1,
        "premise: the lookup ran and failed"
    );
}

/// The full message of `commit` and the value of its `Host:` trailer as git itself parses trailers, which is how
/// status tooling reads it.
fn message_and_host_trailer(repo: &Path, commit: &str) -> (String, String) {
    let (_, _, message) = commit_info(repo, commit);
    let host = git(
        repo,
        &[
            "log",
            "-1",
            "--format=%(trailers:key=Host,valueonly)",
            commit,
        ],
    );
    (message, host.trim().to_string())
}

/// With a tailnet name and a login, a push's commit names its machine in a `Host:` trailer right after `Slot:`, as
/// `<login>@<DNS name>` with the DNS name's trailing dot dropped. This is how status tooling finds which machine a
/// slot runs on to ssh there, so git must parse it as a trailer. The lookup asks tailscale for its own status only.
#[test]
fn commits_name_the_pushing_host() {
    let world = World::new(&["exec-1"]);
    world.fake_tailscale_named("host-a.example.ts.net.");
    world.path("exec-1", "executor");
    world.write("exec-1", "farhelm/executors/exec-1/log.md", "x\n");
    let output = world.run("exec-1", &["push", "executor", "-m", "log: started"]);
    assert_success(&output);
    assert_eq!(stderr(&output), "", "a working lookup prints nothing");
    let commit = world.remote_main().unwrap();

    let (message, host) = message_and_host_trailer(&world.remote, &commit);
    assert_eq!(
        message,
        "log: started\n\nSlot: exec-1\nHost: agent@host-a.example.ts.net\n\n"
    );
    assert_eq!(host, "agent@host-a.example.ts.net");
    assert_eq!(
        world.tailscale_runs(),
        vec![format!(
            "{} status --self --json --peers=false",
            world.checkout("exec-1").display()
        )]
    );
}

/// Without a usable machine identity, a push still succeeds, its commit carries `Slot:` and no `Host:`, and stderr
/// stays empty: a host without tailscale is a normal setup, and executors push before every queue verb, so the
/// trailer must never cost a push or add noise. Covered: tailscale absent from `PATH`, failing (with its own stderr),
/// printing junk, printing no name, a login with characters status tooling could not pass to ssh safely, and no login
/// at all. `LOGNAME` stands in when `USER` is unset.
#[test]
fn commits_without_a_usable_host_carry_only_the_slot() {
    /// One way of having no usable identity (or, for the `LOGNAME` case, of still having one).
    struct Case {
        name: &'static str,
        /// The stand-in `tailscale`'s shell body.
        tailscale: &'static str,
        /// Whether `PATH` holds any `tailscale` at all.
        on_path: bool,
        user: Option<&'static str>,
        logname: Option<&'static str>,
        /// The `Host:` value expected, empty for none.
        host: &'static str,
    }
    let named = "cat <<'JSON'\n{\"Self\":{\"DNSName\":\"host-a.example.ts.net.\"}}\nJSON";
    let base = Case {
        name: "",
        tailscale: named,
        on_path: true,
        user: Some("agent"),
        logname: None,
        host: "",
    };
    let cases = [
        Case {
            name: "tailscale not installed",
            on_path: false,
            ..base
        },
        Case {
            name: "tailscale fails",
            tailscale: "echo 'Logged out.' >&2\nexit 1",
            ..base
        },
        Case {
            name: "junk output",
            tailscale: "echo 'not json'",
            ..base
        },
        Case {
            name: "empty DNS name",
            tailscale: "echo '{\"Self\":{\"DNSName\":\"\"}}'",
            ..base
        },
        Case {
            name: "option-like login",
            user: Some("-oProxyCommand=x"),
            ..base
        },
        Case {
            name: "no login",
            user: None,
            ..base
        },
        Case {
            name: "LOGNAME without USER",
            user: None,
            logname: Some("agent2"),
            host: "agent2@host-a.example.ts.net",
            ..base
        },
    ];
    for case in cases {
        let name = case.name;
        let world = World::new(&["exec-1"]);
        world.fake_tailscale(case.tailscale);
        world.path("exec-1", "executor");
        world.write("exec-1", "farhelm/executors/exec-1/log.md", "x\n");
        let mut command = world.tool("exec-1", &["push", "executor", "-m", "m"]);
        command.env_remove("USER").env_remove("LOGNAME");
        if let Some(user) = case.user {
            command.env("USER", user);
        }
        if let Some(logname) = case.logname {
            command.env("LOGNAME", logname);
        }
        if !case.on_path {
            // Only a directory holding git: no stand-in and no real tailscale either.
            let bin = world.tmp.0.join("git-only-bin");
            fs::create_dir_all(&bin).unwrap();
            std::os::unix::fs::symlink(real_git(), bin.join("git")).unwrap();
            command.env("PATH", &bin);
        }
        let output = command.output().unwrap();
        assert_success(&output);
        assert_eq!(stderr(&output), "", "{name}: no noise on stderr");
        let commit = world.remote_main().unwrap();
        let (message, host) = message_and_host_trailer(&world.remote, &commit);
        assert_eq!(host, case.host, "{name}: {message:?}");
        if case.host.is_empty() {
            assert_eq!(message, "m\n\nSlot: exec-1\n\n", "{name}");
        }
    }
}

/// A tailscale that hangs is killed after the lookup's timeout and the push goes on without `Host:`. Executors push
/// before every queue verb, so a wedged tailscale CLI or daemon must cost each push a few seconds at most, never stall
/// it. The stand-in would hang for two minutes; the bound allows slack for a loaded machine over the three-second
/// timeout while staying far below that.
#[test]
fn a_hanging_tailscale_does_not_hold_up_a_push() {
    let world = World::new(&["exec-1"]);
    world.fake_tailscale("exec sleep 120");
    world.path("exec-1", "executor");
    world.write("exec-1", "farhelm/executors/exec-1/log.md", "x\n");
    let started = Instant::now();
    let output = world.run("exec-1", &["push", "executor", "-m", "m"]);
    let elapsed = started.elapsed();
    assert_success(&output);
    assert!(elapsed < Duration::from_secs(60), "{elapsed:?}");
    assert_eq!(world.tailscale_runs().len(), 1, "premise: the stand-in ran");
    let (message, _) = message_and_host_trailer(&world.remote, &world.remote_main().unwrap());
    assert_eq!(message, "m\n\nSlot: exec-1\n\n");
}

/// The host is looked up once per push, not once per attempt: a push that loses a race to another slot and retries
/// still runs tailscale once, and the commit that lands names the machine. `path` and a refused push never run it.
/// Each slot's lookups are told apart by the working directory the stand-in logs.
#[test]
fn the_host_is_looked_up_once_per_push() {
    let world = World::new(&["exec-1", "exec-2"]);
    world.fake_tailscale_named("host-a.example.ts.net.");
    world.path("exec-1", "plan/one");
    world.path("exec-2", "plan/two");
    assert_exit(
        &world.run("exec-1", &["push", "landing", "-m", "x"]),
        1,
        "may not push",
    );
    assert!(
        world.tailscale_runs().is_empty(),
        "path and a refused push ran tailscale: {:?}",
        world.tailscale_runs()
    );
    world.write("exec-1", "farhelm/plans/one/log.md", "from one\n");
    world.write("exec-2", "farhelm/plans/two/log.md", "from two\n");

    // exec-2 pushes from inside exec-1's first `git push`, so exec-1's first attempt is rejected and it retries.
    let hook = format!(
        "cd '{}' && HOME='{}' '{TOOL}' records --config '{}' push plan/two -m 'from two' > /dev/null 2>&1",
        world.checkout("exec-2").display(),
        world.home.display(),
        world.config.display(),
    );
    let output = world
        .tool("exec-1", &["push", "plan/one", "-m", "from one"])
        .env("PATH", world.git_wrapper(&hook))
        .output()
        .unwrap();
    assert_success(&output);
    let argv = fs::read_to_string(world.argv_log()).unwrap();
    let exec_1_pushes = argv
        .lines()
        .filter(|l| {
            l.contains(" push --porcelain ")
                && l.contains(world.clone_of("exec-1").to_str().unwrap())
        })
        .count();
    assert!(exec_1_pushes >= 2, "premise: exec-1 retried: {argv}");

    let runs_from = |slot: &str| {
        let prefix = format!("{} ", world.checkout(slot).display());
        world
            .tailscale_runs()
            .iter()
            .filter(|run| run.starts_with(&prefix))
            .count()
    };
    assert_eq!(runs_from("exec-1"), 1);
    assert_eq!(runs_from("exec-2"), 1);
    let (_, host) = message_and_host_trailer(&world.remote, &world.remote_main().unwrap());
    assert_eq!(host, "agent@host-a.example.ts.net");
}

// ---------------------------------------------------------------------------------------------------------------------
// Races between slots, and edits racing a push
// ---------------------------------------------------------------------------------------------------------------------

/// Two slots pushing interleaved both land, without force: exec-2's push lands between exec-1's snapshot and exec-1's
/// push (made deterministic by a hook in exec-1's git), so exec-1's first push is rejected as non-fast-forward, and
/// exec-1 rebuilds its commit on exec-2's and pushes again.
#[test]
fn interleaved_pushes_from_two_slots_both_land_without_force() {
    let world = World::new(&["exec-1", "exec-2"]);
    world.path("exec-1", "plan/one");
    world.path("exec-2", "plan/two");
    world.write("exec-1", "farhelm/plans/one/log.md", "from one\n");
    world.write("exec-2", "farhelm/plans/two/log.md", "from two\n");

    let nested_out = world.tmp.0.join("exec-2-push.out");
    let hook = format!(
        "cd '{}' && HOME='{}' '{TOOL}' records --config '{}' push plan/two -m 'from two' > '{}' 2>&1",
        world.checkout("exec-2").display(),
        world.home.display(),
        world.config.display(),
        nested_out.display()
    );
    let path = world.git_wrapper(&hook);
    let output = world
        .tool("exec-1", &["push", "plan/one", "-m", "from one"])
        .env("PATH", path)
        .output()
        .unwrap();
    assert_success(&output);

    let two = fs::read_to_string(&nested_out).unwrap();
    let two = two
        .trim()
        .strip_prefix("pushed ")
        .unwrap_or_else(|| panic!("premise: exec-2 pushed inside the hook: {two}"))
        .to_string();
    let one = stdout(&output)
        .trim()
        .strip_prefix("pushed ")
        .unwrap()
        .to_string();
    assert_eq!(world.remote_main().as_deref(), Some(one.as_str()));
    let (parents, _, _) = commit_info(&world.remote, &one);
    assert_eq!(
        parents,
        vec![two.clone()],
        "exec-1's commit was rebuilt on exec-2's"
    );
    assert_eq!(
        world.remote_file("farhelm/plans/two/log.md").as_deref(),
        Some("from two\n")
    );
    assert_eq!(
        world.remote_file("farhelm/plans/one/log.md").as_deref(),
        Some("from one\n")
    );
    // The retry really happened (two pushes from exec-1), and no push asked for force.
    let argv = fs::read_to_string(world.argv_log()).unwrap();
    let pushes: Vec<&str> = argv
        .lines()
        .filter(|l| l.contains(" push --porcelain "))
        .collect();
    assert!(
        pushes.len() >= 3,
        "exec-1 twice and exec-2 once: {pushes:?}"
    );
    for push in &pushes {
        assert!(
            !push.contains("--force") && !push.contains(" +") && !push.contains("-f "),
            "{push}"
        );
    }
    // exec-1's clone now holds exec-2's record too, brought in by the retry's sync.
    assert_eq!(
        world.read("exec-1", "farhelm/plans/two/log.md").as_deref(),
        Some("from two\n")
    );
    assert!(world.clone_status("exec-1").is_empty());
}

/// Concurrent calls from one slot serialize on the clone's lock: two first-ever `path` calls end up sharing one clone
/// (with no half-created clone left behind), and a push waits while the test itself holds `.git/records.lock`, landing
/// only once it is released. An agent and the landing script can call the tool at the same moment from the same
/// checkout, and two pushes building in one scratch index would corrupt each other.
#[test]
fn concurrent_calls_from_one_slot_serialize() {
    let world = World::new(&["exec-1"]);
    let spawn = |args: &[&str]| {
        world
            .tool("exec-1", args)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .unwrap()
    };
    let first = [spawn(&["path", "plan/a"]), spawn(&["path", "plan/b"])];
    for child in first {
        assert_success(&child.wait_with_output().unwrap());
    }
    let entries: Vec<_> = fs::read_dir(&world.clones)
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    assert_eq!(
        entries,
        vec![OsString::from("exec-1")],
        "no leftovers from the creation race"
    );

    world.write("exec-1", "farhelm/plans/a/log.md", "a\n");
    let lock = fs::File::options()
        .write(true)
        .open(world.clone_of("exec-1").join(".git/records.lock"))
        .unwrap();
    lock.lock().unwrap();
    let mut push = spawn(&["push", "plan/a", "-m", "a"]);
    // Observation window: without the lock this push finishes in well under the window on a local remote, so the
    // push still running at its end, with the remote unmoved, is the lock at work.
    let deadline = Instant::now() + Duration::from_secs(2);
    while Instant::now() < deadline {
        assert!(
            push.try_wait().unwrap().is_none(),
            "the push finished while the slot's lock was held"
        );
        // sleep-ok: the polling interval of the bounded observation window above.
        std::thread::sleep(Duration::from_millis(50));
    }
    assert_eq!(
        world.remote_main(),
        None,
        "nothing was pushed under the held lock"
    );
    lock.unlock().unwrap();
    assert_success(&push.wait_with_output().unwrap());
    assert_eq!(
        world.remote_file("farhelm/plans/a/log.md").as_deref(),
        Some("a\n")
    );
    assert!(world.clone_status("exec-1").is_empty());
    assert_eq!(world.clone_head("exec-1"), world.remote_main());
}

/// Three slots pushing at the same moment all land, as a linear history with one commit per slot. Unlike the
/// interleaved test this relies on real timing, but its outcome does not: each round of rejections lets at least one
/// push through, so three pushers need at most three of the twelve attempts.
#[test]
fn simultaneous_pushes_from_three_slots_all_land() {
    let slots = ["exec-1", "exec-2", "exec-3"];
    let world = World::new(&slots);
    for slot in slots {
        world.path(slot, &format!("plan/{slot}"));
        world.write(slot, &format!("farhelm/plans/{slot}/log.md"), slot);
    }
    let children: Vec<_> = slots
        .iter()
        .map(|slot| {
            world
                .tool(slot, &["push", &format!("plan/{slot}"), "-m", slot])
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn()
                .unwrap()
        })
        .collect();
    for child in children {
        assert_success(&child.wait_with_output().unwrap());
    }
    let log = git(&world.remote, &["log", "--format=%P|%an", "main"]);
    let lines: Vec<&str> = log.lines().collect();
    assert_eq!(lines.len(), 3, "{log}");
    assert!(
        lines
            .iter()
            .all(|l| !l.split('|').next().unwrap().contains(' ')),
        "no merges: {log}"
    );
    for slot in slots {
        assert_eq!(
            world
                .remote_file(&format!("farhelm/plans/{slot}/log.md"))
                .as_deref(),
            Some(slot)
        );
    }
}

/// An edit that lands in a pushed directory after the snapshot (here written by a hook just before the push) is not
/// in the pushed commit and stays a local edit, so the next push carries it. Agents keep writing their logs while a
/// push runs; losing such an edit, or claiming it was pushed, would corrupt the log.
#[test]
fn edits_arriving_after_the_snapshot_stay_local() {
    let world = World::new(&["exec-1"]);
    world.path("exec-1", "plan/a");
    world.write("exec-1", "farhelm/plans/a/log.md", "first\n");
    let log = world.clone_of("exec-1").join("farhelm/plans/a/log.md");
    let hook = format!("printf 'late\\n' >> '{}'", log.display());
    let output = world
        .tool("exec-1", &["push", "plan/a", "-m", "m"])
        .env("PATH", world.git_wrapper(&hook))
        .output()
        .unwrap();
    assert_success(&output);
    assert!(
        world.tmp.0.join("hook-ran").exists(),
        "premise: the late edit happened"
    );

    assert_eq!(
        world.remote_file("farhelm/plans/a/log.md").as_deref(),
        Some("first\n")
    );
    assert_eq!(
        world.read("exec-1", "farhelm/plans/a/log.md").as_deref(),
        Some("first\nlate\n")
    );
    assert_eq!(
        world.clone_status("exec-1"),
        vec![" M farhelm/plans/a/log.md".to_string()]
    );
    world.push("exec-1", &["plan/a"], "m2");
    assert_eq!(
        world.remote_file("farhelm/plans/a/log.md").as_deref(),
        Some("first\nlate\n")
    );
}

// ---------------------------------------------------------------------------------------------------------------------
// Bringing a clone up to date
// ---------------------------------------------------------------------------------------------------------------------

/// `path` brings in what other slots pushed (additions, changes and deletions) and leaves every local edit alone,
/// including one in the record being read. This is how the landing slot reads plan logs and how a resumed executor sees
/// its plan's earlier log.
#[test]
fn path_updates_remote_changes_and_keeps_local_edits() {
    let world = World::new(&["exec-1", "exec-2"]);
    world.path("exec-2", "plan/b");
    world.write("exec-2", "farhelm/plans/b/log.md", "b1\n");
    world.write("exec-2", "farhelm/plans/b/notes/doomed.md", "d\n");
    world.push("exec-2", &["plan/b"], "b1");

    world.path("exec-1", "plan/a");
    world.write("exec-1", "farhelm/plans/a/log.md", "a pushed\n");
    world.push("exec-1", &["plan/a"], "a1");
    world.write("exec-1", "farhelm/plans/a/log.md", "a local edit\n");
    world.write("exec-1", "farhelm/plans/a/untracked.md", "u\n");

    world.write("exec-2", "farhelm/plans/b/log.md", "b2\n");
    fs::remove_file(
        world
            .clone_of("exec-2")
            .join("farhelm/plans/b/notes/doomed.md"),
    )
    .unwrap();
    world.write("exec-2", "farhelm/plans/b/notes/new.md", "n\n");
    let head = world.push("exec-2", &["plan/b"], "b2");

    world.path("exec-1", "plan/b");
    assert_eq!(world.clone_head("exec-1").as_deref(), Some(head.as_str()));
    assert_eq!(
        world.read("exec-1", "farhelm/plans/b/log.md").as_deref(),
        Some("b2\n")
    );
    assert_eq!(
        world.read("exec-1", "farhelm/plans/b/notes/doomed.md"),
        None
    );
    assert_eq!(
        world
            .read("exec-1", "farhelm/plans/b/notes/new.md")
            .as_deref(),
        Some("n\n")
    );
    assert_eq!(
        world.read("exec-1", "farhelm/plans/a/log.md").as_deref(),
        Some("a local edit\n")
    );
    assert_eq!(
        world.clone_status("exec-1"),
        vec![
            " M farhelm/plans/a/log.md".to_string(),
            "?? farhelm/plans/a/untracked.md".to_string()
        ]
    );
}

/// Where a slot's set-aside copies go; asserted empty or absent where nothing should have been set aside.
fn set_aside_dir(world: &World, slot: &str) -> PathBuf {
    world.clone_of(slot).join(".git/records-set-aside")
}

/// Sets up exec-1 with a pushed `plan/a`, then an unpushed local edit to its log (and an untracked note, which the
/// remote never touches), and has exec-2 take the plan over and push its own log plus another record, as after a
/// claim exec-1 no longer holds.
fn stale_claim_world() -> World {
    let world = World::new(&["exec-1", "exec-2"]);
    world.path("exec-1", "plan/a");
    world.write("exec-1", "farhelm/plans/a/log.md", "v1\n");
    world.push("exec-1", &["plan/a"], "v1");
    world.write("exec-1", "farhelm/plans/a/log.md", "v1 stale\n");
    world.write("exec-1", "farhelm/plans/a/notes/draft.md", "draft\n");

    world.path("exec-2", "plan/a");
    world.write("exec-2", "farhelm/plans/a/log.md", "v2 from exec-2\n");
    world.write("exec-2", "farhelm/plans/c/log.md", "c\n");
    world.push("exec-2", &["plan/a", "plan/c"], "takeover");
    world
}

/// A push of a record whose remote copy changed under a local edit refuses, names the file, and changes nothing (not
/// even unrelated remote changes): pushing would mean two writers on one record, which the tool must never merge.
#[test]
fn push_refuses_a_genuine_overlap_in_the_pushed_record() {
    let world = stale_claim_world();
    let before = world.clone_head("exec-1");
    let output = world.run("exec-1", &["push", "plan/a", "-m", "x"]);
    assert_exit(&output, 1, "farhelm/plans/a/log.md");
    // The tool's own overlap check refused, not a later git step that happened to fail on the same file.
    assert!(
        stderr(&output).contains("also have local edits"),
        "{}",
        stderr(&output)
    );
    assert_eq!(world.clone_head("exec-1"), before);
    assert_eq!(
        world.read("exec-1", "farhelm/plans/a/log.md").as_deref(),
        Some("v1 stale\n")
    );
    assert_eq!(
        world.read("exec-1", "farhelm/plans/c/log.md"),
        None,
        "nothing was updated"
    );
    assert!(
        !set_aside_dir(&world, "exec-1").exists(),
        "a refusal sets nothing aside"
    );
    assert_eq!(
        world.clone_status("exec-1"),
        vec![
            " M farhelm/plans/a/log.md".to_string(),
            "?? farhelm/plans/a/notes/draft.md".to_string()
        ]
    );
}

/// A stale local edit left by a claim this slot no longer holds does not block it: the first later call (`push
/// executor`, or `path` of any record) lets the remote win, copies the stale file to `.git/records-set-aside/`, and
/// names it on stderr; a later `path plan/<slug>` (a re-claim) then runs clean. Without this, one forgotten log edit
/// after a release would wedge every later push from the slot.
#[test]
fn stale_records_from_an_ended_claim_are_set_aside() {
    for first in [&["push", "executor", "-m", "log"][..], &["path", "plan/b"]] {
        let world = stale_claim_world();
        // Written directly, not after `path executor`: that call would already sync and set the stale file aside.
        world.write("exec-1", "farhelm/executors/exec-1/log.md", "exec-1 log\n");

        let output = world.run("exec-1", first);
        assert_success(&output);
        let err = stderr(&output);
        let line = err
            .lines()
            .find(|l| l.starts_with("records: set aside local edits to farhelm/plans/a/log.md (changed on the remote by another slot): "))
            .unwrap_or_else(|| panic!("{first:?}: no set-aside line: {err}"));
        let saved = PathBuf::from(line.rsplit(": ").next().unwrap());
        assert!(
            saved.starts_with(set_aside_dir(&world, "exec-1")),
            "{saved:?}"
        );
        assert!(saved.ends_with("farhelm/plans/a/log.md"), "{saved:?}");
        assert_eq!(fs::read_to_string(&saved).unwrap(), "v1 stale\n");
        assert_eq!(
            err.lines().count(),
            1,
            "{first:?}: exactly one file set aside: {err}"
        );

        assert_eq!(
            world.read("exec-1", "farhelm/plans/a/log.md").as_deref(),
            Some("v2 from exec-2\n"),
            "the remote won"
        );
        assert_eq!(
            world.read("exec-1", "farhelm/plans/c/log.md").as_deref(),
            Some("c\n")
        );
        assert_eq!(
            world
                .read("exec-1", "farhelm/plans/a/notes/draft.md")
                .as_deref(),
            Some("draft\n"),
            "local edits the remote did not touch stay"
        );
        if first[0] == "push" {
            assert_eq!(
                world
                    .remote_file("farhelm/executors/exec-1/log.md")
                    .as_deref(),
                Some("exec-1 log\n")
            );
        }

        let again = world.run("exec-1", &["path", "plan/a"]);
        assert_success(&again);
        assert_eq!(stderr(&again), "", "{first:?}: the re-claim runs clean");
    }
}

/// A push the server accepted while the client saw it fail (here git exits 128 after a real push, as with a dropped
/// status report, a timeout or a killed process) heals on the next call: the pushed files already match the remote,
/// so the next `push` (carrying a further edit on top) or `path` succeeds without setting anything aside. Before this
/// rule such a push wedged the slot: every later call refused the overlap with its own commit.
#[test]
fn a_push_accepted_but_reported_failed_heals() {
    for next in ["push", "path"] {
        let world = World::new(&["exec-1"]);
        let lost = accepted_but_reported_failed_push(&world, |w| {
            w.write("exec-1", "farhelm/plans/a/log.md", "v2\n")
        });
        assert_eq!(
            world.remote_file("farhelm/plans/a/log.md").as_deref(),
            Some("v2\n")
        );

        if next == "push" {
            world.write("exec-1", "farhelm/plans/a/notes/more.md", "more\n");
            let output = world.run("exec-1", &["push", "plan/a", "-m", "more"]);
            assert_success(&output);
            assert_eq!(stderr(&output), "");
            let commit = stdout(&output)
                .trim()
                .strip_prefix("pushed ")
                .unwrap()
                .to_string();
            let (parents, _, _) = commit_info(&world.remote, &commit);
            assert_eq!(
                parents,
                vec![lost.clone()],
                "built on the accepted commit, nothing reverted"
            );
            assert_eq!(
                world.remote_file("farhelm/plans/a/log.md").as_deref(),
                Some("v2\n")
            );
            assert_eq!(
                world
                    .remote_file("farhelm/plans/a/notes/more.md")
                    .as_deref(),
                Some("more\n")
            );
        } else {
            let output = world.run("exec-1", &["path", "plan/a"]);
            assert_success(&output);
            assert_eq!(stderr(&output), "");
            assert_eq!(world.clone_head("exec-1").as_deref(), Some(lost.as_str()));
        }
        assert!(
            world.clone_status("exec-1").is_empty(),
            "{next}: {:?}",
            world.clone_status("exec-1")
        );
        assert!(
            !set_aside_dir(&world, "exec-1").exists(),
            "{next}: nothing was set aside"
        );
    }
}

/// Pushes exec-1's `plan/a` (`log.md` "v1" and `notes/x.md` "x") once normally, applies `change`, then pushes again
/// through a git that really pushes but exits 128, as a push looks whose status report was lost. Returns the accepted
/// commit, after checking that the remote has it and the clone does not.
fn accepted_but_reported_failed_push(world: &World, change: impl FnOnce(&World)) -> String {
    world.path("exec-1", "plan/a");
    world.write("exec-1", "farhelm/plans/a/log.md", "v1\n");
    world.write("exec-1", "farhelm/plans/a/notes/x.md", "x\n");
    let base = world.push("exec-1", &["plan/a"], "v1");
    change(world);
    let hook = format!(
        "'{}' \"$@\" > /dev/null 2>&1; exit 128",
        real_git().display()
    );
    let output = world
        .tool("exec-1", &["push", "plan/a", "-m", "v2"])
        .env("PATH", world.git_wrapper(&hook))
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1), "{}", stderr(&output));
    let lost = world.remote_main().unwrap();
    assert_ne!(lost, base, "premise: the server took the push");
    assert_eq!(
        world.clone_head("exec-1").as_deref(),
        Some(base.as_str()),
        "premise: the client recorded nothing"
    );
    lost
}

/// After a push the remote accepted but the client saw fail, the flow writes a note to the same log and resumes. The
/// remote change carries this slot's own stamp, so the local file wins: the next `push` succeeds and carries the
/// appended text on top of the accepted commit, and `path` keeps the appended file as a local edit, with nothing set
/// aside or refused. Without this, the slot would refuse (push) or set aside its own live log (path).
#[test]
fn own_accepted_push_then_append_keeps_the_local_file() {
    for next in ["push", "path"] {
        let world = World::new(&["exec-1"]);
        let lost = accepted_but_reported_failed_push(&world, |w| {
            w.write("exec-1", "farhelm/plans/a/log.md", "v2\n")
        });
        let appended = "v2\nnote: the push failed; resuming\n";
        world.write("exec-1", "farhelm/plans/a/log.md", appended);

        let output = if next == "push" {
            world.run("exec-1", &["push", "plan/a", "-m", "resume"])
        } else {
            world.run("exec-1", &["path", "plan/a"])
        };
        assert_success(&output);
        assert_eq!(stderr(&output), "", "{next}: nothing set aside or refused");
        assert_eq!(
            world.read("exec-1", "farhelm/plans/a/log.md").as_deref(),
            Some(appended),
            "{next}: the live log is kept"
        );
        if next == "push" {
            let commit = stdout(&output)
                .trim()
                .strip_prefix("pushed ")
                .unwrap()
                .to_string();
            let (parents, _, _) = commit_info(&world.remote, &commit);
            assert_eq!(parents, vec![lost.clone()], "built on the accepted commit");
            assert_eq!(
                world.remote_file("farhelm/plans/a/log.md").as_deref(),
                Some(appended)
            );
            assert!(world.clone_status("exec-1").is_empty());
        } else {
            assert_eq!(world.clone_head("exec-1").as_deref(), Some(lost.as_str()));
            assert_eq!(
                world.clone_status("exec-1"),
                vec![" M farhelm/plans/a/log.md".to_string()],
                "the appended note is a local edit on top of the accepted commit"
            );
        }
        assert!(!set_aside_dir(&world, "exec-1").exists(), "{next}");
    }
}

/// The slot's own accepted push deleted a file, and the agent then re-created it (here before the next call). The
/// path's remote change is the slot's own, so the live file wins; the merge would see it as an untracked file in
/// its way, so the tool holds it aside while the merge runs. `path` keeps it as a new local file and `push` carries it.
/// Before the fix the merge refused forever, after having changed the index.
#[test]
fn own_accepted_deletion_then_recreated_file_is_kept() {
    for next in ["push", "path"] {
        let world = World::new(&["exec-1"]);
        let lost = accepted_but_reported_failed_push(&world, |w| {
            fs::remove_file(w.clone_of("exec-1").join("farhelm/plans/a/notes/x.md")).unwrap()
        });
        assert_eq!(
            world.remote_file("farhelm/plans/a/notes/x.md"),
            None,
            "premise: the accepted push deleted the file"
        );
        world.write("exec-1", "farhelm/plans/a/notes/x.md", "x again\n");

        let output = if next == "push" {
            world.run("exec-1", &["push", "plan/a", "-m", "again"])
        } else {
            world.run("exec-1", &["path", "plan/a"])
        };
        assert_success(&output);
        assert_eq!(stderr(&output), "", "{next}");
        assert_eq!(
            world
                .read("exec-1", "farhelm/plans/a/notes/x.md")
                .as_deref(),
            Some("x again\n"),
            "{next}: the re-created file is kept"
        );
        if next == "push" {
            assert_eq!(
                world.remote_file("farhelm/plans/a/notes/x.md").as_deref(),
                Some("x again\n")
            );
            assert!(world.clone_status("exec-1").is_empty());
        } else {
            assert_eq!(world.clone_head("exec-1").as_deref(), Some(lost.as_str()));
            assert_eq!(
                world.clone_status("exec-1"),
                vec!["?? farhelm/plans/a/notes/x.md".to_string()]
            );
        }
        let hold_left: Vec<_> = fs::read_dir(world.clone_of("exec-1").join(".git"))
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|name| name.starts_with("records-hold"))
            .collect();
        assert!(hold_left.is_empty(), "{next}: {hold_left:?}");
    }
}

/// Ownership needs every remote change to the path to be the slot's own: after exec-1's accepted-but-failed push of
/// plan/a, exec-2 takes the plan over and pushes the same log. Now the change is not exec-1's alone, so `push plan/a`
/// from exec-1 refuses (two writers) and `path` lets exec-2's version win, setting exec-1's copy aside.
#[test]
fn own_accepted_push_then_takeover_is_not_own() {
    let world = World::new(&["exec-1", "exec-2"]);
    accepted_but_reported_failed_push(&world, |w| {
        w.write("exec-1", "farhelm/plans/a/log.md", "v2\n")
    });
    world.path("exec-2", "plan/a");
    world.write("exec-2", "farhelm/plans/a/log.md", "v3 from exec-2\n");
    world.push("exec-2", &["plan/a"], "takeover");
    world.write(
        "exec-1",
        "farhelm/plans/a/log.md",
        "v2\nexec-1 kept writing\n",
    );

    let refused = world.run("exec-1", &["push", "plan/a", "-m", "x"]);
    assert_exit(&refused, 1, "also have local edits");
    assert!(!set_aside_dir(&world, "exec-1").exists());

    let output = world.run("exec-1", &["path", "plan/a"]);
    assert_success(&output);
    assert!(
        stderr(&output).contains("records: set aside local edits to farhelm/plans/a/log.md"),
        "{}",
        stderr(&output)
    );
    assert_eq!(
        world.read("exec-1", "farhelm/plans/a/log.md").as_deref(),
        Some("v3 from exec-2\n")
    );
}

/// A stale local deletion that collides with another slot's change has nothing to copy: the remote's file comes back,
/// and one stderr line says the deletion was dropped.
#[test]
fn a_stale_local_deletion_is_dropped_with_a_note() {
    let world = World::new(&["exec-1", "exec-2"]);
    world.path("exec-1", "plan/a");
    world.write("exec-1", "farhelm/plans/a/log.md", "v1\n");
    world.push("exec-1", &["plan/a"], "v1");
    fs::remove_file(world.clone_of("exec-1").join("farhelm/plans/a/log.md")).unwrap();
    world.path("exec-2", "plan/a");
    world.write("exec-2", "farhelm/plans/a/log.md", "v2 from exec-2\n");
    world.push("exec-2", &["plan/a"], "takeover");

    let output = world.run("exec-1", &["path", "plan/b"]);
    assert_success(&output);
    assert_eq!(
        stderr(&output),
        "records: dropped the local deletion of farhelm/plans/a/log.md (changed on the remote by another slot)\n"
    );
    assert_eq!(
        world.read("exec-1", "farhelm/plans/a/log.md").as_deref(),
        Some("v2 from exec-2\n")
    );
    assert!(world.clone_status("exec-1").is_empty());
}

/// File names that look like pathspec magic (`[`, `*`, `?`, a leading `:`) are only names: a set-aside of such a
/// file must not also hit a sibling the pattern would match, and the own-change check must look at that file alone.
/// Notes are free-form, so agents do write such names.
#[test]
fn pattern_like_file_names_are_literal() {
    let world = World::new(&["exec-1", "exec-2"]);
    let odd = ["[a]*.md", "?.md", ":x.md"];
    world.path("exec-1", "plan/a");
    for name in odd {
        world.write("exec-1", &format!("farhelm/plans/a/notes/{name}"), "v1\n");
    }
    // Siblings each odd name would match as a glob or magic pathspec.
    for sibling in ["ab.md", "b.md", "x.md"] {
        world.write(
            "exec-1",
            &format!("farhelm/plans/a/notes/{sibling}"),
            "keep\n",
        );
    }
    world.push("exec-1", &["plan/a"], "v1");
    for name in odd {
        world.write(
            "exec-1",
            &format!("farhelm/plans/a/notes/{name}"),
            "stale\n",
        );
    }
    world.path("exec-2", "plan/a");
    for name in odd {
        world.write("exec-2", &format!("farhelm/plans/a/notes/{name}"), "v2\n");
    }
    world.push("exec-2", &["plan/a"], "takeover");

    let output = world.run("exec-1", &["path", "plan/b"]);
    assert_success(&output);
    assert_eq!(stderr(&output).lines().count(), 3, "{}", stderr(&output));
    for name in odd {
        assert_eq!(
            world
                .read("exec-1", &format!("farhelm/plans/a/notes/{name}"))
                .as_deref(),
            Some("v2\n"),
            "{name}"
        );
    }
    for sibling in ["ab.md", "b.md", "x.md"] {
        assert_eq!(
            world
                .read("exec-1", &format!("farhelm/plans/a/notes/{sibling}"))
                .as_deref(),
            Some("keep\n"),
            "{sibling}"
        );
    }
    assert!(
        world.clone_status("exec-1").is_empty(),
        "{:?}",
        world.clone_status("exec-1")
    );
}

/// Failure messages name the remote as `<records remote>`, never by its URL or path: git quotes the remote in its
/// errors, an Artifacts URL carries the Cloudflare account id, and the messages end up in logs and reports.
#[test]
fn failure_messages_do_not_quote_the_remote() {
    let world = World::new(&["exec-1"]);
    fs::remove_dir_all(&world.remote).unwrap();
    let output = world.run("exec-1", &["path", "executor"]);
    assert_exit(&output, 1, "<records remote>");
    assert!(
        !stderr(&output).contains(world.remote.to_str().unwrap()),
        "{}",
        stderr(&output)
    );
}

/// A remote whose `main` no longer contains the clone's `HEAD` (history rewritten by a force push) is refused, and the
/// clone keeps its commits and files: the tool never resets a clone to match a rewritten remote.
#[test]
fn rewritten_remote_history_is_refused() {
    let world = World::new(&["exec-1"]);
    world.path("exec-1", "executor");
    world.write("exec-1", "farhelm/executors/exec-1/log.md", "x\n");
    let head = world.push("exec-1", &["executor"], "x");
    world.rewrite_remote();

    assert_exit(
        &world.run("exec-1", &["path", "executor"]),
        1,
        "history was rewritten",
    );
    assert_eq!(world.clone_head("exec-1").as_deref(), Some(head.as_str()));
    assert_eq!(
        world
            .read("exec-1", "farhelm/executors/exec-1/log.md")
            .as_deref(),
        Some("x\n")
    );
}

// ---------------------------------------------------------------------------------------------------------------------
// Credentials
// ---------------------------------------------------------------------------------------------------------------------

/// With a token configured, the token reaches git only through the child's environment (as the Bearer header), never
/// on a git command line, in the clone's config, or in the tool's output. Command lines are visible to every user on
/// the host and end up in error messages.
#[test]
fn token_never_reaches_argv_config_or_output() {
    let world = World::new(&["exec-1"]);
    let token_file = world.home.join("records_token");
    fs::write(&token_file, format!("{TOKEN}\n")).unwrap();
    world.write_config(&format!("token_file = {:?}", token_file.to_str().unwrap()));
    let path = world.git_wrapper(":");

    let mut outputs = Vec::new();
    for args in [&["path", "plan/a"][..], &["push", "plan/a", "-m", "m"]] {
        if args[0] == "push" {
            world.write("exec-1", "farhelm/plans/a/log.md", "x\n");
        }
        let output = world
            .tool("exec-1", args)
            .env("PATH", &path)
            .output()
            .unwrap();
        assert_success(&output);
        outputs.push(output);
    }

    let argv = fs::read_to_string(world.argv_log()).unwrap();
    assert!(
        argv.contains("fetch") && argv.contains("push"),
        "premise: the wrapper saw the git calls"
    );
    assert!(!argv.contains("testsecret"), "token on a git command line");
    let env = fs::read_to_string(world.env_log()).unwrap();
    assert!(
        env.contains(&format!("=Authorization: Bearer {TOKEN}")),
        "the whole token travels as a Bearer header value: {env}"
    );
    let config = fs::read_to_string(world.clone_of("exec-1").join(".git/config")).unwrap();
    assert!(!config.contains("testsecret"));
    for output in &outputs {
        assert!(!stdout(output).contains("testsecret") && !stderr(output).contains("testsecret"));
    }
}

/// Against an HTTP remote, git really sends `Authorization: Bearer <whole token>`: this is the end-to-end check that
/// the environment-based header reaches the wire, which a local remote cannot show. The fake server answers 404, so
/// the fetch (and the command) fails.
#[test]
fn token_is_sent_as_a_bearer_header_over_http() {
    let world = World::new(&["exec-1"]);
    let server = FakeHttp::serve(vec![(404, "not here".to_string())]);
    let token_file = world.home.join("records_token");
    fs::write(&token_file, TOKEN).unwrap();
    let remote = format!("http://{}/git/records/agent-records.git", server.addr);
    fs::write(
        &world.config,
        format!(
            "remote = {remote:?}\nclones_dir = {:?}\ntoken_file = {:?}\n[[slot]]\nname = \"exec-1\"\ncheckout = {:?}\n",
            world.clones.to_str().unwrap(),
            token_file.to_str().unwrap(),
            world.checkout("exec-1").to_str().unwrap()
        ),
    )
    .unwrap();

    let output = world.run("exec-1", &["path", "executor"]);
    assert_exit(&output, 1, "fetching the records remote");
    let requests = server.finish();
    assert_eq!(requests.len(), 1, "git's first request was answered");
    assert!(
        requests[0]
            .path
            .starts_with("/git/records/agent-records.git/info/refs"),
        "{}",
        requests[0].path
    );
    assert_eq!(
        requests[0].header("authorization").as_deref(),
        Some(format!("Bearer {TOKEN}").as_str())
    );
    assert!(!stderr(&output).contains("testsecret"));
    // The remote is an http(s) URL here, so the redaction must cover it and its bare host.
    assert!(
        !stderr(&output).contains(&server.addr.to_string()),
        "{}",
        stderr(&output)
    );
}

// ---------------------------------------------------------------------------------------------------------------------
// backup
// ---------------------------------------------------------------------------------------------------------------------

/// A world set up for backup: `[artifacts]` pointing at a fake API server, an API token file, a fake Farhelm checkout
/// whose `scripts/plans-landing/notify.sh` is a stub that records its arguments, and an ntfy topic file in `HOME`, so
/// a failure's notification is observable without sending anything.
struct BackupWorld {
    world: World,
    server: FakeHttp,
    dest: PathBuf,
    notify_log: PathBuf,
    cwd: PathBuf,
}

impl BackupWorld {
    fn new(responses: Vec<(u16, String)>) -> Self {
        let server = FakeHttp::serve(responses);
        let world = World::new(&["exec-1"]);
        let api_token = world.home.join("cloudflare-api_token");
        fs::write(&api_token, "cf-api-token\n").unwrap();
        world.write_config(&format!(
            "[artifacts]\naccount_id = \"acct0123\"\nnamespace = \"records\"\nrepo = \"agent-records\"\n\
             api_token_file = {:?}\napi_base = \"http://{}/client/v4\"\n",
            api_token.to_str().unwrap(),
            server.addr
        ));
        let cwd = world.tmp.0.join("backup-checkout");
        let scripts = cwd.join("scripts/plans-landing");
        fs::create_dir_all(cwd.join(".jj")).unwrap();
        fs::create_dir_all(&scripts).unwrap();
        let notify_log = world.tmp.0.join("notify.log");
        let stub = scripts.join("notify.sh");
        fs::write(
            &stub,
            format!(
                "#!/bin/sh\nprintf '%s|%s\\n' \"$#\" \"$*\" >> '{}'\n",
                notify_log.display()
            ),
        )
        .unwrap();
        fs::set_permissions(&stub, fs::Permissions::from_mode(0o755)).unwrap();
        fs::write(world.home.join(".farhelm-ntfy-topic"), "topic\n").unwrap();
        let dest = world.tmp.0.join("backups/agent-records.git");
        Self {
            world,
            server,
            dest,
            notify_log,
            cwd,
        }
    }

    fn backup(&self) -> Output {
        self.world
            .tool_in(
                &self.cwd,
                &["backup", "--dest", self.dest.to_str().unwrap()],
            )
            .output()
            .unwrap()
    }

    fn notifications(&self) -> Vec<String> {
        fs::read_to_string(&self.notify_log)
            .map(|t| t.lines().map(str::to_string).collect())
            .unwrap_or_default()
    }
}

/// A canned successful mint response in Cloudflare's envelope.
fn minted() -> (u16, String) {
    (
        200,
        r#"{"result":{"id":"0123456789abcdef","plaintext":"art_v1_minted?expires=1760000000","scope":"read",
        "expires_at":"2026-01-01T00:00:00Z"},"success":true,"errors":[],"messages":[]}"#
            .to_string(),
    )
}

/// Backup mints a read token with the documented request, creates the bare backup repo on first use, mirrors the
/// remote's branches, and follows fast-forwards on later runs; then a rewritten remote history is refused, the backup
/// keeps its old history, and exactly one notification goes out. The refusal is the backup's whole point: the remote
/// may not protect itself against force pushes.
#[test]
fn backup_mirrors_fast_forwards_and_refuses_rewritten_history() {
    let bw = BackupWorld::new(vec![minted(), minted(), minted()]);
    let world = &bw.world;
    world.path("exec-1", "executor");
    world.write("exec-1", "farhelm/executors/exec-1/log.md", "1\n");
    let first = world.push("exec-1", &["executor"], "1");

    assert_success(&bw.backup());
    assert_eq!(
        git(&bw.dest, &["rev-parse", "--is-bare-repository"]).trim(),
        "true"
    );
    assert_eq!(
        git(&bw.dest, &["rev-parse", "refs/heads/main"]).trim(),
        first
    );

    world.write("exec-1", "farhelm/executors/exec-1/log.md", "2\n");
    let second = world.push("exec-1", &["executor"], "2");
    assert_success(&bw.backup());
    assert_eq!(
        git(&bw.dest, &["rev-parse", "refs/heads/main"]).trim(),
        second
    );
    assert!(
        bw.notifications().is_empty(),
        "successful runs notify nobody"
    );

    world.rewrite_remote();
    let output = bw.backup();
    assert_exit(&output, 1, "non-fast-forward");
    assert_eq!(
        git(&bw.dest, &["rev-parse", "refs/heads/main"]).trim(),
        second,
        "the backup keeps the history it had"
    );
    let notifications = bw.notifications();
    assert_eq!(notifications.len(), 1, "{notifications:?}");
    assert!(
        notifications[0].starts_with("1|agent-records backup refused a rewritten history"),
        "one argument, one line: {notifications:?}"
    );
    assert!(
        !notifications[0].contains('/'),
        "no paths in a notification"
    );

    let requests = bw.server.finish();
    assert_eq!(requests.len(), 3);
    for request in &requests {
        assert_eq!(request.method, "POST");
        assert_eq!(
            request.path,
            "/client/v4/accounts/acct0123/artifacts/namespaces/records/tokens"
        );
        assert_eq!(
            request.header("authorization").as_deref(),
            Some("Bearer cf-api-token")
        );
        let body: serde_json::Value = serde_json::from_str(&request.body).unwrap();
        assert_eq!(
            body,
            serde_json::json!({"repo": "agent-records", "scope": "read", "ttl": 600})
        );
    }
}

/// A failed mint (Cloudflare's error envelope) fails the backup with the API's own error message, creates no backup
/// repo, and sends one notification; nothing is fetched without a token.
#[test]
fn backup_mint_failure_is_reported_and_notified() {
    let error = r#"{"result":null,"success":false,"errors":[{"code":10000,"message":"Authentication error"}],"messages":[]}"#;
    let bw = BackupWorld::new(vec![(403, error.to_string())]);
    let output = bw.backup();
    assert_exit(&output, 1, "10000 Authentication error");
    assert!(!bw.dest.exists());
    let notifications = bw.notifications();
    assert_eq!(notifications.len(), 1, "{notifications:?}");
    assert!(notifications[0].contains("could not mint a read token"));
    assert!(
        !stderr(&output).contains("cf-api-token"),
        "the API token is never printed"
    );
    assert_eq!(bw.server.finish().len(), 1);
}

/// Without an ntfy topic file a failed backup still exits 1 but sends nothing: hosts without notifications configured
/// must not fail differently.
#[test]
fn backup_without_a_topic_file_does_not_notify() {
    let bw = BackupWorld::new(vec![(500, "{}".to_string())]);
    fs::remove_file(bw.world.home.join(".farhelm-ntfy-topic")).unwrap();
    assert_exit(&bw.backup(), 1, "HTTP 500");
    assert!(bw.notifications().is_empty());
    bw.server.finish();
}

// ---------------------------------------------------------------------------------------------------------------------
// Fake HTTP server
// ---------------------------------------------------------------------------------------------------------------------

/// One request the fake server received.
struct Request {
    method: String,
    path: String,
    headers: Vec<(String, String)>,
    body: String,
}

impl Request {
    fn header(&self, name: &str) -> Option<String> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.clone())
    }
}

/// A one-thread HTTP/1.1 server on 127.0.0.1 that answers successive connections with the given `(status, body)`
/// responses, one connection each, and records the requests.
struct FakeHttp {
    addr: SocketAddr,
    requests: Arc<Mutex<Vec<Request>>>,
}

impl FakeHttp {
    fn serve(responses: Vec<(u16, String)>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let requests = Arc::new(Mutex::new(Vec::new()));
        let recorded = Arc::clone(&requests);
        // The thread is detached and never joined. It stops after its last response, closing the listener so a
        // surplus request is refused rather than left hanging; if a test uses fewer responses than it queued, the
        // thread stays blocked in accept until the test binary exits, which costs nothing and waits for nobody.
        std::thread::spawn(move || {
            for (status, body) in responses {
                let Ok((mut stream, _)) = listener.accept() else {
                    return;
                };
                stream
                    .set_read_timeout(Some(Duration::from_secs(10)))
                    .unwrap();
                // Record before answering: once a client has its response, its request is already in the list.
                if let Some(request) = read_request(&stream) {
                    recorded.lock().unwrap().push(request);
                }
                let _ = write!(
                    stream,
                    "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
            }
        });
        Self { addr, requests }
    }

    /// The requests received so far. Every child that talks to the server has exited by the time a test calls this,
    /// so the list is complete for those children.
    fn finish(&self) -> Vec<Request> {
        std::mem::take(&mut *self.requests.lock().unwrap())
    }
}

/// Reads one request: the request line, headers, and a `Content-Length` body.
fn read_request(stream: &std::net::TcpStream) -> Option<Request> {
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader.read_line(&mut line).ok()?;
    let mut parts = line.split_whitespace();
    let method = parts.next()?.to_string();
    let path = parts.next()?.to_string();
    let mut headers = Vec::new();
    loop {
        let mut header = String::new();
        reader.read_line(&mut header).ok()?;
        let header = header.trim_end();
        if header.is_empty() {
            break;
        }
        let (name, value) = header.split_once(':')?;
        headers.push((name.trim().to_string(), value.trim().to_string()));
    }
    let length: usize = headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("content-length"))
        .and_then(|(_, v)| v.parse().ok())
        .unwrap_or(0);
    let mut body = vec![0; length];
    reader.read_exact(&mut body).ok()?;
    Some(Request {
        method,
        path,
        headers,
        body: String::from_utf8_lossy(&body).into_owned(),
    })
}
