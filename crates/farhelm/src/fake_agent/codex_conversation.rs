//! A deterministic native-Codex conversation fixture for attribution tests.
//!
//! This is deliberately not a general Codex simulator. It only makes the
//! process, transcript, and hook shapes needed to prove that a nested process
//! inheriting its parent's launch credential cannot take over foreground
//! attribution. Everything after the fixture decides to fire a hook remains
//! real: the shipped hook binary, its Unix-socket connection, inherited
//! credential, peer identity, supervisor store, and restart path.

use anyhow::{Context, bail, ensure};
use std::io::{BufRead, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use super::{HOOK_CHILD_DEADLINE, fresh_conversation_id, wait_bounded};

const HOOK_BINARY_FLAG: &str = "--hook-binary";
const RESUME_ID_FLAG: &str = "--resume-id";
const CHILD_FLAG: &str = "--codex-child";

/// Run the root or one-shot nested native-Codex fixture.
///
/// The hook binary is supplied as a trailing argument because the real
/// `farhelm internal fake-agent` parser intentionally accepts vendor flags it
/// does not understand. The fixture must not default to its own executable:
/// its executable is named `codex` for kernel-basename validation, while a
/// hook launched through it would add a second native Codex ancestor.
pub(super) fn run(record_home: Option<PathBuf>) -> anyhow::Result<()> {
    let root = record_home.context("codex-conversation needs --record-home")?;
    let args: Vec<std::ffi::OsString> = std::env::args_os().collect();
    let hook_binary = required_path(&args, HOOK_BINARY_FLAG)?;
    let child = option_value(&args, CHILD_FLAG)?;
    let resume_id = option_value(&args, RESUME_ID_FLAG)?;
    if let Some(kind) = child.as_deref() {
        return nested(&root, &hook_binary, kind);
    }
    let defer_startup = args.iter().any(|arg| arg == "--defer-startup");
    root_conversation(&root, &hook_binary, resume_id.as_deref(), defer_startup)
}

/// Keep one foreground process alive while commands trigger the state
/// transitions the supervisor must attribute to that process alone.
/// Deferred startup leaves a live foreground and valid transcript without
/// reporting either, so a child can exercise admission against a pristine row.
fn root_conversation(
    root: &Path,
    hook_binary: &Path,
    resume_id: Option<&str>,
    defer_startup: bool,
) -> anyhow::Result<()> {
    let cwd = std::env::current_dir().context("reading Codex fixture cwd")?;
    let mut current = match resume_id {
        Some(id) => reopen(root, id)?,
        None => create(root, &cwd)?,
    };
    let source = if resume_id.is_some() {
        "resume"
    } else {
        "startup"
    };
    if !defer_startup {
        report(hook_binary, &current, source, Some(&current.path))?;
    }

    let mut out = std::io::stdout().lock();
    writeln!(out, "CODEX-KERNEL-EXE:{}\r", kernel_executable_basename()?)?;
    if resume_id.is_some() {
        writeln!(out, "CODEX-CONVERSATION RESUMED:{}\r", current.runtime)?;
    }
    writeln!(
        out,
        "CODEX-CONVERSATION READY:{}:{}\r",
        current.runtime,
        current.path.display()
    )?;
    out.flush()?;
    let stdin = std::io::stdin();
    for line in stdin.lock().lines() {
        let line = line?;
        match line.trim() {
            "report-startup" => {
                report(hook_binary, &current, "startup", Some(&current.path))?;
                writeln!(out, "CODEX-STARTUP-REPORTED:{}\r", current.runtime)?;
            }
            "nested-persisted" => nested_from_root(root, hook_binary, "persisted", &mut out)?,
            "nested-ephemeral" => nested_from_root(root, hook_binary, "ephemeral", &mut out)?,
            "nested-shell-persisted" => nested_shell_from_root(root, hook_binary, true, &mut out)?,
            "nested-shell-fileless" => nested_shell_from_root(root, hook_binary, false, &mut out)?,
            "nested-shell-native" => nested_shell_native(root, hook_binary, &mut out)?,
            "clear-pending" => {
                current = pending_clear(root)?;
                report(hook_binary, &current, "clear", Some(&current.path))?;
                writeln!(
                    out,
                    "CODEX-CLEAR-PENDING:{}:{}\r",
                    current.runtime,
                    current.path.display()
                )?;
            }
            "persist" => {
                persist(&current)?;
                writeln!(out, "CODEX-PERSISTED:{}\r", current.runtime)?;
            }
            "compact" => {
                report(hook_binary, &current, "compact", Some(&current.path))?;
                writeln!(out, "CODEX-COMPACT:{}\r", current.runtime)?;
            }
            "recall" => writeln!(out, "CODEX-RECALL:{}\r", recall(&current)?)?,
            text if let Some(value) = text.strip_prefix("remember ") => {
                remember(&current, value)?;
                writeln!(out, "CODEX-REMEMBERED:{}\r", current.runtime)?;
            }
            "quit" => return Ok(()),
            other => bail!("unsupported codex-conversation command {other:?}"),
        }
        out.flush()?;
    }
    Ok(())
}

/// Spawn a second actual `codex` image, retaining the launch environment the
/// supervisor gave its parent. Its stdout is bounded and replayed as a single
/// witness so no child descriptor leaks into the long-lived pane.
fn nested_from_root(
    root: &Path,
    hook_binary: &Path,
    kind: &str,
    out: &mut impl Write,
) -> anyhow::Result<()> {
    let image = std::env::args_os()
        .next()
        .context("the Codex fixture has argv[0]")?;
    let mut child = Command::new(&image)
        .args([
            "internal",
            "fake-agent",
            "--script",
            "codex-conversation",
            "--record-home",
        ])
        .arg(root)
        .arg(HOOK_BINARY_FLAG)
        .arg(hook_binary)
        .arg(CHILD_FLAG)
        .arg(kind)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| {
            format!(
                "spawning nested native Codex image {}",
                PathBuf::from(image).display()
            )
        })?;
    let Some(status) = wait_bounded(&mut child, HOOK_CHILD_DEADLINE, "nested Codex fixture")?
    else {
        writeln!(out, "CODEX-NESTED-HUNG:{kind}\r")?;
        return Ok(());
    };
    let stdout = read_child_pipe(child.stdout.take(), "nested Codex stdout")?;
    let stderr = read_child_pipe(child.stderr.take(), "nested Codex stderr")?;
    ensure!(
        status.success(),
        "nested Codex fixture exited {status}; stderr: {}",
        String::from_utf8_lossy(&stderr),
    );
    ensure!(
        stderr.is_empty(),
        "nested Codex fixture wrote stderr: {}",
        String::from_utf8_lossy(&stderr),
    );
    let witness = String::from_utf8(stdout).context("nested Codex fixture printed UTF-8")?;
    let id = witness
        .strip_prefix("CODEX-NESTED:")
        .and_then(|line| line.lines().next())
        .context("nested Codex fixture did not print its completion witness")?;
    writeln!(out, "CODEX-NESTED:{kind}:{id}\r")?;
    Ok(())
}

