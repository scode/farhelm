# Pi getter/serialization exceptions escaping the silent-hook boundary

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

A failed Pi callback could leave Resume pointing at an earlier conversation.

## Details

F327 — **possible** — `crates/farhelm-supervisor/assets/pi-conversation-v1.ts:18` — Pi getter/serialization exceptions
escaping the silent-hook boundary

Conversation getters and serialization run before the callback's silent-error boundary. If they throw after a switch
from a captured conversation, the new report is omitted while the old file remains valid and resumable. Throwing vendor
behavior was not observed. Include all callback preparation within the intended failure boundary and investigate how
failed switch reporting preserves an accurate Resume target, rather than treating it as only a missing-report warning.

## Evidence and triage context

- pi-conversation-v1.ts:18–25 executes before the try at line 31. service/core.rs:6397–6414 verifies only the stored
  locator's file and ID; it does not discover the vendor's current conversation. agent_kind/mod.rs:1890–1894 resumes
  that stored file. FILTER.md, Rare edge cases in harnesses without first-class support, expressly excludes resuming the
  wrong conversation. Actual throwing vendor behavior remains unverified.
- pi-conversation-v1.ts:18–25 can throw before report enqueueing and the later try. service/core.rs:6397–6414 verifies
  the stored Pi locator, and agent_kind/mod.rs:1890–1894 resumes its file. Thus an unreported switch can leave the prior
  valid target standing. FILTER.md's non-first-class filter expressly excludes resuming the wrong conversation. The
  throwing vendor behavior is unobserved, so this remains possible.

Retained confidence: **possible**. Suggested bucket: **other**.

Possible cover:

- None identified in completed collection evidence.

Caveats:

- No additional caveat recorded.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `sr_data:p1:C2`, `sr_edges:p1:C7`.

- `sr_data:p1:C2`: confidence as filed: not separately tagged in candidate list; suggested bucket as filed: not
  separately tagged in candidate list.
- `sr_edges:p1:C7`: confidence as filed: not separately tagged in candidate list; suggested bucket as filed: not
  separately tagged in candidate list.
