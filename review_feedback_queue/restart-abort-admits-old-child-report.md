# A failed Restart can select a queued child conversation

Reviewed commit: 5e1f1cfffe38f41e450ab70d440ca8fabac8be36

## TLDR

After a failed Restart restores the old terminal, a queued report from a nested Codex can replace the foreground
conversation Farhelm offers to resume. A later Restart may resume and append to that child's conversation.

## Details

Restart's final report drain (`crates/farhelm-supervisor/src/service/core.rs:10536`) precedes stopping the old process
tree (`:10747,10784`). A valid child's selecting report produced between those steps can remain queued. Relaunch then
advances the generation and clears the Codex pin (`store.rs:4170–4195`). A definitive pre-spawn failure, such as the tab
preflight failing at `service/core.rs:11262–11267`, restores the old pane without restoring generation or pin
(`store.rs:4224–4293`). Failure publication associates that pane with the advanced generation
(`service/core.rs:11195–11212`).

Report admission compares current row and entry generations, then assigns the current row generation to the report
(`service/report_files.rs:241–246,299–332`); it has no independent generation for when the report was emitted. The
retained dead pane can still provide the PID anchor (`service/core.rs:15131–15145`; `procs.rs:483–493`). With no pin,
initial Codex attribution can select the child (`service/core/vendor/codex.rs:148–151`). A separately launched native
child's valid `cli` or `exec` record can satisfy the exact-file/runtime checks (`agent_kind/codex.rs:204–291`). The
previous pin would reject a descendant even after the foreground process exited (`agent_kind/codex.rs:51–69`).

The CLI reporter records ancestry but no launch generation or process pin
(`crates/farhelm/src/hook.rs:329–350, 425–443`). Its independent trace corroborates the missing report-emission
generation. This requires an unmarked selecting report; a Stop enrichment alone cannot select an unrelated conversation.

Suggested bucket: **highest**, for wrong-conversation Resume. Confidence: **likely**; the static interaction is
corroborated across restart, report admission and harness integration, but the interleaving was not reproduced. It
requires an actually hook-enabled nested Codex, a valid selecting report and matching record/ancestry, the drain window,
and a definitive pre-spawn failure retaining the old pane. Default nested invocations receiving Farhelm's hook is not
claimed. Forged ancestry, PID reuse and an upgrade are not required by this schedule.

Possible cover: SPEC.md:1953–1961 and SPEC_impl.md:2343–2357 expressly require clearing the pin on abort. They do not
expressly accept queued old descendants acquiring foreground authority. This is a specification tension, not permission
to silently restore the pin. The ledger's `restart-drain-skips-newly-published-conversation.md:13091–13117` accepts
brief retention of the previous foreground conversation, not selection of a nested child. Its
`codex-ancestor-clear-withdraws-unverified-resume.md:13360–13376` decision requires an upgrade. The schedule does not
establish the two independent rare faults required by SPEC.md:1178–1190. No exact Planned, BUGS, queue or filter
coverage was found; the session-history filter excludes loss or replacement of a valid Resume offer.

Proposed drop: none. A focused check should queue a genuine child report after the drain, force definitive pre-spawn
failure, and verify the foreground binding is unchanged. Any remedy must keep old-run reports from gaining new-launch
selection authority while respecting the explicit pin-clear contract.

Provenance: `supervisor-agent-reports-1/f2`, `supervisor-harness-adapters-1/f6`, `supervisor-core-contracts-1/f2`,
`cli-agent-interface-1/f2` (latest `turn2.result.md`), and `supervisor-restart-1/f3` (`turn3.result.md`). Disposition
review 3 retained this exception to blanket stale-report-generation refutations. One underlying issue.
