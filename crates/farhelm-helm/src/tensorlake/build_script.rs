//! Shared recipe-build and environment-readiness scripts, as pure functions
//! so their text can be pinned by tests.
//!
//! Tensorlake runs these under `sh -c` as the sandbox's login user (`tl-user`,
//! with passwordless sudo) through [`super::shell::SandboxShell`]. Podman uses
//! its direct-exec route and observed login account. Final container clones
//! use only the read-only user-manager wait.
//!
//! Three rules hold for all of them:
//!
//! - Output goes to a log on the sandbox, and only a bounded tail of it comes
//!   back, on stderr, when a command fails. The capture on Farhelm's side
//!   fails a command whose output passes 64 KiB, and an apt install prints
//!   far more than that.
//! - Every failure path exits explicitly. Nothing relies on `set -e`, which a
//!   function call in an `||` list silently disables.
//! - No user text reaches a script except the recipe's package words, which
//!   `check_recipe` has already limited to Debian package syntax (never a
//!   leading `-`, which apt-get would read as an option even quoted) and
//!   which are shell-quoted here anyway.
//!
//! The agents come only from their vendors' official installers (Claude
//! Code's and Codex's install scripts), each downloaded completely over HTTPS
//! into a file before it runs, never piped into a shell: a connection that
//! drops mid-download must fail the step, not run half a script.

use crate::ssh::shell_quote;

/// How much of a step's log a failure sends back, in bytes. Enough for apt's
/// last few lines of context around its error.
const LOG_TAIL_BYTES: u32 = 8192;

/// sudo resets the caller's environment. Preserve only the supported proxy
/// names so apt and dpkg inherit exact values without putting secrets in argv.
const PACKAGE_PROXY_ENV: &str = "http_proxy,https_proxy,ftp_proxy,all_proxy,no_proxy,HTTP_PROXY,HTTPS_PROXY,FTP_PROXY,ALL_PROXY,NO_PROXY";

/// The official installers (goal D12).
pub(crate) const CLAUDE_INSTALLER: &str = "https://claude.ai/install.sh";
pub(crate) const CODEX_INSTALLER: &str = "https://chatgpt.com/codex/install.sh";

/// The opening of a logging script: the step's log file, emptied, and a
/// `fail` that sends the log's tail to stderr and exits. `step` is a fixed
/// word chosen in this file, never user text.
fn prelude(step: &str) -> String {
    format!(
        "log=/var/tmp/farhelm-build-{step}.log\n\
         : >\"$log\" || exit 1\n\
         fail() {{ tail -c {LOG_TAIL_BYTES} \"$log\" >&2; exit 1; }}\n"
    )
}

/// Refuse the first missing base requirement before any installation runs.
///
/// Every base needs systemd as PID 1, passwordless sudo, Ubuntu or Debian with
/// apt, and bash, curl and git. These are prerequisites, even when the optional
/// package list could install a missing tool. `degraded` systemd counts because
/// a usable fresh sandbox can have an unrelated failing unit.
///
/// A PID 1 other than systemd fails the build here, before anything is
/// installed: Farhelm runs sessions under a systemd user manager, and a base
/// image that stopped booting systemd (Tensorlake's `ubuntu-minimal` has its
/// own init) would otherwise produce a snapshot no host could use. It is
/// also what keeps dpkg's "Failed to connect to bus" trap away: that needs
/// `/run/systemd/system` to exist without systemd running.
///
/// The state is polled every 2 seconds for up to 180, not read once with
/// `--wait`: right after boot `systemctl` cannot reach the manager yet and
/// answers nothing at once, `--wait` included (seen on a fresh
/// `tensorlake/ubuntu-systemd` builder, 2026-11-03).
pub(crate) fn ready() -> String {
    readiness_script("/proc/1/comm", "/etc/os-release")
}