/// Perform one nested hook report from a SHELL descendant: the reporter's
/// parent is a live shell, not a Codex runtime, so the corridor must
/// refuse it as an unclassified intermediary — even when every other
/// part of the report is valid. The shell inherits the launch credential
/// by normal process inheritance, exactly like a vendor shell-escape
/// would.
///
/// Why a shell descendant matters separately from a native one: the
/// native child is refused by counting two Codex images; the shell child
/// carries no second runtime, pinning that the corridor classifies the
/// intermediaries themselves rather than either proof carrying the whole
/// boundary. Both cases below are valid in every respect EXCEPT
/// ancestry — matching envelope, plausible runtime, SessionStart, and
/// (for persisted) a real record whose metadata is asserted before the
/// report — so a refusal can only come from the corridor.
fn nested_shell_from_root(
    root: &Path,
    hook_binary: &Path,
    persisted: bool,
    out: &mut impl Write,
) -> anyhow::Result<()> {
    let cwd = std::env::current_dir().context("reading nested shell fixture cwd")?;
    let (conversation, source, transcript) = if persisted {
        // A real record for the shell's own runtime: verification would
        // succeed, so only the corridor stands between this report and a
        // rival binding. Its metadata is asserted before anything acts.
        let conversation = create(root, &cwd)?;
        assert_root_record(&conversation)?;
        let path = conversation.path.clone();
        (conversation, "startup", Some(path))
    } else {
        // No transcript at all, reported as a clear: the strongest weapon
        // a descendant holds, since a foreground clear may establish a
        // pending binding. Refusal here is the corridor doing its job.
        let conversation = ephemeral(root, &cwd)?;
        (conversation, "clear", None)
    };
    let payload = serde_json::json!({
        "session_id": conversation.runtime,
        "transcript_path": transcript,
        "hook_event_name": "SessionStart",
        "source": source,
    })
    .to_string();
    let transcript = if persisted {
        run_shell_reporter(root, hook_binary, &payload)?
    } else {
        // The fileless weapon runs through the attacker-shaped `-c`
        // string: hook first, then a redirection and a second command.
        // The witness lines come after the report from the same live
        // shell, which the completion marker proves was alive throughout.
        run_shell_dash_c(root, hook_binary, &payload)?
    };
    writeln!(out, "SHELL-INTERMEDIARY:{}\r", transcript.intermediary)?;
    writeln!(
        out,
        "CODEX-NESTED-SHELL:{}:{}:{}\r",
        if persisted { "persisted" } else { "fileless" },
        conversation.runtime,
        transcript.intermediary,
    )?;
    Ok(())
}

