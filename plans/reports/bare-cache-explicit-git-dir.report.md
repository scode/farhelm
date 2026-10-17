## What this was about

Starting a session in a fresh GitHub checkout failed on hosts using Git's `safe.bareRepository=explicit` setting.
Farhelm created its bare repository cache, then Git refused the fetch because the cache was selected through implicit
repository discovery. The checkout, hook and agent never ran.

The agreed fix names Farhelm's own cache explicitly during fetch. The host's restrictive policy stays enabled. The
implementation specification now shows that command and its reason, and the original feedback item is removed.

## Things you should know

The fix stayed within the agreed scope: the cache fetch's repository selector, its tests and the matching specification.
The cache and clone design did not need a change. The real-Git regression starts with an absent cache, first proves that
implicit discovery is refused by the policy, then reaches a ready checkout and runs the agent under that same policy. It
also checks the checkout's tracked content and cache object directory. Git configuration is supplied only to child
commands; the test process's environment is not changed.

## Open questions and possible follow-ups

None required for this outcome. The regression uses an offline local upstream reached through Git's URL rewriting; it
does not exercise GitHub networking or authentication, and it does not audit other Git safety settings.

## The PRs

- [#2199](https://github.com/scode/farhelm/pull/2199/changes): honor explicit bare-repository policy in checkouts. Draft
  head `9c1fb40fdccc720fec021ecb8485bba3cdfacf5c`, bookmark `plan/bare-cache-explicit-git-dir/01-git-dir`.

## Checks run, reused and skipped

The runtime run captured base `e29365df` plus its complete working patch, fingerprint `ce76b2a0` and tracked-diff hash
`a300b8b4`. Later edits before review only completed the triage ledger and feedback cleanup; executable source stayed
unchanged. The reviewed source revision was `799b3a86`.

- Run: recorded
  `cargo nextest run -p farhelm-supervisor --lib -E 'test(d1_fake_git_hook_agent_run_in_order_with_exact_argv) | test(d2_real_git)'`,
  recorder `952efe6a-bf2a-4f14-b7fd-9aa0ac14c961`: all five selected tests passed, including the new explicit-policy
  regression; 1,130 tests were selected out. Pinned nextest 0.9.148, four slots, zero retries, two cargo build jobs,
  Linux and Git 2.43.0. These shim tests do not use tmux. The report and retained output are complete, with no runtime
  `SKIPPED` diagnostic, output loss or forced cleanup.
- Run: `cargo fmt --all -- --check`, changed-file `dprint check`, `python3 releasing/check-changelog.py format`,
  `python3 scripts/check-agents-md.py`, and `git diff --check`: passed. The isolated test-sleep checker inspected 302
  delays with zero missing rationales.
- Run: `cargo clippy --all-targets -- -D warnings` passed with two build jobs, on the reviewed source before rebase.
- Reused: the successful runtime, source and static checks above after a clean rebase onto `07d244e6`; the patch was
  byte-identical across that rebase. Main added the browser input-method key guard and helm download-admission cleanup,
  plus their specification and bookkeeping. Their complete diffs were inspected: neither changes supervisor launch, Git
  environment handling or the repository-cache contract. The final draft differs from the rebased checked patch only in
  its triage-ledger PR link; Markdown formatting and whitespace checks passed again for that update. Subsequent main
  changes inspected before delivery were plan states and landing notes only. No runtime rerun was warranted.
- Skipped: the full Rust suite, browser, desktop, installer and provisioning tests. The changed behavior is the
  supervisor's Git cache fetch, covered directly by the exact-argv test and all four existing/new real-Git preparation
  tests. No UI, installer or host-management behavior changed.

## Review gate's outcome

The required fresh-context gpt-6.1-sol high source reviewer found no correctness, design, idiom or complexity-gate
violations. It checked the full diff, the real-shim test path, policy isolation, spec and feedback bookkeeping. Its
review was source-only; the recorder run above supplies execution evidence. A fresh gpt-6.1-sol medium reader understood
the commit and PR wording and found no contradicted claim or convention violation. The first wording task arrived as an
unreadable payload; it was resent as plaintext before that reader saw the candidate. No implementation was delegated.

### Landing

Landed in round 77, as #2199, second of the round's two plans and unchanged from what the report describes.

#### What landed since the PR was based, and whether it interacted

Since this PR's base, main gained three specification-only changes (#2193, #2195, #2196) about Claude settings edits,
stale views across GUIs and the trash's scale limit, and the round's first plan, the calm session outage card (#2198),
which changes how the interface shows a session whose host is unavailable. The landing monitor read them against this
PR, and a fresh-context reviewer did the same independently. Neither found an interaction. This PR changes only how the
supervisor names its cached repository when fetching for a managed checkout, which none of those touch. A failed
checkout still shows as the existing failed-checkout state, never as the new outage card. All four share a file with
this PR (SPEC_impl.md, the triage ledger or the review queue index), always in a different section, and the landing
rebase merged them without conflict, leaving the PR's diff identical to the one the executor delivered.

#### Fixes made during the landing

None. The bug came with the shared repository cache, which first shipped in v0.26.1, so the PR's own changelog entry,
filed as a fix, is right. Release curation could say "managed checkouts" where the entry says "fresh GitHub checkouts",
since that is the name the docs site uses for them.

#### Checks

- Run now, at the rebased tip: `dprint check` on SPEC_impl.md, the triage ledger and the review queue index, and
  `python3 releasing/check-changelog.py format`, both clean.
- Reused from the executor's runs on its own base, which the report records, because the landing rebase left the PR's
  diff unchanged and nothing that joined its base touches the supervisor's checkout code: the recorded checkout run
  `952efe6a-bf2a-4f14-b7fd-9aa0ac14c961` (five passed, including the new explicit-policy regression against real Git),
  plus its clippy, formatting, test-sleep and agent-instruction checks.
- Skipped: the full Rust suite, browser, desktop, installer and provisioning tests, for the reasons the report gives.

Nothing in the executor's report became untrue at landing.
