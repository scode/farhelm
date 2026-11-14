# Delete freshness test describes the opposite retained host state

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Source-confirmed review record: the new Delete-scope regression test tells maintainers its retained host is connected,
although it is suspended. That explanation hides why the fixture distinguishes safe handling of failed refreshes from
accidentally trusting stale host state.

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g028/l5`.

### g028/l5

Confidence: Source-confirmed by an existing parent decision; not runtime reproduced.

Latest reviewer record, event 13 (finding, open):

Reviewed commit: 2ef10815f12ed719ab6c51c6e3ac6fcd3f8bc1fa. TLDR: the new Delete-scope regression test tells maintainers
its retained host is connected, although it is suspended. That explanation hides why the fixture distinguishes safe
handling of failed refreshes from accidentally trusting stale host state. Details: the test
environment_delete_for_host_reads_the_current_host_state has a docstring at sandbox_sessions.rs:593-595 saying a failed
newer read falls back to asking 'even when the retained snapshot shows the host connected.' Its actual fixture is
HostPhase::Suspended at 598-605, recorded as the successful snapshot at 607; the first assertion expects Whole at
609-614. After read.record(Err) at 616 it expects Session at 618-619. HostsRead::record retains the same snapshot on
error (hosts.rs:1563-1569), and lookup prioritizes that error at 1612-1615, so the retained snapshot is still suspended,
not connected. The distinction matters to the test's specification: delete_scope at sandbox_sessions.rs:195-203 maps a
known suspended/non-connected host to Whole and a Failed lookup to Session. A connected retained host would yield
Session either way, so changing the fixture to match its prose would eliminate the assertion's sensitivity to using
stale state. Correct the docstring to name the suspended/disconnected retained host and explain why stale state must not
widen the proposed deletion scope. The production code and existing assertions are correct; this reports misleading test
documentation, not a production deletion bug or a currently ineffective assertion. Contract: the supplied test-authoring
review requires fixture premises and observables that distinguish competing mechanisms; root AGENTS.md, Docstrings on
tests, requires the docstring to explain why and what is tested. The product constraint is SPEC.md Sandbox sessions:
unanswered/unreachable deletion must not guess and whole-environment deletion requires its own warning; delete_scope's
own contract at 190-194 states that an uncertain read cannot widen the consent asked for. Suggested bucket: other (test
maintenance/documentation correctness). Confidence: confirmed by pinned source and retention/branch analysis, not
execution. Could-be-highest: no; no shipped destructive route is alleged, and no process/work-loss consequence is
inferred from this prose error. proposed_drop: false. possible_cover: none; parent owns coverage decisions, no
prohibited sources searched. Later-range check: git log for this file after the reviewed commit through inclusive
00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db has no entries; tip lines 593-627 retain the same docstring and suspended
fixture. This is new relative to the complete supplied neighbor records and local l1/l2/l3/c4 history; c4 refutes
production scope widening and does not refute this distinct test-documentation mismatch. No runtime tests, builds or
reproductions performed. All readily checkable premises are settled; no open lead remains under l5.

Returned proposal (claims-016, event 13): **retain-verified**, suggested bucket **other**.

The test docstring says the retained snapshot shows a connected host, but the fixture is Suspended and remains so after
the failed read. The distinction explains why the assertion detects unsafe use of retained state. Production behavior
and assertions are correct; the defect is the explanatory test contract.

Original stronger consequence: The initial lead and final finding both concern misleading test documentation. They
expressly preserve correct production behavior and assertions, and allege no destructive route or user-work loss.

Unresolved premise or limitation: None for the docstring mismatch. No product deletion failure or ineffective existing
assertion is alleged.

Proposed basis: Record as a narrow test-documentation correction: name the retained suspended/nonconnected state and
explain why trusting it after a failed refresh would widen the proposed scope.

Existing parent disposition (event 13): verified. Parent explicitly accepted named retain-verified proposal from
claims-016 after inspecting it. The test docstring says the retained snapshot shows a connected host, but the fixture is
Suspended and remains so after the failed read. The distinction explains why the assertion detects unsafe use of
retained state. Production behavior and assertions are correct; the defect is the explanatory test contract. Record as a
narrow test-documentation correction: name the retained suspended/nonconnected state and explain why trusting it after a
failed refresh would widen the proposed scope. Checked source/results:
["2ef10815f12ed719ab6c51c6e3ac6fcd3f8bc1fa:crates/farhelm-ui/src/sandbox_sessions.rs:593-619: the new docstring says
connected, creates HostPhase::Suspended, first expects Whole, records Err, then expects Session.","Same
revision:crates/farhelm-ui/src/hosts.rs:1563-1569,1612-1625: record(Err) keeps the successful snapshot and lookup
returns Failed before consulting it. sandbox_sessions.rs:190-203 maps known nonconnected state to Whole, but Failed or
connected state to Session. A connected retained fixture would fail to distinguish these mechanisms.","git diff
2ef10815..00de0b6 for sandbox_sessions.rs is empty. The error was introduced by the reviewed patch and remains at the
inclusive tip. No runtime observation is needed to establish the prose/fixture contradiction."] Current
coverage/results: ["SPEC.md:1531-1547 requires Delete not to guess when it cannot ask and requires separate
whole-environment warning; SPEC_impl.md:1069-1097 requires drawn-consent binding. Neither accepts a false test-fixture
explanation. Root AGENTS.md's Docstrings on tests supplies the relevant explanatory-documentation requirement.","TODO.md
Planned is empty. Searches of plans/queue for retained snapshot, Delete scope/freshness and the exact file found no
unlanded correction of this docstring; the related environment-agent-operations plan is landed per
plans/REPORTS.md:28.","BUGS.md, INDEX.md/matching queue text and active TRIAGE_OUTCOMES.md have no exact
connected-versus-suspended test-documentation item. The production-scope refutation g028/c4 is a different claim and
does not cover this one. No archive read.","FILTER.md does not match: this is a stable misleading explanation in
committed test source, not a rare triggering event with a self-correcting display or diagnostic. No other filter has the
required trigger."] Exact cited cover: [] Original stronger consequence and missing premise remain preserved: "The
initial lead and final finding both concern misleading test documentation. They expressly preserve correct production
behavior and assertions, and allege no destructive route or user-work loss."; "None for the docstring mismatch. No
product deletion failure or ineffective existing assertion is alleged.".

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g028 record](records/review-through-00de0b6c/groups/g028.json). Record ID: `l5`.