/// The metadata a persisted shell case asserts about its record before
/// the report may run: the file exists, opens with a session header for
/// THIS runtime from an attributable source. Anything less would let a
/// refusal be blamed on the record instead of the ancestry.
fn assert_root_record(conversation: &Conversation) -> anyhow::Result<()> {
    let text = std::fs::read_to_string(&conversation.path)
        .with_context(|| format!("reading shell record {}", conversation.path.display()))?;
    let header: serde_json::Value = text
        .lines()
        .next()
        .context("shell record has a first line")
        .and_then(|line| serde_json::from_str(line).context("shell record header parses"))?;
    let payload = &header["payload"];
    ensure!(
        payload["id"].as_str() == Some(conversation.persistent.as_str())
            && payload["session_id"].as_str() == Some(conversation.runtime.as_str())
            && payload["source"].as_str() == Some("cli"),
        "shell record metadata must name this runtime from an attributable source: {}",
        header
    );
    Ok(())
}

/// What running the reporter script established: the intermediary's
/// observed image name, and that the hook finished silently under it.
#[derive(Debug)]
struct ShellReport {
    intermediary: String,
}

/// Run one hook report as the child of a live shell: the script prints
/// its own observed image name BEFORE running anything (identity
/// established first), runs the hook as its foreground child, waits for
/// it, and reports its exit. No `exec` anywhere — replacing the shell
/// with the reporter would delete the very intermediary under test.
///
/// The name is observed from the live process (`ps` on its own pid),
/// never a constant: asserting a hardcoded name would prove the fixture
/// ran, not what ran it.
fn run_shell_reporter(
    root: &Path,
    hook_binary: &Path,
    payload: &str,
) -> anyhow::Result<ShellReport> {
    let script = root.join("shell-reporter.sh");
    let payload_path = root.join("shell-payload.json");
    std::fs::write(&payload_path, payload).context("writing shell report payload")?;
    std::fs::write(
        &script,
        "#!/bin/sh\n\
         echo \"SHELL-INTERMEDIARY:$(ps -p $$ -o comm= | tr -d '[:space:]')\"\n\
         \"$1\" internal hook --vendor codex < \"$2\" > hook.out 2> hook.err\n\
         echo \"SHELL-REPORT-DONE:$?\"\n",
    )
    .context("writing shell reporter script")?;
    let mut child = Command::new("/bin/sh")
        .arg(&script)
        .arg(hook_binary)
        .arg(&payload_path)
        .current_dir(root)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("spawning shell intermediary")?;
    let Some(status) = wait_bounded(&mut child, HOOK_CHILD_DEADLINE, "shell intermediary")? else {
        bail!("shell intermediary hung");
    };
    let stdout = read_child_pipe(child.stdout.take(), "shell intermediary stdout")?;
    let stderr = read_child_pipe(child.stderr.take(), "shell intermediary stderr")?;
    ensure!(
        status.success() && stderr.is_empty(),
        "shell intermediary failed: status={status}, stderr={}",
        String::from_utf8_lossy(&stderr),
    );
    let transcript = String::from_utf8(stdout).context("shell intermediary printed UTF-8")?;
    let intermediary = transcript
        .lines()
        .find_map(|line| line.strip_prefix("SHELL-INTERMEDIARY:"))
        .filter(|name| !name.is_empty())
        .context("shell intermediary witnessed its own image before reporting")?
        .to_string();
    transcript
        .lines()
        .find(|line| *line == "SHELL-REPORT-DONE:0")
        .context("the hook finished silently under its shell parent")?;
    for name in ["hook.out", "hook.err"] {
        let bytes = std::fs::read(root.join(name))
            .with_context(|| format!("reading refused hook {name}"))?;
        ensure!(
            bytes.is_empty(),
            "a refused hook stays silent on its own descriptors; {name} held {:?}",
            String::from_utf8_lossy(&bytes)
        );
    }
    Ok(ShellReport { intermediary })
}

