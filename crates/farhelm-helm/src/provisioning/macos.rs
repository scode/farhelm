//! The remote Mac's launchd job and uninstall shell contracts.
//!
//! These builders contain no transport or plan authority. Callers freeze the SSH
//! user's `user/<uid>` domain and the installation's paths before confirmation,
//! then execute the resulting script with the ordinary bounded SSH runner.

use std::path::Path;

use super::backend::{BackendFailure, path_text, shell_path};
use crate::ssh::shell_quote;

/// Render the foreground supervisor job without giving launchd its children.
///
/// `AbandonProcessGroup` is essential: terminal hosts must survive both a crash
/// and a replacement of the supervisor. The Background session type permits an
/// SSH-only user domain. `/usr/bin/env` removes session markers inherited by the
/// user manager before the supervisor starts; arguments are plist strings, never
/// shell text. Shared-directory permissions and plist installation belong to the
/// plan, not this renderer.
pub(super) fn supervisor_plist(
    farhelm: &Path,
    state_dir: &Path,
    label: &str,
) -> Result<String, BackendFailure> {
    validate_label(label)?;
    let program = absolute_path(farhelm)?;
    let state = absolute_path(state_dir)?;
    let parent = farhelm
        .parent()
        .ok_or_else(|| BackendFailure::new("the supervisor program has no parent directory", ""))?;
    let path = format!(
        "{}:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin",
        path_text(parent)?
    );
    let arguments = [
        "/usr/bin/env",
        "-u",
        "FARHELM_SESSION_ID",
        "-u",
        "FARHELM_AGENT_ID",
        "-u",
        "FARHELM_TAB_ID",
        &program,
        "supervisor",
        "run",
        "--state-dir",
        &state,
    ];
    let arguments = arguments
        .iter()
        .map(|argument| format!("        <string>{}</string>\n", xml_text(argument)))
        .collect::<String>();
    Ok(format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n\
<plist version=\"1.0\">\n<dict>\n\
    <key>Label</key><string>{}</string>\n\
    <key>ProgramArguments</key>\n    <array>\n{}    </array>\n\
    <key>EnvironmentVariables</key>\n    <dict>\n\
        <key>PATH</key><string>{}</string>\n    </dict>\n\
    <key>LimitLoadToSessionType</key><string>Background</string>\n\
    <key>RunAtLoad</key><true/>\n\
    <key>KeepAlive</key>\n    <dict>\n\
        <key>SuccessfulExit</key><false/>\n    </dict>\n\
    <key>AbandonProcessGroup</key><true/>\n\
</dict>\n</plist>\n",
        xml_text(label),
        arguments,
        xml_text(&path),
    ))
}

/// Start a known installation, reloading its registration only for Update.
///
/// A loaded job must name this plist before it can be left running or replaced.
/// Rewriting a plist does not change launchd's loaded configuration, so refresh
/// removes that registration before bootstrapping the new file. An ordinary
/// start asks an existing job to run without killing an already running process.
/// Discovery itself remains the caller's responsibility and precedes this action.
pub(super) fn start_script(
    domain: &str,
    label: &str,
    plist: &Path,
    refresh: bool,
) -> Result<String, BackendFailure> {
    absolute_path(plist)?;
    let mut script = job_inspection_script(domain, label)?;
    script.push_str(&format!("expected={}\n", shell_path(plist)?));
    script.push_str(
        r#"if [ "$active" = active ]; then
    loaded=$(readlink -f -- "$fragment") || exit 1
    planned=$(readlink -f -- "$expected") || exit 1
    if [ -z "$loaded" ] || [ "$loaded" != "$planned" ]; then
        printf '%s\n' "launchd job $job_target is loaded from $fragment, not $expected" >&2
        exit 1
    fi
"#,
    );
    if refresh {
        script.push_str(
            r#"    launchctl bootout "$job_target" || exit 1
    inspect_job || exit 1
    [ "$active" = inactive ] || { printf '%s\n' "launchd job $job_target is still loaded" >&2; exit 1; }
else
    :
fi
launchctl bootstrap "$domain" "$expected" || exit 1
"#,
        );
    } else {
        script.push_str(
            r#"    launchctl kickstart "$job_target" || exit 1
else
    launchctl bootstrap "$domain" "$expected" || exit 1
fi
"#,
        );
    }
    Ok(script)
}