/// Wait for the login user's boot-started manager without repairing the image.
/// A container can be Running before its user bus exists. Prepared clones must
/// establish this boundary before provisioning reads that bus. Ordinary image
/// preparation also uses this waiter after establishing logind and linger.
/// Success returns HOME for the preparation backend's existing home contract.
/// Callers bound command execution as well as this nominal 30-second poll.
pub(crate) fn user_manager_ready() -> &'static str {
    r#"
uid=$(id -u) || exit 4
i=0
while [ "$i" -lt 30 ]; do
    s=$(XDG_RUNTIME_DIR=/run/user/$uid systemctl --user is-system-running 2>/dev/null)
    case "$s" in running|degraded) printf '%s' "$HOME"; exit 0;; esac
    i=$((i + 1)); sleep 1
done
echo 'the user manager did not start within 30 seconds' >&2
exit 5
"#
}

/// Keep filesystem probes explicit so tests can run the real refusal script
/// against owned files, without depending on the test host's init or OS.
/// os-release is data: read only exact ID assignments, never source commands or
/// let its other fields change PATH. Standard quoted and unquoted IDs work;
/// derivatives and malformed IDs fail the same declared OS requirement.
fn readiness_script(pid1: &str, os_release: &str) -> String {
    let script = "pid1=$(cat __PID1__ 2>/dev/null)\n\
     [ \"$pid1\" = systemd ] || { echo \"the sandbox's init process is ${pid1:-unknown}, not systemd\" >&2; exit 1; }\n\
     tries=0\n\
     while :; do\n\
     state=$(systemctl is-system-running 2>/dev/null)\n\
     case \"$state\" in running|degraded) break;; esac\n\
     tries=$((tries + 1))\n\
     [ \"$tries\" -lt 90 ] || { echo \"systemd did not finish starting (${state:-no answer})\" >&2; exit 1; }\n\
     sleep 2\n\
     done\n\
     sudo -n true 2>/dev/null || { echo \"the sandbox user cannot use sudo without a password\" >&2; exit 1; }\n\
     ID=\n\
     if [ -r __OS_RELEASE__ ]; then\n\
     while IFS= read -r line || [ -n \"$line\" ]; do\n\
     case \"$line\" in ID=*) ID=${line#ID=};; esac\n\
     done < __OS_RELEASE__\n\
     fi\n\
     case \"$ID\" in ubuntu|debian|\\\"ubuntu\\\"|\\\"debian\\\"|\\'ubuntu\\'|\\'debian\\') ;; *) echo \"the base must be Ubuntu or Debian with apt\" >&2; exit 1;; esac\n\
     command -v apt-get >/dev/null 2>&1 || { echo \"the base is missing apt (apt-get)\" >&2; exit 1; }\n\
     for tool in bash curl git; do\n\
     command -v \"$tool\" >/dev/null 2>&1 || { echo \"the base is missing $tool\" >&2; exit 1; }\n\
     done\n"
        .to_string();
    script
        .replace("__PID1__", &shell_quote(pid1))
        .replace("__OS_RELEASE__", &shell_quote(os_release))
}

/// Install the recipe's packages, after the agent installers' HTTPS tools and
/// gh, whose credential helper enables private GitHub clones. Ubuntu supplies
/// these packages (apt treats already-installed packages as
/// no-ops).
///
/// - `sudo -n --preserve-env=... env ...`: sudo retains only the fixed proxy
///   names from the caller; their values never enter a command argument. The
///   package-manager controls are fixed inputs set on sudo's far side.
/// - `DPkg::Lock::Timeout`: a systemd-booted Ubuntu runs apt's daily timers,
///   which can hold the dpkg lock just after boot.
/// - `dpkg --configure -a` first is cheap and repairs a half-configured dpkg.
/// - `NEEDRESTART_MODE=a` and the `confdef`/`confold` options keep apt from
///   asking anything.
/// - Recommended packages stay on, so a package list behaves as it would in
///   the user's own shell.
pub(crate) fn packages(packages: &[String]) -> String {
    let mut words = vec![
        "ca-certificates".to_string(),
        "curl".to_string(),
        "gh".to_string(),
    ];
    words.extend(packages.iter().map(|package| shell_quote(package)));
    format!(
        "{}apt_get() {{ sudo -n --preserve-env={PACKAGE_PROXY_ENV} env DEBIAN_FRONTEND=noninteractive NEEDRESTART_MODE=a apt-get -q -y -o DPkg::Lock::Timeout=600 -o Dpkg::Options::=--force-confdef -o Dpkg::Options::=--force-confold \"$@\" >>\"$log\" 2>&1 || fail; }}\n\
         sudo -n --preserve-env={PACKAGE_PROXY_ENV} env DEBIAN_FRONTEND=noninteractive dpkg --configure -a >>\"$log\" 2>&1 || fail\n\
         apt_get update\n\
         apt_get install {}\n",
        prelude("packages"),
        words.join(" ")
    )
}

