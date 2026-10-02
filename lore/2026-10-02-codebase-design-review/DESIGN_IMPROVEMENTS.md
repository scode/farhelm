# Design improvements

NOTE: Historical review recorded 2026-10-02; not maintained. Analysis used commit
`959bc3565b00b804f4d842d2fb2db6feacf4582e`.

Completed static review, 2026-10-02. This is an uncommitted proposal log. It does not authorize implementation or imply
that every proposed abstraction is worth building. No runtime reproduction, exploit validation, or dependency advisory
scan was performed.

The review covers first-party code, inline and external tests, scripts, workflows, and configuration at
`959bc3565b00b804f4d842d2fb2db6feacf4582e`. Specs and current documentation provide the contracts. Generated dependency
locks and bundles, vendored libraries, binary assets, static fixtures, and archived plans are inventoried separately;
their consumers and build paths remain in scope.

## Review method and coverage

The inventory contains 760 tracked files. There are 425 first-party code/configuration files, divided into 61 bounded
review units covering 467,515 lines. Large files are divided into explicit line ranges. Reviewers must read every
assigned range, record completion per unit, and identify any unfinished ranges. All 61 units are complete. Their exact
ranges reconcile with the inventory: every first-party line is assigned once, with no gaps or overlapping ownership. The
file-level ledger below records those ranges; other tracked files have explicit inventory classifications.

Reviewers read assigned ranges sequentially, including tests, then traced relevant callers and specification contracts.
Each unit produced a responsibility map, findings or an explicit no-material-finding result, and a completion record.
Unfinished work was reassigned without crediting partial inspection. A second challenge pass checked the first 21
proposals and both flake-sweep proposals against source and accepted scope; some UI proposals were consistency checks by
their original reviewer, not independent second reviews. The remaining proposals received coordinator reconciliation.

The final audit verified that all 760 original tracked-file hashes and HEAD remained unchanged. Validation for this
report consists of coverage reconciliation, source-reference checks and Markdown formatting. No code, tests, queue,
plans, specifications or TODO entries were changed; no commits were made.

## Priority and overlap

The highest-consequence findings mostly overlap existing work. Fix the narrow known defects first, then retain a shared
type or owner only where it removes repeated correctness obligations. "New risk" below means a source-supported
maintenance risk, not a reproduced failure. No new exploitable security bypass was established.

| ID  | Area                                    | Priority      | Relationship to existing work                                    |
| --- | --------------------------------------- | ------------- | ---------------------------------------------------------------- |
| D01 | Accepted session mutations              | High          | Known cancellation defects; reuse existing owner                 |
| D02 | Launch validation and representation    | High / medium | Known validation defects and existing design TODO                |
| D03 | Host connection transitions             | Medium        | Planned retarget fix; additional limited shutdown risk           |
| D04 | In-memory session metadata              | Medium        | New risk of reply-field omissions                                |
| D05 | Launch retry writes                     | Medium        | New risk of losing future session metadata                       |
| D06 | Host setup UI state                     | Medium        | New state-coherence proposal and static diagnostic risk          |
| D07 | Test resource ownership                 | Medium        | New failure-path evidence; consolidate existing helpers          |
| D08 | Launch rollback cleanup                 | High / medium | Known scope-refusal defect; conditional shared policy            |
| D09 | Admission, mutation and reply lifetimes | Medium        | Existing reply/admission defects and additional ownership drift  |
| D10 | Destructive consent                     | High          | Planned defects; narrow immutable consent capture                |
| D11 | Outcomes across sign-in                 | High          | Two queued outcome-loss defects                                  |
| D12 | Terminal lifecycle and authority        | Medium        | Planned teardown/takeover defects                                |
| D13 | Upload outcome lifetime                 | Medium        | Planned remount defect                                           |
| D14 | Process-scope cleanup evidence          | High / medium | Queued whole-session cleanup defect                              |
| D15 | Attachment retirement                   | Medium        | New future-omission risk; no current teardown failure found      |
| D16 | Checked repository identity             | Lower         | New future-validation-omission risk; ingress currently protected |
| D17 | Launch draft transitions                | Medium        | Existing cleanup TODO plus static hidden-state defect            |
| D18 | Platform-aware test evidence            | Medium        | New false-positive and false-failure mechanisms                  |
| D19 | UI operation permits                    | Medium        | Planned claim leaks; strengthen owner-only release               |
| D20 | Menu action identity                    | Lower         | New keyboard-navigation defect by inspection                     |
| D21 | Browser fixture restoration             | Medium        | New failure-path gaps; acknowledged stack scheduling debt        |
| D22 | Platform process evidence               | Medium        | New contradictory Darwin assumptions; reproduce before fixing    |
| D23 | Host setup operation identity           | Medium        | Planned busy/probe/cancellation defects                          |
| D24 | Shell cancellation witnesses            | Medium        | New test-proof gaps                                              |
| D25 | Flake-sweep admission lease             | Medium        | New startup race; bounded orphan-overlap improvement             |
| D26 | Flake-sweep suite inventory             | Medium        | New coverage omissions                                           |
| D27 | Complete terminal fixture records       | Medium        | New test-oracle chunk-boundary defect                            |

## Proposals

These proposals have been reconciled across subsystems and checked for duplicate known work. Source references name the
reviewed snapshot. A claim of a defect here means the control flow or representation supports it by inspection; it does
not mean the failure was reproduced. Proposed validation belongs to future implementation work.

### D01. Give accepted mutations one execution owner

When a user starts Create, Replace or another state-changing action, the helm must finish the accepted work even if the
client disconnects. That rule is currently an opt-in helper at individual HTTP routes. Replace spans a create and a
delete; Create spans remote admission and local history/default updates; marking a session seen spans a database commit
and a notification. A cancelled waiter can abandon the remaining effects.

Start by putting each accepted sequence under the existing `run_owned` helper, keeping reply rendering outside it.
Introduce a service boundary only if it removes repeated ownership decisions across real callers. This implements the
existing specification without adding durable jobs or requiring work to survive process exit. Evidence:
`crates/farhelm-helm/src/sessions.rs:1535`, `:1863`, `:2950`, `:3384`, `:3478`, and the existing helper in
`crates/farhelm-helm/src/lib.rs:1858`. This consolidates already queued/planned cancellation fixes; it is not a newly
discovered set of bugs. Priority: high.

### D02. Normalize and validate launch state once

Create, Clone, Replace and Restart currently pass related invocation, integration, resume and provenance fields through
different representations. Some validation remains a caller checklist: Restart with omits checks that Create performs.
Raw source projection can also discard integration/resume information that the current session record already carries.

Use a validated launch bundle, with a separate explicit provenance value controlling which GUI defaults/history may be
updated. Per-agent option rewriting should share a bounded token grammar where injection and Resume currently walk the
same arguments independently. Keep custom-template escape hatches and intentionally partial agent integrations.
Evidence: `crates/farhelm-helm/src/sessions.rs:1935`, `:2036`, `:2103`, and
`crates/farhelm-supervisor/src/agent_kind/mod.rs:2174`, `:2671`, `:2722`, `:2755`; Create/Restart callsites in
`crates/farhelm-supervisor/src/service/core.rs:7110` and `:10048`. This supplies implementation direction for the
existing launch-simplification TODO and queued validation/selector fixes. Pi's classifier at
`crates/farhelm-supervisor/src/agent_kind/pi.rs:43` and selector stripping at
`crates/farhelm-supervisor/src/agent_kind/mod.rs:1378` illustrate the grammar split; Goose has the same split at
`crates/farhelm-supervisor/src/agent_kind/goose.rs:118` and `mod.rs:1358`. OMP already shares its bounded grammar. This
does not call for a universal vendor parser. The larger representation change remains maintainer-led design work, as the
existing TODO requires; custom resume-template YOLO validation is an accepted exception. Priority: high for known
validation defects, medium for the larger representation cleanup.

### D03. Make host connection publication and retirement one transition

Several manager and actor paths separately withdraw connections, invalidate their generation, clear derived state,
retire the transport, and notify readers. Publication does not atomically check the generation it intends to publish;
shutdown aborts actors without explicitly retiring retained client handles.

A guarded publication helper and common withdrawal helper should validate the expected generation and return withdrawn
clients for retirement outside the state lock. Use it for retarget, failure, removal, revival and shutdown. Evidence:
`crates/farhelm-helm/src/manager.rs:1340`, `:2335`, `:2554`, `:2644`, `:2695`, `:3541`, `:3571`, `:4031`. Existing
coherent snapshots and identity transactions stay. The retarget race is already planned; shutdown lifetime is an
additional static observation whose practical impact is limited by whole-process exit. Priority: medium.

### D04. Keep in-memory session metadata distinct from a reply-ready view

The supervisor keeps a complete wire-shaped session object in its in-memory entry, containing intentional placeholders,
then repairs its status, timestamps, tabs, restart offer and other fields while building replies. A future field can
compile while escaping with an obsolete or placeholder value.

Keep in-memory metadata in a narrower type and explicitly construct the public reply from metadata, current observations
and checkout facts. Preserve the existing generation and runtime-cell boundaries. Evidence:
`crates/farhelm-supervisor/src/service/core.rs:3566`, `:5915`, and
`crates/farhelm-supervisor/src/service/status.rs:412`. Existing reply helpers are a useful starting point. No current
incorrect reply was established. Priority: medium.

### D05. Update a retried launch without recreating the session

An unconfirmed-create retry deletes and reinserts the full session row, first preserving a growing list of session
identity and user metadata. Existing tests already protect renames, ordering, timestamps and checkout provenance, but
each new session-lifetime field must be remembered in a launch-only retry path.

Use a launch-attempt update with narrower inputs and return one committed snapshot. Preserve the reservation predicate,
generation rules, absent-row policy and checkout transaction. Evidence: `crates/farhelm-supervisor/src/store.rs:3660`,
`:3838`, `:3902`, `:11897`. This is a future-regression risk, not a claim that the currently tested fields are lost.
Priority: medium.

### D06. Let one provisioning state value enforce UI transitions

The hosts panel represents one setup/update lifecycle using many independently writable signals: intent, plan,
submission, run identity, acceptance waiting and separate diagnostic families. Correctness includes renderer-specific
rules about borrowing and which writes retrigger effects. Old ADD diagnostics can coexist with later UPDATE diagnostics
while rendering assumes only one family has news.

Start with a renderer-independent state value and transition methods, preserving the distinct, potentially overlapping
lifetimes of planning and accepted submissions. Give diagnostics an operation/attempt identity; a generic event/effect
framework is unnecessary. Evidence: `crates/farhelm-ui/src/provisioning.rs:794`, `:1232`, `:1509`, `:1847`, `:1932`,
`:2010`, `:2234`. Existing epochs, run IDs, target bindings and guards remain valuable. The diagnostic example is
static, not browser reproduced. Priority: medium.

### D07. Give test fixtures explicit ownership through failure and shutdown

Browser tests manually hold and release requests with inconsistent abort/drain behavior. Releasing a held Create in
cleanup and immediately listing sessions can race the newly admitted Create, missing its resource. Rust integration
tests similarly have several independently implemented serving-task guards, including guards armed only after a
readiness await.

Consolidate the existing good patterns into small resource owners: browser route gates with receipt, abort/release and
drain; serving-supervisor fixtures with cleanup armed before startup and awaited shutdown. Evidence:
`e2e/tests/sidebar.spec.ts:6956`, `e2e/tests/sort.spec.ts:822`, `crates/farhelm/tests/e2e/session_lifecycle.rs:264`, and
`crates/farhelm/tests/e2e/hook_identity.rs:97`. Process isolation limits current test leakage but does not repair
misleading failure evidence or restart premises. Priority: medium.

The same consolidation should finish adoption of the existing bounded tmux fixture owner and child runner. Older test
guards still use unbounded `status()` or separate kill/poll/wait loops, losing the shared owner's diagnostics and
verified fallback. Evidence: `crates/farhelm-teststate/src/lib.rs:1019`, `:496`,
`crates/farhelm-supervisor/src/service/core.rs:14694`, contrasted with `crates/farhelm-teststate/src/tmux/guard.rs:24`.
Preserve independent cleanup for tests of the cleanup mechanism itself, the stale sweep's deliberate disk-first policy,
and the runner's direct-child-only guarantee.

The desktop smoke has the same ownership drift: its main cleanup uses an unbounded tmux call and signals a recorded
desktop PID even after `close_app_and_wait` has reaped it. Retire signal authority at reaping and keep private state
until bounded cleanup completes (`scripts/desktop-smoke.sh:250`, `:268`, `:1018`). PID reuse was not reproduced. The
newer answering-supervisor leg already demonstrates bounded tmux cleanup and retired PID records (`:190`, `:211`). Keep
graceful-close assertions distinct from fallback cleanup so the fallback cannot make a broken lifecycle test pass.

### D08. Make launch rollback preserve unconfirmed cleanup

The already queued Create rollback defect can downgrade unconfirmed process-scope cleanup to a warning before retiring
the launch record. Other branches distinguish confirmed absence from uncertainty, but callers independently decide what
successful cleanup means. Losing the last record can remove the handle needed to clean up surviving work.

Apply the existing refusal policy first. If consolidating the affected branches reduces their exposed obligations, use a
shared cleanup result that retains terminal, portable-sweep and required-scope outcomes, and permits record retirement
only when the outcomes required for that launch are confirmed. Never-started launches and retained fresh checkouts keep
their distinct policies. Evidence: `crates/farhelm-supervisor/src/service/core.rs:9305`, `:9532`, `:9581`, `:11200`, and
`:11262`.

