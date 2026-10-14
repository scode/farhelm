# Restart withdrawal can discard a valid new Codex conversation report

Reviewed commit: f7f0b7d98b98cf31802e7707bad20cce4e7b9443

## TLDR

Restart can permanently lose the new conversation’s Resume target while refusing to resume an older, unavailable
conversation.

## Details

Suggested bucket: `highest`. Confidence: likely: the interleaving and permanent report settlement follow from static
control flow; concurrent real-Codex reproduction was not performed.. Source: dedicated `gpt-6.1-sol` high adversarial
review. Proposed drop: none.

At crates/farhelm-supervisor/src/service/core/vendor/codex.rs:12–44, the new Restart verifier changes the stored locator
without taking the capture claim used by report admission at :108–118. Interleaving: Restart snapshots admitted A; A’s
transcript is missing or inconsistent; a subsequent drain starts admitting valid SessionStart B and reads A; Restart
commits A with resumable=false; B’s complete-binding comparison at :230–238 fails. core.rs:14318–14328 converts that
failure to Conflict, and service/report_files.rs:242–256,511–515 settles and deletes B because the generation did not
change. Stop B cannot recover selection (:144–149). Further listings cannot recover it because this commit removes
transcript polling. The agent remains alive, but its current conversation cannot be resumed until another selecting
event arrives. The first parent serialized Codex verification with admission through core.rs:6290–6306. Serialize
withdrawal with admission, or retain/rejudge reports invalidated solely by this readiness change. Scope: one Codex
session; Grok has a similar pre-existing verifier and is not claimed as a newly introduced regression.

Possible cover: SPEC_impl.md:1840–1842 permits some Restart checks without the capture claim, but does not explicitly
accept losing a valid selection. Current SPEC.md:1040–1049 and TRIAGE_OUTCOMES.md heading
restart-resume-older-conversation-report-drain-fails.md accept specific crashes or two independent rare faults. This
sequence can use an ordinary transcript deletion plus concurrent admission on a healthy filesystem. No exact queue,
Planned, BUGS, or filter coverage found.

Status at frozen endpoint: Persists at 4c9e9612d217be9f2ffe4d586c6bffaf8e1cd703. verify_codex_resume remains unchanged
and unclaimed; admission still compares the complete prior binding (vendor/codex.rs:107–118,270–306;
store.rs:4972–5003), and same-generation Conflict still deletes the report (report_files.rs:251–265). No fixing commit
found.

Review coverage: full commit diff read: true. No commit diff content remains unread. Unchanged surrounding code outside
the listed ranges, unrelated queue-file bodies, and unrelated portions of the 13,089-line triage ledger were not read
exhaustively. Initial truncated composite output was not relied upon for missing code/diff content; those portions were
reread in bounded chunks. No independent drop reviewer was spawned, as prohibited; all covered highest candidates remain
explicitly presented for the parent’s independent review.

Runtime evidence: None, as requested by default. Findings rely on historical source, first-parent comparisons,
pinned-main inspection, and triage provenance. No checkout files were created, modified, or deleted.