/// Install one agent with its official installer, put it on every default
/// `PATH`, and print its `--version` line on stdout.
///
/// The installers put the command in `~/.local/bin`, which Ubuntu adds to
/// `PATH` only for login shells; sshd's commands and the systemd user
/// manager do not get it. A symlink in `/usr/local/bin`, which every default
/// `PATH` has, to the `~/.local/bin` entry keeps working when the agent's
/// updater re-points that entry. The last line runs the command by its bare
/// name in this non-login shell, which is the check that it is reachable
/// the way sessions will reach it.
fn agent(step: &str, command: &str, installer: &str, run: &str) -> String {
    format!(
        "{prelude}dir=$(mktemp -d) || fail\n\
         curl --proto '=https' --tlsv1.2 -fsSL --retry 3 -o \"$dir/install.sh\" {installer} >>\"$log\" 2>&1 || fail\n\
         {run} \"$dir/install.sh\" >>\"$log\" 2>&1 || fail\n\
         rm -rf \"$dir\"\n\
         bin=$(PATH=\"$HOME/.local/bin:$PATH\" command -v {command}) || {{ echo \"the installer did not install {command}\" >>\"$log\"; fail; }}\n\
         sudo -n ln -sfn \"$bin\" /usr/local/bin/{command} >>\"$log\" 2>&1 || fail\n\
         {command} --version 2>>\"$log\" || fail\n",
        prelude = prelude(step),
    )
}

/// Claude Code from its official installer, run with bash.
pub(crate) fn claude_code() -> String {
    agent("claude", "claude", CLAUDE_INSTALLER, "bash")
}

/// Codex from its official installer, run with `sh` and
/// `CODEX_NON_INTERACTIVE=1` so it asks nothing.
pub(crate) fn codex() -> String {
    agent(
        "codex",
        "codex",
        CODEX_INSTALLER,
        "CODEX_NON_INTERACTIVE=1 sh",
    )
}