The confirmation-write failure branch also records a sweep error yet allows successful tmux deletion to retire the row.
That divergence is established by inspection, but no supported non-filesystem trigger was established for reaching its
initial database error. It is supporting maintenance evidence, not another confirmed high-priority defect. Do not add an
elaborate evidence framework solely for excluded local filesystem failures. Prefer the existing
`ScopeKillFailure::Refuse` and a shared helper's `Result` before introducing another evidence type. Priority: high for
the known scope refusal defect; medium and conditional for shared cleanup-result design.

### D09. Separate mutation locks from reply delivery, while retaining admission ownership

Supervisor handlers assemble different versions of the same execution pattern. Stop keeps the session lifecycle lock
while awaiting a bounded reply queue; Delete releases domain locks before sending. Tab mutations correctly outlive their
connection, but their admission permit belongs to the cancellable outer waiter. These differences couple operation
availability and resource accounting to client reply behavior.

Use the existing Delete/Rename pattern as a small shared operation adapter: mutation-owned admission, domain guards
released at mutation completion, and a separate reply phase carrying the result and permit. Preserve bounded admission
and correlated failures. Evidence: `crates/farhelm-supervisor/src/service/handlers.rs:1165`, `:1444`, `:1478`, `:2435`,
`:2517`, and `crates/farhelm-supervisor/src/service/connection.rs:1019`. Panic response drift and connection-reader
admission stalls are already recorded elsewhere; this proposal supplies a common home for the ownership rules. Priority:
medium.

Upload final responses have the same split obligation: cancellation uses a single `try_send`, and transfer cleanup can
discard the last owner of a result while the connection remains usable. Separate prompt filesystem cleanup from a
bounded connection-owned obligation to deliver the final response or terminate the connection. Evidence:
`crates/farhelm-supervisor/src/service/uploads.rs:315`, `:748`, `:1336`; already recorded in
`review_feedback_queue/upload-cancellation-drops-final-reply.md`. Preserve Delete's independence from a stalled writer
and the accepted uncertainty of interrupted publication.

### D10. Carry destructive consent unchanged through retries and permission prompts

A prompt's wording and its eventual request do not always share one immutable value. Some Restart paths send broader
consent than the rendered prompt, and Replace can recompute source liveness after a separate YOLO question. A source
restarted meanwhile can therefore receive destructive authority the user did not grant in that prompt.

Capture a small typed consent value alongside the displayed consequences, and carry it with source identity and the
pending action through retries. Only another destructive prompt may widen it. Keep Restart consent distinct from source
deletion's accepted levels. Evidence: `crates/farhelm-ui/src/session_view.rs:1299`, `:1751`, `:1948`, `:1971`, `:2129`,
`:2198`, `:2230`. Existing server preconditions, prompt consumption and operation guards remain. These defects are
already planned; the proposal is a bounded implementation direction. Honor the maintainer's low-complexity requirement:
prefer the narrow capture fix if a shared type grows into a framework. Priority: high.

### D11. Keep action outcomes above authentication-dependent UI lifetimes

Signing in again may discard forms, but must not silently discard the outcome of an action already sent. Both desktop
failure recovery and browser token prompts can unmount the component that owns an action's waiter and its notice. Server
ownership of the work does not preserve the client's account of what happened.

A small stable pending-action/outcome owner above both sign-in gates should record confirmed results or explicit
unknown-outcome notices. A cancellation guard can report lost observation without preserving forms, adding durable jobs,
or automatically retrying mutations. Evidence: `crates/farhelm-ui/src/auth.rs:225`, `crates/farhelm-ui/src/lib.rs:1270`,
`crates/farhelm-ui/src/list/view.rs:1708`, and retry coverage in `crates/farhelm-ui/src/api.rs:3839`. Normal desktop
recovery already preserves the tree; this covers the remaining replacement paths. The two concrete outcome-loss defects
are already queued. Priority: high.

### D12. Make terminal resources and attach authority one explicit lifecycle

The terminal UI tracks live mounts, pending mounts, recovery controllers and retained screens in four maps. Transitions
manually enumerate the maps they know about. Departure misses retained screens; takeover misses some unproved
attachments; omitting a mount argument grants displacing authority even on some unattended paths.

Use a per-terminal lifecycle record and require an explicit attach reason, distinguishing deliberate reclaim from
recovery or discovery. Preserve overlapping live attempts and retained screens where recovery needs both. Evidence:
`crates/farhelm-ui/assets/terminal.js:1040`, `:1121`, `:1145`, `:2520`, `:2606`, `:2625`, `:3336`. The stranded-screen
and unintended-takeover defects are already planned. The lifecycle-record consolidation is optional; it must not delay
their narrow fixes. Explicit attach authority is useful independently. Preserve the existing byte-flow and reconnect
design. Priority: medium.

### D13. Own attachment outcomes for as long as their session pane

Upload state belongs to a short-lived terminal mount even though its status display belongs to the pane. Reconnect tears
down the mount, aborts transfers and clears displayed paths/errors; late completion can return without reporting.

Keep an attachment outcome record at pane/session lifetime, independent of mount listeners and the current input sink.
Transfers may still be cancelled during remount, but cancellation must settle visibly. A newly live sink does not
authorize inserting a late path into a restarted agent: preserve the operation's original attachment authority, or
report the landed path without inserting it. Evidence: `crates/farhelm-ui/assets/terminal.js:1542`, `:2176`, `:2300`,
`:5090`. This is already planned upload-remount work. It does not require resumable uploads, persistence beyond the
view, or delaying Quit. Priority: medium.

### D14. Preserve cleanup scope evidence when combining agent and tab cleanup

Closing one tab distinguishes recorded, possible and derived process-scope ownership. Whole-session Delete rebuilds tab
scope names as merely derived and bases a manager reprobe only on the agent's scope. The same tab can therefore receive
weaker cleanup when deleting its parent session.

Extend the existing scope abstraction with a shared tab-evidence constructor and provenance-preserving merge. Use the
collected evidence to choose the existing reprobe policy before optional enumeration. Evidence:
`crates/farhelm-supervisor/src/service/core.rs:12872`, `crates/farhelm-supervisor/src/service/teardown.rs:364`, `:385`,
and `crates/farhelm-supervisor/src/service/sweep.rs:1206`. This is the design cause of the already queued scoped-tab
Delete failure. Preserve the distinction between proven and inferred scopes; do not require scopes on unsupported hosts.
Priority: high for the known cleanup defect, medium for shared construction.

### D15. Make terminal retirement consume and own all cleanup resources

Six supervisor paths manually remove attachments, publish a teardown barrier, signal forwarders, retain sink leases,
await joins and record failure. The current callers inspected follow the protocol, but a missing step in a future change
can reopen the unsafe output-client teardown problems those barriers were added to prevent.

Return a consuming retirement value from guarded removal. It should own the forwarder and sink together, preserve
signal-all-before-join for batches, and retain cleanup ownership if its waiter disappears. Keep operation-specific
notices and refusal policy outside it. Evidence: `crates/farhelm-supervisor/src/service/terminals.rs:1512`, `:1598`,
`teardown.rs:465`, `:1105`, `handlers.rs:1905`, `connection.rs:638`, `core.rs:11447`, `:12583` in the same service
directory. No new current failure is established. Prototype the boundary before committing to it; it is worthwhile only
if it removes exposed ownership obligations. Priority: medium.

### D16. Preserve validation in the type used to build checkout URLs and names

The shared GitHub repository value is described as validated but exposes writable fields and permissive deserialization.
Every consumer must remember to reparse it before generating a clone URL, checkout basename or accepted history.

Keep permissive wire data for correlated request refusals, then convert at admission into a checked value with private
fields and explicit accessors. URL/name generation should require that checked value. Evidence:
`crates/farhelm-proto/src/github_checkout.rs:70`, `:91`, `:309`, `crates/farhelm-supervisor/src/service/core.rs:7387`,
`crates/farhelm-helm/src/sessions.rs:297`, and `crates/farhelm-helm/src/store.rs:5239`. Current ingress checks protect
execution; this is a future omission risk, not a claimed exploitable bypass. Priority: lower.

### D17. Normalize the session-launch draft through one transition API

Harness changes and restored suggestions update separate model, effort, permissions and trust signals through duplicated
assignment sequences. One no-model harness branch returns before clearing incompatible trust; final request assembly
protects the launched process, but hidden draft state survives and can reappear when switching back.

Represent the structured draft as one value whose transitions always normalize every dependent field. Keep destination
and retry authority separate. Evidence: `crates/farhelm-ui/src/launch_composer.rs:1219`, `:1260`, and
`crates/farhelm-ui/src/list/create_form.rs:278`, `:4512`. This strengthens the existing launcher-cleanup TODO with a
concrete invariant failure; it is not a request to split the form merely because it is large. Priority: medium.

### D18. Make platform capabilities explicit in test observation helpers

Several otherwise portable tests embed Linux-only process or memory observations. Some fail on macOS; others treat
missing `/proc` entries as proof of process death and pass without observing cleanup. Common helpers similarly turn
unreadable observations into dead processes or empty child/marker lists. Separately, tests construct invalid UTF-8
filesystem names even though neighboring tests document that APFS rejects those names.

Use a shared observation contract distinguishing observed absence, zombie, live incarnation and unavailable/malformed
evidence. Keep inherently Linux-specific RSS proofs explicitly gated; preserve the current helper's process identity and
`smaps_rollup` metric. Separate these from portable assertions, and use the actual page size for page-based RSS.
Evidence: `crates/farhelm-supervisor/src/files.rs:1522` (`/proc/self/statm`, literal 4096),
`crates/farhelm/tests/e2e/session_lifecycle.rs:5503` (`/proc` parent lookup),
`crates/farhelm/tests/e2e/terminal_backpressure.rs:939` (Linux RSS reader), and
`crates/farhelm-supervisor/src/service/core.rs:27168` (invalid-byte checkout fixture), contrasted with its platform
guard at `:25302`. Unknown-as-absent examples include `crates/farhelm/tests/e2e/terminal_tabs.rs:2038`, `:2197`, and
`crates/farhelm/tests/e2e/harness.rs:1865`, `:1984`, `:2362`. The pipe-holder scan at
`crates/farhelm/tests/e2e/terminal_tabs.rs:3274` reports incomplete coverage in prose, but its assertion at `:2774`
accepts absence of a text fragment as proof. Return structured evidence and format it separately. These are static
fixture defects, not macOS runtime reproductions or product process/descriptor-leak findings. Priority: medium for test
reliability.

### D19. Make operation permits releasable only by their owner

The UI exposes both guarded claims and unrestricted boolean claim/release operations. Some flows obtain a guard but
still release manually at many exits. An old release is harmless only until another operation acquires the same lock;
the API cannot express which operation owns it. Other flows retain the known unmount-related claim leaks.

Make the public claim return a permit whose early release consumes and disarms it; keep boolean manipulation private.
Use the same narrow ownership pattern for per-row operation counts, preserving concurrency between different rows.
Evidence: `crates/farhelm-ui/src/ops.rs:68`, `:139`, `crates/farhelm-ui/src/list/create_form.rs:3001`, `:3030`, `:3300`,
and `crates/farhelm-ui/src/list/view.rs:728`. The inspected mixed guard/manual exits drop immediately, so no new
cross-owner unlock is claimed. This strengthens the already planned page-lock fix. Priority: medium.

### D20. Track menu navigation by action identity through item changes

Menus keep requested focus as an index but reconcile only actual focus when actions disappear. A still-valid index can
now name a different action: after Mark Seen disappears, Stop moves, yet the next ArrowDown starts from its stale
requested index and wraps to Rename instead of moving to Delete. Both session and host rows maintain this split.

Retain requested and actual focus by action identity; derive indices against the current order. Optionally consolidate
the coupled open/close, withdrawal, focus and measurement transitions if that removes bookkeeping obligations. This
extraction is not a prerequisite for the index correction; leave commands and placement outside it. Evidence:
`crates/farhelm-ui/src/list/row.rs:1120`, `:1288`, `crates/farhelm-ui/src/menu_panel.rs:773`, and
`crates/farhelm-ui/src/hosts.rs:2285`, `:2380`. Existing ordering and focus helpers are the base, not a reason for a new
menu framework. This is a new static correctness finding; no browser reproduction. Priority: lower.

### D21. Give browser fixtures one cleanup scope and a resettable stack owner

Partial setup can modify shared checkout/Git configuration before returning a cleanup handle. During teardown, one
failed session deletion can skip all subsequent configuration restoration. Profiles have the same sequential-cleanup
pattern. The resulting contamination makes unrelated later tests fail within a supported serial run.

Register compensations immediately as setup succeeds, attempt every independent restoration, retain failed resource
identities, and aggregate cleanup errors. Preserve dependency ordering: do not delete scratch checkout data beneath a
session whose cleanup failed. Evidence: `e2e/tests/github-checkouts.spec.ts:105`, `:113`, `:245`,
`e2e/tests/profiles.spec.ts:433`; `e2e/tests/agent-relay.spec.ts:201` already demonstrates the needed pattern.

The shared remote supervisor needs an explicit stack-owned replacement/reset operation too. Today the multihost tests
stop it without restoring it, making suite correctness depend on undocumented Playwright project-array scheduling
(`e2e/playwright.config.ts:47`). Keep replacement processes owned through bounded shutdown and actual reaping. This is
single-run ownership debt; concurrent runs in one checkout are intentionally unsupported and are not a finding.
Priority: medium. Coordinate this with D07 rather than building competing fixture frameworks.

### D22. Give platform process observations one bounded implementation

