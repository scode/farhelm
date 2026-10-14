# Restart can skip a conversation report published during its drain

Reviewed commit: 1ab6c205806ff5b82f1065d2efdfd9fbc234f728

## TLDR

Restart can resume the previous conversation after a conversation switch, then permanently discard the report
identifying the new conversation.

## Details

Suggested bucket: `highest`. Confidence: likely; static control flow establishes the consequence, but the live
hook-publication/Restart interleaving was not reproduced. Source: dedicated `gpt-6.1-sol` high adversarial review.
Proposed drop: none.

At crates/farhelm-supervisor/src/service/report_files.rs:440–468, has_report is determined from the initial directory
listing. When that listing contains only a hook's temporary file, the subsequent is_stale await at :460 can overlap the
hook's successful publication of latest.json. The pass nevertheless returns without checking that slot. Restart uses
this same drain through capture_pass(true) (service/core.rs:10032–10042), then reads the previously stored conversation
(:10061–10067), stops the old agent (:10246), and opens the replacement generation (:10394–10412). A later drain cannot
recover the skipped report: its ancestry names the previous pane process and fails the current-launch anchor
(report_files.rs:299–313). The first parent instead attempts all slot renames after temporary-file processing
(:452–462), consuming a report published during that processing. This introduces an additional loss window affecting
conversation switches, including Claude and Codex. Preserve the final slot checks for decision-time Restart drains;
periodic idle passes can retain the shortcut.

Possible cover: TRIAGE_OUTCOMES.md heading restart-resume-older-conversation-report-drain-fails.md accepts specific
crash or coincident-fault outcomes. Current SPEC.md:1040–1050 limits that acceptance to those conditions; this
interleaving requires neither. FILTER.md's sub-second human-race filter explicitly excludes lost user work. The reviewed
code comment at report_files.rs:420–422 describes deferral, but does not establish specification acceptance of resuming
the wrong conversation.

Status at frozen endpoint: Still possible at pinned main 4c9e9612d217be9f2ffe4d586c6bffaf8e1cd703:
report_files.rs:454–482 retains the shortcut, and core.rs:10237–10260 retains decision-time draining followed by the
stored snapshot. Commit 379b215ea30df2ac0a3c29f34a71858aed131578 adds prompt watcher pickup, reducing exposure, but does
not synchronize pending watcher work with Restart's snapshot/generation transition. Watcher batching, its 100 ms drain
gap, or its supported disabled fallback can leave the report pending. No fixing commit identified.

Review coverage: full commit diff read: true. No commit-diff hunks unread. Unrelated portions of the large
TRIAGE_OUTCOMES.md ledger, complete historical modules outside the listed ranges, unrelated queue-file bodies, and most
specification sections were not read. Initial combined tool output truncated; complete changed diffs and relevant
guidance/excerpts were subsequently reopened in bounded commands. The complete later historical-to-main diff was not
reviewed; current-main checks focused on the reported mechanism and its callers.

Runtime evidence: None; static review only. Applied the supplied test-authoring checklist to changed tests and
assertions. No checkout files created, modified, or deleted; no helper agents spawned.
