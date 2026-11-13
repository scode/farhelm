# failed Restart loses the live-tab preservation regression oracle

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: failed Restart loses the live-tab preservation regression oracle

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket does not approve any proposed lowering.

Contributing records: `g108/c3`.

### g108/c3

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 14 (finding, open):

# carry failed-Restart tab preservation into the host fixture

Reviewed commit: 647f0227b7f5b6c5fe443b2d5f0c4a20de0d5513

## TLDR
Restart must leave an existing shell tab and its work alone even when the replacement fails. This patch deletes the test that proved the tab's original shell survived failure. Retained tests separately check tab preservation after success and agent-terminal rollback after failure; those do not establish that the failed Restart preserves the tab.

## Details
Assigned patch lines 545–685 delete replacement_failures_preserve_tabs_and_clean_temporary_windows (old core.rs:24593 onward). It opened a real tab, captured its pane/window and live shell pid, forced replacement failures, and asserted the same live shell/window and retained tab after each failure. The BeforeCreation/BeforeMarking/BeforeConfirmation tmux fault stages and absence-of-temporary-window assertions belong to a retired mechanism and need not return. The ordinary failure contract (the pre-existing tab's shell/work stays alive and usable) remains SPEC.md:1960–1961, including host replacements.
Actual retained oracles: e2e/host_relaunches.rs:287–366, a_restart_of_a_host_agent_keeps_its_host_tabs, checks the same socket, host process, child pid/start identity, launch record and before/after echo, but only after successful Restart. terminal_tabs.rs:1070–1170 likewise checks a surviving attached tab and daemon after successful Restart. host_relaunches.rs:595–674 and :697–740 meaningfully exercise refused and unusable replacement hosts and validate old agent host/row/history restoration, but create no tab. Its failure-with-orderly-stop helper (:375–464) opens a tab, but intentionally closes its runtime during supervisor shutdown and checks the host pid, not the tab shell identity/liveness or post-failure usability. core/tests/host_terminals.rs:4246–4352 tests failed agent-runtime restoration after shutdown and has no tab. Searches of restart failure fixtures in host_relaunches, restart_under_concurrency, restart_with_resume and service tests found no ordinary failed-Restart fixture carrying the removed live-shell preservation oracle.
Tip check: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db's host_relaunches.rs retains the same three open_tab sites (success test and orderly-stop helper), and the refused/unusable replacement tests remain tab-less. The old tmux reuse/replacement mechanism itself is obsolete at tip; the failure tab-preservation contract is not. Extend a real host replacement-failure fixture to open a tab, pin its child pid/start identity, trigger the failure, require the same running child/record, and exchange a fresh marker afterward.
Current core.rs:13265–13355 and host_restart.rs:603–623 correctly retain shared tabs and target only the recorded old agent pane. That refutes an established destructive production regression but cannot settle this test-quality gap; c3's earlier proposed drop on that basis is superseded. Suggested bucket: other (lost regression coverage). Confidence: source-confirmed test/oracle comparison only; no execution/reproduction. proposed_drop: false. possible_cover: none established; parent-reported g103/f1 concerns unmarked-shell cleanup, distinct from preserving a tab across failed Restart. Could-be-highest: preserve original possibility of Restart killing surviving shell work, including through forged old-window identity, but no such shipped failure route is established; the verified issue is test coverage. Caveats: the host failure path predates this deletion, so do not claim this commit introduced a production bug there; it removes a behavioral regression oracle instead of carrying it into the supported mechanism. Pure forged-marker and recorded-pane guards address identity, not this failed-Restart liveness oracle. Successful Restart coverage is meaningful and is not missing.

Returned proposal (claims-070, event 14): **retain-verified**, suggested bucket **other**.

The deleted failure test checked the same live tab shell after failure. The named retained host success test is meaningful, while the named ordinary host replacement-failure tests create no tab. Correct production tab preservation does not replace that removed failure oracle. Earlier production-route dismissal is superseded.

Original stronger consequence: Restart killing surviving tab shell work, including a forged old-window cleanup route. Retained recorded-pane guards oppose that production route; the established finding is lost failure-path regression coverage.

Unresolved premise or limitation: No missing premise for the deleted oracle versus the named retained fixtures. No current destructive production route is established. This is not a claim that successful Restart tab coverage is absent or that the deletion introduced a host production bug.

Proposed basis: Retain the test-quality finding; independently adjudicate earlier production-route refutation and narrowing from possible loss of tab work.

Existing parent disposition (event 14): under-investigation. Record durably acknowledged; parent source/contract validation, cross-group synthesis, coverage/filter matching and any independent drop check remain pending. This preserves the claim and uncertainty without accepting, dropping or lowering it.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations: [g108 record](records/review-through-00de0b6c/groups/g108.json). Record ID: `c3`.