Two macOS readers fetch the same kernel process-argument buffer using contradictory sizing rules. The environment reader
documents an observed kernel zero-fill bug and sizes from `KERN_ARGMAX`; the argv reader allocates 64 KiB and assumes
undersized reads always fail. Vendor-specific process attribution also performs a Linux `/proc` working-directory read
outside the portable seam.

Share the Darwin fetch policy, while retaining separate accepted argv/environment limits, and route working-directory
observations through the platform boundary with explicit unavailable/unsupported results. Evidence:
`crates/farhelm-supervisor/src/procs.rs:1239`, `:1310`, `:1368`, `:1398`, and
`crates/farhelm-supervisor/src/procs/omp.rs:443`. The conflicting assumptions are established; their current macOS
manifestation has not been reproduced. Do not broaden partial OMP support as a side effect. Clarify the spec before
implementation if its 64 KiB limit constrains the kernel fetch allocation rather than retained argv evidence. Priority:
medium, with a focused Darwin reproduction before claiming a production fix.

### D23. Give each host setup operation an identity-bearing owner

Backend setup stores busy membership, progress views and tasks separately, keyed only by host. Completion paths can
clear current busy state while acting on retained progress from an older attempt. Registration separately spans the
durable host row and manager reconciliation, so request cancellation can split those effects.

Use an operation token that owns claim acquisition, accepted execution and completion; require matching identity when
updating or releasing current state. Keep historical failed progress separate, and make registration/reconciliation one
owned mutation. Evidence: `crates/farhelm-helm/src/provisioning/service.rs:44`, `:466`, `:614`, `:906`, `:1200`,
`:1224`, `:1249`, `:1390`. Existing global bounds, per-host locks and detached Add/Update execution remain. The three
probe/busy/cancellation defects are already planned; reassess the consolidation after their deliberately small fixes,
rather than delaying those fixes for a refactor. This is the server counterpart of D06, not a shared client/server state
machine. Priority: medium.

### D24. Make shell cancellation tests prove the phase they interrupt

The installer interruption test sleeps, conditionally signals a still-live process, then accepts any failure plus
absence of temporary files. It can pass after an unrelated early exit without exercising cleanup. Watcher interruption
tests also proceed without asserting readiness and use an unbounded wait on the signal handler under test.

Use a small owned-child driver with a scenario-specific entered witness, liveness check, bounded signal/exit collection,
and independent fallback cleanup. The installer fixture's HTTP server can hold a witnessed download after staging
exists; no product test hook is needed. Evidence: `scripts/test-install-sh.sh:2222`, `scripts/install.sh:1342`,
`scripts/test-plans-watch.sh:350`. Existing gated installer cases (`scripts/test-install-sh.sh:2494`) and recorder
fixtures (`scripts/test-record-test-run.py:59`) demonstrate the intended pattern. This is a concrete test-proof gap, not
a claim that all cancellation coverage is ineffective. Priority: medium; coordinate fixture ownership with D07.

### D25. Transfer one machine reservation through the flake-sweep process chain

The flake-sweep starter briefly acquires and releases the machine lock before publishing its run; its daemon reacquires
the lock later. Two starts can both report acceptance, while only one daemon wins. The current-run pointer can name the
loser, leaving normal wait/stop aimed at the wrong run. The active command retains a worktree lease but not the machine
lease, so daemon death can admit a second sweep in another checkout while the first command still runs.

Acquire one lease and transfer it from starter to daemon to the existing command-holder shim without an unlocked gap.
Publish the run pointer only after admission. Keep the descriptor out of the actual test command and its potentially
long-lived descendants. Evidence: `deflake/bin/deflake:719`, `:1293`, `:1336`. The current lock already excludes two
live daemons, and the worktree lease already prevents deleting an orphan's checkout; preserve both protections. The lock
covers callers sharing a state root, not different users or arbitrary state roots. The orphan extension lasts while the
daemon or holder lives and the holder owns its direct command; it preserves the quiet-machine intent without promising
complete descendant ownership. The startup race is the concrete defect. Neither issue was reproduced or tied to an
existing misclassified flake. Priority: medium.

### D26. Check runtime-suite inventory against the promised sweep coverage

The full flake sweep claims to mirror the authoritative runtime inventory, but omits the OMP reporter asset scenarios
and installed-uninstall acceptance. Neither runs inside workspace nextest or the UI's different JavaScript harness. A
successful sweep therefore does not cover all the runtime scenarios its specification promises.

Give suites stable identities and require each consumer to execute or explicitly exclude them. A focused consistency
check with justified exclusions is enough; a shared command manifest is optional. Evidence: `deflake/bin/deflake:467`,
`.github/dist-build-setup.yml:276`, `:325`, and the full-battery contract in `deflake/SPEC.md`. Preserve the narrow
evaluation profile and platform-specific selections; do not add formatters or compile-only checks to flake discovery.
New static coverage finding. Priority: medium.

### D27. Share a complete fixture-record observer for terminal test output

Some conversation-identity tests wait for only `RECORD-WRITTEN:` or `RECORD-FORKED:`, then parse the available suffix as
the complete identity or path. A terminal chunk can end partway through that value, turning ordinary chunking into a
misleading attribution failure. Neighboring Codex tests already wait for the complete newline-terminated record and test
every incomplete prefix.

Promote that complete-record observer into shared test support, with explicit first/latest occurrence and transcript
boundary. Keep substring waits for contracts that only need a substring. Evidence:
`crates/farhelm/tests/e2e/conversation_identity_capture.rs:177`, `:189`,
`crates/farhelm/tests/e2e/hook_identity.rs:967`, `:985`, and `crates/farhelm/tests/e2e/codex_identity.rs:34`, `:1002`.
The fixture emits complete lines, but transport events need not preserve them. New static test-oracle defect; frequency
is unknown and no historical failure is attributed to it. Priority: medium.

## Smaller validation improvements

- **Make create-intent invalidation tests change one field at a time.** The title case at
  `e2e/tests/terminal-create-idempotency.spec.ts:182` changes both title and folder (`:209`), so working folder
  invalidation masks broken title invalidation. Use the existing captured-key pattern as a single-field transition
  matrix. This is a concrete coverage gap, not an architecture project.
- **Force the payload-cache first-use race at the actual observation boundary.** The barrier at
  `crates/farhelm-helm/src/provisioning.rs:7925` starts two callers together but does not force both filesystem workers
  to observe absence. Use a narrow filesystem seam or barrier immediately before creation to prove the `AlreadyExists`
  branch. Current concurrent integration coverage remains useful; its comment overstates the interleaving it proves.

- **Held routes must match the shipped request and prove interception.** Two Delete globs at
  `e2e/tests/terminal.spec.ts:1729` and `:2096` omit the client's `only_if_nothing_alive` query. Use method/pathname
  matching, assert query semantics separately, and wait for an entered witness. Corrected examples already exist at
  `:3660` and `:3740`. Fold this into D07's route owner.
- **Temporal tab tests need an observed opportunity for the forbidden event.** `e2e/tests/terminal-tabs.spec.ts:1149`
  finishes A's close before attempting B, so it cannot prove sibling overlap; `:1887` immediately accepts an
  already-zero count rather than proving post-close reconciliation; `:1001` accepts the first unchanged sample before a
  stale retry must run. Use held requests, applied-read witnesses and controlled timers already available in the suite.
  These are test-proof gaps, not inferred product failures.
- **Pass resolved build artifact paths into the screenshot fixture.** `scripts/readme-screenshot.sh:46` lets builders
  inherit `CARGO_TARGET_DIR`, then checks checkout-local outputs; `e2e/readme-hero/start-stack.sh:30` independently uses
  that fixed tree. One invocation can build elsewhere and photograph stale local output. Resolve and pass one set of
  paths, or explicitly force both builders into the tree the fixture consumes. Desktop smoke already normalizes its
  build root. This is a lower-priority static handoff defect, independent of concurrent runs.

## Existing work and limits retained

The current [TODO](../../TODO.md) already covers the supervisor core/module carve-out, splitting helm storage by concern,
working-copy SQL ownership, two session-cache backends behind one interface, migration to shared fake supervisors,
launcher decomposition, removal of heuristic conversation attribution, and making the bundled terminal font mandatory.
Those remain useful context, but repeating them as new discoveries would inflate this review. The proposals above add
specific contracts or failure mechanisms where inspection supplied one.

The main known-defect overlaps are the cancellation records for
[Replace](../../review_feedback_queue/replace-drop-skips-source-delete.md) and
[seen notifications](../../review_feedback_queue/seen-write-cancellation-skips-notification.md),
[Restart validation](../../review_feedback_queue/restart-with-skips-create-validation.md),
[rollback scope retention](../../review_feedback_queue/create-rollback-orphans-unconfirmed-scope.md),
[upload replies](../../review_feedback_queue/upload-cancellation-drops-final-reply.md),
[desktop sign-in outcomes](../../review_feedback_queue/desktop-reauth-failure-loses-action-outcomes.md),
[browser sign-in outcomes](../../review_feedback_queue/browser-signin-loses-action-outcomes.md),
[tab scope cleanup](../../review_feedback_queue/delete-skips-scoped-tab-on-stale-verdict.md), and
[upload remount outcomes](../../review_feedback_queue/uploads-aborted-silently-on-remount.md). The consent, takeover,
provisioning and retarget work also overlaps the existing
[restart/takeover/update plan](../../plans/triage-restart-takeover-update.md),
[confirmation/identity plan](../../plans/triage-confirm-ssh-identity.md), and
[YOLO/Replace plan](../../plans/triage-yolo-sighup-replace.md). These links record overlap at the reviewed snapshot; this
report does not alter their disposition or execution order.

Several plausible candidates were deliberately not promoted:

- Shared build-output or browser-stack races requiring multiple runtime invocations in one checkout contradict the
  documented execution contract. The retained fixture findings occur within a supported run.
- Multiple in-process supervisor instances in restart tests are not inherently invalid: the state-directory owner
  explicitly supports process-local claim reuse. Tests of actual ownership transfer still need a real handoff.
- Repairing damaged release-cache control files can race across different assets because repair shares staging names
  under per-asset locks. Authentication remains intact, normal cache use is unaffected, and the established consequence
  is a safe failed provisioning attempt followed by retry. This rare, self-correcting recovery case did not warrant a
  separate architecture project under the review filter.
- Existing attachment reapers, sink leases and teardown barriers already protect current retirement. D15 concerns future
  omission risk; inspection did not establish that those protections fail.
- Healthy local filesystems, trusted same-account processes, modest fleet/client scale, accepted tmux backlog/death
  behavior and intentionally partial non-Claude/Codex integrations remain constraints. No proposal invents a hostile
  same-account threat model, a general corruption-recovery subsystem or a fleet-scale scheduler.
- File length, repeated seed literals, comments, formatting and intentionally self-contained installer logic do not
  establish consequential design debt on their own. Documentation drift without a material contract consequence was not
  promoted into a proposal.

Source review also covered authentication/origin handling, peer input validation, shell/argv construction, signed
payload verification, archive/path handling, process identity, durable launch ownership, database transactions, terminal
flow control, release/signing automation, evidence collection and website deployment. An area with no proposal was
inspected; it is not an assurance that no defect exists.

## Coverage ledger

Every file below is from the pinned snapshot. "Complete" means the assigned source range was read, including inline
tests, and its reviewer completed the unit's responsibility/invariant assessment. It does not mean every execution path
was tested. Split files list each unit's exact range. The 61 completion records were checked against the original
inventory, and SHA-256 comparison confirmed that the 760 inventoried files remained unchanged.

### First-party code, tests, scripts, workflows and configuration

425 files; 467,515 lines; all complete. These ranges include generated workflow output whose build/release behavior was
in scope, as well as hand-written inputs.

