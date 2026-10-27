## What this was about

With automatic workspace trust enabled, Restart with could trust a folder replaced outside Farhelm because the session
still belonged to the old managed checkout. This also affected ordinary sessions borrowing another session's checkout.
The earlier decision required folder identity to match before automatic trust was granted.

Restart with now uses the same identity-checked ownership read as New for the session's working folder. Stale membership
alone cannot grant trust. The change adds no new identity mechanism and stays within the size gate: 51 changed lines of
code, tests and comments, excluding the feedback cleanup, execution ledger and changelog fragment.

## Things you should know

Plain Restart deliberately keeps its stored launch, including its previous trust choice. Restart with keeps the same
working folder while recomputing trust for its edited launch. A missing or replaced folder can still prevent the
supervisor from restarting the session; this change does not bypass that refusal.

The regression uses the plan's accepted scripted-supervisor approach: a borrowing session retains checkout membership,
but the folder identity read says it is no longer managed. The test verifies that both the reusable launch and a named
conversation launch omit automatic trust. Separate existing supervisor tests exercise real missing and replacement
folders. No vendor agent was run.

## Open questions and possible follow-ups

None. The recorded scope and size gate were met.

## The PRs

- [#2738](https://github.com/scode/farhelm/pull/2738/changes) — Restart with checks checkout identity before granting
  automatic trust; draft, not merged.

## Checks run, reused and skipped

Both runtime commands ran through `python3 scripts/record-test-run.py --runner nextest`, kind `development`, with four
slots, zero retries, pinned nextest 0.9.148 and required repository-built tmux 3.7c with matching binary identity. Their
reports, source capture, output and cleanup were complete; neither printed a runtime `SKIPPED` marker.

- `cargo nextest run -p farhelm-helm --lib -E 'test(/workspace_trust|restart/)'`: 26 passed, including the changed
  Restart-with regression and New/Replace trust cases; 1289 selected out. Run `b93a9182-18c0-4e39-8d22-57ff3e805b7a`.
- `cargo nextest run -p farhelm-supervisor --lib -E 'test(managed_directory_snapshot)'`: 2 passed, covering allocated
  ancestry and missing/replaced folders; 1322 selected out. Run `d561a3b1-50f9-4e41-95c9-ce39fb2b3cc4`.
- `cargo clippy -p farhelm-helm --all-targets -- -D warnings`: passed for the changed crate.
- `cargo fmt --all -- --check`, `dprint check` on the changed applicable files, and
  `python3 releasing/check-changelog.py format`: passed. The isolated interpreter's
  `python -B scripts/check-test-sleeps.py` inspected 322 delays with no missing rationale.
- `python3 scripts/check-agents-md.py`: passed with zero errors and two existing citation warnings, unrelated to this
  change.

The commands above covered the source patch on base `78846898`; the helm runtime preceded only a helper doc-comment
clarification and bookkeeping edits. The supervisor runtime and Clippy covered the final Rust behavior. After inspecting
the complete upstream diff, the change was rebased onto `6376dae9`: only other plans' queue claims had changed, and the
patch was byte-identical before and after. Those successful commands are reused individually; the later execution-ledger
PR link changes no executable input. No failed command was reused.

Browser, desktop, installer and whole-workspace suites were skipped because the change is confined to the helm's trust
decision and the targeted protocol and identity tests cover its behavior. There are no UI or installer changes.

## Review gate's outcome

The required single fresh-context gpt-6.1-sol reviewer at high effort reported NO FINDINGS. It checked the fixed-folder
canonical-path reasoning, the scripted borrower regression, the acceptance criteria and the full test-authoring
checklist. The separate documentation pass and commit/PR wording cold read are complete.

### Landing

Landed as built. With automatic workspace trust turned on, Restart with could trust a folder that had been replaced
outside Farhelm, because the session still counted as belonging to the old managed checkout; sessions borrowing another
session's checkout were affected too. Restart with now asks the same identity-checked question New asks about the
session's folder, so membership left over from a replaced checkout no longer grants trust: Restart with then goes ahead
without automatic trust rather than being refused. A session whose managed folder is intact, borrowing or not, still
gets automatic trust. Plain Restart is unchanged on purpose: it keeps its stored launch, including the trust choice made
when the session was first launched, and the supervisor's own check that the folder is still the recorded one can refuse
it.

- #2738: check the checkout's identity before Restart with trusts it.

#### What landed since the stack was based, and whether it interacted

Since the plan was based, main gained the Steel foundation (the interface restyle) and review-queue and triage records;
none of them touches restarting or workspace trust. named-restart-reporter-clear in this round also changes Restart
with, but in the supervisor's handling of which conversation a later Restart resumes, not in the helm's trust decision;
the two share no state. A fresh-context reviewer and I read all four plans together and against what main gained, and
found nothing that interacts.

#### Fixes made at landing

None.

#### Checks

This round landed four plans together, in this order: no overwrites during the Mac installer's command link placement
(installer-link-noreplace), folder identity for Restart with's automatic trust (restart-with-trust-identity), a link
refusal kept below an open session confirmation (link-refusal-confirm-layer) and clearing a stale Resume target when a
Pi or OMP reporter cannot be prepared (named-restart-reporter-clear). All four were built before the Steel foundation
landed earlier the same day. This round ran the following on all four stacked together on main with the Steel
foundation. The review queue's index and the triage record conflicted mechanically between these plans and entries main
had added; the resolutions keep main's entries, apply each plan's own update to its triage entry, and remove from the
index exactly the five findings these plans address (two for the installer plan, one for each of the others).

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D warnings`, the check that every test delay states its reason, `dprint check`
  across the repository, `python3 releasing/check-changelog.py format`, `python3 scripts/check-agents-md.py` (two
  warnings, both about citations main already had), the workspace debug build and the UI's release web build: all
  passed.
- The tests of the UI's own JavaScript and stylesheet (`node --test` in `crates/farhelm-ui/js-tests`), run
  `ff182543-0d7a-46fb-a5f8-3b3c6214ad44`: 358 passed.
- The terminal link-click browser spec, `npx playwright test tests/terminal-link-clicks.spec.ts` in `e2e/`, on Chromium
  and WebKit, three workers each with its own helm and supervisors, no retries, run
  `eff1253f-b467-4151-bf3b-9f5081e7c1b3`: all 34 passed, including the new tests that keep a Delete confirmation's
  warning above a link refusal in both opening orders, now with the Steel restyle's buttons and fonts.
- The whole Rust workspace, `cargo nextest run --workspace --exclude farhelm-desktop`, with the pinned test runner and
  tmux, four tests at a time, no retries, run `8433a77d-dde8-4a20-95c7-b1bd78c1f791`: 4455 passed, 40 skipped by design
  (helper entries that only run as child processes of other tests, and tests that need a real signed-in Claude or
  Codex), and 1 failed: the terminal backpressure test "shallow pause resumes without reset or replay", a known flake
  with three entries in the flake log (the latest records it failing 2 of 3 times when run alone). Run alone on the same
  code three times, it passed each time (runs `a3b3ea0f-39c8-45ee-be09-e64b81ed92a3`,
  `76989247-159f-4240-9c5b-325259d1109c` and `b525f8e0-98e9-41b4-a217-3984ca31bf47`). No plan in this round touches the
  terminal's flow control. The deflake sweep skips this test on behalf of a TODO.md Deflake entry that does not exist;
  either add the entry or drop the skip.
- The Mac installer fixture, `bash scripts/test-install-sh.sh --farhelm target/debug/farhelm` after
  `cargo build -p farhelm` (it runs the installer against fake releases on Linux, standing in for macOS), run
  `a4f7acf6-7f4f-490e-b15f-e76c5143a4cf`: all 603 checks passed.
- Reused: none. Skipped: the full browser suite (only one plan changes the UI, and its spec file ran on both engines),
  the desktop app checks (no desktop code changed), real Pi, OMP, Claude and Codex runs (the reports say what their
  tests stand in for), and a real Mac installation (not runnable here).

The report's checks were the helm's session tests; this round ran the whole Rust workspace with the other three plans.
Nothing in the executor's report became untrue at landing.
