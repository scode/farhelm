# What this was about

Five triaged findings about Linux setup/uninstall overlap, supervisor lock cleanup, helm shutdown, large template
catalogs and Pi conversation reporting. The maintainer requested one PR and small fixes, with a complexity gate for each
outcome. Three fixes are carried out. Setup/uninstall exclusion and template discovery return pending for the decisions
their gates require. Nothing is silently discarded.

# Things you should know

Cancelled supervisor requests no longer leave dead per-key lock entries after the next lookup. A live holder or waiter
keeps its same mutex. Helm shutdown now withdraws and retires published supervisor connections even if callers retain
copies. This has no demonstrated consequence in a shipped configuration, where the process or runtime ends anyway.
During panic unwinding, shutdown still withdraws clients and cancels host tasks but skips explicit retirement to avoid a
second panic from poisoned client bookkeeping.

When Pi changes conversations, its reporter tells the supervisor which conversation Resume should use. If reading Pi's
current conversation or preparing that report throws, this one report is now dropped silently instead of the error
escaping into Pi. Later reports can still run; Resume can remain on the earlier conversation until another report
succeeds. Report order, the check that rejects an older conversation's queued report, and the two-second child timeout
stay intact. A new published reporter file lets hosts already holding the old immutable file receive the correction. No
vendor getter failure was observed; no website caveat or new JavaScript execution harness was added. Runtime validation
here covers file publication and content pinning; the error-containment behavior was checked from source.

The three completed findings' feedback files and index entries are removed, and their Execution fields identify this
change and PR. Their recorded Outcomes are unchanged. The two pending findings retain their feedback and index entries.
No user-facing effect was established for the three shipped fixes; the plan expressly requires no changelog fragment
when its setup and template gates leave this scope.

# Open questions and possible follow-ups

**Setup/uninstall overlap remains unfixed.** On a fresh Linux installation, overlapping `farhelm helm setup` and
`farhelm uninstall` can still leave services pointing at a program uninstall removed while uninstall reports success.
The proposed fix took the existing service lock before first setup and rechecked the executable before publication.
Final review found that uninstall can still reach its last check without being able to contact the service manager,
while another invocation can contact it and publish services. The two invocations can also select different unit
directories. A read-only check for the installer lock leaves a race with its creation. The partial fix, its tests,
specification changes and changelog fragment were removed; this finding's Execution remains `pending`, waiting on the
maintainer, as the agreed gate requires.

The smallest policy change is to refuse standalone uninstall when the manager cannot identify its authoritative unit
directory, giving up today's CLI-only removal without a reachable manager. To preserve that support, separately scope
shared exclusion tied to the executable rather than manager discovery. Reusing the installer lock needs a precise rule
for which supported installations participate; taking it for every setup would newly require writing beside arbitrary
binaries, including root-owned system installations. An executable-keyed lock elsewhere would change the lock layout.
Recommend a separately scoped shared-exclusion change that preserves current support and settles that participation
contract. The review established scope limits, not that a complete fix is impossible.

**Template discovery remains unfixed.** Agents on any host can still save enough valid templates to make discovery
unusable on every host until some templates are deleted. The agreed cheap proposal capped stored names and fields, but
the reply also repeats each template's current host display name. SSH destinations have no length bound, and host
renaming, clearing an alias or later identity resolution can enlarge the reply after a template was saved. Per-row
encoding overhead also needs accounting. A save-only cap cannot guarantee a future complete listing fits its 8 MiB frame
under the existing supported inputs.

The template finding remains in the feedback queue with Execution `pending`, waiting on the maintainer. Real choices are
a smaller save-time snapshot guarantee that leaves later host changes outside it; bounded host display names plus
complete catalog accounting, which requires deciding whether long names are shortened in listings or refused as host
inputs and how existing hosts are treated; or complete byte-bounded pagination, which also handles existing oversized
catalogs. Pagination requires a continuation rule and agents that fetch all pages, with compatibility for existing
agents settled explicitly. That changes listing behavior beyond the agreed save-time cap and merits a separate plan.
Recommend that byte-bounded listing/pagination plan. No cap, schema, listing protocol, template specification or website
behavior changed in this PR. This is the plan's explicit gate return for a cross-host issue, never a discard.

# The PRs

