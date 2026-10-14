# Cached launch failure can permanently override a completed Stop

Reviewed commit: e0c5ca1effae1e59604e6ca9c5c1df56cb00c96a

## TLDR

A session stopped during checkout preparation can continue showing a launch error instead of its recorded stopped
status, even after repeated refreshes.

## Details

Suggested bucket: `other`. Confidence: likely — the interleaving between observation and successful StopCompleted was
established by code inspection but not reproduced. Source: dedicated `gpt-6-astra` high adversarial review. Proposed
drop: none.

The new cache is populated before outcome arbitration at crates/farhelm-supervisor/src/service/ticker.rs:1207–1224.
Consider stopping a session during its initial clone or post-clone hook: the observer snapshots StopRequested, sees its
dead pane, and obtains incomplete-preparation evidence through status.rs:619–624,658–705. If StopCompleted commits
before the observer's batch, store.rs:474–480 correctly rejects SentinelError against the annotated exit. However,
hints.rs:97–108 clears launch_error only for committed Error, leaving the rejected classification cached.
status.rs:459–473 then lets that cache override the durable annotated exit indefinitely. Later observations skip launch
evidence for annotated exits (launch_artifacts.rs:122–130), so refreshes cannot repair it. Before this commit,
sentinel_hits belonged to one reply; the next reply recovered. Preserve the store's annotated-exit precedence when
publishing or reading the cache, including concurrent publication. Scope is persistent misclassification of the affected
session; no additional process or conversation loss established.

Possible cover: FILTER.md, 'Session status and history after a crash or a partly failed operation', could cover variants
involving failed operations. The described trigger uses a successful ordinary Stop during preparation, with no crash or
failed store operation, so full coverage is uncertain. The rare self-correcting-glitch filter does not cover a cache
that remains wrong until restart/relaunch.

Status at frozen endpoint: Persists at 4c9e9612d217be9f2ffe4d586c6bffaf8e1cd703: ticker.rs:1227–1264 still publishes
before arbitration; hints.rs:97–108 still clears only on Error; status.rs:459–473 still overrides the annotated exit. No
fixing commit identified.

Review coverage: full commit diff read: true. No reviewed-commit diff hunks remain unread. An initially aggregated tool
response was truncated; affected diff content was reopened in smaller calls. Unchanged surrounding modules were
inspected selectively, not exhaustively. The 13,089-line triage ledger was searched across headings and relevant terms
rather than read body-for-body; only the stated relevant bodies were read. Individual queue finding files were not
opened because the frozen index supplied no matching candidate. No Sol reports or working-copy queue additions were
read. The report-drain and cached-error race were not reproduced.

Runtime evidence: None, as requested by default for this review. Static historical/current-main comparison only. Applied
the supplied test-authoring checklist to changed tests and fixtures. No checkout files or scratch artifacts were
created, modified, or deleted; no fetch, rebase, or helper agents.
