# Fresh-checkout provenance: fix the stale fresh-checkout e2e test and the list refresh that fails during Replace with

Written against main at fbc89316 on 2026-10-03. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

This plan has no dependency on another plan.

## The goal

The TODO.md `Near term` entry "A fresh-checkout end-to-end test fails on main, and Replace with logs a provenance error"
(verbatim below) covers two problems that planning research found to be unrelated. Fix both, one PR each:

1. The e2e test `lost_fresh_checkout_success_replays_after_settings_change_and_helm_restart` passes again and still
   checks what it was written to check: that a new create carrying a preview taken before a checkout-settings change is
   refused with the "checkout settings changed" refusal.
2. A host's session-list refresh no longer fails with "the session's fresh-checkout provenance does not match its
   registry evidence" when a session with a fresh checkout is deleted (as Replace with does to its source) while the
   refresh runs.

Acceptance criteria: the test passes on main's substrate; a deterministic regression test reproduces the provenance
failure before the fix and passes after it; the second PR removes the TODO entry.

## Requirement sources

**The user's request:** "use planning system to plan: Fresh-checkout test and provenance error, install.sh output,
uninstall after move", later "make sure we plan these as separate plans". The TODO.md entry, verbatim as of fbc89316:

- "**A fresh-checkout end-to-end test fails on main, and Replace with logs a provenance error.** The e2e test
  `lost_fresh_checkout_success_replays_after_settings_change_and_helm_restart` fails every time on main (reproduced on
  2026-10-02 by the boundary-checks plan, on unmodified main): it expects the refusal that names changed checkout
  settings and instead gets the refusal for a host connection that changed across the helm restart. Separately, while a
  fresh checkout replaced another session (Replace with), the helm's periodic session-list refresh once failed with the
  supervisor reporting "the session's fresh-checkout provenance does not match its registry evidence". The next refresh
  succeeded and nothing visible followed; the same error appears in a 2026-09-30 browser run of the existing
  Replace-with test. Both concern fresh-checkout bookkeeping, so they are investigated together; whether they share a
  cause is unknown."

**The user's decisions (2026-10-03):** the user agreed with both proposed fixes as stated in the outline below (fix the
test, not the product's check order; fix the race in the supervisor with a deterministic test). Review gate: a
fresh-context Opus 5.5 reviewer at high effort, no swarm. No-workhorse mode.

**Planning research, verified by reading code on main at fbc89316 (not by running anything):**