- `.config/nextest.toml` — complete: 1–71 (.config-01).
- `.github/dist-build-setup.yml` — complete: 1–560 (.github-01).
- `.github/nextest-pins.json` — complete: 1–17 (.github-01).
- `.github/release/source-pins.env` — complete: 1–8 (.github-01).
- `.github/workflows/ci.yml` — complete: 1–446 (.github-01).
- `.github/workflows/release.yml` — complete: 1–560 (.github-01).
- `.github/workflows/sign-sums.yml` — complete: 1–588 (.github-01).
- `.gitignore` — complete: 1–60 (root-config-01).
- `Cargo.toml` — complete: 1–283 (root-config-01).
- `crates/farhelm-desktop/Cargo.toml` — complete: 1–34 (farhelm-desktop-01).
- `crates/farhelm-desktop/src/main.rs` — complete: 1–33 (farhelm-desktop-01).
- `crates/farhelm-fixtures/Cargo.toml` — complete: 1–31 (farhelm-fixtures-01).
- `crates/farhelm-fixtures/src/fake_agent.rs` — complete: 1–3186 (farhelm-fixtures-01).
- `crates/farhelm-fixtures/src/fake_agent/codex_conversation.rs` — complete: 1–816 (farhelm-fixtures-01).
- `crates/farhelm-fixtures/src/main.rs` — complete: 1–296 (farhelm-fixtures-01).
- `crates/farhelm-helm/Cargo.toml` — complete: 1–60 (farhelm-helm-01).
- `crates/farhelm-helm/build.rs` — complete: 1–177 (farhelm-helm-01).
- `crates/farhelm-helm/http-contract/host-list.json` — complete: 1–189 (farhelm-helm-01).
- `crates/farhelm-helm/http-contract/session-list.json` — complete: 1–66 (farhelm-helm-01).
- `crates/farhelm-helm/src/agent_requests.rs` — complete: 1–5296 (farhelm-helm-01).
- `crates/farhelm-helm/src/aggregate.rs` — complete: 1–1520 (farhelm-helm-01).
- `crates/farhelm-helm/src/auth.rs` — complete: 1–941 (farhelm-helm-01).
- `crates/farhelm-helm/src/checkout_config.rs` — complete: 1–1739 (farhelm-helm-01).
- `crates/farhelm-helm/src/client.rs` — complete: 1–8166 (farhelm-helm-02).
- `crates/farhelm-helm/src/client_log.rs` — complete: 1–825 (farhelm-helm-02).
- `crates/farhelm-helm/src/clipboard.rs` — complete: 1–304 (farhelm-helm-02).
- `crates/farhelm-helm/src/embedded_ui.rs` — complete: 1–54 (farhelm-helm-03).
- `crates/farhelm-helm/src/ensure.rs` — complete: 1–429 (farhelm-helm-03).
- `crates/farhelm-helm/src/events.rs` — complete: 1–1298 (farhelm-helm-03).
- `crates/farhelm-helm/src/feed.rs` — complete: 1–206 (farhelm-helm-03).
- `crates/farhelm-helm/src/hosts.rs` — complete: 1–2738 (farhelm-helm-03).
- `crates/farhelm-helm/src/http_contract_tests.rs` — complete: 1–250 (farhelm-helm-03).
- `crates/farhelm-helm/src/launches.rs` — complete: 1–1418 (farhelm-helm-03).
- `crates/farhelm-helm/src/lib.rs` — complete: 1–2767 (farhelm-helm-03).
- `crates/farhelm-helm/src/manager.rs` — complete: 1–840 (farhelm-helm-03); 841–9635 (farhelm-helm-04).
- `crates/farhelm-helm/src/middleware.rs` — complete: 1–629 (farhelm-helm-04).
- `crates/farhelm-helm/src/precondition.rs` — complete: 1–122 (farhelm-helm-05).
- `crates/farhelm-helm/src/preferences.rs` — complete: 1–407 (farhelm-helm-05).
- `crates/farhelm-helm/src/profiles.rs` — complete: 1–596 (farhelm-helm-05).
- `crates/farhelm-helm/src/provisioning.rs` — complete: 1–7994 (farhelm-helm-05).
- `crates/farhelm-helm/src/provisioning/assets.rs` — complete: 1–467 (farhelm-helm-05).
- `crates/farhelm-helm/src/provisioning/backend.rs` — complete: 1–2591 (farhelm-helm-06).
- `crates/farhelm-helm/src/provisioning/e2e.rs` — complete: 1–428 (farhelm-helm-06).
- `crates/farhelm-helm/src/provisioning/http.rs` — complete: 1–234 (farhelm-helm-06).
- `crates/farhelm-helm/src/provisioning/payloads.rs` — complete: 1–1184 (farhelm-helm-06).
- `crates/farhelm-helm/src/provisioning/plan.rs` — complete: 1–573 (farhelm-helm-06).
- `crates/farhelm-helm/src/provisioning/release_payloads.rs` — complete: 1–3486 (farhelm-helm-06).
- `crates/farhelm-helm/src/provisioning/service.rs` — complete: 1–1504 (farhelm-helm-06); 1505–1612 (farhelm-helm-07).
- `crates/farhelm-helm/src/rest_harness.rs` — complete: 1–1582 (farhelm-helm-07).
- `crates/farhelm-helm/src/session_cache.rs` — complete: 1–208 (farhelm-helm-07).
- `crates/farhelm-helm/src/sessions.rs` — complete: 1–3588 (farhelm-helm-07).
- `crates/farhelm-helm/src/sessions_tests.rs` — complete: 1–4514 (farhelm-helm-07); 4515–10197 (farhelm-helm-08).
- `crates/farhelm-helm/src/ssh.rs` — complete: 1–622 (farhelm-helm-08).
- `crates/farhelm-helm/src/store.rs` — complete: 1–3695 (farhelm-helm-08); 3696–13695 (farhelm-helm-09); 13696–14527
  (farhelm-helm-10).
- `crates/farhelm-helm/src/terminal.rs` — complete: 1–1875 (farhelm-helm-10).
- `crates/farhelm-helm/src/test_capture.rs` — complete: 1–121 (farhelm-helm-10).
- `crates/farhelm-helm/src/token_control.rs` — complete: 1–1123 (farhelm-helm-10).
- `crates/farhelm-helm/src/transport.rs` — complete: 1–432 (farhelm-helm-10).
- `crates/farhelm-helm/src/units.rs` — complete: 1–1112 (farhelm-helm-10).
- `crates/farhelm-helm/src/uploads.rs` — complete: 1–1964 (farhelm-helm-10).
- `crates/farhelm-helm/src/yolo_guard.rs` — complete: 1–92 (farhelm-helm-10).
- `crates/farhelm-helm/tests/fixtures/helm-v26.sql` — complete: 1–129 (farhelm-helm-10).
- `crates/farhelm-helm/tests/fixtures/release/SHA256SUMS` — complete: 1–6 (farhelm-helm-10).
- `crates/farhelm-helm/tests/fixtures/release/tmux-aarch64-unknown-linux-musl` — complete: 1–2 (farhelm-helm-10).
- `crates/farhelm-helm/tests/fixtures/release/tmux-x86_64-unknown-linux-musl` — complete: 1–2 (farhelm-helm-10).
- `crates/farhelm-helm/tests/fixtures/release/variants/other-version/SHA256SUMS` — complete: 1–6 (farhelm-helm-10).
- `crates/farhelm-helm/tests/fixtures/release/variants/two-member/SHA256SUMS` — complete: 1–6 (farhelm-helm-10).
- `crates/farhelm-helm/tests/fixtures/release/variants/without-tmux/SHA256SUMS` — complete: 1–5 (farhelm-helm-10).
- `crates/farhelm-helm/tests/fixtures/ui/assets/app.js` — complete: 1–4 (farhelm-helm-10).
- `crates/farhelm-helm/tests/fixtures/ui/assets/space name.js` — complete: 1–8 (farhelm-helm-10).
- `crates/farhelm-helm/tests/fixtures/ui/index.html` — complete: 1–6 (farhelm-helm-10).
- `crates/farhelm-helm/units/farhelm-helm.service.in` — complete: 1–9 (farhelm-helm-10).
- `crates/farhelm-helm/units/farhelm-supervisor.service.in` — complete: 1–14 (farhelm-helm-10).
- `crates/farhelm-proto/Cargo.toml` — complete: 1–33 (farhelm-proto-01).
- `crates/farhelm-proto/src/github_checkout.rs` — complete: 1–907 (farhelm-proto-01).
- `crates/farhelm-proto/src/http.rs` — complete: 1–93 (farhelm-proto-01).
- `crates/farhelm-proto/src/io.rs` — complete: 1–907 (farhelm-proto-01).
- `crates/farhelm-proto/src/launch.rs` — complete: 1–585 (farhelm-proto-01).
- `crates/farhelm-proto/src/lib.rs` — complete: 1–7475 (farhelm-proto-01); 7476–8523 (farhelm-proto-02).
- `crates/farhelm-proto/src/text.rs` — complete: 1–272 (farhelm-proto-02).
- `crates/farhelm-proto/src/time.rs` — complete: 1–74 (farhelm-proto-02).
- `crates/farhelm-proto/src/yolo.rs` — complete: 1–387 (farhelm-proto-02).
- `crates/farhelm-supervisor/Cargo.toml` — complete: 1–56 (farhelm-supervisor-01).
- `crates/farhelm-supervisor/asset-js-tests/mock-child-process.mjs` — complete: 1–43 (farhelm-supervisor-01).
- `crates/farhelm-supervisor/asset-js-tests/omp-conversation.test.mjs` — complete: 1–145 (farhelm-supervisor-01).
- `crates/farhelm-supervisor/asset-js-tests/scenarios.mjs` — complete: 1–521 (farhelm-supervisor-01).
- `crates/farhelm-supervisor/assets/omp-conversation-v1.ts` — complete: 1–139 (farhelm-supervisor-01).
- `crates/farhelm-supervisor/assets/pi-conversation-v1.ts` — complete: 1–50 (farhelm-supervisor-01).
- `crates/farhelm-supervisor/src/agent_kind/capture.rs` — complete: 1–1319 (farhelm-supervisor-01).
- `crates/farhelm-supervisor/src/agent_kind/claude.rs` — complete: 1–20 (farhelm-supervisor-01).
- `crates/farhelm-supervisor/src/agent_kind/codex.rs` — complete: 1–535 (farhelm-supervisor-01).
- `crates/farhelm-supervisor/src/agent_kind/goose.rs` — complete: 1–157 (farhelm-supervisor-01).
- `crates/farhelm-supervisor/src/agent_kind/grok.rs` — complete: 1–547 (farhelm-supervisor-01).
- `crates/farhelm-supervisor/src/agent_kind/mod.rs` — complete: 1–5079 (farhelm-supervisor-01).
- `crates/farhelm-supervisor/src/agent_kind/omp.rs` — complete: 1–520 (farhelm-supervisor-01).
- `crates/farhelm-supervisor/src/agent_kind/pi.rs` — complete: 1–111 (farhelm-supervisor-01).
- `crates/farhelm-supervisor/src/agent_kind/screen_fixtures.rs` — complete: 1–354 (farhelm-supervisor-02).
- `crates/farhelm-supervisor/src/agent_kind/screen_reader.rs` — complete: 1–825 (farhelm-supervisor-02).
- `crates/farhelm-supervisor/src/attachments.rs` — complete: 1–821 (farhelm-supervisor-02).
- `crates/farhelm-supervisor/src/bounded_command.rs` — complete: 1–398 (farhelm-supervisor-02).
- `crates/farhelm-supervisor/src/db.rs` — complete: 1–152 (farhelm-supervisor-02).
- `crates/farhelm-supervisor/src/files.rs` — complete: 1–1671 (farhelm-supervisor-02).
- `crates/farhelm-supervisor/src/launch.rs` — complete: 1–4161 (farhelm-supervisor-02).
- `crates/farhelm-supervisor/src/lib.rs` — complete: 1–288 (farhelm-supervisor-02).
- `crates/farhelm-supervisor/src/pi_extension.rs` — complete: 1–292 (farhelm-supervisor-02).
- `crates/farhelm-supervisor/src/procs.rs` — complete: 1–1038 (farhelm-supervisor-02); 1039–3172
  (farhelm-supervisor-03).
- `crates/farhelm-supervisor/src/procs/claude.rs` — complete: 1–84 (farhelm-supervisor-03).
- `crates/farhelm-supervisor/src/procs/codex.rs` — complete: 1–78 (farhelm-supervisor-03).
- `crates/farhelm-supervisor/src/procs/grok.rs` — complete: 1–98 (farhelm-supervisor-03).
- `crates/farhelm-supervisor/src/procs/omp.rs` — complete: 1–511 (farhelm-supervisor-03).
- `crates/farhelm-supervisor/src/repository_discovery.rs` — complete: 1–1083 (farhelm-supervisor-03).
- `crates/farhelm-supervisor/src/scope.rs` — complete: 1–2179 (farhelm-supervisor-03).
- `crates/farhelm-supervisor/src/service/agent_relay.rs` — complete: 1–1388 (farhelm-supervisor-03).
- `crates/farhelm-supervisor/src/service/capture.rs` — complete: 1–2445 (farhelm-supervisor-03); 2446–2963
  (farhelm-supervisor-04).
- `crates/farhelm-supervisor/src/service/connection.rs` — complete: 1–3092 (farhelm-supervisor-04).
- `crates/farhelm-supervisor/src/service/core.rs` — complete: 1–6390 (farhelm-supervisor-04); 6391–16390
  (farhelm-supervisor-05); 16391–26390 (farhelm-supervisor-06); 26391–31051 (farhelm-supervisor-07).
- `crates/farhelm-supervisor/src/service/handlers.rs` — complete: 1–5339 (farhelm-supervisor-07); 5340–10012
  (farhelm-supervisor-08).
- `crates/farhelm-supervisor/src/service/hints.rs` — complete: 1–327 (farhelm-supervisor-08).
- `crates/farhelm-supervisor/src/service/launch_artifacts.rs` — complete: 1–712 (farhelm-supervisor-08).
- `crates/farhelm-supervisor/src/service/listing.rs` — complete: 1–399 (farhelm-supervisor-08).
- `crates/farhelm-supervisor/src/service/mod.rs` — complete: 1–118 (farhelm-supervisor-08).
- `crates/farhelm-supervisor/src/service/status.rs` — complete: 1–1325 (farhelm-supervisor-08).
- `crates/farhelm-supervisor/src/service/sweep.rs` — complete: 1–2446 (farhelm-supervisor-08); 2447–3418
  (farhelm-supervisor-09).
- `crates/farhelm-supervisor/src/service/teardown.rs` — complete: 1–3909 (farhelm-supervisor-09).
- `crates/farhelm-supervisor/src/service/terminals.rs` — complete: 1–4223 (farhelm-supervisor-09).
- `crates/farhelm-supervisor/src/service/ticker.rs` — complete: 1–896 (farhelm-supervisor-09); 897–6088
  (farhelm-supervisor-10).
- `crates/farhelm-supervisor/src/service/uploads.rs` — complete: 1–2130 (farhelm-supervisor-10).
- `crates/farhelm-supervisor/src/store.rs` — complete: 1–2678 (farhelm-supervisor-10); 2679–12018
  (farhelm-supervisor-11).