/// Remove the exact job, then confirm every terminal host stopped before cleanup.
///
/// The caller has already removed the plist and checked uninstall authority.
/// Bootout alone deliberately preserves children; the installed command's exit
/// status 0 is the stable proof for hosts. Only leftover socket files are removed
/// afterwards. A retry after lib removal succeeds only if `hosts` is absent;
/// permission errors, unsupported commands and unexpected entries remain failures.
/// Stdout is `absent` only when neither the job nor the hosts entry existed.
pub(super) fn stop_script(
    domain: &str,
    label: &str,
    farhelm_program: &Path,
    state_dir: &Path,
) -> Result<String, BackendFailure> {
    absolute_path(farhelm_program)?;
    absolute_path(state_dir)?;
    let mut script = job_inspection_script(domain, label)?;
    script.push_str(&format!(
        "farhelm={}\ndata={}\nhosts=$data/{}\nchanged=no\n",
        shell_path(farhelm_program)?,
        shell_path(state_dir)?,
        farhelm_supervisor::hosts::HOSTS_DIR_NAME,
    ));
    script.push_str(
        r#"if [ "$active" = active ]; then
    launchctl bootout "$job_target" || exit 1
    inspect_job || exit 1
    [ "$active" = inactive ] || { printf '%s\n' "launchd job $job_target is still loaded" >&2; exit 1; }
    changed=yes
fi
if inspection=$(stat -f '%HT' "$hosts" 2>&1); then
    if program=$(stat -L -f '%HT' "$farhelm" 2>&1); then :; else
        case "$program" in
            "stat: $farhelm: stat: No such file or directory")
                printf '%s\n' "terminal hosts may still be running: $hosts exists, but $farhelm, which would stop them, is gone; end every process whose command line names a socket in $hosts, remove $hosts, and run uninstall again" >&2 ;;
            *) printf '%s\n' "$program" >&2 ;;
        esac
        exit 1
    fi
    if error=$("$farhelm" internal stop-terminal-hosts --state-dir "$data" 2>&1); then
        changed=yes
    else
        status=$?
        [ -z "$error" ] || printf '%s\n' "$error" >&2
        printf '%s\n' "$farhelm internal stop-terminal-hosts exited with status $status; terminal hosts in $hosts may still be running" >&2
        exit "$status"
    fi
    others=$(find "$hosts" -mindepth 1 -maxdepth 1 ! -type s -print 2>&1) || {
        printf '%s\n' "$others" "cannot list $hosts to remove what its stopped terminal hosts left" >&2
        exit 1
    }
    if [ -n "$others" ]; then
        printf '%s\n' "the terminal hosts stopped, but $hosts holds entries that are not terminal host sockets, which uninstall does not remove; remove them and run uninstall again:" "$others" >&2
        exit 1
    fi
    removal=$(find "$hosts" -mindepth 1 -maxdepth 1 -type s -exec rm -f -- {} \; 2>&1) || {
        printf '%s\n' "$removal" >&2
        exit 1
    }
    if error=$(rmdir -- "$hosts" 2>&1); then :; else
        left=$(ls -A -- "$hosts" 2>&1)
        printf '%s\n' "$error" "the terminal hosts stopped, but $hosts could not be removed; left in it:" "$left" >&2
        exit 1
    fi
else
    case "$inspection" in
        "stat: $hosts: stat: No such file or directory") ;;
        *) printf '%s\n' "$inspection" >&2; exit 1 ;;
    esac
fi
if [ "$changed" = no ]; then printf absent; fi
"#,
    );
    Ok(script)
}

/// Inspect launchd without confusing an unavailable manager with a missing job.
///
/// This fragment defines and runs `inspect_job`, leaving `active` (`active` or
/// `inactive`) and `fragment` (the absolute loaded plist path or empty) for the
/// common uninstall response encoder. Loaded-but-idle jobs are active because
/// KeepAlive can start them again. Only the measured missing-service diagnostic
/// with status 113, after a successful domain print, proves absence.
pub(super) fn job_inspection_script(domain: &str, label: &str) -> Result<String, BackendFailure> {
    let uid = validate_domain(domain)?;
    validate_label(label)?;
    let missing =
        format!("Bad request.\nCould not find service \"{label}\" in domain for uid: {uid}");
    Ok(format!(
        "export LC_ALL=C\ndomain={}\njob_target={}\nmissing_job={}\n{}inspect_job || exit 1\n",
        shell_quote(domain),
        shell_quote(&format!("{domain}/{label}")),
        shell_quote(&missing),
        JOB_INSPECTION_SH,
    ))
}