- The test (`crates/farhelm/tests/e2e/github_checkouts.rs`) creates fresh checkouts under checkout root A, drops the
  replies, switches the configuration to root B, restarts only the helm (which gives it a new connection claim), replays
  the original requests successfully, then sends one request with a new intent key and expects a 409 whose body says
  "checkout settings changed". In the helm (`create_fresh_session` in `crates/farhelm-helm/src/sessions.rs`), the
  incarnation checks (`incarnation_holds`, refusing with `IncarnationStale` from
  `crates/farhelm-helm/src/precondition.rs`, "host N is not the connection this request was prepared against …") run
  before `github_checkout_resolution` compares the preview's configuration revision. That order dates from #725. What
  changed is #1455 (commit 47752522, 2026-10-02, "fix: start connection numbers at a random value in each helm
  process"): before it, a restarted helm counted connections from 1 again, so the pre-restart preview's incarnation
  matched by coincidence and the configuration check produced the expected text. #1455 removed exactly that coincidence.
  SPEC.md "Fresh GitHub checkouts" and SPEC_impl.md (the composer's preview binding, "Checkout configuration and
  discovery") do not rank the two refusals, and a helm restart is a reconnection, which `IncarnationStale` is documented
  to cover. So the product is right and the test is stale.
- The provenance error comes from `SessionStore::origin_working_copy` in `crates/farhelm-supervisor/src/store.rs`.
  Inside one store call it reads the session row's `fresh_checkout_id` (with `.optional()?.flatten()`, which turns "no
  session row" and "row with no fresh checkout" into the same `None`) and the `working_copies` row whose
  `origin_session_id` is the session. `(None, Some(_))` is an error. Session Delete
  (`delete_session_archiving_memberships`) removes the session row but can leave the checkout's registry row in place
  with `origin_session_id` still naming the deleted session (a retired row, or one still borrowed).
  `status::interrupted_preparation_detail` (`crates/farhelm-supervisor/src/service/status.rs`) calls `store.session(id)`
  and then, in a separate store call, `store.origin_working_copy(id)`; `ListSessions` (`service/listing.rs`) reaches it
  for a session whose pane is dead or absent, which is what Replace with's source is while it is being stopped and
  deleted. If the Delete commits between the two store calls, the second sees no session row and the leftover origin
  row, and the `?` fails the whole list request. The helm then keeps its previous cache, logs a warning, and the Hosts
  panel briefly shows "the last session refresh failed" until the next refresh. This is a hypothesis to confirm, not a
  verified reproduction.

## Implementation outline

Two PRs, in this order. Line numbers drift; find code by name.

### PR 1: the fresh-checkout replay test checks the settings refusal again (`test:`)

- Reproduce first with the narrowest run (it should fail on the first attempt), through the recorder:
  `cargo nextest run -p farhelm --test e2e -E 'test(=github_checkouts::lost_fresh_checkout_success_replays_after_settings_change_and_helm_restart)'`
  as the child command of `scripts/record-test-run.py` with `--runner nextest --kind repetition`, pinned nextest and
  tmux on PATH per `docs/test-run-evidence.md`, `--tmux required`. Keep the failed run directory per root `AGENTS.md`.
- Fix the test, not the product: before sending the never-accepted request, rebind that request's preview incarnation
  and its `expected_incarnation` to the restarted helm's current claim (`stack.claim.incarnation` or however the test
  holds it after `restart_helm`), so the incarnation checks pass and the configuration-revision check is what refuses.
  Keep every other assertion. Update the test's docstring to say why the rebinding is needed (the restart gives the helm
  a new connection number since #1455, which would otherwise refuse first) and what the step now proves.
- If you find the refusal order is not what this file says, or the test cannot exercise the settings refusal without a
  product change, stop and block rather than reorder the helm's checks.
- No changelog fragment (`test:`).

### PR 2: a session deleted during a list refresh no longer fails the refresh (`fix:`)

- First, a deterministic regression test at the supervisor level that commits a Delete of a fresh-checkout session
  between the two store reads of `interrupted_preparation_detail` (or calls `origin_working_copy` for a session whose
  row was deleted while its registry row kept `origin_session_id`), and shows the error. If the hypothesis turns out
  wrong, investigate the real cause (the 2026-09-30 browser run of the Replace-with test,
  `"replacement preserves borrowers and archives only the released checkout"` in `e2e/tests/github-checkouts.spec.ts`,
  is where it was seen) and log what you find; if no cause can be established, block with the evidence instead of
  guessing a fix.
- Fix it in the observer, not in `origin_working_copy`'s shared contract: other callers rely on its fail-closed mismatch
  error. The direct production callers are `interrupted_preparation_detail` in `service/status.rs`, and in
  `service/core.rs` `validate_retry`, `launch_reserved` (twice; its second call `.expect`s the registry row, and its
  first decides whether a Retry reuses a fresh plan) and `restart_session`; `reload_sessions` reaches it through
  `interrupted_preparation_detail`. `service/handlers.rs` and `ticker.rs` reach it only through `status::observe_entry`.
  Preferred: give `interrupted_preparation_detail` one store call that reads the session row and its origin together
  (one `conn.call` cannot interleave with Delete's transaction, so the race disappears structurally, and a missing row
  means `Ok(None)`). Acceptable alternative: when `origin_working_copy` errors there, re-read the session and return
  `Ok(None)` if the row is gone. Do not change `origin_working_copy`'s behavior for the other callers. Document the rule
  and why in the new or changed function's docstring.