/// Run one hook report as the child of a live shell whose `-c` string is
/// the hook-first chained shape: the hook with an input redirection,
/// then a second command. The witness lines print AFTER the report from
/// the same shell invocation — sequential commands in one `-c` string
/// run in one process, so the completion marker proves the witnessing
/// shell is the process that parented the hook, not something observed
/// later. The executable path is quoted with the installer's own quoting
/// so paths with spaces or metacharacters stay one literal word; every
/// other metacharacter in the string is live shell syntax, which is the
/// point: the corridor must refuse it while the hook still runs (and so
/// must exit silently, exactly like the script shape). The bare `echo`
/// after the witness is load-bearing: `tr -d '[:space:]'` consumes the
/// newline too, and without it the witness and the completion marker
/// merge onto one line the parser cannot match. `hook_status=$?` runs
/// immediately after the hook, before any witness command overwrites
/// `$?`, so the completion marker reports the hook's exit — a silently
/// failing reporter fails the completion check instead of hiding behind
/// printf's status.
fn run_shell_dash_c(root: &Path, hook_binary: &Path, payload: &str) -> anyhow::Result<ShellReport> {
    let payload_path = root.join("shell-payload.json");
    std::fs::write(&payload_path, payload).context("writing shell -c report payload")?;
    let command = format!(
        "{} internal hook --vendor codex < {} > hook.out 2> hook.err; hook_status=$?; ps -p $$ -o comm= | tr -d '[:space:]' | sed 's/^/SHELL-INTERMEDIARY:/'; echo; printf 'SHELL-REPORT-DONE:%s\n' \"$hook_status\"",
        shell_words::quote(hook_binary.to_string_lossy().as_ref()),
        shell_words::quote(payload_path.to_string_lossy().as_ref()),
    );
    let mut child = Command::new("/bin/sh")
        .arg("-c")
        .arg(&command)
        .current_dir(root)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("spawning shell -c intermediary")?;
    let Some(status) = wait_bounded(&mut child, HOOK_CHILD_DEADLINE, "shell -c intermediary")?
    else {
        bail!("shell -c intermediary hung");
    };
    let stdout = read_child_pipe(child.stdout.take(), "shell -c intermediary stdout")?;
    let stderr = read_child_pipe(child.stderr.take(), "shell -c intermediary stderr")?;
    ensure!(
        status.success() && stderr.is_empty(),
        "shell -c intermediary failed: status={status}, stderr={}",
        String::from_utf8_lossy(&stderr),
    );
    let transcript = String::from_utf8(stdout).context("shell -c intermediary printed UTF-8")?;
    let intermediary = transcript
        .lines()
        .find_map(|line| line.strip_prefix("SHELL-INTERMEDIARY:"))
        .filter(|name| !name.is_empty())
        .context("shell -c intermediary witnessed its own live image")?
        .to_string();
    transcript
        .lines()
        .find(|line| *line == "SHELL-REPORT-DONE:0")
        .context("the hook finished silently under its shell -c parent")?;
    for name in ["hook.out", "hook.err"] {
        let bytes = std::fs::read(root.join(name))
            .with_context(|| format!("reading refused hook {name}"))?;
        ensure!(
            bytes.is_empty(),
            "a refused hook stays silent on its own descriptors; {name} held {:?}",
            String::from_utf8_lossy(&bytes)
        );
    }
    Ok(ShellReport { intermediary })
}

