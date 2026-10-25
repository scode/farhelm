## What this was about

Claude already worked through one wrapper that stayed running around it, but two or more such launchers prevented
Farhelm from learning which conversation Restart should resume. A new session therefore could not gain Restart from that
hook. The maintainer requested the same approach already used for Codex for consistency; no Claude incident prompted
this work.

The stack lets Claude report through wrappers at any depth. Farhelm follows the first reporting process, permits its
recorded parents to correct a child-first selection, and tells you what an earlier Restart would have resumed. A new
end-of-turn report lets the parent correct that selection without waiting for another start, resume, clear or compact.

## Things you should know

The accepted child-first gap remains: when the parent's report is lost, a nested Claude carrying a reporting hook can
report first, and Restart can resume that child's conversation until the parent reports and corrects it. Farhelm
remembers which process may supply Restart's conversation; that remembered authority is the process pin. Sessions
already running begin without one after upgrade, so their first report may also come from a nested child. They retain
their old command until relaunch and therefore gain end-of-turn correction only then. Existing Claude Restart offers
remain available.

Claude still reports a plain conversation ID without checking its saved transcript. Its single newest-report file
remains sufficient because both start and end-of-turn reports name the current conversation. A nested end-of-turn report
that the established parent excludes is acknowledged without a warning or any change to the saved conversation.

Installed Claude Code 2.1.310 was observed reporting the current ID on Stop before and after clear. Clear itself emitted
SessionStart, with no Stop observed during that transition. An actual background sub-agent completed through
SubagentStop with an agent marker; the surrounding Stop callbacks carried the parent's ID without that marker. These
observations cover one installed version and inherited permission settings, not every Claude mode or version.

The silent observation hook still coincided with Claude displaying its own “running Stop hook” line. The existing status
heuristic now recognizes only the observed prefix followed by an ASCII digit, under its existing composer and proximity
checks. Synthetic tests cover the two observed forms and near misses; captured agent screens were left unchanged. The
observation did not establish how much of the displayed duration the added hook caused.

The shared process pin preserves the existing database schema and serialized format; Codex's selection and confirmation
rules are unchanged. Documentation was updated on the Claude, Agent wrappers and Agent hook injection pages, together
with the specifications and one changelog fragment. The final PR removes the original TODO entry.

## Open questions and possible follow-ups

None required to complete this plan. Later Claude changes may require refreshing the observed callback and screen facts
through the existing real-agent observation procedure.

## The PRs

