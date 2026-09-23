//! Grok's durable selection fence and exact two-file resume evidence.
//!
//! The locator deliberately carries two different kinds of proof. The
//! `selected_at` key orders `SessionStart` events inside one live Grok
//! process, while `session_file` plus `resumable` records whether the exact
//! vendor files currently agree with that selected UUID. Keeping both in the
//! existing captured-conversation token lets the shared capture CAS commit a
//! selection, its readiness, and its ordering fence as one value.

#[cfg(test)]
use super::RECORD_PREFIX_BYTES;
use super::{
    MAX_LOCATOR_BYTES, MAX_SESSION_PATH_BYTES, is_plausible_conversation_id, parse_rfc3339,
};
use anyhow::{Context, ensure};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::cmp::Ordering;
use std::path::Path;

pub(crate) const PREFIX: &str = "grok:";

/// A canonical, timezone-independent ordering key for one RFC 3339 event.
///
/// Fractions stay decimal rather than being rounded to nanoseconds. Grok's
/// event order must not collapse two valid timestamps merely because they
/// carry more precision than Farhelm otherwise needs for capture windows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct EventTime {
    seconds: i64,
    fraction: String,
}

impl EventTime {
    /// Normalize one RFC 3339 value without throwing away fractional precision.
    /// Offsets are folded into epoch seconds and insignificant trailing zeroes
    /// are removed, so equivalent instants persist with the same ordering key.
    fn parse(value: &str) -> anyhow::Result<Self> {
        let seconds = parse_rfc3339(value).context("Grok timestamp is not RFC 3339")?;
        let rest = value
            .get(19..)
            .context("Grok timestamp is shorter than its validated prefix")?;
        let fraction = rest
            .strip_prefix('.')
            .map(|after_dot| {
                let count = after_dot.bytes().take_while(u8::is_ascii_digit).count();
                after_dot[..count].trim_end_matches('0').to_string()
            })
            .unwrap_or_default();
        Ok(Self { seconds, fraction })
    }

    /// Compare decimal fractions by padding the shorter one with zeroes.
    /// The stored form has no trailing zeroes, so equal instants have one
    /// representation even when the vendor spelled them differently.
    pub(crate) fn cmp(&self, other: &Self) -> Ordering {
        self.seconds.cmp(&other.seconds).then_with(|| {
            let left = self.fraction.as_bytes();
            let right = other.fraction.as_bytes();
            (0..left.len().max(right.len()))
                .map(|index| {
                    left.get(index)
                        .copied()
                        .unwrap_or(b'0')
                        .cmp(&right.get(index).copied().unwrap_or(b'0'))
                })
                .find(|ordering| *ordering != Ordering::Equal)
                .unwrap_or(Ordering::Equal)
        })
    }

    /// Refuse serialized keys that bypassed [`Self::parse`] and are not in
    /// its canonical decimal form.
    fn validate(&self) -> anyhow::Result<()> {
        ensure!(
            self.fraction.bytes().all(|byte| byte.is_ascii_digit())
                && !self.fraction.ends_with('0'),
            "Grok selection timestamp is not canonical"
        );
        Ok(())
    }
}

/// The only durable Grok conversation token Farhelm accepts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct GrokLocator {
    version: u8,
    pub(crate) session_id: String,
    pub(crate) session_file: Option<String>,
    pub(crate) selected_at: Option<EventTime>,
    pub(crate) resumable: bool,
}

impl GrokLocator {
    /// Build one untrusted hook report. `selected_at` is present only on
    /// `SessionStart`; enrichment reports inherit the durable key in the
    /// supervisor while it holds the shared capture claim.
    pub(crate) fn reported(
        session_id: String,
        session_file: Option<String>,
        selected_at: Option<&str>,
    ) -> anyhow::Result<Self> {
        let locator = Self {
            version: 1,
            session_id,
            session_file,
            selected_at: selected_at.map(EventTime::parse).transpose()?,
            resumable: false,
        };
        locator.validate()?;
        Ok(locator)
    }