/// Perform one nested native-Codex report launched THROUGH a shell: the
/// real shell-to-harness-to-hook chain, all three processes live — the
/// shell script runs the nested image as its child, and the nested image
/// runs its hook as its own. Two Codex images refuse this regardless of
/// the shell between them, pinning that wrapping a nested runtime in a
/// shell does not smuggle it past the corridor.
fn nested_shell_native(
    root: &Path,
    hook_binary: &Path,
    out: &mut impl Write,
) -> anyhow::Result<()> {
    let image = std::env::args_os()
        .next()
        .context("the Codex fixture has argv[0]")?;
    let script = root.join("shell-native.sh");
    std::fs::write(
        &script,
        format!(
            "#!/bin/sh\n\
             echo \"SHELL-INTERMEDIARY:$(ps -p $$ -o comm= | tr -d '[:space:]')\"\n\
             {}\n",
            shell_words::join([
                &*image.to_string_lossy(),
                "internal",
                "fake-agent",
                "--script",
                "codex-conversation",
                "--record-home",
                &*root.to_string_lossy(),
                "--hook-binary",
                &*hook_binary.to_string_lossy(),
                CHILD_FLAG,
                "persisted",
            ]),
        ),
    )
    .context("writing shell native script")?;
    let mut child = Command::new("/bin/sh")
        .arg(&script)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("spawning shell native launcher")?;
    let Some(status) = wait_bounded(&mut child, HOOK_CHILD_DEADLINE, "shell native launcher")?
    else {
        bail!("shell native launcher hung");
    };
    let stdout = read_child_pipe(child.stdout.take(), "shell native stdout")?;
    let stderr = read_child_pipe(child.stderr.take(), "shell native stderr")?;
    ensure!(
        status.success() && stderr.is_empty(),
        "shell native launcher failed: status={status}, stderr={}",
        String::from_utf8_lossy(&stderr),
    );
    let transcript = String::from_utf8(stdout).context("shell native printed UTF-8")?;
    let intermediary = transcript
        .lines()
        .find_map(|line| line.strip_prefix("SHELL-INTERMEDIARY:"))
        .filter(|name| !name.is_empty())
        .context("shell launcher witnessed its own image")?;
    let id = transcript
        .lines()
        .find_map(|line| {
            line.strip_prefix("CODEX-NESTED:")
                .and_then(|rest| rest.contains(':').then(|| rest.to_string()))
        })
        .context("nested native fixture did not print its completion witness")?;
    writeln!(out, "SHELL-INTERMEDIARY:{intermediary}\r")?;
    writeln!(out, "CODEX-NESTED-SHELLNATIVE:persisted:{id}\r")?;
    Ok(())
}

