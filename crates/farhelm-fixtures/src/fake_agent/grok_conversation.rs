//! A deterministic native-Grok conversation fixture for report-file tests.
//!
//! Tests launch this fixture as a versioned Grok image with `--no-leader`.
//! It fires the real `farhelm internal hook --vendor grok` as its direct child
//! with Grok's own callback payloads. A nested copy can report before its
//! parent to exercise correction under the same terminal root.
//! Everything after the fixture decides to fire a hook is real: the shipped
//! hook binary, its recorded ancestry, the report file it drops, and the
//! supervisor that applies it.
//!
//! Grok is the one vendor whose reports depend on order — `SessionStart`
//! selects a conversation and later callbacks only enrich it — so this exists
//! to prove that a selection survives enrichments made while no supervisor is
//! running to apply either.
//!
//! Commands, one per input line:
//!
//! - `select <uuid>` fires `SessionStart` (source `new`) for `<uuid>`, with a
//!   selection timestamp that grows with every selection this process makes,
//!   and prints `GROK-SELECTED:<uuid>` once the hook has exited.
//! - `select-child <uuid>` runs a nested copy that writes the record pair,
//!   selects it with one `SessionStart`, and exits. The parent keeps running.
//! - `enrich <uuid>` writes `<uuid>`'s exact record pair under the record
//!   home (`<uuid>/updates.jsonl` and its sibling `summary.json`, the shapes
//!   Grok's verification reads) and fires `Stop` naming that record, then
//!   prints `GROK-ENRICHED:<uuid>`.
//! - `quit` exits.

use anyhow::{Context, bail, ensure};
use std::io::{BufRead, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use super::{HOOK_CHILD_DEADLINE, TEST_HOOK_BUDGET_MS, wait_bounded};

const HOOK_BINARY_FLAG: &str = "--hook-binary";

/// Run the fixture until `quit` or end of input.
///
/// The explicit hook binary keeps the reporter below the fixture whose
/// process identity the supervisor should record.
pub(super) fn run(record_home: Option<PathBuf>) -> anyhow::Result<()> {
    let root = record_home.context("grok-conversation needs --record-home")?;
    let args: Vec<std::ffi::OsString> = std::env::args_os().collect();
    let hook_binary = args
        .iter()
        .position(|arg| arg == HOOK_BINARY_FLAG)
        .and_then(|index| args.get(index + 1))
        .map(PathBuf::from)
        .with_context(|| format!("grok-conversation requires {HOOK_BINARY_FLAG}"))?;
    let mut selections = 0u32;
    let mut out = std::io::stdout().lock();
    writeln!(out, "GROK-CONVERSATION READY\r")?;
    out.flush()?;
    for line in std::io::stdin().lock().lines() {
        let line = line?;
        match line.trim().split_once(' ') {
            Some((command @ ("select" | "select-ready"), uuid)) => {
                // A child selects later than its parent's first report. A second
                // child can then be refused only by the pin, not timestamp ordering.
                let ready = command == "select-ready";
                selections += if ready { 2 } else { 1 };
                let timestamp = format!("2026-10-01T00:00:{selections:02}Z");
                let mut payload = serde_json::json!({
                    "sessionId": uuid,
                    "hookEventName": "SessionStart",
                    "source": "new",
                    "timestamp": timestamp,
                });
                if ready {
                    payload["transcriptPath"] =
                        serde_json::to_value(write_record_pair(&root, uuid)?)?;
                }
                fire(&hook_binary, payload)?;
                writeln!(out, "GROK-SELECTED:{uuid}\r")?;
            }
            Some(("select-child", uuid)) => {
                select_in_child(&args, uuid)?;
                writeln!(out, "GROK-CHILD-SELECTED:{uuid}\r")?;
            }
            Some(("enrich", uuid)) => {
                let updates = write_record_pair(&root, uuid)?;
                fire(
                    &hook_binary,
                    serde_json::json!({
                        "sessionId": uuid,
                        "hookEventName": "Stop",
                        "transcriptPath": updates,
                    }),
                )?;
                writeln!(out, "GROK-ENRICHED:{uuid}\r")?;
            }
            None if line.trim() == "quit" => return Ok(()),
            _ => bail!("unsupported grok-conversation command {line:?}"),
        }
        out.flush()?;
    }
    Ok(())
}

/// Run the same owned-backend command as a child so its real hook reports
/// ancestry through the parent. A separate input pipe leaves the terminal
/// with the parent, which will report its own first selection next.
fn select_in_child(args: &[std::ffi::OsString], uuid: &str) -> anyhow::Result<()> {
    let mut child = Command::new(std::env::current_exe()?)
        .args(&args[1..])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .spawn()
        .context("starting the nested Grok fixture")?;
    let sent = write!(
        child.stdin.take().context("nested Grok stdin is piped")?,
        "select-ready {uuid}\nquit\n"
    );
    // The fixture's outer deadline must leave time for its hook's own deadline and exit.
    let status = wait_bounded(&mut child, HOOK_CHILD_DEADLINE * 2, "nested Grok fixture")?;
    sent.context("sending the nested Grok selection")?;
    ensure!(
        status.is_some_and(|status| status.success()),
        "nested Grok fixture failed: {status:?}"
    );
    Ok(())
}

/// Write the exact record pair Grok verification reads for `uuid` and
/// return the `updates.jsonl` path a `Stop` callback names.
fn write_record_pair(root: &Path, uuid: &str) -> anyhow::Result<PathBuf> {
    let dir = root.join(uuid);
    std::fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;
    let updates = dir.join("updates.jsonl");
    let record = serde_json::json!({
        "method": "_x.ai/session/update",
        "params": {"sessionId": uuid},
    });
    std::fs::write(&updates, format!("{record}\n")).context("writing updates.jsonl")?;
    let summary = serde_json::json!({"info": {"id": uuid}});
    std::fs::write(dir.join("summary.json"), format!("{summary}\n"))
        .context("writing summary.json")?;
    Ok(updates)
}

/// Fire the real hook binary as this process's direct child with one Grok
/// callback payload, and require it to finish silently and successfully.
fn fire(hook_binary: &Path, payload: serde_json::Value) -> anyhow::Result<()> {
    let mut child = Command::new(hook_binary)
        .env("FARHELM_TEST_HOOK_BUDGET_MS", TEST_HOOK_BUDGET_MS)
        .args(["internal", "hook", "--vendor", "grok"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("spawning the hook binary {}", hook_binary.display()))?;
    child
        .stdin
        .take()
        .context("hook stdin is piped")?
        .write_all(payload.to_string().as_bytes())?;
    let Some(status) = wait_bounded(&mut child, HOOK_CHILD_DEADLINE, "Grok hook binary")? else {
        bail!("Grok hook binary hung")
    };
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    if let Some(mut pipe) = child.stdout.take() {
        pipe.read_to_end(&mut stdout)?;
    }
    if let Some(mut pipe) = child.stderr.take() {
        pipe.read_to_end(&mut stderr)?;
    }
    ensure!(
        status.success() && stdout.is_empty() && stderr.is_empty(),
        "Grok hook was not silent and successful: status={status}, stdout={}, stderr={}",
        String::from_utf8_lossy(&stdout),
        String::from_utf8_lossy(&stderr),
    );
    Ok(())
}