- `crates/farhelm-supervisor/src/tmux.rs` — complete: 1–6170 (farhelm-supervisor-12).
- `crates/farhelm-supervisor/src/tmux/control_codec.rs` — complete: 1–1413 (farhelm-supervisor-12).
- `crates/farhelm-supervisor/src/tmux/input.rs` — complete: 1–486 (farhelm-supervisor-12).
- `crates/farhelm-supervisor/src/tmux/query_strip.rs` — complete: 1–214 (farhelm-supervisor-12).
- `crates/farhelm-supervisor/src/tmux/sink.rs` — complete: 1–505 (farhelm-supervisor-12).
- `crates/farhelm-supervisor/src/tmux/stream.rs` — complete: 1–1212 (farhelm-supervisor-12); 1213–3451
  (farhelm-supervisor-13).
- `crates/farhelm-supervisor/src/tmux/test_support.rs` — complete: 1–361 (farhelm-supervisor-13).
- `crates/farhelm-supervisor/src/working_copies.rs` — complete: 1–4019 (farhelm-supervisor-13).
- `crates/farhelm-supervisor/tests/fixtures/supervisor-v17.sql` — complete: 1–56 (farhelm-supervisor-13).
- `crates/farhelm-teststate/Cargo.toml` — complete: 1–30 (farhelm-teststate-01).
- `crates/farhelm-teststate/src/lib.rs` — complete: 1–1134 (farhelm-teststate-01).
- `crates/farhelm-teststate/src/process.rs` — complete: 1–1193 (farhelm-teststate-01).
- `crates/farhelm-teststate/src/thread.rs` — complete: 1–738 (farhelm-teststate-01).
- `crates/farhelm-teststate/src/tmux.rs` — complete: 1–1805 (farhelm-teststate-01).
- `crates/farhelm-teststate/src/tmux/diagnostics.rs` — complete: 1–833 (farhelm-teststate-01).
- `crates/farhelm-teststate/src/tmux/guard.rs` — complete: 1–323 (farhelm-teststate-01).
- `crates/farhelm-testtrace-macros/Cargo.toml` — complete: 1–23 (farhelm-testtrace-macros-01).
- `crates/farhelm-testtrace-macros/src/lib.rs` — complete: 1–260 (farhelm-testtrace-macros-01).
- `crates/farhelm-testtrace-macros/tests/compile_contract.rs` — complete: 1–329 (farhelm-testtrace-macros-01).
- `crates/farhelm-testtrace/Cargo.toml` — complete: 1–23 (farhelm-testtrace-01).
- `crates/farhelm-testtrace/src/lib.rs` — complete: 1–3780 (farhelm-testtrace-01).
- `crates/farhelm-testtrace/src/persistence.rs` — complete: 1–1166 (farhelm-testtrace-01).
- `crates/farhelm-testtrace/tests/libtest_contract.rs` — complete: 1–1249 (farhelm-testtrace-01).
- `crates/farhelm-testtrace/tests/support/process.rs` — complete: 1–595 (farhelm-testtrace-01).
- `crates/farhelm-ui/Cargo.toml` — complete: 1–124 (farhelm-ui-01).
- `crates/farhelm-ui/Dioxus.toml` — complete: 1–28 (farhelm-ui-01).
- `crates/farhelm-ui/assets/app.css` — complete: 1–5575 (farhelm-ui-01).
- `crates/farhelm-ui/assets/click-detail.js` — complete: 1–171 (farhelm-ui-01).
- `crates/farhelm-ui/assets/client-log-shim.js` — complete: 1–511 (farhelm-ui-01).
- `crates/farhelm-ui/assets/clipboard-name.js` — complete: 1–85 (farhelm-ui-01).
- `crates/farhelm-ui/assets/copy-on-select.js` — complete: 1–80 (farhelm-ui-01).
- `crates/farhelm-ui/assets/desktop-auth.js` — complete: 1–144 (farhelm-ui-01).
- `crates/farhelm-ui/assets/events.js` — complete: 1–460 (farhelm-ui-01).
- `crates/farhelm-ui/assets/shift-enter-key.js` — complete: 1–169 (farhelm-ui-01).
- `crates/farhelm-ui/assets/term-bytes.js` — complete: 1–60 (farhelm-ui-01).
- `crates/farhelm-ui/assets/terminal-links.js` — complete: 1–458 (farhelm-ui-01).
- `crates/farhelm-ui/assets/terminal-theme.js` — complete: 1–32 (farhelm-ui-01).
- `crates/farhelm-ui/assets/terminal.js` — complete: 1–2103 (farhelm-ui-01); 2104–5199 (farhelm-ui-02).
- `crates/farhelm-ui/build.rs` — complete: 1–23 (farhelm-ui-02).
- `crates/farhelm-ui/js-tests/app-css-contrast.test.js` — complete: 1–268 (farhelm-ui-02).
- `crates/farhelm-ui/js-tests/app-css-tokens.test.js` — complete: 1–355 (farhelm-ui-02).
- `crates/farhelm-ui/js-tests/click-detail.test.js` — complete: 1–257 (farhelm-ui-02).
- `crates/farhelm-ui/js-tests/client-log-shim.test.js` — complete: 1–708 (farhelm-ui-02).
- `crates/farhelm-ui/js-tests/clipboard-name.test.js` — complete: 1–121 (farhelm-ui-02).
- `crates/farhelm-ui/js-tests/copy-on-select.test.js` — complete: 1–75 (farhelm-ui-02).
- `crates/farhelm-ui/js-tests/desktop-auth.test.js` — complete: 1–218 (farhelm-ui-02).
- `crates/farhelm-ui/js-tests/events.test.js` — complete: 1–398 (farhelm-ui-02).
- `crates/farhelm-ui/js-tests/shift-enter-key.test.js` — complete: 1–179 (farhelm-ui-02).
- `crates/farhelm-ui/js-tests/term-bytes.test.js` — complete: 1–98 (farhelm-ui-02).
- `crates/farhelm-ui/js-tests/terminal-links.test.js` — complete: 1–340 (farhelm-ui-02).
- `crates/farhelm-ui/js-tests/terminal-theme.test.js` — complete: 1–43 (farhelm-ui-02).
- `crates/farhelm-ui/src/activity.rs` — complete: 1–549 (farhelm-ui-02).
- `crates/farhelm-ui/src/api.rs` — complete: 1–3272 (farhelm-ui-02); 3273–4906 (farhelm-ui-03).
- `crates/farhelm-ui/src/api/http_contract_tests.rs` — complete: 1–274 (farhelm-ui-03).
- `crates/farhelm-ui/src/app_bar.rs` — complete: 1–832 (farhelm-ui-03).
- `crates/farhelm-ui/src/attachments.rs` — complete: 1–952 (farhelm-ui-03).
- `crates/farhelm-ui/src/auth.rs` — complete: 1–758 (farhelm-ui-03).
- `crates/farhelm-ui/src/desktop.rs` — complete: 1–1588 (farhelm-ui-03).
- `crates/farhelm-ui/src/desktop/assets.rs` — complete: 1–341 (farhelm-ui-03).
- `crates/farhelm-ui/src/desktop/bundle.rs` — complete: 1–163 (farhelm-ui-03).
- `crates/farhelm-ui/src/desktop/state.rs` — complete: 1–256 (farhelm-ui-03).
- `crates/farhelm-ui/src/desktop/tmux_preflight.rs` — complete: 1–769 (farhelm-ui-03).
- `crates/farhelm-ui/src/desktop/window_state.rs` — complete: 1–714 (farhelm-ui-03).
- `crates/farhelm-ui/src/feed.rs` — complete: 1–692 (farhelm-ui-03).
- `crates/farhelm-ui/src/github_checkout.rs` — complete: 1–487 (farhelm-ui-03).
- `crates/farhelm-ui/src/hosts.rs` — complete: 1–4527 (farhelm-ui-04).
- `crates/farhelm-ui/src/hosts/settings_dialog.rs` — complete: 1–501 (farhelm-ui-04).
- `crates/farhelm-ui/src/icons.rs` — complete: 1–363 (farhelm-ui-04).
- `crates/farhelm-ui/src/launch_composer.rs` — complete: 1–3243 (farhelm-ui-04).
- `crates/farhelm-ui/src/launch_controls.rs` — complete: 1–456 (farhelm-ui-04).
- `crates/farhelm-ui/src/lib.rs` — complete: 1–910 (farhelm-ui-04); 911–2486 (farhelm-ui-05).
- `crates/farhelm-ui/src/list.rs` — complete: 1–60 (farhelm-ui-05).
- `crates/farhelm-ui/src/list/create_form.rs` — complete: 1–5913 (farhelm-ui-05).
- `crates/farhelm-ui/src/list/row.rs` — complete: 1–2451 (farhelm-ui-05); 2452–3235 (farhelm-ui-06).
- `crates/farhelm-ui/src/list/shared.rs` — complete: 1–1104 (farhelm-ui-06).
- `crates/farhelm-ui/src/list/view.rs` — complete: 1–3837 (farhelm-ui-06).
- `crates/farhelm-ui/src/main.rs` — complete: 1–82 (farhelm-ui-06).
- `crates/farhelm-ui/src/menu_panel.rs` — complete: 1–2181 (farhelm-ui-06).
- `crates/farhelm-ui/src/modal_isolation.rs` — complete: 1–186 (farhelm-ui-06).
- `crates/farhelm-ui/src/ops.rs` — complete: 1–839 (farhelm-ui-06).
- `crates/farhelm-ui/src/peer.rs` — complete: 1–383 (farhelm-ui-06).
- `crates/farhelm-ui/src/profiles.rs` — complete: 1–2836 (farhelm-ui-07).
- `crates/farhelm-ui/src/provisioning.rs` — complete: 1–3078 (farhelm-ui-07).
- `crates/farhelm-ui/src/reader.rs` — complete: 1–1161 (farhelm-ui-07).
- `crates/farhelm-ui/src/reconnect.rs` — complete: 1–329 (farhelm-ui-07).
- `crates/farhelm-ui/src/rename.rs` — complete: 1–279 (farhelm-ui-07).
- `crates/farhelm-ui/src/restart_with.rs` — complete: 1–433 (farhelm-ui-07).
- `crates/farhelm-ui/src/rows.rs` — complete: 1–1281 (farhelm-ui-07).
- `crates/farhelm-ui/src/session_view.rs` — complete: 1–3554 (farhelm-ui-08).
- `crates/farhelm-ui/src/skew.rs` — complete: 1–446 (farhelm-ui-08).
- `crates/farhelm-ui/src/status.rs` — complete: 1–1040 (farhelm-ui-08).
- `crates/farhelm-ui/src/tabs.rs` — complete: 1–512 (farhelm-ui-08).
- `crates/farhelm-ui/src/tmux_probe.rs` — complete: 1–119 (farhelm-ui-08).
- `crates/farhelm-ui/src/webview_watchdog.rs` — complete: 1–351 (farhelm-ui-08).
- `crates/farhelm-ui/src/window_chrome.rs` — complete: 1–417 (farhelm-ui-08).
- `crates/farhelm-ui/src/yolo_confirm.rs` — complete: 1–319 (farhelm-ui-08).
- `crates/farhelm/Cargo.toml` — complete: 1–109 (farhelm-01).
- `crates/farhelm/src/agent_client.rs` — complete: 1–775 (farhelm-01).
- `crates/farhelm/src/agent_instructions.rs` — complete: 1–515 (farhelm-01).
- `crates/farhelm/src/goose_hook.rs` — complete: 1–138 (farhelm-01).
- `crates/farhelm/src/hook.rs` — complete: 1–2371 (farhelm-01).
- `crates/farhelm/src/main.rs` — complete: 1–1562 (farhelm-01).
- `crates/farhelm/src/render.rs` — complete: 1–578 (farhelm-01).
- `crates/farhelm/src/setup.rs` — complete: 1–3952 (farhelm-01); 3953–4215 (farhelm-02).
- `crates/farhelm/src/uninstall.rs` — complete: 1–569 (farhelm-02).
- `crates/farhelm/src/uninstall/locks.rs` — complete: 1–371 (farhelm-02).
- `crates/farhelm/src/uninstall/ownership.rs` — complete: 1–1344 (farhelm-02).
- `crates/farhelm/src/uninstall/removal.rs` — complete: 1–299 (farhelm-02).
- `crates/farhelm/tests/agent_cli.rs` — complete: 1–2041 (farhelm-02).
- `crates/farhelm/tests/checkout_config_cli.rs` — complete: 1–192 (farhelm-02).
- `crates/farhelm/tests/cli_support/mock_supervisor.rs` — complete: 1–166 (farhelm-02).
- `crates/farhelm/tests/cli_support/mod.rs` — complete: 1–77 (farhelm-02).
- `crates/farhelm/tests/e2e/agent_listing_real_stack.rs` — complete: 1–993 (farhelm-02).
- `crates/farhelm/tests/e2e/agent_relay.rs` — complete: 1–954 (farhelm-02).
- `crates/farhelm/tests/e2e/attachment_uploads.rs` — complete: 1–2717 (farhelm-02).
- `crates/farhelm/tests/e2e/boot_id_durable_outcome.rs` — complete: 1–773 (farhelm-03).
- `crates/farhelm/tests/e2e/change_hints.rs` — complete: 1–194 (farhelm-03).
- `crates/farhelm/tests/e2e/codex_identity.rs` — complete: 1–1017 (farhelm-03).
- `crates/farhelm/tests/e2e/conversation_identity_capture.rs` — complete: 1–1426 (farhelm-03).
- `crates/farhelm/tests/e2e/create_idempotency.rs` — complete: 1–1073 (farhelm-03).
- `crates/farhelm/tests/e2e/github_checkouts.rs` — complete: 1–1068 (farhelm-03).
- `crates/farhelm/tests/e2e/harness.rs` — complete: 1–2799 (farhelm-03).
- `crates/farhelm/tests/e2e/harness/raw_peer.rs` — complete: 1–84 (farhelm-03).
- `crates/farhelm/tests/e2e/harness/tmux_guard.rs` — complete: 1–208 (farhelm-03).
- `crates/farhelm/tests/e2e/hook_identity.rs` — complete: 1–1358 (farhelm-03); 1359–2214 (farhelm-04).
- `crates/farhelm/tests/e2e/host_connection.rs` — complete: 1–492 (farhelm-04).
- `crates/farhelm/tests/e2e/launch_sentinel_error_status.rs` — complete: 1–518 (farhelm-04).
- `crates/farhelm/tests/e2e/main.rs` — complete: 1–72 (farhelm-04).
- `crates/farhelm/tests/e2e/marker_model.rs` — complete: 1–203 (farhelm-04).
- `crates/farhelm/tests/e2e/merged_hosts.rs` — complete: 1–267 (farhelm-04).
- `crates/farhelm/tests/e2e/real_agent_capture.rs` — complete: 1–1297 (farhelm-04).
- `crates/farhelm/tests/e2e/replay_marker.rs` — complete: 1–1058 (farhelm-04).
- `crates/farhelm/tests/e2e/restart_under_concurrency.rs` — complete: 1–733 (farhelm-04).
- `crates/farhelm/tests/e2e/restart_with_resume.rs` — complete: 1–1682 (farhelm-04).
- `crates/farhelm/tests/e2e/session_lifecycle.rs` — complete: 1–2822 (farhelm-04); 2823–6578 (farhelm-05).
- `crates/farhelm/tests/e2e/session_rename.rs` — complete: 1–957 (farhelm-05).
- `crates/farhelm/tests/e2e/structured_launches.rs` — complete: 1–1334 (farhelm-05).
- `crates/farhelm/tests/e2e/supervisor_stop.rs` — complete: 1–272 (farhelm-05).
- `crates/farhelm/tests/e2e/tab_lifecycle_edges.rs` — complete: 1–962 (farhelm-05).
- `crates/farhelm/tests/e2e/terminal_backpressure.rs` — complete: 1–1911 (farhelm-05).
- `crates/farhelm/tests/e2e/terminal_backpressure/rss.rs` — complete: 1–154 (farhelm-05).
- `crates/farhelm/tests/e2e/terminal_tabs.rs` — complete: 1–3727 (farhelm-06).
- `crates/farhelm/tests/e2e/wrapper_launch.rs` — complete: 1–978 (farhelm-06).
- `crates/farhelm/tests/helm_setup_cli.rs` — complete: 1–67 (farhelm-06).
- `crates/farhelm/tests/spawn_cli.rs` — complete: 1–553 (farhelm-06).
- `crates/farhelm/tests/stdio_exit.rs` — complete: 1–35 (farhelm-06).
- `crates/farhelm/tests/supervisor_tether.rs` — complete: 1–39 (farhelm-06).
- `deflake/bin/deflake` — complete: 1–1513 (deflake-01).
- `deflake/eval-setup.sh` — complete: 1–92 (deflake-01).
- `deflake/test-deflake.py` — complete: 1–233 (deflake-01).
- `dist-workspace.toml` — complete: 1–290 (root-config-01).
- `docs/readme-hero/scenario.json5` — complete: 1–114 (docs-01).
- `dprint.json` — complete: 1–21 (root-config-01).
- `e2e/global-setup.ts` — complete: 1–70 (e2e-01).
- `e2e/harness-tests/child-runner.ts` — complete: 1–570 (e2e-01).
- `e2e/harness-tests/child.config.ts` — complete: 1–13 (e2e-01).
- `e2e/harness-tests/playwright.config.ts` — complete: 1–12 (e2e-01).
- `e2e/harness-tests/supervise-child.py` — complete: 1–193 (e2e-01).
- `e2e/harness-tests/test-supervise-child.py` — complete: 1–138 (e2e-01).
- `e2e/harness-tests/timeline-child.failure.ts` — complete: 1–46 (e2e-01).
- `e2e/harness-tests/timeline.contract.ts` — complete: 1–920 (e2e-01).
- `e2e/package.json` — complete: 1–12 (e2e-01).
- `e2e/playwright.config.ts` — complete: 1–173 (e2e-01).
- `e2e/readme-hero.config.ts` — complete: 1–70 (e2e-01).
- `e2e/readme-hero/capture.spec.ts` — complete: 1–293 (e2e-01).
- `e2e/readme-hero/scenario.ts` — complete: 1–135 (e2e-01).
- `e2e/readme-hero/start-stack.sh` — complete: 1–201 (e2e-01).
- `e2e/recorded-policy-reporter.cjs` — complete: 1–97 (e2e-01).
- `e2e/stack-port.ts` — complete: 1–78 (e2e-01).
- `e2e/start-stack.sh` — complete: 1–547 (e2e-01).
- `e2e/tests/agent-relay.spec.ts` — complete: 1–734 (e2e-01).
- `e2e/tests/auth.spec.ts` — complete: 1–395 (e2e-01).
- `e2e/tests/buttons.spec.ts` — complete: 1–451 (e2e-01).
- `e2e/tests/chrome.spec.ts` — complete: 1–197 (e2e-01).
- `e2e/tests/clone.spec.ts` — complete: 1–1137 (e2e-01).
- `e2e/tests/composer-word-search.spec.ts` — complete: 1–364 (e2e-01).
- `e2e/tests/cursor-composer.spec.ts` — complete: 1–51 (e2e-01).
- `e2e/tests/delete-precondition.spec.ts` — complete: 1–123 (e2e-01).
- `e2e/tests/destination-authority.spec.ts` — complete: 1–255 (e2e-01).
- `e2e/tests/f20_visual_capture.spec.ts` — complete: 1–234 (e2e-01).
- `e2e/tests/feed.spec.ts` — complete: 1–1106 (e2e-01).
- `e2e/tests/filters.spec.ts` — complete: 1–297 (e2e-01).
- `e2e/tests/github-checkout-composer.spec.ts` — complete: 1–575 (e2e-01).
- `e2e/tests/github-checkouts.spec.ts` — complete: 1–547 (e2e-02).
- `e2e/tests/goose-pi-composer.spec.ts` — complete: 1–142 (e2e-02).
- `e2e/tests/grok-composer.spec.ts` — complete: 1–75 (e2e-02).
- `e2e/tests/header.spec.ts` — complete: 1–617 (e2e-02).
- `e2e/tests/helpers/device-auth.ts` — complete: 1–82 (e2e-02).
- `e2e/tests/helpers/evidence.ts` — complete: 1–603 (e2e-02).
- `e2e/tests/helpers/fleet.ts` — complete: 1–1263 (e2e-02).
- `e2e/tests/helpers/focus-trace.ts` — complete: 1–344 (e2e-02).
- `e2e/tests/helpers/helm-build.ts` — complete: 1–15 (e2e-02).
- `e2e/tests/helpers/real-agent.ts` — complete: 1–463 (e2e-02).
- `e2e/tests/helpers/route-gate.ts` — complete: 1–16 (e2e-02).
- `e2e/tests/helpers/scratch.ts` — complete: 1–45 (e2e-02).
- `e2e/tests/helpers/term.ts` — complete: 1–179 (e2e-02).
- `e2e/tests/helpers/terminal-readiness.ts` — complete: 1–351 (e2e-02).
- `e2e/tests/helpers/terminal-suite.ts` — complete: 1–656 (e2e-02).
- `e2e/tests/helpers/timeline.ts` — complete: 1–477 (e2e-02).
- `e2e/tests/launch-button-alignment.spec.ts` — complete: 1–75 (e2e-02).
- `e2e/tests/m6-5-debts.spec.ts` — complete: 1–538 (e2e-02).
- `e2e/tests/modal-focus.spec.ts` — complete: 1–74 (e2e-02).
- `e2e/tests/model-defaults.spec.ts` — complete: 1–43 (e2e-02).
- `e2e/tests/mouse-modes.spec.ts` — complete: 1–432 (e2e-02).
- `e2e/tests/omp-composer.spec.ts` — complete: 1–156 (e2e-02).
- `e2e/tests/opencode.spec.ts` — complete: 1–81 (e2e-02).
- `e2e/tests/profiles-delete.spec.ts` — complete: 1–154 (e2e-02).
- `e2e/tests/profiles.spec.ts` — complete: 1–2572 (e2e-02); 2573–3387 (e2e-03).
- `e2e/tests/provisioning.spec.ts` — complete: 1–3298 (e2e-03).
- `e2e/tests/readers.spec.ts` — complete: 1–300 (e2e-03).
- `e2e/tests/real-agent.spec.ts` — complete: 1–533 (e2e-03).
- `e2e/tests/remembered-permissions.spec.ts` — complete: 1–211 (e2e-03).
- `e2e/tests/replace.spec.ts` — complete: 1–529 (e2e-03).
- `e2e/tests/restart-with.spec.ts` — complete: 1–890 (e2e-03).
- `e2e/tests/scratch.spec.ts` — complete: 1–32 (e2e-03).
- `e2e/tests/shell-scroll.spec.ts` — complete: 1–124 (e2e-03).
- `e2e/tests/sidebar.spec.ts` — complete: 1–3268 (e2e-03); 3269–7891 (e2e-04).
- `e2e/tests/sort.spec.ts` — complete: 1–1405 (e2e-04).
- `e2e/tests/spawn.spec.ts` — complete: 1–259 (e2e-04).
- `e2e/tests/terminal-attachments.spec.ts` — complete: 1–1878 (e2e-04).
- `e2e/tests/terminal-clipboard.spec.ts` — complete: 1–753 (e2e-04).
- `e2e/tests/terminal-create-idempotency.spec.ts` — complete: 1–342 (e2e-04).
- `e2e/tests/terminal-flood.spec.ts` — complete: 1–1299 (e2e-05).
- `e2e/tests/terminal-font.spec.ts` — complete: 1–277 (e2e-05).
- `e2e/tests/terminal-keys.spec.ts` — complete: 1–425 (e2e-05).
- `e2e/tests/terminal-links.spec.ts` — complete: 1–1203 (e2e-05).
- `e2e/tests/terminal-multihost.spec.ts` — complete: 1–3452 (e2e-05).
- `e2e/tests/terminal-reconnect.spec.ts` — complete: 1–2016 (e2e-05).
- `e2e/tests/terminal-replay-rename.spec.ts` — complete: 1–1328 (e2e-05); 1329–1812 (e2e-06).
- `e2e/tests/terminal-restart.spec.ts` — complete: 1–1081 (e2e-06).
- `e2e/tests/terminal-scroll-freeze.spec.ts` — complete: 1–750 (e2e-06).
- `e2e/tests/terminal-tabs.spec.ts` — complete: 1–2103 (e2e-06).
- `e2e/tests/terminal.spec.ts` — complete: 1–4301 (e2e-06).
- `e2e/tests/window-chrome.spec.ts` — complete: 1–159 (e2e-06).
- `e2e/tests/work-start-order.spec.ts` — complete: 1–233 (e2e-06).
- `e2e/tests/yolo-guard.spec.ts` — complete: 1–233 (e2e-06).
- `packaging/farhelm-desktop/dist.toml` — complete: 1–81 (packaging-01).
- `releasing/check-changelog.py` — complete: 1–841 (releasing-01).
- `scripts/build-desktop-binary.sh` — complete: 1–129 (scripts-01).
- `scripts/build-pinned-tmux-ci.sh` — complete: 1–71 (scripts-01).
- `scripts/build-private-tmux.sh` — complete: 1–136 (scripts-01).
- `scripts/build-tmux-assets.sh` — complete: 1–68 (scripts-01).
- `scripts/capture-agent-screens.py` — complete: 1–739 (scripts-01).
- `scripts/check-desktop-assets.sh` — complete: 1–257 (scripts-01).
- `scripts/check-release-archive.py` — complete: 1–413 (scripts-01).
- `scripts/check-static-elf.sh` — complete: 1–223 (scripts-01).
- `scripts/check-test-sleeps.py` — complete: 1–193 (scripts-01).
- `scripts/check-tmux-cutover.py` — complete: 1–301 (scripts-01).
- `scripts/desktop-smoke.sh` — complete: 1–1284 (scripts-01).
- `scripts/hunt-browser-tests.py` — complete: 1–189 (scripts-01).
- `scripts/hunt-rust-tests.py` — complete: 1–278 (scripts-01).
- `scripts/install-pinned-nextest.py` — complete: 1–160 (scripts-01).
- `scripts/install.sh` — complete: 1–1976 (scripts-01).
- `scripts/plan-test-hunts.py` — complete: 1–317 (scripts-01).
- `scripts/plans-watch.sh` — complete: 1–252 (scripts-01).
- `scripts/publish-readme-hero.sh` — complete: 1–323 (scripts-01).
- `scripts/readme-screenshot.sh` — complete: 1–72 (scripts-01).
- `scripts/record-test-run.py` — complete: 1–1893 (scripts-01).
- `scripts/summarize-test-runs.py` — complete: 1–106 (scripts-02).
- `scripts/test-check-test-sleeps.py` — complete: 1–160 (scripts-02).
- `scripts/test-hunt-browser-tests.py` — complete: 1–199 (scripts-02).
- `scripts/test-hunt-tests.py` — complete: 1–453 (scripts-02).
- `scripts/test-install-pinned-nextest.py` — complete: 1–134 (scripts-02).
- `scripts/test-install-sh.sh` — complete: 1–3333 (scripts-02).
- `scripts/test-nextest-cleanup.py` — complete: 1–172 (scripts-02).
- `scripts/test-plan-test-hunts.py` — complete: 1–242 (scripts-02).
- `scripts/test-plans-watch.sh` — complete: 1–380 (scripts-02).
- `scripts/test-playwright-policy-reporter.cjs` — complete: 1–115 (scripts-02).
- `scripts/test-playwright-report.py` — complete: 1–336 (scripts-02).
- `scripts/test-provision-centos.sh` — complete: 1–455 (scripts-02).
- `scripts/test-record-test-run.py` — complete: 1–2106 (scripts-02).
- `scripts/test-start-stack-cleanup.sh` — complete: 1–363 (scripts-02).
- `scripts/test-summarize-test-runs.py` — complete: 1–134 (scripts-02).
- `scripts/test-test-run-inventory.py` — complete: 1–257 (scripts-02).
- `scripts/test-test-run-nextest.py` — complete: 1–173 (scripts-02).
- `scripts/test-test-run-summary.py` — complete: 1–164 (scripts-02).
- `scripts/test-test-run-traces.py` — complete: 1–258 (scripts-03).
- `scripts/test-test-sleep-syntax.py` — complete: 1–187 (scripts-03).
- `scripts/test-tmux-pinned-shutdown.sh` — complete: 1–64 (scripts-03).
- `scripts/test-uninstall.py` — complete: 1–408 (scripts-03).
- `scripts/test_hunt.py` — complete: 1–255 (scripts-03).
- `scripts/test_run_inventory.py` — complete: 1–385 (scripts-03).
- `scripts/test_run_nextest.py` — complete: 1–281 (scripts-03).
- `scripts/test_run_playwright.py` — complete: 1–426 (scripts-03).
- `scripts/test_run_summary.py` — complete: 1–322 (scripts-03).
- `scripts/test_run_traces.py` — complete: 1–277 (scripts-03).
- `scripts/test_sleep_syntax.py` — complete: 1–408 (scripts-03).
- `website/astro.config.mjs` — complete: 1–80 (website-01).
- `website/package.json` — complete: 1–15 (website-01).
- `website/scripts/outline-wordmark.py` — complete: 1–80 (website-01).
- `website/scripts/render-svgs.mjs` — complete: 1–443 (website-01).
- `website/scripts/vercel-deploy.sh` — complete: 1–87 (website-01).
- `website/src/components/FarhelmPanel.astro` — complete: 1–15 (website-01).
- `website/src/components/ThemedImage.astro` — complete: 1–39 (website-01).
- `website/src/content.config.ts` — complete: 1–12 (website-01).
- `website/src/styles/farhelm.css` — complete: 1–345 (website-01).
- `website/vercel.json` — complete: 1–5 (website-01).

