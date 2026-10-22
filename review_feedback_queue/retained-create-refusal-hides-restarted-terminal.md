# Retained create refusal can hide a restarted live terminal

Reviewed commit: 9d266e28595e8c2c07b378174f35eedf84258a70

## TLDR

A failed create retry can replace a later Restart's live terminal entry with an error. A subsequent Delete can then stop
that running agent without the usual live-agent warning. The complete failure sequence is unverified, and the
specification accepts some compounded-fault variants; whether that acceptance covers every supported trigger is
unresolved.

## Details

The retained-refusal branch waits for lifecycle ownership after validation has failed, rereads the current session, then
settles the refusal without checking whether Restart advanced its generation
(`crates/farhelm-supervisor/src/service/core.rs:7760–7779`). The transaction changes any matching Launching or
Interrupted row to Error using only its ID and outcome (`store.rs:3610–3622`). Successful settlement republishes the row
with `terminal=None`, replacing its current in-memory entry (`core.rs:14828–14833,14871–14945`). A failed settlement
instead preserves an existing entry; that failure branch is not the reported overwrite.

An already-admitted Restart can hold lifecycle ownership after releasing directory admission. Retry validation can
therefore run before Restart finishes. The reported schedule needs retry's initial launch evidence to be absent, a
validation failure classified as retained refusal, and Restart to publish a new live terminal while its durable
ConfirmRunning write fails. That fresh-terminal failure branch leaves the row Launching and deliberately publishes the
terminal to keep the agent reachable (`core.rs:12021–12090`). After Restart releases lifecycle ownership, the waiting
refusal can change that newer row to Error and discard its terminal. Normal successful confirmation leaves Running,
which the refusal transaction rejects. The preserved-existing-session confirmation branch has different cleanup behavior
and is not this example.

The consequence extends beyond a misleading status. The conservative terminal-less guard protects only Launching
(`core.rs:4742–4748`); `still_alive_for_delete` returns no refusal for terminal-less Error (`12741–12759`). Delete
checks that entry under lifecycle ownership (`service/handlers.rs:1142–1182`), then teardown independently finds the
durable tmux session and can kill it (`service/teardown.rs:574–626`). This establishes the conditional source path to an
omitted live-work warning, not an observed kill.

Prerequisites include a Pending create reservation, initially absent retry evidence, Restart already admitted, failed
validation and confirmation followed by successful refusal settlement, and no observation repairing the outcome first. A
managed origin additionally requires Allocated, matching directory identity and Ready preparation. Ready and Pending can
coexist after nonfatal final create-settlement failure (`launch.rs:1854–1867`, `core.rs:9389–9405`); this does not prove
the later schedule. Row, token, checkout ownership, saved launch, capture and generation are not deleted by this
transaction. Reload can rediscover an Error-row pane (`core.rs:6056–6112`); permanent corruption is not established.

Possible cover is material: `SPEC.md:1333–1346` expressly accepts untracked processes and later kills without the usual
warning after an exact-moment crash or two independent rare faults. The independent-fault variant is covered. Counting
two failed database operations does not establish independent faults, however. The retained provenance read
(`core.rs:8570–8576`, `store.rs:3030–3041`) and confirmation (`store.rs:4632–4662`) share one connection.
`db.rs:1–28,119–145` describes temporary SQLite contention during supported handoff overlap and propagates ordinary
errors without poisoning later calls. A common resource failure is therefore a source-grounded possibility, but no
complete single-failure schedule is established. Earlier reads and relaunch admission must succeed, later reads and
settlement must recover, and a prior fault that left Pending may itself bring a concrete scenario within the exception.

A generation/reservation check before retained settlement and publication would prevent an old create refusal from
replacing a newer run. This branch is distinct from the successful takeover in
`create-retry-takeover-replaces-restarted-session.md` and from ordinary rollback fixed under
`TRIAGE_OUTCOMES.md:17612–17629`, `refused-create-retry-can-delete-restarted-session.md`.

Suggested bucket: `highest`, conditional on the unverified live-work consequence and unsettled coverage. Confidence:
source-established mechanism, likely conditional consequence; supported timing, common-cause failure sequence and
incidence are unverified. Provenance: A24-restart A24-f2, with independent static-check-22 retaining the ambiguity.
Proposed drop: none for the unresolved remainder; preserve the expressly covered independent-fault subcase. The
session-status filter does not itself cover bypassed confirmation. No exact Planned, BUGS, queue or ledger coverage
settles the remaining trigger question. No tests, builds, probes or reproductions were performed for this finding.