/// Perform one nested hook report, with or without a durable transcript.
fn nested(root: &Path, hook_binary: &Path, kind: &str) -> anyhow::Result<()> {
    let cwd = std::env::current_dir().context("reading nested Codex fixture cwd")?;
    let conversation = match kind {
        "persisted" => create(root, &cwd)?,
        "ephemeral" => ephemeral(root, &cwd)?,
        other => bail!("unknown nested Codex shape {other:?}"),
    };
    report(
        hook_binary,
        &conversation,
        "startup",
        (kind == "persisted").then_some(conversation.path.as_path()),
    )?;
    println!(
        "CODEX-NESTED:{}:{}",
        conversation.runtime,
        kernel_executable_basename()?
    );
    Ok(())
}

#[derive(Debug, Clone)]
struct Conversation {
    runtime: String,
    persistent: String,
    path: PathBuf,
}

/// Write a complete root record before reporting it, matching the normal
/// startup ordering native Codex exposes to the hook.
fn create(root: &Path, cwd: &Path) -> anyhow::Result<Conversation> {
    let id = fresh_conversation_id();
    let path = root.join(format!("{id}.jsonl"));
    let conversation = Conversation {
        runtime: id.clone(),
        persistent: id,
        path,
    };
    persist_with_cwd(&conversation, cwd)?;
    Ok(conversation)
}

/// `/clear` has an exact path before its transcript exists. The path is
/// intentionally private and not under the scanned Codex home tree.
fn pending_clear(root: &Path) -> anyhow::Result<Conversation> {
    let id = fresh_conversation_id();
    Ok(Conversation {
        runtime: id.clone(),
        persistent: id.clone(),
        path: root.join(format!("{id}.jsonl")),
    })
}

/// Model a nested runtime conversation that has not created a transcript.
///
/// Its placeholder path never reaches the hook: native Codex reports this
/// shape as `transcript_path: null`, unlike a foreground clear whose exact
/// future path is already known.
fn ephemeral(root: &Path, _cwd: &Path) -> anyhow::Result<Conversation> {
    let id = fresh_conversation_id();
    Ok(Conversation {
        runtime: id.clone(),
        persistent: id,
        path: root.join("ephemeral").join("never-written.jsonl"),
    })
}

/// Reopen only the requested durable record. This never turns a missing or
/// malformed target into a fresh conversation, because that would make a
/// restart look successful while abandoning the user's history.
fn reopen(root: &Path, id: &str) -> anyhow::Result<Conversation> {
    let path = root.join(format!("{id}.jsonl"));
    let record = load(&path)?;
    ensure!(
        record.persistent == id,
        "resume id does not match its record"
    );
    Ok(record)
}

/// Materialize the exact transcript a preceding clear report named, without
/// emitting another report that could hide a broken refresh path.
fn persist(conversation: &Conversation) -> anyhow::Result<()> {
    persist_with_cwd(
        conversation,
        &std::env::current_dir().context("reading Codex fixture cwd")?,
    )
}

/// Use a root header rather than a body-only record so the production validator
/// must establish both runtime and persistent identity before offering resume.
fn persist_with_cwd(conversation: &Conversation, cwd: &Path) -> anyhow::Result<()> {
    if let Some(parent) = conversation.path.parent() {
        std::fs::create_dir_all(parent).context("creating private Codex transcript directory")?;
    }
    let timestamp =
        farhelm_supervisor::agent_kind::format_rfc3339(farhelm_supervisor::agent_kind::now_unix());
    let header = serde_json::json!({
        "timestamp": timestamp,
        "type": "session_meta",
        "payload": {
            "id": conversation.persistent,
            "session_id": conversation.runtime,
            "source": "cli",
            "cwd": cwd,
            "timestamp": timestamp,
        },
    });
    std::fs::write(&conversation.path, format!("{}\n", header))
        .with_context(|| format!("writing Codex transcript {}", conversation.path.display()))
}