    /// Decode a bounded Grok token without granting readiness.
    pub(crate) fn parse(value: &str) -> anyhow::Result<Self> {
        ensure!(
            value.len() <= MAX_LOCATOR_BYTES,
            "Grok locator exceeds its byte bound"
        );
        let json = value.strip_prefix(PREFIX).context("not a Grok locator")?;
        let locator: Self = serde_json::from_str(json).context("malformed Grok locator")?;
        locator.validate()?;
        Ok(locator)
    }

    /// Encode the complete selection/readiness value used by the shared CAS.
    pub(crate) fn encode(&self) -> anyhow::Result<String> {
        self.validate()?;
        let encoded = format!("{PREFIX}{}", serde_json::to_string(self)?);
        ensure!(
            encoded.len() <= MAX_LOCATOR_BYTES,
            "Grok locator exceeds its byte bound"
        );
        Ok(encoded)
    }

    /// Return the UUID only when the exact file pair currently verifies.
    pub(crate) fn resume_id(&self) -> Option<&str> {
        self.resumable.then_some(self.session_id.as_str())
    }

    /// Apply one normalized lifecycle report to the durable selection.
    ///
    /// The caller serializes this decision with the shared capture claim.
    /// Keeping the vendor rule here makes the persisted timestamp the only
    /// ordering state: restart, concurrency, and ordinary delivery all ask
    /// the same small transition instead of maintaining an in-memory ledger.
    pub(crate) fn merge_report(
        previous: Option<Self>,
        mut incoming: Self,
        event: &str,
    ) -> anyhow::Result<Self> {
        match event {
            "SessionStart" => {
                let reported_time = incoming
                    .selected_at
                    .as_ref()
                    .context("Grok SessionStart has no selection timestamp")?;
                let Some(previous) = previous else {
                    return Ok(incoming);
                };
                let current_time = previous
                    .selected_at
                    .as_ref()
                    .context("the durable Grok selection has no ordering timestamp")?;
                let ordering = reported_time.cmp(current_time);
                if previous.session_id != incoming.session_id {
                    ensure!(
                        ordering == Ordering::Greater,
                        "Grok refused an equal or older SessionStart for another conversation"
                    );
                    return Ok(incoming);
                }

                if ordering == Ordering::Less {
                    return Ok(previous);
                }
                if incoming.session_file.is_none() {
                    incoming.session_file = previous.session_file.clone();
                }
                if ordering == Ordering::Equal {
                    incoming.selected_at = previous.selected_at;
                    // An equal duplicate may fill a previously absent path,
                    // but it has no ordering authority to replace one that
                    // the durable selection already names.
                    if previous.session_file.is_some() {
                        incoming.session_file = previous.session_file;
                    }
                }
                Ok(incoming)
            }
            "UserPromptSubmit" | "Stop" => {
                ensure!(
                    incoming.selected_at.is_none(),
                    "Grok enrichment cannot carry a selection timestamp"
                );
                let previous =
                    previous.context("Grok enrichment arrived before a selected conversation")?;
                ensure!(
                    previous.session_id == incoming.session_id,
                    "Grok enrichment does not match the selected conversation"
                );
                incoming.selected_at = previous.selected_at;
                if incoming.session_file.is_none() {
                    incoming.session_file = previous.session_file;
                }
                Ok(incoming)
            }
            _ => anyhow::bail!("unsupported Grok lifecycle event"),
        }
    }

    /// Recompute readiness from only the reported `updates.jsonl` and its
    /// sibling `summary.json`.
    ///
    /// Every unavailable, unsafe, malformed, or mismatched shape is the same
    /// conservative answer: keep the selected UUID, but withdraw Resume.
    /// No error from vendor-owned files is allowed to redirect the locator or
    /// trigger a history search.
    pub(crate) async fn verify(&mut self) {
        self.resumable = self.verify_exact_files().await.unwrap_or(false);
    }