- [#1934 — close helm connections and contain reporter errors](https://github.com/scode/farhelm/pull/1934/changes),
  draft; three small fixes and two pending returns, one change `qmtvpknn` and bookmark
  `plan/helm-supervisor-small-fixes/01-small-fixes`.

# Checks run, reused and skipped

- Run earlier, reused for unchanged shipped code: focused recorder-backed nextest, run
  `d25a8407-712b-4f48-9a31-0bc7da6e6cde`: 115 selected tests passed, zero failures or flaky results, four slots and zero
  retries, pinned nextest 0.9.148 and repository-built tmux 3.7c. Selection covers setup/uninstall unit modules,
  cancelled lock waiters and existing handoff, Pi asset publication/pin tests, and helm
  client-retirement/terminal-shutdown tests. Complete retained output contains no runtime `SKIPPED` markers; 2,201 cases
  were selected out rather than executed. Setup coverage belongs to the discarded draft and does not prove the
  unresolved race fixed. Its tests and source were restored together.
- Run earlier: Clippy for supervisor/helm libraries and tests, CLI all targets, and shipped CLI binaries without
  supervisor test seams; changelog-format lint; isolated test-delay checker, 288 delays and zero missing rationale. The
  supervisor/helm lint evidence is reused for the unchanged shipped code. The final delay check after rebase inspected
  289 delays and found zero missing rationales. CLI lint and changelog checks apply only to the removed setup draft and
  are not final-scope coverage requirements.
- Run now: Cargo formatting, dprint for changed Markdown and `git diff --check` passed after the gate return and careful
  rebase. Recorder run `44ac083e-376f-499b-b508-2b583ce849c9` then passed all six selected helm shutdown/retirement
  tests, including the strengthened shutdown/reconciliation interleaving from main. The run used four slots, zero
  retries and the same pinned tools; complete retained output has no runtime `SKIPPED` markers. The other 1,038 cases
  were selected out.
- Carefully rebased from `67b0220a` through `1d6b9eac` to `16b27d5b`, reading every intervening diff. Main added
  separate end-to-end test oracles, the earlier supervisor/helm unit-test corrections, test-state cleanup and queue
  bookkeeping. The feedback-index adjacency conflict preserved both plans' removals. Keyed-lock and Pi product paths
  stayed unchanged. The strengthened pending-registry shutdown test interacts with retirement; it receives a narrow
  combined-behavior run, which passed. A subsequent clean rebase onto `82e95f34` included queue state changes and
  another plan's report only; all source and runtime evidence still applies. No broader product or specification
  conflict was found.
- Skipped: full workspace lint/runtime, browser/e2e, installer, provisioning, desktop and website checks. The selected
  lint configurations and regression modules cover the changed paths; none of those broader surfaces changed. No doctest
  behavior changed. macOS uninstall acceptance cannot execute on Linux; no uninstall code remains in this PR, so no
  hosted macOS run was dispatched. No tests or setup/uninstall action used the operator's live installation.

# Review gate outcome

The plan-required fresh gpt-6.1-sol high acceptance review approved the three shipped fixes and template return, and
found the remaining setup race described above. A second fresh scope review on the same requested model/effort
recommended setup's authorized pending return. Its artifact was read and the underlying lock, manager-discovery,
ownership and removal ordering checked by the executor. The bounded acceptance follow-up found no further defects in the
removal and rebase, including no lock cycle with main's strengthened shutdown test. Implementation and validation stayed
with the executor, as requested. The first commit-wording cold reader found a shutdown-observability overclaim and
uppercase title. A fresh gpt-6.1-sol high reader confirmed the rewritten message's claims and conventions; it was
accepted after reading the complete artifact. The fresh report reader found no public-repository hygiene violation and
requested clearer product wording for Pi and concrete tradeoffs for the template options. Both were incorporated; the
reader accepted the revised account as sufficient for a standalone maintainer decision and confirmed public hygiene.
Private galaxy-brain session evidence is referenced in the working log. Native tools expose neither actual model
attribution nor token usage; one local-decision event was recorded late with an explicit gap, not reconstructed timing.

### Landing

Landed on 2026-10-12 (UTC) as #1934. The rebase met no conflicts.

#### Fix made while landing

In #1934, before it merged: the PR had no changelog fragment, which every `fix:` PR needs (with `kind: none` and a
reason when nothing is user-facing); the plan's own wording to skip one conflicted with that rule. A `kind: fixed`
fragment now says the Pi conversation reporter drops a failed report quietly instead of letting the error reach Pi, and
that the helm closes host connections cleanly on shutdown.

#### Review before merging

No other findings. Shutdown now withdraws and retires each published host client the way host removal does, exactly once
and safely from `Drop`; dead per-key supervisor locks are pruned without touching live ones; and every read in the Pi
reporter now runs inside its contained step. The changed asset is published under a new name,
`farhelm-conversation-v3.ts`, with its hash pinned, so running Pi sessions keep the old file. Notes left as they are:

- Two outcomes hit their complexity gates and went back to you as pending: the setup and uninstall race, and the
  template catalog cap. Their findings stay in the review queue.
- A host actor running on another thread can still publish a client just after shutdown withdraws, as host removal
  already can.
- The Pi reporter now checks the session file when the report runs rather than when the event fires, as the OMP reporter
  does.

#### Checks

These two plans were landed together, supervisor-and-test-followups first with helm-supervisor-small-fixes on top.

- Run now, on the two stacked after the fixes: `cargo fmt --all -- --check`, `dprint check`, the changelog lint, the
  test-delay checker (291 delays, none without a reason), `cargo clippy --all-targets -- -D warnings` and
  `cargo clippy -p farhelm --bins -- -D warnings`; the supervisor and helm unit tests with the hook-identity,
  session-lifecycle and terminal-tabs end-to-end tests, through the recorder with pinned tmux 3.7c, nextest 0.9.148,
  four slots and no retries (run `8af684a9`: 2293 of 2294, the one failure described in supervisor-and-test-followups'
  notes, then the ticker tests after its fix in run `32ad57a7`, 58 of 58); the supervisor's reporter asset tests (10 of
  10); and on Chromium and WebKit with one worker and no retries the spawn and real-agent specs, which start the browser
  test stack whose launcher changed (run `9cacf976`, 18 passed, the four opt-in real-agent tests skipped as designed).
- Between those runs and the merges, main changed only plan bookkeeping.

Nothing in the report above was made untrue by the landing.