/// Read only the top-level path field; nested launchd diagnostics are not paths.
///
/// A domain proof is repeated on every call, including after bootout. Unexpected
/// print errors and successful prints without exactly one path fail closed. The
/// loaded path remains in its original spelling for the shared uninstall path
/// resolver, which owns home-alias normalization and same-file checks.
const JOB_INSPECTION_SH: &str = r#"inspect_job() {
    if domain_output=$(launchctl print "$domain" 2>&1); then :; else
        printf '%s\n' "$domain_output" >&2
        return 1
    fi
    active=inactive
    fragment=''
    if job_output=$(launchctl print "$job_target" 2>&1); then
        active=active
        paths=0
        tab=$(printf '\t')
        while IFS= read -r line; do
            case "$line" in
                "$tab"'path = '*)
                    paths=$((paths + 1))
                    fragment=${line#"$tab"'path = '}
                    ;;
            esac
        done <<EOF_JOB
$job_output
EOF_JOB
        case "$fragment" in
            /*) ;;
            *) printf '%s\n' "cannot read the loaded plist path of launchd job $job_target" >&2; return 1 ;;
        esac
        if [ "$paths" != 1 ]; then
            printf '%s\n' "launchd job $job_target has an ambiguous loaded plist path" >&2
            return 1
        fi
    else
        status=$?
        if [ "$status" != 113 ] || [ "$job_output" != "$missing_job" ]; then
            printf '%s\n' "$job_output" "cannot inspect launchd job $job_target (status $status)" >&2
            return 1
        fi
    fi
}
"#;

/// Enumerate mounts on or below a path, preserving spaces in mount-point names.
///
/// macOS `mount` has no machine-readable `-p` mode. Its C-locale display uses
/// `source on path (options)`, so ambiguous delimiters or malformed lines refuse
/// the whole inspection rather than silently missing a nested mount. The caller
/// supplies the same trusted printf separator (`\0` or `\n`) as the Linux helper
/// and must honor a nonzero return, including after partial output.
pub(super) const MAC_MOUNT_POINTS_SH: &str = r#"mount_points() {
    if mount_table=$(LC_ALL=C mount 2>&1); then :; else
        printf '%s\n' "$mount_table" >&2
        return 1
    fi
    if [ -z "$mount_table" ]; then
        printf '%s\n' 'cannot inspect an empty mount table' >&2
        return 1
    fi
    while IFS= read -r mount_line; do
        case "$mount_line" in
            *' on '*' ('*')') ;;
            *) printf '%s\n' "cannot parse mount table line: $mount_line" >&2; return 1 ;;
        esac
        mount_rest=${mount_line#*' on '}
        mount_source=${mount_line%%' on '*}
        mount_point=${mount_rest%%' ('*}
        mount_options=${mount_rest#*' ('}
        case "$mount_rest" in
            *' on '*) printf '%s\n' "ambiguous mount table line: $mount_line" >&2; return 1 ;;
        esac
        case "$mount_source$mount_options" in
            *' ('*) printf '%s\n' "ambiguous mount table line: $mount_line" >&2; return 1 ;;
        esac
        case "$mount_point" in
            /*) ;;
            *) printf '%s\n' "non-absolute mount point: $mount_line" >&2; return 1 ;;
        esac
        case "$mount_point" in
            "$1"|"$1"/*) printf "%s$2" "$mount_point" ;;
        esac
    done <<EOF_MOUNTS
$mount_table
EOF_MOUNTS
}
"#;

/// Keep service targets within one explicit per-user domain, never a GUI alias.
fn validate_domain(domain: &str) -> Result<&str, BackendFailure> {
    let uid = domain.strip_prefix("user/").unwrap_or_default();
    if uid.is_empty()
        || !uid.bytes().all(|byte| byte.is_ascii_digit())
        || !uid
            .parse::<u32>()
            .is_ok_and(|value| value.to_string() == uid)
    {
        return Err(BackendFailure::new("invalid launchd user domain", ""));
    }
    Ok(uid)
}

/// Labels cross launchctl's service-target grammar as well as the shell boundary.
fn validate_label(label: &str) -> Result<(), BackendFailure> {
    if label.is_empty()
        || !label
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte))
    {
        return Err(BackendFailure::new("invalid launchd job label", ""));
    }
    Ok(())
}

/// The foreground command and cleanup CLI must never reinterpret relative paths.
fn absolute_path(path: &Path) -> Result<String, BackendFailure> {
    if !path.is_absolute() {
        return Err(BackendFailure::new(
            "launchd provisioning requires an absolute path",
            "",
        ));
    }
    path_text(path)
}

/// Escape plist text without applying shell or systemd argument quoting to argv.
fn xml_text(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::{fs, os::unix::fs::PermissionsExt, process::Output};

    /// Execute generated POSIX shell against owned tools, never the host manager.
    ///
    /// Child-only environment injection keeps tests independent of the runner's
    /// launchd, PATH and session markers. The command log distinguishes a refusal
    /// before mutation from an error raised after a destructive command ran.
    struct ShellFixture {
        directory: tempfile::TempDir,
    }

    impl ShellFixture {
        /// Allocate one private command namespace and log for this test case.
        fn new() -> Self {
            Self {
                directory: tempfile::tempdir().unwrap(),
            }
        }

        /// Install a stub whose argv is retained even when it fails deliberately.
        fn tool(&self, name: &str, body: &str) {
            let path = self.directory.path().join(name);
            fs::write(
                &path,
                format!("#!/bin/sh\nprintf '%s\\n' \"{name} $*\" >> \"$COMMAND_LOG\"\n{body}\n"),
            )
            .unwrap();
            fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
        }

        /// Make the domain and job answers independently controllable.
        fn launchctl(&self, domain_status: u8, job_status: u8, job_output: &str) {
            self.tool(
                "launchctl",
                &format!(
                    "if [ \"$2\" = user/501 ]; then printf '%s\\n' domain; exit {domain_status}; fi\n\
                     printf '%s\\n' {}\nexit {job_status}",
                    shell_quote(job_output)
                ),
            );
        }

        /// Only the spawned shell receives the fixture environment.
        fn run(&self, script: &str) -> Output {
            std::process::Command::new("/bin/sh")
                .args(["-c", script])
                .env_clear()
                .env(
                    "PATH",
                    format!("{}:/usr/bin:/bin", self.directory.path().display()),
                )
                .env("COMMAND_LOG", self.directory.path().join("commands"))
                .output()
                .unwrap()
        }

        /// Retain the exact reached commands for assertions about mutation order.
        fn commands(&self) -> String {
            fs::read_to_string(self.directory.path().join("commands")).unwrap()
        }
    }

    /// The job must preserve children and pass paths as individual XML arguments.
    #[test]
    fn plist_preserves_children_and_escapes_arguments() {
        let plist = supervisor_plist(
            Path::new("/owned/a & b/farhelm"),
            Path::new("/owned/state <one>"),
            "io.farhelm.test",
        )
        .unwrap();
        assert!(plist.contains("<key>AbandonProcessGroup</key><true/>"));
        assert!(plist.contains("<key>SuccessfulExit</key><false/>"));
        assert!(plist.contains("<string>/owned/a &amp; b/farhelm</string>"));
        assert!(plist.contains("<string>/owned/state &lt;one&gt;</string>"));
        assert!(plist.contains("<string>FARHELM_SESSION_ID</string>"));
        assert!(!plist.contains("FARHELM_TMUX"));
    }

    /// Invalid service targets must fail before a host command can be formed.
    #[test]
    fn builders_refuse_relative_paths_and_non_user_targets() {
        assert!(job_inspection_script("gui/501", "io.farhelm.test").is_err());
        assert!(job_inspection_script("user/0501", "io.farhelm.test").is_err());
        assert!(job_inspection_script("user/501", "foreign/job").is_err());
        assert!(
            start_script("user/501", "io.farhelm.test", Path::new("job.plist"), false).is_err()
        );
        assert!(supervisor_plist(Path::new("relative"), Path::new("/state"), "test").is_err());
    }

    /// Refresh must replace registration after checking the old loaded path.
    #[test]
    fn refresh_reloads_registration_without_killing_a_process_group() {
        let script = start_script(
            "user/501",
            "io.farhelm.test",
            Path::new("/owned/job's file.plist"),
            true,
        )
        .unwrap();
        let compare = script.find("[ \"$loaded\" != \"$planned\" ]").unwrap();
        let bootout = script.find("launchctl bootout").unwrap();
        let bootstrap = script.find("launchctl bootstrap").unwrap();
        assert!(compare < bootout && bootout < bootstrap);
        assert!(!script.contains("kill "));
        assert!(!script.contains("launchctl disable"));
    }

    /// A failed print is absence only for the measured status and diagnostic.
    #[test]
    fn inspection_distinguishes_absence_from_manager_and_protocol_failures() {
        let missing =
            "Bad request.\nCould not find service \"io.farhelm.test\" in domain for uid: 501";
        for (domain_status, job_status, output, succeeds) in [
            (0, 113, missing, true),
            (0, 1, missing, false),
            (0, 113, "permission denied", false),
            (1, 113, missing, false),
            (0, 0, "job = {\n\tstate = waiting\n}", false),
        ] {
            let fixture = ShellFixture::new();
            fixture.launchctl(domain_status, job_status, output);
            let script = job_inspection_script("user/501", "io.farhelm.test").unwrap()
                + "printf '%s|%s' \"$active\" \"$fragment\"\n";
            let result = fixture.run(&script);
            assert_eq!(
                result.status.success(),
                succeeds,
                "domain={domain_status}, job={job_status}, stderr={}",
                String::from_utf8_lossy(&result.stderr),
            );
            if succeeds {
                assert_eq!(result.stdout, b"inactive|");
            }
            if domain_status != 0 {
                assert_eq!(fixture.commands(), "launchctl print user/501\n");
            }
        }
    }

    /// An idle loaded job can restart; its path must still constrain uninstall.
    #[test]
    fn inspection_keeps_idle_jobs_active_and_preserves_path_spaces() {
        let fixture = ShellFixture::new();
        fixture.launchctl(
            0,
            0,
            "job = {\n\tpath = /owned/Launch Agents/job.plist\n\tstate = waiting\n}",
        );
        let script = job_inspection_script("user/501", "io.farhelm.test").unwrap()
            + "printf '%s|%s' \"$active\" \"$fragment\"\n";
        let result = fixture.run(&script);
        assert!(result.status.success());
        assert_eq!(result.stdout, b"active|/owned/Launch Agents/job.plist");
    }

    /// Refresh must not boot out a same-label job loaded from another plist.
    #[test]
    fn refresh_refuses_a_foreign_loaded_plist_before_mutation() {
        let fixture = ShellFixture::new();
        fixture.launchctl(0, 0, "job = {\n\tpath = /foreign/job.plist\n}");
        fixture.tool("readlink", "printf '%s\\n' \"$3\"");
        let result = fixture.run(
            &start_script(
                "user/501",
                "io.farhelm.test",
                Path::new("/owned/job.plist"),
                true,
            )
            .unwrap(),
        );
        assert!(!result.status.success());
        assert!(String::from_utf8_lossy(&result.stderr).contains("loaded from"));
        let commands = fixture.commands();
        assert!(!commands.contains("launchctl bootout"));
        assert!(!commands.contains("launchctl bootstrap"));
    }

    /// Replacing a registration must observe its removal before bootstrapping.
    #[test]
    fn refresh_boots_out_then_bootstraps_only_after_verified_absence() {
        let fixture = ShellFixture::new();
        let loaded = fixture.directory.path().join("loaded");
        fs::write(&loaded, "").unwrap();
        fixture.tool(
            "launchctl",
            &format!(
                r#"loaded={}
case "$1 $2" in
    'print user/501') exit 0 ;;
    'print user/501/io.farhelm.test')
        if [ -f "$loaded" ]; then
            printf 'job = {{\n\tpath = /owned/job.plist\n}}\n'
        else
            printf '%s\n' 'Bad request.' 'Could not find service "io.farhelm.test" in domain for uid: 501' >&2
            exit 113
        fi ;;
    'bootout user/501/io.farhelm.test') rm "$loaded" ;;
    'bootstrap user/501')
        [ ! -e "$loaded" ] || exit 90
        [ "$3" = /owned/job.plist ] || exit 91
        : > "$loaded" ;;
    *) exit 92 ;;
esac"#,
                shell_path(&loaded).unwrap()
            ),
        );
        fixture.tool("readlink", "printf '%s\\n' \"$3\"");
        let result = fixture.run(
            &start_script(
                "user/501",
                "io.farhelm.test",
                Path::new("/owned/job.plist"),
                true,
            )
            .unwrap(),
        );
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(loaded.exists(), "the replacement job was never registered");
        let commands = fixture.commands();
        assert!(commands.ends_with(
            "launchctl bootout user/501/io.farhelm.test\nlaunchctl print user/501\nlaunchctl print user/501/io.farhelm.test\nlaunchctl bootstrap user/501 /owned/job.plist\n"
        ));
    }

    /// Command status 2 must retain hosts even if an older binary knows no verb.
    #[test]
    fn stop_refuses_an_unsupported_host_stop_before_socket_cleanup() {
        let fixture = ShellFixture::new();
        fixture.launchctl(
            0,
            113,
            "Bad request.\nCould not find service \"io.farhelm.test\" in domain for uid: 501",
        );
        fixture.tool("stat", "printf '%s\\n' Directory");
        fixture.tool("farhelm", "printf '%s\\n' 'unknown subcommand' >&2; exit 2");
        fixture.tool("find", "exit 99");
        let result = fixture.run(
            &stop_script(
                "user/501",
                "io.farhelm.test",
                &fixture.directory.path().join("farhelm"),
                Path::new("/owned/state"),
            )
            .unwrap(),
        );
        assert_eq!(result.status.code(), Some(2));
        assert!(!fixture.commands().contains("find "));
        assert!(String::from_utf8_lossy(&result.stderr).contains("unknown subcommand"));
    }

    /// A lib-gone retry may skip the command only for a positively missing hosts.
    #[test]
    fn stop_retry_distinguishes_missing_hosts_from_a_denied_lookup() {
        for (message, succeeds) in [
            ("No such file or directory", true),
            ("Permission denied", false),
        ] {
            let fixture = ShellFixture::new();
            fixture.launchctl(
                0,
                113,
                "Bad request.\nCould not find service \"io.farhelm.test\" in domain for uid: 501",
            );
            fixture.tool(
                "stat",
                &format!(
                    "printf '%s\\n' {} >&2; exit 1",
                    shell_quote(&format!("stat: /owned/state/hosts: stat: {message}"))
                ),
            );
            let result = fixture.run(
                &stop_script(
                    "user/501",
                    "io.farhelm.test",
                    Path::new("/owned/lib/farhelm"),
                    Path::new("/owned/state"),
                )
                .unwrap(),
            );
            assert_eq!(result.status.success(), succeeds);
            if succeeds {
                assert_eq!(result.stdout, b"absent");
            }
            assert!(!fixture.commands().contains("internal stop-terminal-hosts"));
        }
    }

    /// Nested mounts must be caught without confusing a path's spaces or prefix.
    #[test]
    fn mount_guard_preserves_spaces_and_rejects_ambiguous_tables() {
        for (table, succeeds, expected) in [
            (
                "disk on /owned/lib (apfs, local)\nother on /owned/lib/nested mount (apfs)\nthird on /owned/library (apfs)",
                true,
                "/owned/lib\0/owned/lib/nested mount\0",
            ),
            ("source on name on /owned/lib/nested (apfs)", false, ""),
            ("disk on /owned/lib/odd (name (apfs)", false, ""),
            ("disk on /owned/lib/new\nline (apfs)", false, ""),
        ] {
            let fixture = ShellFixture::new();
            fixture.tool("mount", &format!("printf '%s\\n' {}", shell_quote(table)));
            let result =
                fixture.run(&(MAC_MOUNT_POINTS_SH.to_owned() + "mount_points /owned/lib '\\0'\n"));
            assert_eq!(result.status.success(), succeeds);
            if succeeds {
                assert_eq!(result.stdout, expected.as_bytes());
            }
        }
    }
}