    /// Require the reported update file and its sibling summary to name the
    /// selected UUID. Missing files return false; malformed evidence is an
    /// error that [`Self::verify`] deliberately collapses to the same refusal.
    async fn verify_exact_files(&self) -> anyhow::Result<bool> {
        let Some(path) = self.session_file.as_deref() else {
            return Ok(false);
        };
        let path = Path::new(path);
        ensure!(
            path.file_name().and_then(|name| name.to_str()) == Some("updates.jsonl"),
            "Grok callback path is not updates.jsonl"
        );
        let Some(updates) = super::read_bounded_regular_file(path).await? else {
            return Ok(false);
        };
        let Some(first) = updates.lines().next() else {
            return Ok(false);
        };
        let update: Value = serde_json::from_str(first).context("malformed Grok update record")?;
        ensure!(
            update.get("method").and_then(Value::as_str) == Some("_x.ai/session/update")
                && update
                    .get("params")
                    .and_then(|params| params.get("sessionId"))
                    .and_then(Value::as_str)
                    == Some(self.session_id.as_str()),
            "unsupported Grok update record or id mismatch"
        );

        let summary_path = path
            .parent()
            .context("Grok callback path has no parent")?
            .join("summary.json");
        let Some(summary_text) = super::read_complete_bounded_regular_file(&summary_path).await?
        else {
            return Ok(false);
        };
        let summary: Value =
            serde_json::from_str(&summary_text).context("malformed Grok summary")?;
        ensure!(
            summary
                .get("info")
                .and_then(|info| info.get("id"))
                .and_then(Value::as_str)
                == Some(self.session_id.as_str()),
            "Grok summary id mismatch"
        );
        Ok(true)
    }

    /// Enforce the durable locator's bounds and readiness invariant.
    /// This runs after decoding as well as before encoding, so a hand-written
    /// token cannot claim Resume without the evidence fields verification uses.
    fn validate(&self) -> anyhow::Result<()> {
        ensure!(self.version == 1, "unsupported Grok locator version");
        ensure!(
            is_plausible_conversation_id(&self.session_id),
            "invalid Grok session id"
        );
        if let Some(path) = &self.session_file {
            ensure!(
                path.len() <= MAX_SESSION_PATH_BYTES
                    && Path::new(path).is_absolute()
                    && !path.chars().any(char::is_control)
                    && Path::new(path).file_name().and_then(|name| name.to_str())
                        == Some("updates.jsonl"),
                "Grok transcript path is not an absolute updates.jsonl path"
            );
        }
        if let Some(selected_at) = &self.selected_at {
            selected_at.validate()?;
        }
        ensure!(
            !self.resumable || (self.session_file.is_some() && self.selected_at.is_some()),
            "a resumable Grok locator requires exact files and a selection timestamp"
        );
        Ok(())
    }
}

/// Encode the vendor-specific evidence carried by one Grok hook report.
///
/// This is public only for the short-lived `farhelm internal hook` process;
/// the supervisor still decides whether the report may replace the durable
/// selection.
pub fn encode_report(
    session_id: String,
    session_file: Option<String>,
    selected_at: Option<&str>,
) -> anyhow::Result<String> {
    GrokLocator::reported(session_id, session_file, selected_at)?.encode()
}