- Changelog fragment `kind: fixed` (a host's session list could briefly fail to refresh, showing "the last session
  refresh failed" in the Hosts panel, while Replace with removed a session that had its own GitHub checkout).
- Remove the TODO.md entry. This is the plan's last PR.

### What not to build

- No change to the order of the helm's create checks, and no new refusal ranking in the specs.
- No new locking or cross-call transaction protocol in the supervisor store unless the narrow fix above is shown wrong.
- No change to what Delete keeps in the checkout registry.

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is
`farhelm-plan-fresh-checkout-provenance-log.md` in the parent directory of the checkout you run in, derived as that
section says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory root
`AGENTS.md` describes, not in the checkout.

### Resume protocol

Your first action is to invoke the `agent-resumeable` skill with the log file's absolute path. If the log exists, read
it and resume where the previous session left off, cross-checking it against reality (the jj graph, bookmarks, open PRs,
TODO.md and `plans/queue/INDEX.md`) rather than starting over. If it does not exist, this is a fresh start. A plan that
an earlier executor worked on, or that came back from review, also gets the resume check in `plans/AGENTS.md` (Executing
one plan, step 7) before any work.

### Galaxy-brain, no-workhorse

The user requires you to use `$scode-galaxy-brain` to achieve this entire goal. Invoke it immediately after setting up
the resume protocol and keep it active for the whole run. The user forbids delegating any unit of your own
decomposition, read-only or writing: you do all of that work yourself and do not ask routing about it, and this demand
overrides galaxy-brain's own judgment of what is worth delegating. The spawns this file calls for (the review gate and
the scope reassessment review) are still routed and launched through galaxy-brain. The sub-agents `plans/AGENTS.md`
requires of every plan (the resume check, and the cold reads of a blocked question and of the report) are exempt from
the no-delegation demand and run as that file says, not through galaxy-brain.

### Resource watchdog

Immediately after activating galaxy-brain, start a resource watchdog as a background process (not an agent) and keep it
running for the whole run. Every 60 seconds it samples free space on the filesystems holding the checkout, the agent
scratch directory and `/tmp`, plus available memory and swap (`df`, and `free` or `/proc/meminfo` on Linux; `vm_stat`
and `sysctl` on macOS). It writes each sample to a private heartbeat/status file in the scratch directory, and it emits
a notification (in Claude Code, a stdout line of a monitor started with the Monitor tool; elsewhere the harness's
equivalent) when any watched filesystem drops under 10% or under 5 GB free, or available memory under 10%, whichever
comes first, again when the number keeps falling, on recovery, and if the monitor itself fails. A file update alone is
not a notification. Before relying on it, verify delivery with a harmless synthetic alert, and verify failure detection
by killing a throwaway monitor and confirming you are told. If you omit periodic heartbeat checks, also stall a
throwaway monitor without killing it and confirm you are notified within two sample intervals; a monitor cannot detect
its own sampling loop hanging. If any of these notifications is unavailable or unverified, say so in the log and check
the status file at least once a minute.

On every path, check heartbeat freshness before each new build, test run or review launch. A dead monitor or a heartbeat
stale for two sample intervals pauses new launches until monitoring is restored; restart a dead watchdog. An alert is an
instruction to act: stop launching work, remove build output and scratch you own, wait for or stop the job most likely
responsible, and resume only when the watchdog reports headroom. Record the watchdog's handle, watched paths, status
path and delivery mechanism in the log, include its state in every handoff note so a resumed session reconciles or
restarts it, and stop it when the plan closes. Do not lengthen the sampling interval to save turns.

### PR discipline

- Use the `jjstack` skill. The stack's base is `main@origin`, or this plan's own open PRs when it resumes, set up per
  `plans/AGENTS.md` (Executing one plan, step 6); never another plan's PRs. Bookmarks are
  `plan/fresh-checkout-provenance/<nn>-<short-name>`.
- PR 1 then PR 2, one commit, bookmark and draft PR each. Within this run, if a PR needs correcting, restructure it
  rather than stacking a correction on top.
- Conventional Commits; PR 1 is `test:`, PR 2 is `fix:` and adds its changelog fragment under `releasing/changelog.d/`
  in the same commit, per root `AGENTS.md` (Releases and the changelog); validate with
  `python3 releasing/check-changelog.py format`.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read. Leave
  a PR description empty when the diff and title say everything.