/// Append a durable fixture entry so resume can prove it reopened this file.
fn remember(conversation: &Conversation, value: &str) -> anyhow::Result<()> {
    let mut file = std::fs::OpenOptions::new()
        .append(true)
        .open(&conversation.path)
        .with_context(|| format!("reopening Codex transcript {}", conversation.path.display()))?;
    writeln!(
        file,
        "{}",
        serde_json::json!({"type":"conversation_entry","remember":value})
    )?;
    file.flush()?;
    Ok(())
}

/// Read the latest durable entry rather than accepting a value from terminal
/// input or resume argv.
fn recall(conversation: &Conversation) -> anyhow::Result<String> {
    let text = std::fs::read_to_string(&conversation.path)
        .with_context(|| format!("reading Codex transcript {}", conversation.path.display()))?;
    text.lines()
        .rev()
        .find_map(|line| {
            serde_json::from_str::<serde_json::Value>(line)
                .ok()
                .and_then(|entry| {
                    entry
                        .get("remember")
                        .and_then(serde_json::Value::as_str)
                        .map(str::to_owned)
                })
        })
        .context("Codex transcript has no durable remembered entry")
}

/// Parse the minimum root metadata required to resume one exact transcript.
fn load(path: &Path) -> anyhow::Result<Conversation> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("reading Codex transcript {}", path.display()))?;
    let first = text.lines().next().context("Codex transcript is empty")?;
    let value: serde_json::Value =
        serde_json::from_str(first).context("Codex transcript header is malformed")?;
    let payload = value
        .get("payload")
        .and_then(serde_json::Value::as_object)
        .context("Codex transcript has no payload")?;
    let persistent = payload
        .get("id")
        .and_then(serde_json::Value::as_str)
        .context("Codex transcript has no persistent id")?
        .to_owned();
    let runtime = payload
        .get("session_id")
        .and_then(serde_json::Value::as_str)
        .context("Codex transcript has no runtime id")?
        .to_owned();
    Ok(Conversation {
        runtime,
        persistent,
        path: path.to_path_buf(),
    })
}