### Contract and context documentation

189 files inventoried. Specifications, instructions, current plans, known-issue records and relevant documentation were
consulted to judge code contracts and overlap. This list does not claim a line-by-line editorial review of every
document; that was outside the agreed code-review scope. Executable website source and configuration are in the complete
list above.

- `.agents/narrow-tests.md`
- `.agents/test-authoring.md`
- `AGENTS.md`
- `BUGS.md`
- `BUGS_BURNDOWN.md`
- `CHANGELOG.md`
- `CLAUDE.md`
- `FLAKES.md`
- `README.md`
- `SPEC.md`
- `SPEC_impl.md`
- `THIRD_PARTY_NOTICES.md`
- `TODO.md`
- `TRIAGE_OUTCOMES.md`
- `crates/farhelm-desktop/README.md`
- `crates/farhelm-helm/tests/fixtures/release/README.md`
- `deflake/AGENTS.md`
- `deflake/CLAUDE.md`
- `deflake/EVAL.md`
- `deflake/SPEC.md`
- `docs/agent-screen-fixtures.md`
- `docs/browser-limitations.md`
- `docs/codex-input-investigation.md`
- `docs/codex-replace-investigation.md`
- `docs/desktop-web-triage.md`
- `docs/github-checkouts.md`
- `docs/harness-marks.md`
- `docs/install_uninstall.md`
- `docs/manual-mac-checklist.md`
- `docs/old_readme.md`
- `docs/readme-hero/SPEC.md`
- `docs/security.md`
- `docs/test-run-evidence.md`
- `docs/test-sleep-check.md`
- `docs/watch-items.md`
- `docs/window-restoration-investigation.md`
- `e2e/harness-tests/README.md`
- `e2e/tests/fixture-contracts.md`
- `plans/AGENTS.md`
- `plans/CLAUDE.md`
- `plans/INDEX.md`
- `plans/triage-clipboard-terminal-limit.md`
- `plans/triage-confirm-ssh-identity.md`
- `plans/triage-feed-sink-identity.md`
- `plans/triage-restart-takeover-update.md`
- `plans/triage-signin-status-paths.md`
- `plans/triage-yolo-sighup-replace.md`
- `releasing/AGENTS.md`
- `releasing/CLAUDE.md`
- `releasing/EDITORIAL_GUIDANCE.md`
- `releasing/changelog.d/README.md`
- `releasing/changelog.d/claude-last-option-waiting.md`
- `releasing/changelog.d/codex-last-option-waiting.md`
- `releasing/changelog.d/codex-working-widget.md`
- `releasing/changelog.d/delete-waits-for-terminal-reader.md`
- `releasing/changelog.d/desktop-signin-keeps-action.md`
- `releasing/changelog.d/desktop-signin-retry.md`
- `releasing/changelog.d/duplicate-hosts-simplified.md`
- `releasing/changelog.d/event-feed-keepalive.md`
- `releasing/changelog.d/hosts-panel-lock.md`
- `releasing/changelog.d/non-utf8-cwd-refused.md`
- `releasing/changelog.d/non-utf8-supervisor-paths.md`
- `releasing/changelog.d/pi-extension-v2.md`
- `releasing/changelog.d/profile-popup-lock.md`
- `releasing/changelog.d/seen-toggle-no-crash.md`
- `review_feedback_queue/AGENTS.md`
- `review_feedback_queue/CLAUDE.md`
- `review_feedback_queue/FILTER.md`
- `review_feedback_queue/INDEX.md`
- `review_feedback_queue/agent-create-aborted-on-retire.md`
- `review_feedback_queue/attach-reports-generic-timeout.md`
- `review_feedback_queue/browser-signin-loses-action-outcomes.md`
- `review_feedback_queue/build-metadata-false-old-version.md`
- `review_feedback_queue/checkout-reconciliation-blocks-terminal-reader.md`
- `review_feedback_queue/checkout-retry-raw-device-check.md`
- `review_feedback_queue/claude-capture-warns-forever.md`
- `review_feedback_queue/claude-clear-report-dropped-on-claim-timeout.md`
- `review_feedback_queue/claude-resume-template-selector-collision.md`
- `review_feedback_queue/claude-scan-budget-never-settles.md`
- `review_feedback_queue/claude-scan-claims-foreign-record.md`
- `review_feedback_queue/clipboard-writes-unbounded-blocking-admission.md`
- `review_feedback_queue/codex-draft-mistaken-for-question.md`
- `review_feedback_queue/codex-resume-template-duplicates-selector.md`
- `review_feedback_queue/confirmed-nothing-alive-prompt-kills-live-agent.md`
- `review_feedback_queue/create-dialog-empty-catalog-refuses.md`
- `review_feedback_queue/create-directory-wait-blocks-terminal-reader.md`
- `review_feedback_queue/create-rollback-orphans-unconfirmed-scope.md`
- `review_feedback_queue/delete-skips-scoped-tab-on-stale-verdict.md`
- `review_feedback_queue/desktop-auth-ready-with-stale-webview-credential.md`
- `review_feedback_queue/desktop-clipboard-fetch-backlog.md`
- `review_feedback_queue/desktop-copy-fallback-never-runs.md`
- `review_feedback_queue/desktop-protocol-filesystem-fallback.md`
- `review_feedback_queue/desktop-reauth-failure-loses-action-outcomes.md`
- `review_feedback_queue/desktop-start-fails-on-skewed-supervisor.md`
- `review_feedback_queue/drop-on-hidden-terminal-navigates-away.md`
- `review_feedback_queue/dropped-create-skips-bookkeeping.md`
- `review_feedback_queue/env-wrapper-hides-command-not-found.md`
- `review_feedback_queue/escape-token-clamp-too-short.md`
- `review_feedback_queue/event-feed-cap-refusal-invisible.md`
- `review_feedback_queue/folder-picker-skips-symlinks.md`
- `review_feedback_queue/header-actions-skip-listing-read.md`
- `review_feedback_queue/header-replace-recomputes-alive.md`
- `review_feedback_queue/host-write-lock-split-on-actor-respawn.md`
- `review_feedback_queue/hostnotfound-refresh-keeps-serving.md`
- `review_feedback_queue/incarnation-counter-restarts-per-process.md`
- `review_feedback_queue/list-admission-blocks-terminal-reader.md`
- `review_feedback_queue/list-ingress-id-validation-gap.md`
- `review_feedback_queue/merged-list-crowded-by-one-host.md`
- `review_feedback_queue/new-tab-mount-displaces-owner-during-recovery.md`
- `review_feedback_queue/omp-corridor-uncounted-pane-runtime.md`
- `review_feedback_queue/opencode-bare-model-rejected.md`
- `review_feedback_queue/opencode-bare-model-switches-harness.md`
- `review_feedback_queue/partial-release-download-left-behind.md`
- `review_feedback_queue/pi-pointer-overrides-user-prompt.md`
- `review_feedback_queue/pi-resume-selector-option-boundaries.md`
- `review_feedback_queue/probe-cancellation-leaves-helper-processes.md`
- `review_feedback_queue/probe-drops-add-busy-claim.md`
- `review_feedback_queue/probe-register-not-helm-owned.md`
- `review_feedback_queue/probe-reregister-drops-terminals.md`
- `review_feedback_queue/process-snapshot-requires-supervisor-witness.md`
- `review_feedback_queue/profile-body-accepts-unknown-fields.md`
- `review_feedback_queue/provision-lock-map-grows-per-requested-id.md`
- `review_feedback_queue/provisioning-child-output-drain-deadline.md`
- `review_feedback_queue/provisioning-update-replaces-binary-before-setup-guard.md`
- `review_feedback_queue/refresh-starved-by-seeds.md`
- `review_feedback_queue/rename-admission-blocks-terminal-reader.md`
- `review_feedback_queue/replace-drop-skips-source-delete.md`
- `review_feedback_queue/replace-with-kills-running-source-unwarned.md`
- `review_feedback_queue/restart-admission-blocks-terminal-reader.md`
- `review_feedback_queue/restart-can-still-deselect-session.md`
- `review_feedback_queue/restart-with-skips-create-validation.md`
- `review_feedback_queue/retarget-race-republishes-old-client.md`
- `review_feedback_queue/row-menu-drifts-on-row-height-change.md`
- `review_feedback_queue/seen-write-cancellation-skips-notification.md`
- `review_feedback_queue/session-detail-drains-full-list.md`
- `review_feedback_queue/session-view-leaks-page-lock.md`
- `review_feedback_queue/sessions-changed-hint-unthrottled.md`
- `review_feedback_queue/sidebar-replace-recomputes-alive.md`
- `review_feedback_queue/sighup-skips-orderly-shutdown.md`
- `review_feedback_queue/ssh-config-remotecommand-blocks-host.md`
- `review_feedback_queue/ssh-forwarding-inherited.md`
- `review_feedback_queue/stop-admission-blocks-terminal-reader.md`
- `review_feedback_queue/stop-restart-panic-no-reply.md`
- `review_feedback_queue/tab-cleanup-blocks-status-sampling.md`
- `review_feedback_queue/tab-reap-budget-starved-by-failures.md`
- `review_feedback_queue/takeover-latch-misses-attaching-tabs.md`
- `review_feedback_queue/terminal-font-promise-leak.md`
- `review_feedback_queue/terminal-output-queue-missing-byte-budget.md`
- `review_feedback_queue/terminal-tombstone-never-buried.md`
- `review_feedback_queue/tilde-in-remote-path-fields.md`
- `review_feedback_queue/tmux-build-script-bash32.md`
- `review_feedback_queue/update-silently-downgrades-newer-hosts.md`
- `review_feedback_queue/upload-cancellation-drops-final-reply.md`
- `review_feedback_queue/uploads-aborted-silently-on-remount.md`
- `review_feedback_queue/yolo-guard-fails-open-without-row.md`
- `review_feedback_queue/yolo-guard-misses-codex-option-form.md`
- `review_feedback_queue/yolo-guard-misses-env-prefix.md`
- `review_feedback_queue/yolo-guard-misses-equivalent-spellings.md`
- `review_feedback_queue/yolo-guard-skips-resume-template.md`
- `review_feedback_queue/yolo-safe-survives-identity-adoption.md`
- `website/AGENTS.md`
- `website/CLAUDE.md`
- `website/src/content/docs/docs/agents/agent-hook-injection.md`
- `website/src/content/docs/docs/agents/agent-wrappers.md`
- `website/src/content/docs/docs/agents/claude.md`
- `website/src/content/docs/docs/agents/codex.md`
- `website/src/content/docs/docs/agents/cursor.md`
- `website/src/content/docs/docs/agents/custom-commands.md`
- `website/src/content/docs/docs/agents/goose.md`
- `website/src/content/docs/docs/agents/grok.md`
- `website/src/content/docs/docs/agents/index.md`
- `website/src/content/docs/docs/agents/muse.md`
- `website/src/content/docs/docs/agents/omp.md`
- `website/src/content/docs/docs/agents/opencode.md`
- `website/src/content/docs/docs/agents/pi.md`
- `website/src/content/docs/docs/get-started/add-a-remote-host.md`
- `website/src/content/docs/docs/get-started/choose-your-setup.md`
- `website/src/content/docs/docs/get-started/first-session.md`
- `website/src/content/docs/docs/get-started/install.md`
- `website/src/content/docs/docs/how-it-works/security-model.md`
- `website/src/content/docs/docs/how-it-works/the-pieces.md`
- `website/src/content/docs/docs/how-it-works/what-survives-what.md`
- `website/src/content/docs/docs/index.mdx`
- `website/src/content/docs/docs/using/manage-hosts.md`
- `website/src/content/docs/docs/using/session-list.md`
- `website/src/content/docs/docs/using/start-a-session.md`
- `website/src/content/docs/docs/using/stop-restart-resume.md`
- `website/src/content/docs/docs/using/update-and-uninstall.md`
- `website/src/content/docs/docs/using/work-in-a-session.md`