/// Validate a callback timestamp without turning an enrichment event into a
/// new selection fence. Grok repeats `timestamp` on every subscribed hook;
/// only `SessionStart` persists it, but malformed values are still rejected
/// at the hook boundary instead of being silently ignored.
pub fn validate_event_time(value: &str) -> anyhow::Result<()> {
    EventTime::parse(value).map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// Equivalent RFC 3339 spellings collapse to one ordering key, while
    /// sub-second distinctions survive rather than being truncated.
    #[farhelm_testtrace::test]
    fn event_time_is_canonical_and_preserves_fractional_order() {
        let utc = EventTime::parse("2026-09-22T12:00:00.1200Z").expect("UTC timestamp");
        let offset =
            EventTime::parse("2026-09-22T14:00:00.12+02:00").expect("equivalent offset timestamp");
        let later = EventTime::parse("2026-09-22T12:00:00.1201Z").expect("later timestamp");
        assert_eq!(utc, offset);
        assert_eq!(utc.cmp(&later), Ordering::Less);
    }

    /// Both exact vendor files must have their supported complete shapes, and
    /// any discriminator or identity change withdraws readiness without
    /// changing the selected UUID.
    #[farhelm_testtrace::test]
    async fn exact_updates_and_sibling_summary_gate_resume() {
        let root = farhelm_teststate::tempdir().expect("fixture directory");
        let updates = root.path().join("updates.jsonl");
        fs::write(
            &updates,
            "{\"method\":\"_x.ai/session/update\",\"params\":{\"sessionId\":\"g-1\"}}\n",
        )
        .expect("write updates");
        fs::write(
            root.path().join("summary.json"),
            "{\"info\":{\"id\":\"g-1\"}}\n",
        )
        .expect("write summary");

        let mut locator = GrokLocator::reported(
            "g-1".to_string(),
            Some(updates.to_string_lossy().into_owned()),
            Some("2026-09-22T12:00:00.1Z"),
        )
        .expect("reported locator");
        locator.verify().await;
        assert_eq!(locator.resume_id(), Some("g-1"));

        fs::write(
            root.path().join("summary.json"),
            "{\"info\":{\"id\":\"other\"}}\n",
        )
        .expect("rewrite summary");
        locator.verify().await;
        assert_eq!(locator.resume_id(), None);
        assert_eq!(locator.session_id, "g-1");

        fs::write(
            root.path().join("summary.json"),
            "{\"info\":{\"id\":\"g-1\"}}\n",
        )
        .expect("restore matching summary");
        for unsupported in [
            "{\"params\":{\"sessionId\":\"g-1\"}}\n",
            "{\"method\":\"unknown\",\"params\":{\"sessionId\":\"g-1\"}}\n",
        ] {
            fs::write(&updates, unsupported).expect("write unsupported update record");
            locator.verify().await;
            assert_eq!(
                locator.resume_id(),
                None,
                "missing or unknown methods cannot establish Resume"
            );
        }

        fs::write(
            &updates,
            "{\"method\":\"_x.ai/session/update\",\"params\":{\"sessionId\":\"g-1\"}}\n",
        )
        .expect("restore supported update record");
        let mut oversized = "{\"info\":{\"id\":\"g-1\"}}".to_string();
        oversized.push_str(&" ".repeat(RECORD_PREFIX_BYTES - oversized.len()));
        oversized.push('x');
        fs::write(root.path().join("summary.json"), oversized)
            .expect("write oversized summary with a valid prefix");
        locator.verify().await;
        assert_eq!(
            locator.resume_id(),
            None,
            "a complete summary must fit the bound instead of only starting with valid JSON"
        );
    }

    /// The persisted timestamp alone orders replacement UUIDs, including
    /// concurrent delivery resolved in either arrival order and a delayed
    /// callback replayed after a supervisor reload.
    #[farhelm_testtrace::test]
    fn different_sessions_require_a_strictly_newer_selection_time() {
        let a = GrokLocator::reported(
            "session-a".to_string(),
            None,
            Some("2026-09-22T12:00:00.1Z"),
        )
        .expect("selection A");
        let b = GrokLocator::reported(
            "session-b".to_string(),
            None,
            Some("2026-09-22T12:00:00.2Z"),
        )
        .expect("selection B");

        let selected = GrokLocator::merge_report(Some(a.clone()), b.clone(), "SessionStart")
            .expect("newer B replaces A");
        assert_eq!(selected.session_id, "session-b");
        assert!(
            GrokLocator::merge_report(Some(selected.clone()), a, "SessionStart").is_err(),
            "a delayed A cannot restore the displaced UUID"
        );

        let selected = GrokLocator::merge_report(Some(b), selected, "SessionStart")
            .expect("an equal repeat of the selected UUID is idempotent");
        let equal_other = GrokLocator::reported(
            "session-c".to_string(),
            None,
            Some("2026-09-22T12:00:00.2Z"),
        )
        .expect("equal competing selection");
        assert!(
            GrokLocator::merge_report(Some(selected), equal_other, "SessionStart").is_err(),
            "equal timestamps cannot choose between different UUIDs"
        );
    }

    /// Repeated selection for one UUID never lowers its durable key. Equal
    /// reports may add a missing path but cannot replace an established one;
    /// a newer repeat may advance both timestamp and path.
    #[farhelm_testtrace::test]
    fn repeated_session_start_is_monotonic_for_one_uuid() {
        let previous = GrokLocator::reported(
            "session-a".to_string(),
            Some("/tmp/a/updates.jsonl".to_string()),
            Some("2026-09-22T12:00:00.2Z"),
        )
        .expect("durable selection");
        let older = GrokLocator::reported(
            "session-a".to_string(),
            Some("/tmp/old/updates.jsonl".to_string()),
            Some("2026-09-22T12:00:00.1Z"),
        )
        .expect("older repeat");
        assert_eq!(
            GrokLocator::merge_report(Some(previous.clone()), older, "SessionStart")
                .expect("older same-UUID reports are idempotent"),
            previous
        );

        let equal = GrokLocator::reported(
            "session-a".to_string(),
            Some("/tmp/equal/updates.jsonl".to_string()),
            Some("2026-09-22T12:00:00.200Z"),
        )
        .expect("equal repeat");
        let equal = GrokLocator::merge_report(Some(previous.clone()), equal, "SessionStart")
            .expect("equal same-UUID reports are idempotent");
        assert_eq!(equal.session_file, previous.session_file);
        assert_eq!(equal.selected_at, previous.selected_at);

        let newer = GrokLocator::reported(
            "session-a".to_string(),
            Some("/tmp/new/updates.jsonl".to_string()),
            Some("2026-09-22T12:00:00.3Z"),
        )
        .expect("newer repeat");
        let newer = GrokLocator::merge_report(Some(previous.clone()), newer, "SessionStart")
            .expect("newer same-UUID report advances");
        assert_eq!(
            newer.session_file.as_deref(),
            Some("/tmp/new/updates.jsonl")
        );
        assert_eq!(
            newer
                .selected_at
                .expect("advanced timestamp")
                .cmp(previous.selected_at.as_ref().expect("previous timestamp")),
            Ordering::Greater
        );
    }

    /// Enrichment can make only the selected UUID ready and inherits the
    /// selection's ordering fence, so a delayed event from A cannot replace B.
    #[farhelm_testtrace::test]
    fn enrichment_is_confined_to_the_selected_uuid() {
        let selected = GrokLocator::reported(
            "session-b".to_string(),
            None,
            Some("2026-09-22T12:00:00.2Z"),
        )
        .expect("selected B");
        let enrich_b = GrokLocator::reported(
            "session-b".to_string(),
            Some("/tmp/b/updates.jsonl".to_string()),
            None,
        )
        .expect("B enrichment");
        let enriched =
            GrokLocator::merge_report(Some(selected.clone()), enrich_b, "UserPromptSubmit")
                .expect("matching enrichment");
        assert_eq!(enriched.selected_at, selected.selected_at);
        assert_eq!(
            enriched.session_file.as_deref(),
            Some("/tmp/b/updates.jsonl")
        );

        let delayed_a = GrokLocator::reported(
            "session-a".to_string(),
            Some("/tmp/a/updates.jsonl".to_string()),
            None,
        )
        .expect("delayed A enrichment");
        assert!(
            GrokLocator::merge_report(Some(enriched), delayed_a, "Stop").is_err(),
            "an old UUID cannot enrich or replace the selected UUID"
        );
    }
}