- [#2719](https://github.com/scode/farhelm/pull/2719/changes): share the existing process pin without changing Codex
  behavior.
- [#2725](https://github.com/scode/farhelm/pull/2725/changes): capture Claude through resident wrappers while preserving
  existing Restart offers.
- [#2727](https://github.com/scode/farhelm/pull/2727/changes): correct child-first capture at the parent's end of turn.

## Checks run, reused and skipped

All Rust execution used the recorder, nextest 0.9.148, four shared slots, zero retries and the repository-built tmux
3.7c with matching executable hash. Retained reports, source identities, output through each summary and cleanup records
were complete; no runtime `SKIPPED` return was observed. Selected-out cases are not claimed as coverage.

- PR1: `cargo nextest run -p farhelm-supervisor --lib -E 'test(codex_pin) | test(report_pin_replacement)'`, run
  `47be7052-d52b-454c-ae4f-a229518a33fd`, six passed. It covered the original PR1 tree on `afc9fb55`; removal of an
  unused re-export afterward did not change that behavior.
- PR2:
  `cargo nextest run -p farhelm-supervisor --lib -E 'test(procs::tests::claude_) | test(service::core::tests::claude_pin::) | test(codex_pin) | test(report_pin_) | test(report_survives_the_former_claim_deadline)'`,
  run `0a79d3a1-e10a-4966-a624-5b78dac93a17`, 22 passed. This covers attribution, admission, shared store persistence
  and capture-claim contention.
- PR2: `cargo nextest run -p farhelm --test e2e -E 'test(hook_identity::) | test(codex_identity::)'`, run
  `5eddc6cb-3230-4722-9ca3-235a51085071`, 27 tests plus fixture setup passed. Both PR2 runs covered the tree committed
  as `09bfb586`.
- PR3:
  `cargo nextest run -p farhelm-supervisor --lib -E 'test(service::core::tests::claude_pin::) | test(codex_pin) | test(report_pin_) | test(hook_argv_) | test(agent_kind::screen_reader::tests::claude_)'`,
  run `88cb5ad5-cb8e-49c2-a6eb-76575a833132`, 24 passed. This includes Stop selection, correction, silent refusals with
  a positive warning control, hook quoting and observed screen shapes.
- PR3: `cargo nextest run -p farhelm --bin farhelm -E 'test(hook::)'`, run `eca9e862-cad1-4abd-982a-34a01bc9376a`, 38
  passed, including two Goose hook tests also matched by the filter. It covers callback parsing, report-slot
  replacement, child markers and silence.
- PR3: `cargo nextest run -p farhelm --test e2e -E 'test(hook_identity::) | test(codex_identity::)'`, run
  `a4d2926d-b521-4f1b-9aa5-ba29fc090378`, 28 tests plus fixture setup passed. This observes the child selection before
  the parent's actual injected Stop, the correction notice once, actual Restart argv and the replacement's fresh Stop.
  All three PR3 runs covered `09bfb586` plus the final PR3 patch, with source fingerprint
  `022825fdff2fa050ccdd299a8b684bb25dfb9bf3ad4f4d9efcfc0adaad46b213`.

Final `cargo clippy -p farhelm-supervisor -p farhelm -p farhelm-fixtures --all-targets -- -D warnings` and
`cargo clippy -p farhelm --bins -- -D warnings` passed, covering test and shipped configurations.
`cargo fmt --all -- --check`, dprint on changed Markdown, changelog format validation and the source-only test-delay
checker passed; 323 delays had reasons. Formatting checks passed again after rebase.

The stack was carefully rebased onto `27f5a969`. Newer main contained an isolated create-request test-fixture fix,
bookkeeping and prospective plans/static mockups. No capture contract changed. All three per-PR interdiffs were empty
after rebase, and the upstream fixture and TODO edits were retained. The successful commands above are reused for those
unchanged patches; no extra runtime run was justified solely by the rebase.

The full Rust battery, browser, desktop, installer and provisioning suites were skipped because the focused checks
exercise the affected capture, hook and status boundaries. No web behavior, page routes or headings changed, so Markdown
checks covered the incidental docs edits without a website build or preview. No live installation was changed. The
installed-Claude observations above supplement the fake-agent tests; they are not a cross-version compatibility test.

## Review gate's outcome

Each PR passed its required single fresh gpt-6.1-sol reviewer at high effort, with no actionable findings. The first
PR's unused re-export cleanup also passed the same reviewer's follow-up. Commit and PR wording passed independent cold
reads. A separate own-model scope assessment accepted the small observed status-line exception within the existing
heuristic; no new status subsystem or broader capture rule was added.

### Landing

Landed with one comment corrected. Claude sessions launched through more than one wrapper that stays running around
Claude could not tell Farhelm which conversation to resume, so they never gained Restart. Claude tells Farhelm its
conversation through a hook, a small Farhelm command Claude runs at moments such as starting a conversation. Farhelm now
accepts that report through wrappers at any depth and remembers which Claude process reported first. If a Claude started
from inside the session (by the agent itself, say) happens to report before the session's own Claude, the session's own
Claude, as its parent, can still correct it. A new end-of-turn report makes that correction happen at the end of each
turn instead of waiting for the next restart or clear. This is the approach Codex already uses. The report describes the
remaining gap: until the parent reports, Restart could resume the nested Claude's conversation. The plan also teaches
the status reading to treat Claude's "running Stop hook" line as work, and updates three docs pages (Claude, Agent
wrappers and Agent hook injection).

- #2719: share Codex's record of which process may report the conversation.
- #2725: capture Claude through resident wrappers.
- #2727: correct Claude capture at end of turn.

#### What landed since the stack was based, and whether it interacted

Since the plan was based, main gained the previous landing round and a test-only supervisor change (#2720), neither
touching conversation capture. codex-trust-hint in this round also edits the supervisor's screen reader, but Codex's
trust question, not the Claude working line this plan changes. A fresh-context reviewer and I read all five plans
together. The reviewer found no blocker and nothing that needs your decision.

#### Fixes made at landing

- A supervisor test's explanation, on the test that a Claude report nobody can attribute is refused, still described the
  old rule (only the session's own process or its direct child may report). It now describes the new one: the report is
  attributed by its position under the session's process, and the first process admitted is remembered. The test itself
  was already right; only its comment changed.

#### Checks

This round landed five plans together, in this order: the Codex trust dialog's Quit variant (codex-trust-hint), emoji
widths in the terminal (emoji-streaming-corruption), Claude capture through resident wrappers (claude-wrapper-capture),
progress while Trash deletes (trash-delete-blocking) and a test pinning the tunnel listener's address-reuse option
(tunnel-bind-reuse-test). This round ran the following on all five stacked together on this round's main, before the one
comment fix made at landing (in claude-wrapper-capture):

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, the web-target UI lint
  `cargo clippy -p farhelm-ui --target wasm32-unknown-unknown --features web -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D warnings`, `cargo check -p farhelm-ui --features desktop`, the check that every
  test delay states its reason, `dprint check` on the changed Markdown files,
  `python3 releasing/check-changelog.py format`, `python3 scripts/check-agents-md.py` (two warnings, both about
  citations main already had), the docs website build, the workspace debug build, the UI's release web build and
  `scripts/check-desktop-assets.sh` (the desktop app bundles exactly the files the web UI asks for, now 28 with the new
  emoji width add-on): all passed.
- The whole Rust workspace, `cargo nextest run --workspace --exclude farhelm-desktop`, with the pinned test runner and
  tmux, four tests at a time, no retries, run `b0c70ae7-5815-4e5d-bed3-20a73a84d43f`: all 4450 passed; 40 more were
  skipped by design (helper entries that only run as child processes of other tests, and tests that need a real
  signed-in Claude or Codex). The UI's desktop-only Rust tests, `cargo nextest run -p farhelm-ui --features desktop`,
  run `a58ba32c-0ad8-4f6c-835a-313a7052cd31`: 959 passed. The tests of the UI's own JavaScript and stylesheet, including
  its animation rules (`node --test` in `crates/farhelm-ui/js-tests`), run `8b354894-f313-4ca4-bb23-28ea92406be7`: 334
  passed.
- `npx playwright test` in `e2e/`, the whole browser suite on Chromium and WebKit, three workers each with its own helm
  and supervisors, no retries, run `e1283879-7e6b-4fcd-99ab-c94dfb2b139d`: 2644 passed, 14 skipped by design (WebKit
  cannot be granted clipboard access), 2 failed, both on Chromium:
  - "expanded setup failure closes the menu below it" timed out waiting for the host actions panel to settle; alone it
    passed 3 of 3 (run `71120673-042e-49b4-8a82-db714b41f755`). No plan in this round touches the hosts panel; one
    failure under a heavily loaded machine is not recorded as a flake.
  - "inline checkout folder setup refreshes discovery and preview and respects host overrides" failed inside the test's
    own wait, which reads the body of every repository discovery response and hit one whose body the browser had already
    discarded. Alone on this stack it failed 1 of 3 (run `f76349e5-6e48-4ebf-b7c3-141e0573b0f7`) and then passed 15 of
    15 (run `49b4a7d3-6340-43dd-9873-b57b51776458`); on this round's main without the five plans it passed 15 of 15 (5
    in run `babb6cc9-3874-4c69-ad5f-d7effd7bf9a1`, 10 in run `a70b9775-910a-47b4-9128-e3f0b60d96ee`). No plan touches
    repository discovery or that test, and 2 failures in 19 runs against 0 in 15 is too few to tie it to the round. A
    possible follow-up, which belongs to no plan in this round and is listed here once for all five: have that wait
    check the response's address before reading its body, so an earlier response it does not need cannot fail the test.
- The desktop smoke test (`scripts/desktop-smoke.sh` with the pinned tmux and its own cargo target directory), run
  `9ba28070-489e-4b87-859f-db818a511d1e`: passed on WebKitGTK under Xvfb. The app's asset handler served every requested
  file, including the new add-on, and the relaunched app attached a terminal to its remembered session.
- After the landing fix, which changes only a test's comment: `cargo fmt -p farhelm-supervisor -- --check` passed.
- Reused: none. Skipped: the doctests (no documentation example changed), real Claude and Codex runs (the reports record
  what their executors observed), and the native Mac app (not runnable here).

The report's checks were the supervisor's capture and pin tests, the hook command's tests and the end-to-end tests that
run the real hook from Farhelm's stand-in agent; the executor skipped the website build for its three docs pages. This
round ran the website build, the whole Rust workspace and the whole browser suite with the other four plans. Nothing in
the executor's report became untrue at landing.
