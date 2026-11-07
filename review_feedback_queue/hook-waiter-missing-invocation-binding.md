# Repeated hook checks lack an invocation-bound completion result

Reviewed commit: 407a49435278844248d573083fdea6e23fd96c02

## TLDR

A repeated hook callback can be credited with an earlier callback's terminal result. Report-write counts prove that
reports ran and were accepted, but do not prove the second callback exited successfully and stayed silent. The exact
false-pass scheduling remains unverified after investigation.

## Details

Source coordinates from the original records: `crates/farhelm/tests/e2e/hook_identity.rs:479-501,1335-1348,1416-1440`;
`crates/farhelm/tests/e2e/codex_identity.rs:186-200,1364-1405`;
`crates/farhelm/tests/e2e/hook_identity.rs:479-501,1337,1416-1440`. References below use these paths' basenames unless
another path or revision is stated.

Recorder disposition: **unverified-after-investigation**. Triage priority: **other**. Source-confirmed conditional
behavior does not imply an observed incident. Reviewer suggestions, narrower refutations and unresolved premises below
remain part of the finding.

Reviewed change: f42178f9402ce22c60530360edc10d6721ef5b16, surviving at the pinned tip. hook_identity.rs:466-470
acknowledges grid retransmission beyond saved transcript offsets, yet write_hook_report_client:479-562 still waits for
identical HOOK-REPORTED:<id> and HOOK-STDOUT-EMPTY strings. Same-stream repeats occur at1416/1417 and1315/1337. In the
repeated Stop case the later identity/notice assertions already hold before that callback; Restart1350-1353 can stop the
emitter before its result is consumed. In the repeated SessionStart case,1428-1463 independently establishes two writes
and two admissions. Keep that positive evidence: it is not the fake agent's later child-exit/descriptor/silence verdict.
hook.rs:285-298,350-358 writes the report and records its outcome; fake_agent.rs:1474-1512 separately waits for child
exit, reads stdout/stderr and emits the completion/silence or dirty/exit result.

Grid source can resend old marker rows (grid_stream.rs:756-781;grid_output.rs:403-439), and harness.rs:159-200 appends
row versions without invocation identity. This establishes ambiguity in the oracle. The fixed1000x24 fixture does not
itself force a scroll, so no deterministic resend before the second callback result was established. The investigation
audited all report/stop/nested-report callers and their unique identifiers/gates. No claim is made that every helper
caller is affected; the controlled Codex cases retain separate gate/log-offset evidence, and many external callers use
one report or different IDs. Related compact helper uncertainty is g042-l7, with its exact source/caller record
retained. No runtime experiment ran. A retained frame/child-result trace or production-equivalent controlled resend for
these exact fixtures would settle the missing scheduling premise. Give each invocation a distinct terminal
completion/silence witness and consume it in the shared helper; do not replace it only with report-write counts.

Suggested bucket remains other: this is a test oracle concern with no established product/security/data-loss route.
Source-level waiter ambiguity is confirmed; exact-fixture false-negative occurrence is unverified after investigation.
proposed_drop false. No exact coverage was established in the recording sweep.

Review provenance: trailing-review run `7c7c7b55-12fc-4573-b10e-01d690187f50`; original records `g042/g042-l3`,
`g042/g042-l7`, `g041/g041-s2`, `g046/g042-l3`. The retained append-only histories preserve all earlier claims and
corrections. Reviewed source references name their own revisions; the inclusive reviewed tip is the hash above. This
review used source inspection only, with no product runtime experiments or live-install changes.
