# Systematic deflake: end state and operating expectations

Historical record written 2026-09-08 about the systematic deflake work completed on 2026-09-07. This records the
implementation, evidence and expectations at that point; it is not maintained guidance.

## State at completion

The work ended as 102 open draft PRs, rooted at `4a378329ad9151b4ef384f56fe61a2eb477b2258` and ending at
`16dda90879e3a9337fd88683890f1b9e092d906a` (#516). The stack had not been merged or released. Adoption on main and in
its CI therefore still depended on landing it. Each review unit received an independent adversarial review using
Astra at high reasoning, with separate cold review of commit and PR prose. No review swarm was used.

The objective was to remove recurring sources of nondeterminism and make subsequent failures easier to investigate.
It was not a campaign to close every known flake ticket. The result combines concrete fixture corrections, shared
test infrastructure and repository-owned practices. A clean final run supports that implementation; it does not measure
a long-term reduction in the flake rate.

## What changed

**Hosted test cost moved to releases.** Expensive Rust, pinned-tmux teardown, desktop runtime and CentOS provisioning
checks run through release artifact jobs, where failure blocks publication. PR/main CI retains formatting, lint,
compilation, JS unit tests, installer validation and generated-workflow checks, with draft-PR suppression preserved.
There are no nightly stress loops or hosted hunt jobs. The existing exclusion of the `farhelm` crate's tmux-driven
e2e target from the release gate and the disabled browser CI suite remain explicit; restoring those is separate work.

**Execution produces evidence.** The run recorder gives each attempt a private identity and retains its command,
selection, concurrency, source state, resolved tmux executable identity, output and runner results. Later passing
attempts cannot overwrite failures. Collection is bounded, and incomplete source identity, missing output, skipped
coverage and interrupted cleanup remain visible. Failure diagnostics include persistent test traces and event
timelines, with async propagation and private-tmux teardown handled deliberately. These mechanisms improve evidence
for participating tests; they do not promise complete diagnostics after every possible abnormal exit.

**Fixtures establish the boundary their assertions require.** Rust and browser helpers distinguish created, ready,
replayed, revealed and intentionally mid-launch states. Tests choose those premises explicitly. Live-byte assertions
use their own post-replay attachment, and browser input establishes the relevant focus and pointer state. An
intentional race remains an unsettled-boundary test: waiting away the product behavior under examination would be a
regression, not a deflake.

**Rust execution is isolated and budgeted.** Nextest supplies a process per test, four global slots, an e2e cap of four,
and a supervisor-tmux cap of two. Expensive measurements reserve the necessary slots; RSS observations distinguish the
owned process from unrelated co-resident allocations. Controlled runs use zero retries. Doctests remain a separate
invocation because nextest does not execute them. A combined run that already covers the pinned shutdown scenarios
does not require another local execution of that subset merely to tick a second box.

**Local hunts and reporting are explicit tools.** Finite Rust and browser hunts retain each attempt and expose their
selection and cost. A changed-input planner proposes scopes for review rather than running them automatically. Manual
summaries distinguish recorded commands, reported cases and latent-flake ledger entries, including incomplete
discovery. Repetition provides evidence; no fixed repetition count became a prerequisite for every edit or PR.

**Authoring and review have a shared checklist.** Repository instructions require checking fixture premises, named
readiness, peer lifetime, descriptor inheritance, owned-process measurements, discriminating observables, bounded
polling, and pointer/focus state. The checklist is supplied verbatim to the selected independent reviewer; it does not
request an extra reviewer. A source-only check in the existing formatting job rejects unexplained test delays.
Legitimate observation windows and polling intervals carry a `sleep-ok` rationale. An annotation records intent; a
reviewer still has to judge whether the premise and observable are sound.

Two final corrections illustrate the distinction between evidence and retries. The 1x1 terminal test received the
whole READY marker split across rows, yet its literal substring check reported missing output. #515 corrected that
observation while retaining the exact size assertion and explicit mid-launch boundary. #516 repaired two stale
recorder test fixtures: an isolated copy omitted required helper modules, and cancellation was injected before the
spawn whose cleanup the test intended to exercise. The original failures were retained alongside the corrected runs.

## Evidence at completion

Final combined validation recorded:

- Rust: 2,302 reported passes and 36 runner skips, including execution of all ten pinned shutdown scenarios in separate
  nextest processes. Eleven additional runtime skip messages required interpretation beyond the JUnit pass counts.
- Desktop-feature UI: 343 passes with no skips; the shipped desktop binary compiled. All-target Clippy passed, and the
  separate workspace doctest command passed with no doctest cases discovered.
- Browser: 475 Chromium passes and two skips; 466 WebKit passes and eleven skips. All 954 selected cases were accounted
  for, with 941 passes, 13 skips, zero failures and zero retries. Four skips required authenticated real agents; nine
  concerned unsupported WebKit clipboard permissions.
- Local tooling: 54 recorder tests, 97 report/hunt/selection tests, 27 parser/checker tests and 119 JS tests passed.
  The static check inspected 205 delays with no missing rationale. Formatting, generated-workflow checks and relevant
  release-validator self-tests also passed.

Expensive builds and suites ran on disposable workers. Two focused tests exercised real systemd user-manager scope
contracts locally using a worker-built binary. Five end-to-end cgroup cases, three systemd-dependent provisioning
cases and a two-version tmux adoption case remained unexercised on their required substrate. The process-tree fallback
and those two scope unit tests are narrower evidence, not substitutes for every skipped contract. No current CentOS
container boot, macOS runtime, native desktop-window smoke or hosted release execution was claimed.

Validation reuse was per command and based on unchanged inputs. Rust was tested at `62ec9998`; browser tests at
`81b92677` used the full web build from `92a564fe` and a refreshed CLI from `c2fcabf5`. The intervening changes to the
final tip did not alter those inputs. Reports retained the distinct source and artifact identities rather than
relabelling every build as having been made at the final commit. All owned workers and snapshots were removed after
results were collected; agents and monitoring were stopped.

## Expectations going forward

Normal development should use the maintained repository instructions. The stack already updates `AGENTS.md`, the
narrow-test recipe and review checklist, so adoption does not require a second round of global agent-skill or CI
changes. The main operational change is what a satisfactory test investigation leaves behind: the failed run, its
actual inputs and substrate, a narrow investigation, and an explicit disposition. A green retry alone is insufficient.

Agents should select validation by risk and cost, keep expensive work on sandboxes, and run combined coverage when it
meaningfully covers the accumulated changes. Shared fixture or terminal changes can justify wider coverage; ordinary
prose cannot. Passing PR CI does not imply that expensive runtime tests or browser tests ran. Browser coverage remains
an explicit pre-merge responsibility under the repository instructions.

Retain failed UUID directories and batch indexes while diagnosing failures. Keep raw output and environment identities
private, and download useful release failure artifacts before hosted retention expires. Release collection retains
failure evidence, not records of every successful release, so it cannot alone establish an overall failure-rate
denominator. Manual summaries can inform a later assessment, but there is no scheduled reporting job or automatic
dashboard to operate.

The specific latent-flake backlog remains in `TODO.md` and `FLAKES.md`. A single clean combined run does not close it.
The completed systematic plan was removed, while three deferred triggers survived: harness-budget scaling after an
actual budget-class recurrence; product-level cause attribution without changing shared reason strings; and a runner
concurrency experiment after restoration of the release integration gate. These are conditional follow-ups, not work
that must start to benefit from the completed changes.

At the time of writing, the maintained entry points were `AGENTS.md`, `.agents/narrow-tests.md`,
`.agents/test-authoring.md`, `docs/test-run-evidence.md` and `docs/test-sleep-check.md`. Those files own future operating
instructions; this entry preserves why the completed work was expected to help and the limits of its evidence.