/// Prepare the filesystem for the snapshot: drop apt's package cache (it
/// would be copied into every host), empty `/etc/machine-id`, and remove the
/// build logs.
///
/// The machine ID is emptied, not deleted: an empty file makes each sandbox
/// started from the snapshot generate its own ID at boot, where a missing
/// one can leave systemd unable to start, and a kept one would give every
/// host the same ID. Farhelm's logind and linger setup is not here; each
/// host gets it at setup, independently of the recipe's base.
pub(crate) fn prepare() -> String {
    format!(
        "{}sudo -n apt-get clean >>\"$log\" 2>&1 || fail\n\
         sudo -n truncate -s 0 /etc/machine-id >>\"$log\" 2>&1 || fail\n\
         rm -f /var/tmp/farhelm-build-*.log\n\
         sync\n",
        prelude("prepare")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Own every OS probe and command visible to the readiness child. Keeping
    /// its PATH separate avoids changing the test process environment; a missing
    /// tool also hides later tools to expose incorrect refusal ordering.
    #[cfg(unix)]
    fn run_readiness(missing: usize, os_release_text: Option<&str>) -> std::process::Output {
        use std::os::unix::fs::PermissionsExt as _;
        let dir = tempfile::tempdir().unwrap();
        let pid1 = dir.path().join("pid1");
        let os_release = dir.path().join("os-release");
        std::fs::write(
            &pid1,
            if missing == 0 {
                "other-init"
            } else {
                "systemd"
            },
        )
        .unwrap();
        if let Some(text) = os_release_text {
            std::fs::write(&os_release, text).unwrap();
        }
        let bin = dir.path().join("bin");
        std::fs::create_dir(&bin).unwrap();
        let mut commands = vec![
            ("cat", "exec /bin/cat \"$@\""),
            ("systemctl", "echo running"),
        ];
        commands.push(("sudo", if missing == 1 { "exit 1" } else { "exit 0" }));
        for (index, tool) in ["apt-get", "bash", "curl", "git"].into_iter().enumerate() {
            if missing > index + 3 {
                commands.push((tool, "exit 0"));
            }
        }
        for (name, body) in commands {
            let path = bin.join(name);
            std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
        }
        let script = readiness_script(pid1.to_str().unwrap(), os_release.to_str().unwrap());
        std::process::Command::new("/bin/sh")
            .args(["-c", &script])
            .env_clear()
            .env("PATH", &bin)
            .output()
            .unwrap()
    }

    /// Execute the production requirement checks with an isolated child PATH.
    /// Later requirements are missing too, so each refusal must name the first
    /// unmet prerequisite rather than accidentally reporting a later tool.
    #[cfg(unix)]
    #[farhelm_testtrace::test]
    fn readiness_refuses_the_first_missing_requirement() {
        for missing in 0..=8 {
            let os = if missing == 2 {
                "ID=alpine"
            } else if missing == 8 {
                "ID=ubuntu"
            } else {
                "ID=debian"
            };
            let output = run_readiness(missing, Some(os));
            let message = String::from_utf8(output.stderr).unwrap();
            if missing >= 7 {
                assert!(output.status.success(), "{message}");
            } else {
                assert!(!output.status.success());
                let expected = [
                    "not systemd",
                    "sudo without a password",
                    "Ubuntu or Debian with apt",
                    "missing apt",
                    "missing bash",
                    "missing curl",
                    "missing git",
                ][missing];
                assert!(message.contains(expected), "missing {missing}: {message}");
            }
        }
    }

    /// os-release is data even when it contains shell syntax. Quoted standard
    /// IDs work, while derivatives, absent IDs and malformed assignments name
    /// the OS requirement instead of changing PATH or executing that syntax.
    #[cfg(unix)]
    #[farhelm_testtrace::test]
    fn readiness_parses_exact_os_ids_without_executing_os_release() {
        for (text, accepted) in [
            (Some("ID=\"ubuntu\"\n"), true),
            (Some("ID='debian'"), true),
            (Some("ID=ubuntu\nPATH=/missing\nexit 23\n"), true),
            (Some("ID=linuxmint\nID_LIKE=\"ubuntu debian\"\n"), false),
            (Some("ID_LIKE=ubuntu\n"), false),
            (Some("ID=\"ubuntu\n"), false),
            (Some("ID=$(echo ubuntu)\n"), false),
            (Some("ID=ubuntu; echo ran >&2\n"), false),
            (Some("not valid shell syntax (\n"), false),
            (None, false),
        ] {
            let output = run_readiness(9, text);
            let message = String::from_utf8(output.stderr).unwrap();
            assert_eq!(output.status.success(), accepted, "{text:?}: {message}");
            assert!(
                output.stdout.is_empty(),
                "metadata must not execute shell commands"
            );
            if accepted {
                assert!(message.is_empty(), "{message}");
            } else {
                assert_eq!(
                    message.trim(),
                    "the base must be Ubuntu or Debian with apt",
                    "{text:?}"
                );
            }
        }
    }

    /// Run `script` with `sh -n`, which parses without executing.
    fn parses(script: &str) {
        let output = std::process::Command::new("sh")
            .args(["-n", "-c", script])
            .output()
            .expect("run sh -n");
        assert!(
            output.status.success(),
            "{script}\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    /// Spec: every script parses as POSIX sh; the logging scripts open their
    /// own log and define `fail` before using it.
    ///
    /// Why: the scripts run on a machine where a syntax error only shows as
    /// a failed build; parsing them here catches a quoting slip at once.
    #[farhelm_testtrace::test]
    fn every_script_parses_and_logs_to_its_own_file() {
        let packages = packages(&["ripgrep".into(), "pkg=1.2-3ubuntu1".into()]);
        for (script, log) in [
            (packages.as_str(), Some("packages")),
            (&claude_code(), Some("claude")),
            (&codex(), Some("codex")),
            (&prepare(), Some("prepare")),
            (&ready(), None),
        ] {
            parses(script);
            if let Some(step) = log {
                assert!(
                    script.starts_with(&format!(
                        "log=/var/tmp/farhelm-build-{step}.log\n: >\"$log\" || exit 1\nfail() {{"
                    )),
                    "{script}"
                );
            }
        }
    }

    /// Spec: the package script installs `ca-certificates` and `curl`, then
    /// the recipe's packages in order, each quoted as one word, after
    /// repairing dpkg and updating the package lists; apt runs through sudo
    /// with its non-interactive settings on sudo's far side.
    ///
    /// Why: package words are the one piece of user text that reaches a
    /// build script, and the order and quoting decide what apt receives.
    #[farhelm_testtrace::test]
    fn packages_are_quoted_after_the_base_packages() {
        let script = packages(&["ripgrep".into(), "g++".into(), "pkg:amd64=1.0".into()]);
        assert!(script.ends_with(
            "apt_get update\napt_get install ca-certificates curl gh 'ripgrep' 'g++' 'pkg:amd64=1.0'\n"
        ), "{script}");
        assert!(script.contains(&format!("sudo -n --preserve-env={PACKAGE_PROXY_ENV} env DEBIAN_FRONTEND=noninteractive NEEDRESTART_MODE=a apt-get -q -y -o DPkg::Lock::Timeout=600")));
        assert!(script.contains(&format!("sudo -n --preserve-env={PACKAGE_PROXY_ENV} env DEBIAN_FRONTEND=noninteractive dpkg --configure -a")));
        let words = shell_words::split(script.lines().last().unwrap()).unwrap();
        assert_eq!(
            words,
            [
                "apt_get",
                "install",
                "ca-certificates",
                "curl",
                "gh",
                "ripgrep",
                "g++",
                "pkg:amd64=1.0"
            ]
        );
        assert!(packages(&[]).ends_with("apt_get install ca-certificates curl gh\n"));
    }

    /// Execute the production package script across an actual env-resetting
    /// child boundary. The sudo stand-in preserves only explicitly named
    /// proxies, and apt/dpkg record their own environment and argv. Searching
    /// script text alone would miss a proxy silently removed by sudo policy.
    #[cfg(unix)]
    #[farhelm_testtrace::test]
    async fn package_children_inherit_exact_proxies_through_named_sudo_preservation() {
        use std::collections::BTreeMap;
        use std::os::unix::fs::{PermissionsExt as _, symlink};
        use std::process::Stdio;
        let root = tempfile::tempdir().unwrap();
        let bin = root.path().join("bin");
        std::fs::create_dir(&bin).unwrap();
        // Python supplies structured byte-safe observations and a real exec
        // environment reset. It is the existing Unix runtime-test substrate,
        // not an implementation of sudo's authentication or privilege policy.
        let stand_in = r#"#!/usr/bin/python3
import json, os, pathlib, subprocess, sys
root = pathlib.Path(__ROOT__)
names = 'http_proxy,https_proxy,ftp_proxy,all_proxy,no_proxy,HTTP_PROXY,HTTPS_PROXY,FTP_PROXY,ALL_PROXY,NO_PROXY'.split(',')
program = pathlib.Path(sys.argv[0]).name
args = sys.argv[1:]
entry = {'program': program, 'argv': args}
if program == 'sudo':
    preserve = []
    while args and args[0].startswith('-'):
        option, args = args[0], args[1:]
        if option == '-n':
            continue
        if option.startswith('--preserve-env='):
            preserve = option.split('=', 1)[1].split(',')
            assert all(name in names for name in preserve)
            assert len(preserve) == len(set(preserve))
        else:
            raise AssertionError('unsupported sudo option')
    entry['preserve'] = preserve
    with (root / 'events.jsonl').open('a') as output:
        output.write(json.dumps(entry) + '\n')
    environment = {'PATH': str(root / 'bin')}
    environment.update({name: os.environ[name] for name in preserve if name in os.environ})
    sys.exit(subprocess.run(args, env=environment, check=False).returncode)
assert program in ('apt-get', 'dpkg')
entry['proxy'] = {name: os.environ[name] for name in names if name in os.environ}
entry['unrelated'] = 'UNRELATED_PROXY_TEST' in os.environ
entry['frontend'] = os.environ.get('DEBIAN_FRONTEND')
with (root / 'events.jsonl').open('a') as output:
    output.write(json.dumps(entry) + '\n')
"#.replace("__ROOT__", &serde_json::to_string(root.path().to_str().unwrap()).unwrap());
        for name in ["sudo", "apt-get", "dpkg"] {
            let path = bin.join(name);
            std::fs::write(&path, &stand_in).unwrap();
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700)).unwrap();
        }
        symlink("/usr/bin/env", bin.join("env")).unwrap();
        symlink("/usr/bin/tail", bin.join("tail")).unwrap();
        let names = [
            "http_proxy",
            "https_proxy",
            "ftp_proxy",
            "all_proxy",
            "no_proxy",
            "HTTP_PROXY",
            "HTTPS_PROXY",
            "FTP_PROXY",
            "ALL_PROXY",
            "NO_PROXY",
        ];
        let proxy = names
            .iter()
            .filter(|name| **name != "no_proxy")
            .map(|name| {
                (
                    (*name).to_string(),
                    if *name == "HTTP_PROXY" {
                        String::new()
                    } else {
                        format!(
                            "https://u:package-private-marker a'$x\\\"@proxy.example/{name}\nline"
                        )
                    },
                )
            })
            .collect::<BTreeMap<_, _>>();
        assert!(!proxy.contains_key("no_proxy") && proxy.get("HTTP_PROXY") == Some(&String::new()));
        // Only the log coordinate differs from the production script. The
        // privileged commands and preservation option execute unchanged; PATH
        // has no route to the machine's installed sudo, apt or dpkg.
        let script = packages(&["ripgrep".into()]).replacen(
            "log=/var/tmp/farhelm-build-packages.log",
            &format!(
                "log={}",
                shell_quote(root.path().join("packages.log").to_str().unwrap())
            ),
            1,
        );
        assert!(!script.contains("package-private-marker"));
        let path = root.path().join("packages.sh");
        std::fs::write(&path, script).unwrap();
        let mut command = tokio::process::Command::new("/bin/sh");
        command
            .arg(&path)
            .env_clear()
            .env("PATH", &bin)
            .envs(&proxy)
            .env("UNRELATED_PROXY_TEST", "must not survive sudo")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        assert!(
            command
                .as_std()
                .get_args()
                .all(|arg| !arg.to_string_lossy().contains("package-private-marker"))
        );
        crate::provisioning::isolate_process_group(&mut command);
        let output = crate::provisioning::capture_child(
            command.spawn().unwrap(),
            std::time::Duration::from_secs(10),
            "the proxy package fixture",
        )
        .await
        .unwrap();
        assert_eq!(output.code, Some(0), "{}", output.stderr);
        let events = std::fs::read_to_string(root.path().join("events.jsonl"))
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(
            events.len(),
            6,
            "sudo and each of dpkg/update/install must actually run"
        );
        for event in &events {
            let argv = event["argv"].as_array().unwrap();
            assert!(
                argv.iter()
                    .all(|arg| !arg.as_str().unwrap().contains("package-private-marker")),
                "a privileged argv leaked a proxy"
            );
            if event["program"] == "sudo" {
                assert_eq!(event["preserve"], serde_json::json!(names));
            } else {
                assert_eq!(event["proxy"], serde_json::to_value(&proxy).unwrap());
                assert_eq!(event["unrelated"], false);
                assert_eq!(event["frontend"], "noninteractive");
            }
        }
        assert_eq!(events[1]["program"], "dpkg");
        assert_eq!(events[3]["program"], "apt-get");
        assert_eq!(events[5]["program"], "apt-get");
        assert!(
            events[3]["argv"]
                .as_array()
                .unwrap()
                .contains(&serde_json::json!("update"))
        );
        assert!(
            events[5]["argv"]
                .as_array()
                .unwrap()
                .contains(&serde_json::json!("install"))
        );
    }

    /// Spec: each agent script downloads its official installer over HTTPS
    /// into a file and runs that file (bash for Claude Code, `sh` with
    /// `CODEX_NON_INTERACTIVE=1` for Codex), never a pipe into a shell;
    /// links the installed command into `/usr/local/bin`; and ends by running
    /// the bare command's `--version`.
    ///
    /// Why: goal D12 requires the official installers, downloaded completely
    /// before running, and the agents reachable from non-login commands.
    #[farhelm_testtrace::test]
    fn agents_come_from_downloaded_official_installers() {
        for (script, command, url, run) in [
            (
                claude_code(),
                "claude",
                CLAUDE_INSTALLER,
                "bash \"$dir/install.sh\"",
            ),
            (
                codex(),
                "codex",
                CODEX_INSTALLER,
                "CODEX_NON_INTERACTIVE=1 sh \"$dir/install.sh\"",
            ),
        ] {
            assert!(
                script.contains(&format!(
                    "curl --proto '=https' --tlsv1.2 -fsSL --retry 3 -o \"$dir/install.sh\" {url} "
                )),
                "{script}"
            );
            assert!(url.starts_with("https://"));
            assert!(
                script.contains(&format!("{run} >>\"$log\" 2>&1 || fail\n")),
                "{script}"
            );
            assert!(
                !script.contains("| bash") && !script.contains("| sh"),
                "{script}"
            );
            assert!(script.contains(&format!(
                "sudo -n ln -sfn \"$bin\" /usr/local/bin/{command} "
            )));
            assert!(
                script.ends_with(&format!("{command} --version 2>>\"$log\" || fail\n")),
                "{script}"
            );
        }
    }

    /// Spec: the readiness check refuses a PID 1 other than systemd, polls
    /// for up to 180 seconds until systemd says it finished starting
    /// (accepting `degraded`), and requires
    /// passwordless sudo; the preparation empties `/etc/machine-id` rather
    /// than deleting it and removes the build logs.
    ///
    /// Why: a snapshot without systemd as init cannot host Farhelm sessions,
    /// and clones that share a machine ID are indistinguishable to systemd
    /// and to anything keyed on it.
    #[farhelm_testtrace::test]
    fn readiness_requires_systemd_and_preparation_empties_the_machine_id() {
        let ready = ready();
        assert!(ready.contains("[ \"$pid1\" = systemd ] ||"));
        assert!(ready.contains("state=$(systemctl is-system-running 2>/dev/null)"));
        assert!(ready.contains("case \"$state\" in running|degraded) break;; esac"));
        assert!(ready.contains("[ \"$tries\" -lt 90 ] ||"));
        assert!(ready.contains("sudo -n true"));
        let prepare = prepare();
        assert!(prepare.contains("sudo -n truncate -s 0 /etc/machine-id"));
        assert!(!prepare.contains("rm -f /etc/machine-id"));
        assert!(prepare.contains("rm -f /var/tmp/farhelm-build-*.log"));
    }
}