### Static data and fixtures

53 files inventoried; excluded from line-by-line review by the agreed scope. Owning code, dependency declarations and
relevant build/consumption paths were reviewed.

- `.github/release/ziglang-requirements.txt`
- `LICENSE`
- `crates/farhelm-supervisor/tests/fixtures/screens/claude/2.1.285/idle-after-turn.title`
- `crates/farhelm-supervisor/tests/fixtures/screens/claude/2.1.285/idle-after-turn.txt`
- `crates/farhelm-supervisor/tests/fixtures/screens/claude/2.1.285/idle-clear-hint.title`
- `crates/farhelm-supervisor/tests/fixtures/screens/claude/2.1.285/idle-clear-hint.txt`
- `crates/farhelm-supervisor/tests/fixtures/screens/claude/2.1.285/idle-fresh.title`
- `crates/farhelm-supervisor/tests/fixtures/screens/claude/2.1.285/idle-fresh.txt`
- `crates/farhelm-supervisor/tests/fixtures/screens/claude/2.1.285/unknown-model-picker.title`
- `crates/farhelm-supervisor/tests/fixtures/screens/claude/2.1.285/unknown-model-picker.txt`
- `crates/farhelm-supervisor/tests/fixtures/screens/claude/2.1.285/unknown-transcript.title`
- `crates/farhelm-supervisor/tests/fixtures/screens/claude/2.1.285/unknown-transcript.txt`
- `crates/farhelm-supervisor/tests/fixtures/screens/claude/2.1.285/waiting-derived-last-option.title`
- `crates/farhelm-supervisor/tests/fixtures/screens/claude/2.1.285/waiting-derived-last-option.txt`
- `crates/farhelm-supervisor/tests/fixtures/screens/claude/2.1.285/waiting-permission.title`
- `crates/farhelm-supervisor/tests/fixtures/screens/claude/2.1.285/waiting-permission.txt`
- `crates/farhelm-supervisor/tests/fixtures/screens/claude/2.1.285/waiting-question.title`
- `crates/farhelm-supervisor/tests/fixtures/screens/claude/2.1.285/waiting-question.txt`
- `crates/farhelm-supervisor/tests/fixtures/screens/claude/2.1.285/waiting-trust.title`
- `crates/farhelm-supervisor/tests/fixtures/screens/claude/2.1.285/waiting-trust.txt`
- `crates/farhelm-supervisor/tests/fixtures/screens/claude/2.1.285/working-thinking.title`
- `crates/farhelm-supervisor/tests/fixtures/screens/claude/2.1.285/working-thinking.txt`
- `crates/farhelm-supervisor/tests/fixtures/screens/claude/2.1.285/working-tool-1.title`
- `crates/farhelm-supervisor/tests/fixtures/screens/claude/2.1.285/working-tool-1.txt`
- `crates/farhelm-supervisor/tests/fixtures/screens/claude/2.1.285/working-tool-2.title`
- `crates/farhelm-supervisor/tests/fixtures/screens/claude/2.1.285/working-tool-2.txt`
- `crates/farhelm-supervisor/tests/fixtures/screens/codex/0.159.0/idle-after-turn.title`
- `crates/farhelm-supervisor/tests/fixtures/screens/codex/0.159.0/idle-after-turn.txt`
- `crates/farhelm-supervisor/tests/fixtures/screens/codex/0.159.0/idle-derived-weekly-footer.title`
- `crates/farhelm-supervisor/tests/fixtures/screens/codex/0.159.0/idle-derived-weekly-footer.txt`
- `crates/farhelm-supervisor/tests/fixtures/screens/codex/0.159.0/idle-fresh.title`
- `crates/farhelm-supervisor/tests/fixtures/screens/codex/0.159.0/idle-fresh.txt`
- `crates/farhelm-supervisor/tests/fixtures/screens/codex/0.159.0/idle-recap.title`
- `crates/farhelm-supervisor/tests/fixtures/screens/codex/0.159.0/idle-recap.txt`
- `crates/farhelm-supervisor/tests/fixtures/screens/codex/0.159.0/unknown-model-picker.title`
- `crates/farhelm-supervisor/tests/fixtures/screens/codex/0.159.0/unknown-model-picker.txt`
- `crates/farhelm-supervisor/tests/fixtures/screens/codex/0.159.0/waiting-derived-last-option.title`
- `crates/farhelm-supervisor/tests/fixtures/screens/codex/0.159.0/waiting-derived-last-option.txt`
- `crates/farhelm-supervisor/tests/fixtures/screens/codex/0.159.0/waiting-permission.title`
- `crates/farhelm-supervisor/tests/fixtures/screens/codex/0.159.0/waiting-permission.txt`
- `crates/farhelm-supervisor/tests/fixtures/screens/codex/0.159.0/waiting-trust.title`
- `crates/farhelm-supervisor/tests/fixtures/screens/codex/0.159.0/waiting-trust.txt`
- `crates/farhelm-supervisor/tests/fixtures/screens/codex/0.159.0/working-streaming.title`
- `crates/farhelm-supervisor/tests/fixtures/screens/codex/0.159.0/working-streaming.txt`
- `crates/farhelm-supervisor/tests/fixtures/screens/codex/0.159.0/working-tool-1.title`
- `crates/farhelm-supervisor/tests/fixtures/screens/codex/0.159.0/working-tool-1.txt`
- `crates/farhelm-supervisor/tests/fixtures/screens/codex/0.159.0/working-tool-2.title`
- `crates/farhelm-supervisor/tests/fixtures/screens/codex/0.159.0/working-tool-2.txt`
- `deflake/known-flakes.txt`
- `docs/readme-hero/filler-claude.txt`
- `docs/readme-hero/filler-codex.txt`
- `docs/readme-hero/transcript.txt`
- `scripts/test-sleep-requirements.txt`

