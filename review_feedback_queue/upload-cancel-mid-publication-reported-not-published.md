# An upload cancelled mid-publication can be reported as "definitely not published"

Reviewed commit: 423770e10621b745ca94ab56e3c30595c874074a

## TLDR

When an attachment upload is cancelled at the exact moment the session's host is publishing it, Farhelm can tell the
user the file definitely was not saved on the host — even though the host's own answer says the outcome is unknown and a
complete file may remain. The user may re-upload (creating a duplicate) or believe the attach failed when it actually
succeeded. SPEC.md "Attachments" requires every failure response to distinguish "definitely unpublished" from
"publication outcome unknown"; this path collapses that distinction.

## Details

**other.** Source inspection; commissioner confirmed every link of the chain at the cited lines. Bucketed by the
established consequence: a misleading failure report that may cause a duplicate retry — no loss of user-owned work (a
published attachment exists and is retained until session deletion). The user-visible reachability is narrow and
unverified (below); a shown route to actual loss, or a common misleading outcome, would move the bucket up.

The chain, at the reviewed commit:

1. `crates/farhelm-supervisor/src/service/uploads.rs:1228-1262` — `commit_upload`'s bounded publication wait
   (`await_disk_stage`) can end `DiskStage::Cancelled`. The reply's message says "publication completion is unknown; a
   complete attachment may remain until session deletion, and retrying may create another copy" (or the session-gone
   variant), but its kind is `ErrorKind::InvalidRequest` (`NotFound` when `session_gone`). The abandoned blocking
   publication may still complete — `files.rs` `publish_no_clobber`'s own doc says "A caller that stops awaiting this
   operation cannot conclude that no file was published."
2. `crates/farhelm-helm/src/lib.rs:1860-1877` `supervisor_reply_status` maps `InvalidRequest` → 400 and `NotFound` →
   404; the relay answers the GUI with that status (`crates/farhelm-helm/src/uploads.rs` ~340-345 via
   `client.rs:4596-4637` `commit()`).
3. `crates/farhelm-helm-client/src/transfers.rs:134-145` `upload_outcome` classifies ANY 4xx as
   `UploadOutcome::NotPublished`; its doc assumes "A 4xx is the helm refusing the request itself … so nothing was
   published" — false for this supervisor-decided unknown-publication reply.

SPEC.md "Attachments" (~3717-3754): "A failure response must distinguish a definitely unpublished upload from one whose
publication outcome is unknown", and cancellation during final publication "may leave a complete attachment even though
the client receives no acknowledged path … Retrying may create an additional copy under a different name."

Unverified premise (reviewer's, kept): reachability to a still-waiting GUI. It requires the supervisor's read loop to
end mid-publication with its write path still live, and the helm to dispatch that reply before `fail_all` fails the
commit. Verified mitigations make it narrow: delete cannot signal mid-publication (the commit holds the session
lifecycle claim across publication; teardown runs under the same claim — `service/uploads.rs` ~1100-1270,
`service/teardown.rs:100-220,517`); a client `AbortUpload` mid-commit only occurs when the relay/guard is dropped (GUI
already gone), since `commit()` consumes the guard and the relay sends no abort while awaiting it; connection-teardown
signals are `tell_client:false` (`service/connection.rs:770-780`) and the reply only lands if the write path outlives
the read loop (`client.rs:1880-1990`). What would establish the route: a shipped cancellation path that reaches a commit
whose publication wait is still running while the GUI still reads the answer (e.g. a future caller of the cancel signal,
or a teardown ordering change). The kind/message contradiction is a contract defect regardless: any path that exercises
it misreports.

Verify: drive a commit whose publication wait receives a cancellation signal while the helm is still awaiting the reply,
and observe the HTTP status vs the "unknown" message; or inspect that `supervisor_reply_status(InvalidRequest) = 400`
feeds `upload_outcome`'s 4xx → NotPublished arm. Fix shape: answer the mid-publication-cancellation commit with a
5xx-mapped kind (`ErrorKind::Internal`) so the GUI reads Unknown, consistent with the message; or carry the unknown
outcome in a dedicated field/header the GUI keys on rather than inferring it from status alone. (The
session-gone/NotFound arm deserves the same treatment: session deletion can partially fail and leave attachments behind,
per SPEC.md "Attachments".)

Reviewer metadata (area 1 chunk C, file transfers): suggested bucket: other; confidence: likely — semantic mismatch
confirmed in code, reachability premise unverified; proposed_drop: none; possible_cover: none (SPEC.md "Attachments"
states the requirement, not an acceptance). Commissioner validation: chain confirmed at all cited lines; coverage
searched (queue, TODO.md, BUGS.md, TRIAGE_OUTCOMES.md, plans/queue) — no match. FILTER.md rare-glitch filter considered:
unclear match (the consequence is a misleading outcome report, and the exclusion list names only the reverse "success
reported for an operation that failed"), so per "when it is unclear whether a finding matches, it does not match" —
recorded.
