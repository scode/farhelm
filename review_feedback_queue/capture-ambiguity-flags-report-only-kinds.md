# Capture marks report-only agents ambiguous

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

Running two Codex (or Goose, Pi, OMP, Grok) sessions in one folder logs false "can't capture" warnings.

## Details

Paths are relative to `crates/farhelm-supervisor/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0508-2b597e9-5788` (audit of Area 6, supervisor session state machine) as
`F16 / COR-AMBIGUOUS-REPORT-ONLY`, tagged **definite**. Anchors and title: `service/capture.rs:1115-1127`,
`store.rs:5022-5036`, `agent_kind/mod.rs:601` — The capture pass marks report-only agents ambiguous and logs that their
conversation can't be captured

Conversation capture has two sources. Some agent kinds report their own conversation id: Goose, Pi, Grok, OMP and Codex
are report-only, and their integration's `record_root` returns `None` (e.g. agent_kind/mod.rs:601). Claude additionally
supports a fallback scan of on-disk records. The scan has one safety rule. If two sessions of the same kind in the same
directory took their first input within overlapping windows (5 s before to 60 s after each), a record could belong to
either. Neither may then claim one, and both are marked "ambiguous".

In `capture_pass` (service/capture.rs:1115-1127), that overlap check runs before the check for whether the kind has a
record directory at all. So two Codex sessions started within about a minute of each other in the same repo, neither of
which has reported yet, are both put through `declare_ambiguous`. That logs a warning saying "neither will have its
conversation identity captured for this launch", sets an in-memory `Ambiguous` state, and writes `capture_ambiguous = 1`
to the database (store.rs:5022-5036). For these kinds the scan was never going to run, so the check guards nothing. The
usual result is that the agent's report arrives shortly after and overrides the flag; the store's docs say a report
dominates an ambiguity finding. The warning was false. For a report-only kind that never reports, such as Grok without
hooks, the flag just stays set without meaning anything.

This matters mainly for diagnostics. SPEC_impl treats this log line as the explanation owed to the user when capture
falls back, and for report-only kinds it is wrong and alarming. The trigger is common. The fix is to skip kinds with no
record root before the overlap check, and to leave them out of the `occupied` rival groups. This is safe because rivals
are grouped by (kind, directory), so a report-only session can only ever be rival to another session of the same
report-only kind, which never scans either.
