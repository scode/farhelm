//! A deterministic native-Grok conversation fixture for report-file tests.
//!
//! This is deliberately not a Grok simulator. It only makes the process and
//! hook shapes Grok's attribution requires: an executable named `grok`
//! launched with `--no-leader` (the test hard-links this fixture binary under
//! that name and passes the flag), which fires the real `farhelm internal
//! hook --vendor grok` as its direct child with Grok's own callback payloads.
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
/// The hook binary is a flag, as in the Codex fixture, because this
/// process's own executable is named `grok`: a hook launched through it
/// would put a second native Grok in its own ancestry.
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
            Some(("select", uuid)) => {
                selections += 1;
                // Strictly newer for every selection this process makes,
                // which Grok's ordering fence requires of a new UUID.
                let timestamp = format!("2026-10-01T00:00:{selections:02}Z");
                fire(
                    &hook_binary,
                    serde_json::json!({
                        "sessionId": uuid,
                        "hookEventName": "SessionStart",
                        "source": "new",
                        "timestamp": timestamp,
                    }),
                )?;
                writeln!(out, "GROK-SELECTED:{uuid}\r")?;
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