- PRs stay drafts. Never mark one ready and never merge; landing waits until the maintainer has reviewed this plan's
  report (`plans/AGENTS.md`).

### Validation

Follow root `AGENTS.md` "Finishing work" and `.agents/narrow-tests.md`: the smallest set of checks likely to expose a
regression, through `scripts/record-test-run.py` (pinned nextest and tmux per `docs/test-run-evidence.md`). Typical
choices, not a checklist:

- Both PRs: `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`.
- PR 1: the test itself, a few repetitions to show it is not flaky, then the `github_checkouts` e2e module.
- PR 2: the new regression test, the supervisor store and status tests around it, and the `github_checkouts` e2e module.
  The Playwright Replace-with specs in `e2e/tests/github-checkouts.spec.ts` on Chromium only if you changed behavior a
  browser could observe beyond the refresh; the bug was intermittent there, so a browser loop is not proof.
- `python -B scripts/check-test-sleeps.py` per `docs/test-sleep-check.md`, and apply `.agents/test-authoring.md`, since
  both PRs change tests.

### Review gate

Before finishing each PR, use the active galaxy-brain skill to delegate one review of that PR's changes. The user
demands a fresh-context agent on Opus 5.5 at high effort, shelled out to the other harness if the executing one cannot
reach that model natively. No review swarm. The prompt carries the full charter, because the reviewer has nothing else:

> Review the changes in this PR for general correctness (bugs, unhandled cases, broken invariants, wrong behavior
> against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and the goal,
> and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the named file,
> most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing and touch no
> VCS state.

The prompt must also name the repository root (the executing checkout), the bookmark or commit range, the findings file
(in the scratch directory), and the acceptance criteria: that PR's section above and The goal. Include the full text of
`.agents/test-authoring.md` verbatim, as root `AGENTS.md` requires for test changes. Address what the reviewer finds
before moving on, and log the DECISION where you decline a finding. Do not write a launch command here or in the log;
the harness-shellout skill owns launch mechanics.

### Scope reassessment

Before implementing a substantial departure from the outline above (reordering the helm's create checks, a new lock or
transaction protocol across store calls, changing what Delete keeps in the checkout registry; these are examples, not a
blacklist), and whenever the same component has needed repeated corrective review rounds, run a fresh-context review
through galaxy-brain with this charter, supplying the request, the user decisions above, this outline, the current diff
and the proposed departure (what changed, why it is necessary, and which simpler alternative was ruled out):

> Identify where this plan does more than the user needs. For each substantial new mechanism, explain which requirement
> makes it necessary and whether an existing facility or narrower supported behavior would suffice. Propose the smallest
> coherent design. Challenge planner-authored requirements; identify any simplification that would change an explicit
> user requirement or repository constraint. Report concrete findings and alternatives, or say no unnecessary complexity
> was found. Do not manufacture objections or add safeguards to justify an unnecessary subsystem. Write findings to the
> supplied private file; change no product files or VCS state.

Reuse an existing review checkpoint when it can answer the question; do not schedule periodic reviews.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular how the test rebinds the preview, whether the race hypothesis held and what the
regression test does, which observer-side fix you chose and why, and every reviewer finding you declined.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them. A material scope expansion, a weakened
guarantee, or an omitted required behavior needs an agreed fallback or the user's decision; a review finding or a log
entry is not authorization. Anything that needs a decision, including a product change to make the test pass or a
provenance cause other than the one above that needs a design change: record the concrete tradeoff and block per
`plans/AGENTS.md` (Executing one plan, step 10). The two PRs are independent in content, so if PR 2 blocks, PR 1 may
still be delivered with the block explained; if PR 1 blocks, do not build PR 2 on top of it.

## Done criterion

The plan is complete when the two draft PRs exist as one linear stack, each satisfies its section above, each has passed
the review gate, and the second has removed the TODO.md entry. Open, not merged: merging happens only after the
maintainer has reviewed this plan's report. If a `## Decisions` section exists, its latest entry must also be satisfied.
Then close the plan per `plans/AGENTS.md` (Executing one plan, steps 11 and 12): deliver its report through the queue
script, write a closing entry in its log, and stop the watchdog. Never edit `plans/` yourself.
