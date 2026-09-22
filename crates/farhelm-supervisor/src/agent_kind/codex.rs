//! Exact Codex records, after foreground-process attribution by the supervisor.

use anyhow::{Context, bail, ensure};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::Path;

use super::{
    MAX_LOCATOR_BYTES, MAX_SESSION_PATH_BYTES, RECORD_PREFIX_BYTES, is_plausible_conversation_id,
};

pub(crate) const PREFIX: &str = "codex:";

/// The vendor's own foreground-transition vocabulary: the hook `source`
/// values a genuine foreground Codex lifecycle event carries.
///
/// A single predicate so the doorway's raw check and admission's
/// sanitized backstop cannot drift apart: both consult this, and neither
/// invents its own list. `agent_type` alone is not consulted here — a
/// legitimate top-level `--agent` invocation carries one — while a
/// present subagent `agent_id` is rejected separately at the doorway,
/// before sanitation could blur it.
pub(crate) fn is_foreground_source(source: &str) -> bool {
    matches!(source, "startup" | "resume" | "clear" | "compact")
}

/// Runtime session identity and persistent thread identity are not interchangeable.
/// A pending clear retains its exact path without offering the discarded thread.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CodexLocator {
    version: u8,
    pub(crate) runtime_session_id: String,
    pub(crate) session_file: Option<String>,
    pub(crate) thread_id: Option<String>,
    pub(crate) resumable: bool,
}

impl CodexLocator {
    /// Preserve missing versus malformed hook evidence before touching a file.
    /// The caller must establish process attribution; this only validates the
    /// report's shape and never makes it resumable by itself.
    pub(crate) fn reported(
        runtime_session_id: String,
        transcript_path: Option<Value>,
        hook_event_name: Option<Value>,
    ) -> anyhow::Result<Self> {
        ensure!(
            hook_event_name.as_ref().and_then(Value::as_str) == Some("SessionStart"),
            "Codex requires a foreground SessionStart report"
        );
        let session_file = match transcript_path {
            None | Some(Value::Null) => None,
            Some(Value::String(path)) => Some(path),
            Some(_) => bail!("Codex transcript path is not a string"),
        };
        let locator = Self {
            version: 1,
            runtime_session_id,
            session_file,
            thread_id: None,
            resumable: false,
        };
        locator.validate()?;
        Ok(locator)
    }

    /// Decode only this vendor's bounded durable token. Bare historical IDs
    /// cannot supply the provenance or exact path required for resume.
    pub(crate) fn parse(value: &str) -> anyhow::Result<Self> {
        ensure!(
            value.len() <= MAX_LOCATOR_BYTES,
            "Codex locator exceeds its byte bound"
        );
        let json = value.strip_prefix(PREFIX).context("not a Codex locator")?;
        let locator: Self = serde_json::from_str(json).context("malformed Codex locator")?;
        locator.validate()?;
        Ok(locator)
    }

    /// Keep the serialized token within the durable capture field's bound.
    pub(crate) fn encode(&self) -> anyhow::Result<String> {
        self.validate()?;
        let encoded = format!("{PREFIX}{}", serde_json::to_string(self)?);
        ensure!(
            encoded.len() <= MAX_LOCATOR_BYTES,
            "Codex locator exceeds its byte bound"
        );
        Ok(encoded)
    }

    /// Return a persistent thread only after exact-file verification succeeded.
    pub(crate) fn resume_id(&self) -> Option<&str> {
        self.resumable
            .then_some(self.thread_id.as_deref())
            .flatten()
    }

    fn validate(&self) -> anyhow::Result<()> {
        ensure!(self.version == 1, "unsupported Codex locator version");
        ensure!(
            is_plausible_conversation_id(&self.runtime_session_id),
            "invalid Codex runtime session id"
        );
        if let Some(id) = &self.thread_id {
            ensure!(
                is_plausible_conversation_id(id),
                "invalid Codex persistent thread id"
            );
        }
        if let Some(path) = &self.session_file {
            ensure!(
                path.len() <= MAX_SESSION_PATH_BYTES
                    && Path::new(path).is_absolute()
                    && !path.chars().any(char::is_control),
                "Codex transcript path is not bounded absolute text"
            );
        }
        ensure!(
            !self.resumable || (self.thread_id.is_some() && self.session_file.is_some()),
            "a resumable Codex locator requires an exact file and persistent thread"
        );
        Ok(())
    }