/// Use the original Farhelm binary for the real hook process.
///
/// The payload includes the native Codex fields production validates, rather
/// than a direct control message that would bypass socket peer attribution.
fn report(
    hook_binary: &Path,
    conversation: &Conversation,
    source: &str,
    transcript_path: Option<&Path>,
) -> anyhow::Result<()> {
    // `None` is the observed ephemeral shape. A clear deliberately supplies
    // `Some` even before its file exists, because that exact future path is
    // what capture refresh may later promote.
    let payload = serde_json::json!({
        "session_id": conversation.runtime,
        "transcript_path": transcript_path,
        "hook_event_name": "SessionStart",
        "source": source,
    })
    .to_string();
    let mut child = Command::new(hook_binary)
        // The envelope discriminator comes from this fixture's own entry
        // point — the real injection installs `--vendor codex` the same
        // way — never from the payload.
        .args(["internal", "hook", "--vendor", "codex"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("spawning original hook binary {}", hook_binary.display()))?;
    child
        .stdin
        .take()
        .context("hook stdin is piped")?
        .write_all(payload.as_bytes())?;
    let Some(status) = wait_bounded(&mut child, HOOK_CHILD_DEADLINE, "Codex hook binary")? else {
        bail!("Codex hook binary hung")
    };
    let stdout = read_child_pipe(child.stdout.take(), "Codex hook stdout")?;
    let stderr = read_child_pipe(child.stderr.take(), "Codex hook stderr")?;
    ensure!(
        status.success() && stdout.is_empty() && stderr.is_empty(),
        "Codex hook was not silent and successful: status={status}, stdout={}, stderr={}",
        String::from_utf8_lossy(&stdout),
        String::from_utf8_lossy(&stderr),
    );
    Ok(())
}

/// Require a fixture-only trailing path flag without changing the public CLI.
fn required_path(args: &[std::ffi::OsString], flag: &str) -> anyhow::Result<PathBuf> {
    option_value(args, flag)?
        .map(PathBuf::from)
        .with_context(|| format!("codex-conversation requires {flag}"))
}

/// Read one trailing fixture flag and reject duplicate or valueless forms.
fn option_value(args: &[std::ffi::OsString], flag: &str) -> anyhow::Result<Option<String>> {
    let mut occurrences = args.iter().enumerate().filter(|(_, arg)| *arg == flag);
    let Some((index, _)) = occurrences.next() else {
        return Ok(None);
    };
    ensure!(
        occurrences.next().is_none(),
        "codex-conversation received {flag} more than once"
    );
    let value = args
        .get(index + 1)
        .with_context(|| format!("codex-conversation received valueless {flag}"))?
        .to_string_lossy()
        .into_owned();
    ensure!(
        !value.is_empty(),
        "codex-conversation received empty {flag}"
    );
    Ok(Some(value))
}

/// Read the bounded diagnostic output retained after a reaped fixture child.
fn read_child_pipe(pipe: Option<impl Read>, what: &'static str) -> anyhow::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    let mut pipe = pipe.context(what)?;
    pipe.by_ref()
        .take(8192)
        .read_to_end(&mut bytes)
        .context(what)?;
    Ok(bytes)
}

/// Return the executable basename the kernel assigned this process, not the
/// user-controlled argv0 spelling. The test reads this marker before relying
/// on the production ancestry rule, whose whole predicate is this basename.
fn kernel_executable_basename() -> anyhow::Result<String> {
    #[cfg(target_os = "linux")]
    let path =
        std::fs::read_link("/proc/self/exe").context("reading the kernel executable link")?;
    #[cfg(target_os = "macos")]
    let path = {
        use std::ffi::CStr;
        use std::os::unix::ffi::OsStrExt;

        let mut buffer = [0_i8; libc::PROC_PIDPATHINFO_MAXSIZE as usize];
        // SAFETY: the writable, zero-initialized buffer has the capacity passed
        // to proc_pidpath; a successful result is a NUL-terminated path.
        let size = unsafe {
            // `proc_pidpath` returns the current process's kernel image,
            // which is macOS's equivalent of Linux `/proc/self/exe`.
            libc::proc_pidpath(
                std::process::id() as libc::pid_t,
                buffer.as_mut_ptr().cast(),
                buffer.len() as u32,
            )
        };
        ensure!(
            size > 0,
            "reading the kernel executable path failed: {}",
            std::io::Error::last_os_error()
        );
        // SAFETY: successful proc_pidpath above supplies a NUL-terminated path
        // inside the still-live buffer.
        let bytes = unsafe { CStr::from_ptr(buffer.as_ptr()) }.to_bytes();
        PathBuf::from(std::ffi::OsStr::from_bytes(bytes))
    };
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    compile_error!(
        "codex-conversation needs a kernel executable-path implementation on this platform"
    );
    path.file_name()
        .and_then(|name| name.to_str())
        .map(str::to_owned)
        .context("the kernel executable has a UTF-8 basename")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The `-c` completion marker must carry the hook's exit, not a
    /// witness command's: run the real helper against a reporter that
    /// exits nonzero without output and require the completion check to
    /// fail. Without the saved `hook_status`, the marker would read
    /// printf's success and a silently failing reporter would pass.
    #[farhelm_testtrace::test]
    fn a_silently_failing_reporter_fails_the_shell_completion_check() {
        let root = tempfile::tempdir().expect("fixture needs a scratch root");
        let failure = run_shell_dash_c(
            root.path(),
            Path::new("/bin/false"),
            "{\"session_id\":\"fake\",\"hook_event_name\":\"SessionStart\",\"source\":\"clear\"}",
        )
        .expect_err("a nonzero hook exit must fail the completion check");
        assert!(
            failure.to_string().contains("finished silently"),
            "the failure must come from the completion check, not setup: {failure}"
        );
    }
}