### Binary assets and generated geometry

30 files inventoried; excluded from line-by-line review by the agreed scope. Owning code, dependency declarations and
relevant build/consumption paths were reviewed.

- `crates/farhelm-helm/src/provisioning/farhelm-release.pub`
- `crates/farhelm-helm/tests/fixtures/release/SHA256SUMS.minisig`
- `crates/farhelm-helm/tests/fixtures/release/farhelm-aarch64-apple-darwin.tar.gz`
- `crates/farhelm-helm/tests/fixtures/release/farhelm-aarch64-unknown-linux-musl.tar.gz`
- `crates/farhelm-helm/tests/fixtures/release/farhelm-desktop-aarch64-apple-darwin.tar.gz`
- `crates/farhelm-helm/tests/fixtures/release/farhelm-x86_64-unknown-linux-musl.tar.gz`
- `crates/farhelm-helm/tests/fixtures/release/test-key.pub`
- `crates/farhelm-helm/tests/fixtures/release/variants/legacy-signature/SHA256SUMS.minisig`
- `crates/farhelm-helm/tests/fixtures/release/variants/other-version/SHA256SUMS.minisig`
- `crates/farhelm-helm/tests/fixtures/release/variants/two-member/SHA256SUMS.minisig`
- `crates/farhelm-helm/tests/fixtures/release/variants/two-member/farhelm-x86_64-unknown-linux-musl.tar.gz`
- `crates/farhelm-helm/tests/fixtures/release/variants/without-tmux/SHA256SUMS.minisig`
- `crates/farhelm-ui/src/icons/goose.svgpath`
- `crates/farhelm-ui/src/icons/openai-blossom.svgpath`
- `crates/farhelm/tests/fixtures/helm-v26.db`
- `docs/farhelm-architecture.svg`
- `packaging/farhelm-desktop/Farhelm.icns`
- `packaging/farhelm-desktop/icon.png`
- `packaging/farhelm-desktop/icon.svg`
- `packaging/farhelm-desktop/wordmark-dark.svg`
- `packaging/farhelm-desktop/wordmark-light.svg`
- `website/scripts/wordmark-outline.json`
- `website/src/assets/intro/header-dark.svg`
- `website/src/assets/intro/header-light.svg`
- `website/src/assets/intro/how-it-works-dark.svg`
- `website/src/assets/intro/how-it-works-light.svg`
- `website/src/assets/intro/pillars-dark.svg`
- `website/src/assets/intro/pillars-light.svg`
- `website/src/site-mark-dark.svg`
- `website/src/site-mark-light.svg`

### Vendored dependencies and fonts

22 files inventoried; excluded from line-by-line review by the agreed scope. Owning code, dependency declarations and
relevant build/consumption paths were reviewed.

- `crates/farhelm-supervisor/src/service/core/vendor/claude.rs`
- `crates/farhelm-supervisor/src/service/core/vendor/codex.rs`
- `crates/farhelm-supervisor/src/service/core/vendor/grok.rs`
- `crates/farhelm-supervisor/src/service/core/vendor/mod.rs`
- `crates/farhelm-supervisor/src/service/core/vendor/omp.rs`
- `crates/farhelm-ui/assets/fonts/JetBrainsMonoNerdFont-Bold.woff2`
- `crates/farhelm-ui/assets/fonts/JetBrainsMonoNerdFont-Regular.woff2`
- `crates/farhelm-ui/assets/fonts/OFL.txt`
- `crates/farhelm-ui/assets/vendor/addon-clipboard-LICENSES.txt`
- `crates/farhelm-ui/assets/vendor/addon-clipboard.js`
- `crates/farhelm-ui/assets/vendor/addon-fit.js`
- `crates/farhelm-ui/assets/vendor/addon-web-links-LICENSES.txt`
- `crates/farhelm-ui/assets/vendor/addon-web-links.js`
- `crates/farhelm-ui/assets/vendor/xterm.css`
- `crates/farhelm-ui/assets/vendor/xterm.js`
- `website/public/fonts/Inter-Bold.woff2`
- `website/public/fonts/Inter-Regular.woff2`
- `website/public/fonts/Inter-SemiBold.woff2`
- `website/public/fonts/JetBrainsMonoNerdFont-Bold.woff2`
- `website/public/fonts/JetBrainsMonoNerdFont-Regular.woff2`
- `website/public/fonts/OFL-Inter.txt`
- `website/public/fonts/OFL-JetBrainsMono.txt`

### Generated dependency locks

3 files inventoried; excluded from line-by-line review by the agreed scope. Owning code, dependency declarations and
relevant build/consumption paths were reviewed.

- `Cargo.lock`
- `e2e/package-lock.json`
- `website/bun.lock`

### Archived historical documentation

38 files inventoried; excluded from line-by-line review by the agreed scope. Current specifications and maintained plans
governed the review.

- `lore/2026-07-26-m1-desktop-is-a-thin-client.md`
- `lore/2026-07-26-m1-review-swarm-findings.md`
- `lore/2026-07-27-m2-process-tree-stop.md`
- `lore/2026-07-27-m2-sqlite-restart-gap.md`
- `lore/2026-07-29-m2-5-backpressure-via-tmux-pause-after.md`
- `lore/2026-07-29-m3-planning-decisions.md`
- `lore/2026-08-20-protocol-version-changelog.md`
- `lore/2026-08-24-agent-reported-conversation-identity.md`
- `lore/2026-08-30-fly-sprites-as-a-host-kind.md`
- `lore/2026-09-01-tensorlake-sandboxes-as-a-host-kind.md`
- `lore/2026-09-04-codebase-simplification-assessment.md`
- `lore/2026-09-06-security-risk-surface-assessment.md`
- `lore/2026-09-08-systematic-deflake-end-state.md`
- `lore/2026-09-12-libghostty-assessment.md`
- `lore/2026-09-12-scroll-freeze-workaround.md`
- `lore/2026-09-16-island-cap-never-connected-first-mount.md`
- `lore/2026-09-16-rotation-recovery-unanswered-reads.md`
- `lore/2026-09-17-github-clone-launch-brainstorm.md`
- `lore/2026-09-20-harness-conversation-ownership.md`
- `lore/2026-09-23-goose-session-tracking.md`
- `lore/2026-09-25-protocol-version-notes-to-v30.md`
- `lore/2026-09-29-harness-boundary-refactor-proposal.md`
- `lore/2026-09-29-per-session-sandboxes-as-the-remote-host-goal.md`
- `lore/AGENTS.md`
- `lore/CLAUDE.md`
- `lore/PLAN.md`
- `lore/PLAN_M0.md`
- `lore/PLAN_M1.md`
- `lore/PLAN_M2.md`
- `lore/PLAN_M2_5.md`
- `lore/PLAN_M3.md`
- `lore/PLAN_M4.md`
- `lore/PLAN_M5.md`
- `lore/PLAN_M6.md`
- `lore/PLAN_M6_5.md`
- `lore/PLAN_M6_75.md`
- `lore/PLAN_M7.md`
- `lore/PLAN_desktop_web_bug_triage.md`