    /// Recheck only the attributed file. Missing or incomplete metadata is pending;
    /// conflicting metadata is an error. Neither case forgets a previously bound
    /// persistent ID, so replacing a file cannot redirect an old locator.
    pub(crate) async fn verify(&mut self) -> anyhow::Result<()> {
        self.resumable = false;
        let Some(path) = &self.session_file else {
            return Ok(());
        };
        let Some(text) = super::read_bounded_regular_file(Path::new(path)).await? else {
            return Ok(());
        };
        let Some((record, runtime)) = parse_record(&text)? else {
            return Ok(());
        };
        let runtime = runtime.as_deref().unwrap_or(&record.conversation);
        ensure!(
            runtime == self.runtime_session_id,
            "Codex record disagrees with the reported runtime session"
        );
        if let Some(expected) = &self.thread_id {
            ensure!(
                record.conversation == *expected,
                "Codex record changed its persistent thread identity"
            );
        }
        if self.thread_id.is_none() {
            self.thread_id = Some(record.conversation);
        }
        self.resumable = true;
        Ok(())
    }
}

/// Root metadata only. An internal thread can share the process and runtime ID,
/// so process ancestry alone cannot authorize its persistent record.
pub(super) fn parse_record(
    text: &str,
) -> anyhow::Result<Option<(super::RecordCorrelators, Option<String>)>> {
    let Some((line, _)) = text.split_once('\n') else {
        ensure!(
            text.len() < RECORD_PREFIX_BYTES,
            "Codex metadata exceeds the record prefix bound"
        );
        return Ok(None);
    };
    let header: Value = serde_json::from_str(line).context("malformed Codex record header")?;
    ensure!(
        header.get("type").and_then(Value::as_str) == Some("session_meta"),
        "Codex record has no session_meta header"
    );
    let metadata = header
        .get("payload")
        .and_then(Value::as_object)
        .context("Codex session_meta has no payload")?;
    ensure!(
        matches!(
            metadata.get("source").and_then(Value::as_str),
            Some("cli" | "exec")
        ),
        "Codex record is not an attributable root conversation"
    );
    let id = metadata
        .get("id")
        .and_then(Value::as_str)
        .context("Codex record has no persistent thread id")?;
    let runtime = match metadata.get("session_id") {
        None | Some(Value::Null) => None,
        Some(Value::String(runtime)) => {
            ensure!(
                is_plausible_conversation_id(runtime),
                "Codex record has an invalid runtime session id"
            );
            Some(runtime.clone())
        }
        Some(_) => bail!("Codex record has an invalid runtime session id"),
    };
    let cwd = metadata
        .get("cwd")
        .and_then(Value::as_str)
        .context("Codex record has no working directory")?;
    ensure!(
        cwd.len() <= MAX_SESSION_PATH_BYTES
            && Path::new(cwd).is_absolute()
            && !cwd.chars().any(char::is_control),
        "Codex record has an invalid working directory"
    );
    let timestamp = header
        .get("timestamp")
        .and_then(Value::as_str)
        .or_else(|| metadata.get("timestamp").and_then(Value::as_str))
        .context("Codex record has no timestamp")?;
    Ok(Some((
        super::correlators_from(id, cwd, timestamp)?,
        runtime,
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent_kind::{IntegrationSnapshot, RestartOffer};
    use serde_json::json;

    /// Write the smallest root record a locator can use. The runtime id is
    /// deliberately independent from the thread id: Codex reports the former
    /// to identify this running process, but `codex resume` requires the latter.
    fn root_record(runtime: &str, thread: &str) -> String {
        json!({
            "timestamp": "2026-09-19T12:00:05Z",
            "type": "session_meta",
            "payload": {
                "source": "cli",
                "id": thread,
                "session_id": runtime,
                "cwd": "/work"
            }
        })
        .to_string()
            + "\n"
    }

    /// A valid report binds a runtime process identity to the persistent
    /// thread id that must reach `codex resume`; those two vendor values are
    /// not interchangeable.
    #[farhelm_testtrace::test]
    async fn a_verified_codex_locator_resumes_its_persistent_thread_not_its_runtime_session() {
        let dir = farhelm_teststate::tempdir().expect("transcript directory");
        let transcript = dir.path().join("thread.jsonl");
        std::fs::write(&transcript, root_record("runtime-42", "thread-17"))
            .expect("write root transcript");

        let mut locator = CodexLocator::reported(
            "runtime-42".to_string(),
            Some(json!(transcript)),
            Some(json!("SessionStart")),
        )
        .expect("report shape");
        locator.verify().await.expect("verify root transcript");
        let stored = locator.encode().expect("encode verified locator");

        let snapshot = IntegrationSnapshot::resolve(
            &[
                "codex".to_string(),
                "--yolo".to_string(),
                "-m".to_string(),
                "gpt-5-codex".to_string(),
            ],
            None,
            None,
        )
        .expect("Codex integration");
        assert_eq!(
            snapshot.restart_offer(Some(&stored), 0),
            RestartOffer::Resume
        );
        assert_eq!(
            snapshot.filled_resume_argv(&stored),
            Some(vec![
                "codex".to_string(),
                "--yolo".to_string(),
                "-m".to_string(),
                "gpt-5-codex".to_string(),
                "resume".to_string(),
                "thread-17".to_string(),
            ]),
            "the persistent record id, rather than the runtime SessionStart id, is resumable \
             without dropping launch options"
        );
    }

    /// Exact Resume requires version 1 for a kind with an implemented
    /// proof — except the deliberate Codex exception: valid `codex:` v1
    /// tokens produced under the two-proof contract before the version
    /// column existed stay resumable at 0, while bare IDs stay excluded
    /// and unknown versions refuse.
    ///
    /// Why this test matters: it pins the interim offer behavior that
    /// the upgrade depends on. Old validated tokens must keep resuming
    /// (no upgrade may strand a proven binding), old bare IDs must never
    /// start resuming (no grandfathering), and a future version must fail
    /// closed (data preserved, Resume refused) until a contract knows it.
    #[farhelm_testtrace::test]
    fn codex_resume_requires_provenance_except_its_documented_v1_tokens() {
        let snapshot = IntegrationSnapshot::resolve(&["codex".to_string()], None, None)
            .expect("Codex integration");
        // A token exactly as a verified admission encodes one: version 1,
        // a plausible runtime id, an exact absolute record path, a
        // persistent thread, and the resumable bit verification sets.
        let stored = "codex:{\"version\":1,\"runtime_session_id\":\"0194fdc4-8c7c-7a1c-9f2e-abcdef012345\",\"session_file\":\"/tmp/rollout-17.jsonl\",\"thread_id\":\"thread-17\",\"resumable\":true}";
        assert_eq!(
            snapshot.restart_offer(Some(stored), 1),
            RestartOffer::Resume,
            "a binding admitted under the contract offers Resume"
        );
        assert_eq!(
            snapshot.restart_offer(Some(stored), 0),
            RestartOffer::Resume,
            "a valid v1 token predating the version column stays resumable"
        );
        assert_eq!(
            snapshot.restart_offer(Some(stored), 2),
            RestartOffer::FreshOnly,
            "an unknown provenance version refuses exact Resume"
        );
        assert_eq!(
            snapshot.restart_offer(Some(stored), -1),
            RestartOffer::FreshOnly,
            "a negative provenance version refuses exact Resume"
        );
        assert_eq!(
            snapshot.restart_offer(Some("historical-thread"), 1),
            RestartOffer::FreshOnly,
            "version 1 never blesses a bare id the verifier rejects"
        );
    }

    /// Old rows can retain a bare thread id, but that value has no foreground
    /// provenance. It remains stored for history while both user-visible
    /// restart decisions fail closed instead of guessing which transcript owns it.
    #[farhelm_testtrace::test]
    fn a_legacy_bare_codex_thread_never_offers_resume() {
        let snapshot = IntegrationSnapshot::resolve(&["codex".to_string()], None, None)
            .expect("Codex integration");
        assert_eq!(
            snapshot.restart_offer(Some("historical-thread"), 0),
            RestartOffer::FreshOnly
        );
        assert_eq!(snapshot.filled_resume_argv("historical-thread"), None);
    }

    /// Object-valued internal-source metadata must not become a root record,
    /// even when its runtime ID matches. This synthetic shape tests the
    /// root-only boundary, not the historical incident's unknown emitter.
    #[farhelm_testtrace::test]
    fn codex_rejects_internal_metadata_even_when_its_runtime_id_matches() {
        let internal = json!({
            "timestamp": "2026-09-19T12:00:05Z",
            "type": "session_meta",
            "payload": {
                "source": { "subagent": {} },
                "id": "thread-17",
                "session_id": "runtime-42",
                "cwd": "/work"
            }
        })
        .to_string()
            + "\n";
        assert!(parse_record(&internal).is_err());

        let root = parse_record(&root_record("runtime-42", "thread-17"))
            .expect("root record parses")
            .expect("root metadata is present");
        assert_eq!(root.0.conversation, "thread-17");
        assert_eq!(root.1.as_deref(), Some("runtime-42"));
    }

    /// A clear report may arrive before Codex has finished its new transcript.
    /// It stays pending until that exact file supplies matching root metadata;
    /// another transcript and a later replacement must never redirect it.
    #[farhelm_testtrace::test]
    async fn a_pending_clear_promotes_only_its_exact_unchanged_transcript() {
        let dir = farhelm_teststate::tempdir().expect("transcript directory");
        let exact = dir.path().join("clear.jsonl");
        let unrelated = dir.path().join("other.jsonl");
        std::fs::write(&unrelated, root_record("runtime-clear", "other-thread"))
            .expect("write unrelated transcript");

        let mut locator = CodexLocator::reported(
            "runtime-clear".to_string(),
            Some(json!(exact)),
            Some(json!("SessionStart")),
        )
        .expect("clear report shape");
        locator
            .verify()
            .await
            .expect("missing exact transcript is pending");
        assert_eq!(
            locator.resume_id(),
            None,
            "an unrelated record is not scanned"
        );

        std::fs::write(&exact, root_record("wrong-runtime", "wrong-thread"))
            .expect("write conflicting transcript");
        assert!(locator.verify().await.is_err());
        assert_eq!(
            locator.resume_id(),
            None,
            "conflicting metadata cannot promote a clear"
        );

        std::fs::write(&exact, root_record("runtime-clear", "clear-thread"))
            .expect("write exact transcript");
        locator
            .verify()
            .await
            .expect("exact transcript promotes clear");
        assert_eq!(locator.resume_id(), Some("clear-thread"));

        std::fs::write(&exact, root_record("runtime-clear", "replacement-thread"))
            .expect("replace transcript metadata");
        assert!(locator.verify().await.is_err());
        assert_eq!(
            locator.resume_id(),
            None,
            "a replacement cannot redirect a thread that was already bound"
        );
    }

    /// Missing transcript metadata is the one pending-clear shape. Typed JSON
    /// fields are not silently collapsed into that absence, because malformed
    /// hook data must leave the current foreground claim untouched instead.
    #[farhelm_testtrace::test]
    fn malformed_codex_report_fields_are_rejected_not_treated_as_pending() {
        let missing_path =
            CodexLocator::reported("runtime-42".to_string(), None, Some(json!("SessionStart")))
                .expect("an absent path is a pending locator");
        assert_eq!(missing_path.resume_id(), None);

        assert!(
            CodexLocator::reported(
                "runtime-42".to_string(),
                Some(json!({ "path": "/work/thread.jsonl" })),
                Some(json!("SessionStart")),
            )
            .is_err()
        );
        assert!(
            CodexLocator::reported(
                "runtime-42".to_string(),
                Some(json!("/work/thread.jsonl")),
                Some(json!(["SessionStart"])),
            )
            .is_err()
        );
    }
}
