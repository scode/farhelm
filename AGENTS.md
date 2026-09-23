# Farhelm agent instructions

Intent and preferences only — for what the system is and how it works, read SPEC.md (product behavior) and SPEC_impl.md
(implementation choices and their motivations). Those documents are authoritative; do not restate them here and do not
change behavior they specify without surfacing the conflict. The historical build-order docs (PLAN.md / PLAN_M*.md,
cited by name throughout code comments) are archived under lore/ and are not maintained.

This project is either public now, or may become public in the future. No content in this project should contain
personal information such as personal usernames, hostnames, details about the local environments, etc.

# Conventional Commits

All commit messages and PR titles must use Conventional Commit format: `<type>: <short summary>`

Allowed types: `feat`, `fix`, `docs`, `perf`, `refactor`, `style`, `test`, `chore`, `ci`, `revert`.

Append `!` after the type for breaking changes (e.g. `feat!: remove legacy
endpoint`). Scope is optional.

Rules:

- Type reflects the user-visible effect, not the implementation activity. A bug fix that requires heavy refactoring is
  `fix`, not `refactor`. A new CLI flag is `feat`, not `chore`.
- The summary after the colon is lowercase, imperative mood, no trailing period.
- Keep the first line under 72 characters.

# Finishing work

For changes to tests or their fixtures, apply [.agents/test-authoring.md](.agents/test-authoring.md) during authoring
and supply its full text verbatim to the independent reviewer. Use the review process selected for the task; this
checklist does not request an additional reviewer or a review swarm. Report concrete violations of the relevant
contracts, not mechanical demands for every bullet to appear in every test.

Run `python -B scripts/check-test-sleeps.py` with the isolated interpreter prepared in
[docs/test-sleep-check.md](docs/test-sleep-check.md) when changing Rust or browser tests, their helpers, or the checker.
The CI workflow's formatting job runs this source-only check and requires zero unannotated delays. A `sleep-ok` reason
explains a deliberate observation window, scheduling stimulus, or polling interval; it does not replace a readiness
oracle or excuse an unverified fixture premise.

Before creating, updating, or merging a PR, or claiming work is done, make a judgment call about what validation would
add useful evidence. Never mechanically run the entire test battery. A PR, rebase, stack completion, or merge request is
not by itself a reason to launch it. The list below is the authoritative inventory of available CI-equivalent checks and
their coverage, not a checklist to execute in full.

Start with the actual diff, the successful checks already available, and the concrete risks still uncovered. Choose the
smallest set of checks likely to expose a regression in the affected behavior; no additional runtime tests is a valid
choice when existing evidence still covers the change. Widen only when a specific material risk or unresolved failure
needs broader coverage. Cross-cutting, security, lifecycle, installer, or release changes deserve careful assessment,
not an automatic full run. Before choosing a full battery, explain what risk it covers that existing evidence and
targeted checks cannot adequately cover. Cost does not excuse ignoring a material risk, but running more tests without
identifying one is not diligence.

For a documentation-only change that cannot affect generated artifacts or executable examples — ordinary Markdown
documentation, a lore entry, agent instructions, or comments with no doctest or generation role — do NOT run Rust,
JavaScript, desktop, installer, provisioning, or browser tests. Run only formatters, linters, link checkers, or other
checks that actually inspect the changed files, if any exist. Do not run a check solely because it appears in the full
gate list.

When building a stack, use targeted checks at each review unit to catch likely regressions while they are cheap to
localize. At the tip, assess the combined diff for interactions those checks did not cover. Stack completion does not
require another battery or the highest-cost gates; run broader checks only for identified coverage gaps. Apply the same
judgment when landing the stack.

Reuse is per command. Compare the tested revision with the current head, including upstream changes and conflict
resolutions after a rebase. A new commit hash does not invalidate a successful run. If extensive validation just
finished and a minor rebase preserves the tested behavior, reuse it without more runtime tests. If the rebase changes
behavior, run targeted checks most likely to expose a problem in those changes or their interactions; widen only when
that leaves a material gap. Resolve uncertainty by inspecting the diff before defaulting to execution. Failed,
interrupted, poorly identified runs, or runs whose coverage no longer applies are not evidence of a pass. Report checks
run now; checks reused, with the covered revision and why their coverage still applies; and checks skipped, with the
reason.

For Rust execution, first put the pinned nextest and tmux on PATH using the guarded setup in
`docs/test-run-evidence.md`. Run from the checkout root with Python 3.11+ (Python 3.13+ on macOS). Use owned sandboxes
for expensive, slow or memory-heavy checks; small focused checks can run locally. The recorder's nextest mode enforces
four global slots, zero retries and failure on empty selection, and retains each run's configuration and JUnit report.
Runtime `SKIPPED` messages still require reading the retained output: an early-return test is a JUnit pass, not proof
that its required systemd or SSH substrate ran.

- `cargo fmt --all -- --check`
- `cargo clippy --all-targets -- -D warnings`
- `python3 scripts/record-test-run.py --runner nextest --kind development --selection 'workspace Rust targets' --concurrency '4 nextest slots; retries 0' --tmux required -- cargo nextest run --workspace --exclude farhelm-desktop`
  — executes applicable unit and integration targets, including the support crates omitted from default-members. Nextest
  starts a process per test and bounds concurrency across binaries; the e2e group has four slots, supervisor tmux
  modules share two, the two-host fixture reserves two, and RSS reserves all four. The release gate retains its narrower
  package/target selections; ordinary CI does not run this suite. Do not also run the old full libtest battery.
- `python3 scripts/record-test-run.py --kind development --selection 'workspace doctests' --concurrency '4 doctest threads' --tmux required -- cargo test --locked --doc --workspace --exclude farhelm-desktop -- --show-output --test-threads=4`
  — preserves doctest coverage separately because nextest does not execute it.
- `scripts/test-tmux-pinned-shutdown.sh` — the x86_64 Linux release gate builds the exact checksummed release named by
  `.github/release/source-pins.env` on cache miss, then runs every focused output-client teardown regression in its own
  test process with its own recorded nextest report. Keep this explicit release subset while the full e2e target remains
  excluded there. A successful local workspace nextest run already covers these tests on the same pinned substrate; do
  not duplicate them merely because this script is listed. A distro tmux loses the affected-version coverage.
- `cd crates/farhelm-ui/js-tests && node --test` — the JS unit harness for the asset-JS layer's pure functions
  (PLAN_M6_5.md item 1); node is already a CI requirement for Playwright below, so this adds no dependency. Run from
  inside the directory rather than as a glob from the repo root: node's no-argument default discovery (every `*.test.js`
  in cwd) is the oldest, most version-portable form the test runner has, whereas quoted-glob CLI arguments are newer and
  CI pins no node version.
- `cd crates/farhelm-supervisor/asset-js-tests && node --test` — the OMP conversation-reporter asset's own execution
  proof: dispatch ordering against a controlled subprocess mock (including a deliberately nonserialized variant the
  proof must reject), stale-id cancellation, execution-time file recheck, the subscribed transition events, and the
  silent-failure boundary, run against the real shipped asset. Same node terms as the UI harness above; the release gate
  runs it through the recorder with the `OMP reporter asset scenarios` selection.
- `cargo check -p farhelm-ui --features desktop` — the desktop renderer compiles nowhere else; needs the webkit2gtk/gtk
  dev packages (see the CI job for the apt list).
- `python3 scripts/record-test-run.py --runner nextest --kind development --selection 'desktop Rust targets' --concurrency '4 nextest slots; retries 0' --tmux none -- cargo nextest run -p farhelm-ui --features desktop`
  — exercises desktop-only persistence and IPC seams with the same system dependencies as the compile check.
- `python3 scripts/record-test-run.py --kind development --selection 'desktop doctests' --concurrency '4 doctest threads' --tmux none -- cargo test --locked --doc -p farhelm-ui --features desktop -- --show-output --test-threads=4`
  — preserves the desktop feature's separate doctest coverage. Desktop execution remains release-only in hosted CI.
- `cargo check -p farhelm-desktop` — the shipped desktop binary sits outside `default-members` (so ordinary builds never
  compile WebKit), which means `-p` is the only thing that ever compiles it.
- `scripts/check-desktop-assets.sh` — holds the desktop build's `asset!()` set and the web bundle's files to the same
  set, in both directions. The shipped desktop-artifact builder calls `--compare` on its actual embedded bundle, so
  release validation preserves parity without a duplicate CI rebuild.
- `PATH="$(scripts/build-pinned-tmux-ci.sh):$PATH" scripts/desktop-smoke.sh` — the x86_64 Linux release gate runs this
  with an isolated `CARGO_TARGET_DIR`; before any of it needs Xvfb, four pre-display process legs drive
  `farhelm-desktop`'s own entry point directly: a missing tmux and a below-floor tmux each refuse with one exact plain
  stderr line and exit status 1 (the app's own preflight, run before it would spawn its managed supervisor), and a
  non-tmux bootstrap failure (an unusable state directory) exits 1 through the ordinary fallback path with no panic. The
  non-pixel Xvfb integration gate that follows covers the embedded helm, managed supervisor, desktop authentication, the
  tmux override reaching the managed supervisor, hard-exit tether, restart persistence, and that every asset the window
  requested went through the desktop asset handler. The optional coordinate-driven leg is not part of CI.
- `sh -n scripts/install.sh && shellcheck scripts/install.sh scripts/test-install-sh.sh` — the curl-installer is POSIX
  `sh` run unread on a fresh machine (`curl | sh`), so a syntax slip or a shellcheck-catchable bug in it gets caught
  before the slower Rust-side asset-table parity test in `assets.rs` would notice. `test-install-sh.sh` is bash, not
  POSIX `sh` (it is never piped into a stranger's shell, so nothing forces the same portability constraint), which is
  why only `install.sh` goes through `sh -n`.
- `bash scripts/test-install-sh.sh` — drives `install.sh` as a real child process against a fixture HTTP server: fresh
  install, update, a forced-failure rollback (macOS-shaped, via a `uname` shim), 404, checksum mismatch, two
  malformed-archive shapes, version normalization (including `-rc.N` and `-dev.N` prereleases), invalid versions,
  missing prerequisites (via an isolated `PATH`), the exact closing-message contract across five tmux fixtures, the
  `Farhelm.app` bundle a macOS-shaped install assembles (layout, rebuild on update, the pre-icon/opt-out/foreign-bundle
  edge shapes), and that nothing outside `FARHELM_INSTALL_DIR` and that bundle — no `systemctl`/`launchctl` call,
  nothing else under `$HOME` — ever changes. Every invocation goes through `env -i` with an explicit environment, never
  this process's own.
- `python3 scripts/record-test-run.py --kind development --selection 'installed uninstall acceptance' --concurrency 'one fixture at a time' --tmux none -- python3 scripts/test-uninstall.py --binary target/debug/farhelm --installer scripts/install.sh`
  — after building the CLI, runs the actual installer and installed uninstaller against private fixture homes and local
  release archives. Covers updates, confirmation, dry-run preservation, foreign files and native macOS bundles. Linux
  children use a fixture service-manager command; actual service tests must use an owned systemd container. The focused
  macOS CI job and macOS release gate also run `cargo nextest run -p farhelm --bin farhelm -E 'test(uninstall::)'`
  through the recorder with four slots and `--tmux none`.
- `scripts/test-provision-centos.sh` — the x86_64 Linux release gate boots a systemd CentOS Stream 9 container and makes
  the helm provision it over ssh, which is the only coverage of a helm installing onto a distribution other than its
  own. Needs docker and `musl-tools`: the payloads it pushes are the release's musl-static `farhelm` and static tmux,
  because the workspace's glibc debug binary cannot exec on CentOS 9. A few minutes, most of it the musl build; the
  container image and the tmux build are both cached after the first run.
- `dprint check`
- `cd website && bun install --frozen-lockfile && bun run build` — the docs website (Astro + Starlight) must build; this
  is what catches a page with missing frontmatter or a sidebar slug that names no page. Bun is the package manager the
  Vercel project is configured with, so the lockfile is `bun.lock`; commit it with any dependency change.
- `dist generate --check` — `release.yml` is generated from `dist-workspace.toml` plus `.github/dist-build-setup.yml`,
  and the release `plan` job refuses a stale one; this asks the same question before a tag has to. Needs the pinned
  cargo-dist (`cargo install --locked cargo-dist --version 0.32.0`, the version `dist-workspace.toml` names).
- `python3 releasing/check-changelog.py format` and `python3 releasing/check-changelog.py --self-test` — the changelog
  layout lint the release gate runs on every tag, and the checker's own fixtures (format variants, a synthetic git
  history for the fragment sweep, synthetic dist manifests for the announcement comparison). Run the lint when
  `CHANGELOG.md` or a fragment changes, the self-test when the checker does.

This inventory names the relevant local checks. The CI workflow (`ci.yml`) covers formatter, Clippy, desktop
compilation, the JS harness, the website build, installer validation, and generated-workflow validation; costly Rust,
pinned-tmux, desktop runtime, and CentOS gates run in the x86_64 Linux release artifact job. If either workflow changes,
update this list in the same change.

The CI workflow runs ONLY on demand: it has no push or pull-request trigger (removed 2026-09-12), so neither a PR, a
`gh pr ready`, nor a merge to main starts a run. The release build gate is the validation that decides whether a build
ships; the local checks above are what validates a change before it lands. The focused headless macOS uninstall suite
runs with `gh workflow run ci.yml --ref <branch> -f suite=uninstall-macos`. `gh workflow run ci.yml --ref <branch>` runs
the whole baseline for any ref when a hosted verdict is wanted. PRs are still opened as drafts and marked ready when the
user asks to publish them, or as part of landing them, never on the agent's own initiative; `pr-base.yml` (the only
required check) still runs on every PR and only verifies that the base is main.

The RELEASE workflow is not on that list: it is generated, runs on tag pushes only, and carries its own gate, which is
not something to run locally. `releasing/AGENTS.md` describes it, the parts of it that CAN be checked locally (the
changelog checks above among them), and the unsigned-release recovery procedure.

Browser end-to-end validation follows the same judgment and reuse rules, including at merge time. Its CI job is disabled
(`if: false` in ci.yml), so CI green does not include browser coverage. Assess whether the change leaves a concrete
browser integration risk untested; choose relevant specs when they cover it, and run the full suite only when the risk
needs that breadth. Do not start the full suite merely because a PR is about to merge. Documentation-only changes need
no browser tests, and a minor rebase does not discard existing browser evidence.

When browser validation is warranted, run the selected specs through the recorder on Chromium and WebKit (WebKit stands
in for the desktop app's engine family). The full-suite child command is `cd e2e && npx playwright test`. It needs
`cargo build` and `cd crates/farhelm-ui && dx build --package farhelm-ui --platform web --release` first, because it
drives the built web UI against a real helm and supervisor, plus a one-time
`cd e2e && npm install && npx playwright install chromium webkit`. Reuse matching builds and successful test results
when the intervening diff leaves their coverage intact; see `docs/test-run-evidence.md` for recorded selections.

# Releases and the changelog

`releasing/AGENTS.md` is the procedure for cutting a stable, RC, or dev release, and the full rules for `CHANGELOG.md`
and its fragments. Read it before any of those; in short, no release tag is pushed before its base and its version are
settled with the maintainer per that file, a tag name is never reused, and a tag is never deleted. The one rule that
applies to ordinary PRs lives here because every PR-making agent has to follow it without being asked:

A PR whose title type is `feat`, `fix`, `perf`, `style`, or `revert`, or whose type carries `!`, adds a changelog
fragment under `releasing/changelog.d/` in the same commit as the change. The file name is a free mnemonic
(`cursor-launch.md`); the content is a leading `---` front matter block holding a `kind:` line (`breaking`, `added`,
`changed`, `fixed`, `removed`, or `none` for a change with nothing user-facing, with the reason as its body), then a
draft entry written for someone running Farhelm: what changed for them, with caveats, and without internal mechanics,
following the maintainer's wording rules in `releasing/EDITORIAL_GUIDANCE.md`. It is not reviewed at PR time; the
release-time curation rewrites it. `python3 releasing/check-changelog.py format` validates it. Other types add one only
when the change has a user-facing effect.

# Reproducing failures: narrow tests first

Run local and worker runtime tests through `python3 scripts/record-test-run.py`, including each reproduction attempt.
Use Python 3.13 or newer on macOS: the recorder requires `os.waitid` with `WNOWAIT` to keep child ownership through
cleanup. The release setup selects Python 3.13 explicitly on that platform. The commands in the gate inventory and
narrow-test recipe are the child argv after its `--`. Supply the actual selection and concurrency, `--kind development`
for ordinary validation or `--kind repetition` for a focused attempt, and an appropriate tmux mode. Use
`--tmux required` with the built `.ci-tmux` first on PATH for controlled comparisons; `warn` records a different
developer substrate visibly, and `none` is for checks that do not use tmux. The wrapper scrubs ambient `FARHELM_*`
before test-process startup. Retain a variable only by naming an intentional test input with `--keep-farhelm-env`;
values stay out of metadata. The recorder reserves `FARHELM_TEST_TRACE_DIR` for a fresh private directory per command,
overriding any ambient value even when retention requests it. Never fix this by changing the test process's own
environment.

Keep the failed UUID run directory before retrying, and record its path and disposition in the session's working log. A
later pass is another observation, not a replacement for the failure. Preserve failure evidence until a fix PR or
latent-flake entry records what happened; preserve same-session failures privately without adding them to FLAKES.md.
Keep paths, raw logs, and host identities out of public source. Public summaries use redacted commands, portable
substrate details, run IDs, and confidence about the cause. Record an unavailable or incomplete identity as such rather
than reconstructing it from what a machine was supposed to run. No mandatory repetition count or extra CI run follows
from this retention rule. Storage, schema, interruption limits, and examples are in `docs/test-run-evidence.md`.

When reporting across retained runs, use `python3 scripts/summarize-test-runs.py --root /path/to/retained-runs` and
archive its portable JSON where the operator chooses. Supply additional roots or individual `--run`/`--batch`
directories explicitly. Record incomplete discovery and coverage gaps alongside totals; retained failure-only release
artifacts are not a denominator for all releases. This is a manual reporting aid, not another validation gate.

The existing release gates use the same recorder. Collection follows those gates and uploads selected bounded records
when a failure has occurred by that point; later packaging failures cannot trigger it. Download useful failure artifacts
before hosted retention expires. Successful jobs do not upload run records, so those artifacts alone cannot establish a
release pass/fail denominator. Collection also exports the fixed, bounded persistent trace layout, including after a
recorder death; keep its collection summary and raw traces when recovery is incomplete. The recovery command and limits
are in `docs/test-run-evidence.md`. To validate changes to the recorder itself, use
`python3 scripts/test-record-test-run.py` and `python3 scripts/test-test-run-traces.py`, choosing affected contracts as
usual; this adds no ordinary CI test job.

The finishing-work list above is a gate, not a debugging tool. When investigating a failing, flaky, or suspicious test —
including one first seen in a full-battery run — reproduce it with the narrowest run that could show it, and widen only
when the narrow run will not reproduce: the exact test in a repetition loop, then its module or spec file, then its test
binary or engine, then the full battery. `.agents/narrow-tests.md` is the recipe, with exact commands and caveats for
every suite. Running a many-minute battery to check a one-test hypothesis is the failure mode this rule exists to
prevent.

# FLAKES.md

`FLAKES.md` is the append-only log of LATENT flakes: tests that pass where they were written and fail elsewhere,
sometimes, or under load, whose cause was not obvious when they failed. When you establish that a failure is a true
flake of that kind — pre-existing, or one that survived past the session that wrote it — add a dated entry: one
paragraph naming the test and its file (no line numbers), what was observed and where, the cause found or suspected, and
the disposition (fixed in which PR, ignored with what reason, or open). Fixed ones stay; a fixed flake that recurs gets
a new dated entry. Do NOT log a test that was written, flaked, and was fixed within the same session: that is ordinary
development, and logging it buries the tech debt the file exists to show. Open flakes also get a deflake entry in
TODO.md; the log keeps the history, TODO.md keeps the work.

For new entries, follow FLAKES.md's evidence fields and its `Class:` / `Cause:` lines. A guessed cause is a hypothesis,
even if a later run passed. Do not rewrite older entries to make them fit the new fields.

# TODO.md

`TODO.md` is the maintainer's running list of wanted fixes and features. When a PR addresses an entry, remove that entry
in the same PR — the file only ever describes what is still wanted. Do not add entries on your own initiative; they are
the maintainer's. The exception is a deflake run (below), which records the flakes it finds as Deflake entries. When a
Deflake entry is removed, remove the matching line in `deflake/known-flakes.txt` in the same PR: the deflake sweep reads
that file as its exclusion list, so a stale line is a test that never runs again.

"tldr todo", "what's in the todo", and similar requests mean the FULL list, grouped under the file's own bucket
headings, in the file's order, one to two sentences per entry: what it is and, when the entry says so, why or the first
step. Every entry, not a selection — the point is to see the whole board at a glance. Bold a short handle at the start
of each line so an entry can be referred to by name afterwards.

# Deflake runs

"Start a deflake run" (optionally "and repeat until I say stop") means the full-suite flake-discovery sweep in
`deflake/`. Read `deflake/AGENTS.md` and follow it exactly; `deflake/SPEC.md` holds the requirements, chiefly that the
agent never polls and only takes a turn when `deflake/bin/deflake wait` returns. A change to the driver or to those
instructions is validated with the end-to-end procedure in `deflake/EVAL.md`, a few minutes with a low-power delegate.

# plans/

`plans/*.md` holds plan files for TODO.md entries: temporary working documents that exist while a piece of work is
planned but not yet done, so its shape, effort, and return can be judged before anyone commits to it. A TODO entry may
or may not have a plan; when it does, the entry links the file and carries the plan's effort estimate as one word — low,
medium, or high — so the board can be read at a glance without opening plans; best judgment, no precision implied, and
the plan itself says what the word rests on. A plan file is deleted in the same PR that finishes the work, together with
the TODO entry it served. Name the file after the work with a mnemonic slug, so the name alone says what it is about
(`plans/deflake-attach-boundary.md`, `plans/pre-upgrade-state-backups.md`), never a number or a date.

Every plan starts with the commit hash and the date of the codebase it was written against. Plans are allowed to go
stale; that is the point of recording the anchor. They are not refreshed on every push, only when they are about to be
used.

"Make a plan for <todo item>" means: read the code the entry touches, ask the maintainer the major decisions and
questions up front (the usual planning flow, batched rather than dribbled), then write the plan in enough detail that
the changes are roughly known file by file and the effort can be estimated, and add the reference to the TODO entry. The
plan is the deliverable; it does not start the work.

"Execute plans/<name>.md", "build plans/<name>.md", or a request to turn a plan into a goal file means: FIRST assess how
stale the plan is against what has changed in the repository since its anchor commit (`git log <hash>..main` over the
paths the plan names, and the specs it relies on), re-plan whatever that movement invalidates, asking again where the
answer changed, and only then execute or write the goal. Never feed a plan to execution or goal creation unread against
the current tree, however recent it looks.

# review_feedback_queue/

`review_feedback_queue/` holds review feedback that outlived its review: one file per finding, indexed by
`review_feedback_queue/INDEX.md`. When asked to record review feedback, write it there per
`review_feedback_queue/AGENTS.md`; when asked to address a queued item, remove the item (or narrow it, if partially
addressed) in the same commit, change, or PR as the fix.

## Triage step

"Triage review feedback" means decide what to do with individual items, WITH the user. It does not authorize fixes or PR
creation. Read `review_feedback_queue/AGENTS.md`, the index, root `TRIAGE_OUTCOMES.md` if it exists, and the `Planned`
bucket in `TODO.md`. Work through items in index order unless the user chooses another order, skipping items with a
recorded decision unless asked to revisit them.

For each item, complete these steps before asking the user for an outcome:

1. Understand the feedback. Read the whole feedback file, trace the relevant current code paths, and read the
   authoritative spec sections. Identify the claimed behavior, the conditions that trigger it, and the consequence.
2. Verify whether the feedback is correct; do not take the reviewer's claim as established fact. Check its premises
   against the current code and specs, including guards or lifecycle rules that could invalidate it. Use a focused
   reproduction or check when code inspection cannot settle the claim, following the verification rules above.
   Distinguish a current problem from one that only existed at the reviewed commit. If verification is blocked or
   inconclusive, say exactly what remains unverified and why; do not present it as confirmed.
3. State the assessment to the user, with the evidence and any uncertainty: confirmed, partly correct, incorrect,
   already addressed, or unresolved. Keep correctness separate from whether the issue is worth addressing.

Before presenting an item for a decision, check whether its behavior is explicitly accepted by the current SPEC.md or
SPEC_impl.md, or its fix is already covered by an item in TODO.md's `Planned` bucket. Verify the actual trigger,
consequence, and scope against that acceptance or planned work; sharing a subsystem or keyword is not enough. If fully
covered, skip renewed discussion and record `other` with the exact spec section or planned item as the reason. Remove
the feedback file and its index entry immediately, and record execution as complete for that queue cleanup; do not leave
the item to be skipped again in later sessions. Already planned means acknowledged work, not an implemented fix. Do not
implement it, broaden its scope, or create another TODO merely because the same issue appears in feedback. If only part
is covered, bring the uncovered part to the user. Briefly report skipped items and their basis, then continue to the
next undecided item. This is an exception to the per-item decision question below.

Assume the user knows Farhelm as a tool but has read neither the feedback nor the relevant code. Name the feedback
filename and explain the affected feature or operation, the triggering scenario, expected versus actual behavior, and
the practical consequence. Give enough context to understand the assessment without opening the file or knowing internal
symbols; explain implementation details only where they are needed to understand the issue.

Then recommend an outcome with its reason and ask the user to decide before recording an outcome or moving to the next
item. A recommendation is not a decision; unresolved items stay undecided. The outcomes are:

- `discard`: not worth the human's time at this time. This says nothing about the item's validity, severity, or merit.
  Drop it from the queue without remedial action; do not turn it into a spec exception, code fix, or TODO.
- `fix spec`: clarify the underlying principle in `SPEC.md` and/or `SPEC_impl.md` so similar feedback is suppressed for
  the right reason, not by blacklisting a particular finding. For example, an agreed target of roughly 100 sessions can
  make million-session scalability concerns out of scope. That example is not itself a new product requirement.
- `fix code`: the feedback is legitimate; address the problem within the existing specification.
- `fix spec+code`: make both the agreed specification change and the corresponding code change.
- `other`: negotiate a concrete custom outcome with the user, including what to do with the queue item.

After each decision, create or update root `TRIAGE_OUTCOMES.md`. Use one heading per feedback filename, spelled exactly
as it appears in `review_feedback_queue/`, with these fields:

- Outcome: one of the five values above.
- Assessment: the correctness verdict, supporting evidence, and any unverified claims or limitations, separate from the
  user's chosen outcome.
- Decision: the user's rationale and agreed scope; preserve their meaning rather than substituting the recommendation.
  For spec changes, include the underlying principle; for `other`, include the negotiated action.
- Completion criteria: what execution must accomplish, including any deliberately retained part of the feedback.
- Execution: `pending` initially; later `in progress`, `blocked` with the reason, or `complete`, with the stable jj
  change ID, bookmark, and PR URL as they become available.

Keep previous decisions and execution records; do not overwrite the file on a new triage session. If the user revises a
decision, retain the prior decision and note what supersedes it. Do not create entries for undecided items. Triaged
items are no longer awaiting a triage decision, but their feedback files and index entries stay until execution,
including for `discard`. This preserves the input for the separate execution step and its per-item PR. The automatic
spec-covered or already-planned cleanup above is the exception: those queue items are removed during triage itself.

## Execute triage outcomes

"Execute triage outcomes" is a separate user request. Read root `TRIAGE_OUTCOMES.md` and execute its pending decisions
(or the subset the user names); resume incomplete execution rather than making duplicate PRs. Do not silently triage
undecided items or change an agreed outcome. If current code or specs invalidate a decision, return that item to the
user for clarification.

Load and follow the `jjstack` skill. Make one reviewable commit, stable bookmark, and draft PR per triaged outcome,
including `discard` and `other`, in a single linear stack. Use ledger order unless dependencies require another order;
state the order before starting. Base each PR on the preceding bookmark, with the bottom PR based on the chosen stack
base per jjstack. Do not combine items into one PR or split `fix spec+code` across PRs.

Each item's PR contains its agreed changes, the corresponding `TRIAGE_OUTCOMES.md` execution update, and removal of its
feedback file and index entry (or the agreed narrowing for a partial/custom outcome). A discard PR contains only that
queue removal and outcome bookkeeping, with no spec or code change. Retain completed ledger entries so another execution
request can distinguish completed work from pending work. Record the change ID and bookmark before creating the PR, then
add its URL to the same change and update that PR; do not make a separate bookkeeping PR.

Validate each change using the "Finishing work" rules above. Completion means the agreed work is verified and its draft
PR exists, not that it has merged. Report the item-to-PR mapping and any blocked items. This workflow never marks PRs
ready, enables auto-merge, or merges them; publishing or landing requires a separate user request.

# Harness marks in the sidebar

`docs/harness-marks.md` records where every harness mark in `crates/farhelm-ui/src/icons.rs` comes from, which are
official and which Farhelm drew, the brand terms that decided that, and the one sizing rule they all share. Read it
before adding or changing a mark; vendored geometry gets an entry in `THIRD_PARTY_NOTICES.md`.

# Brand marks: the icon and the wordmark

The app icon is `packaging/farhelm-desktop/icon.svg` (the source of `icon.png` and `Farhelm.icns` beside it). The
wordmark, "farhelm" in JetBrains Mono Nerd Font Bold with the icon's block cursor after it, is
`packaging/farhelm-desktop/wordmark-dark.svg` and `wordmark-light.svg`, one per ground. Its letters are outlines pulled
from the font the UI crate vendors by `docs/readme/outline-wordmark.py` into `docs/readme/wordmark-outline.json`, and
`docs/readme/render-svgs.mjs` inlines them into the wordmark files, the README's header, pillars, and how-it-works
blocks under `docs/readme/`, and the docs website's header mark (`website/src/site-mark-dark.svg` and `-light.svg`).
Anything that needs the name as a mark uses those files; anything that changes the mark changes the script and re-runs
it, never the SVGs by hand, and the outline step only reruns when the word or the font changes.

# Desktop/web UI bug triage

`docs/desktop-web-triage.md` is the recipe: which engine comparison localizes a UI bug, where the unified log lives, and
what a bridge-death line looks like. Start there before investigating any "the desktop UI is broken" report.

# README hero screenshot

The image at the top of README.md is a real capture of the web UI against a staged fleet, governed by
`docs/readme-hero/SPEC.md`. "Refresh the README screenshot" means exactly: run `scripts/readme-screenshot.sh`, look at
the PNG it prints, run `scripts/publish-readme-hero.sh` on it, and commit the one-line README change that leaves behind.
The design (`docs/readme-hero/scenario.json5` and the transcripts beside it) changes only when the maintainer asks for a
different picture. The publish script is the only thing that pushes the `readme-assets` branch; never run that push by
hand, and never commit the PNG on main. Neither script is a gate: nothing in CI runs or checks the image. Changing the
publish script means running its `--self-test`, which is its whole validation and needs no network.

# The live install is off-limits

This machine runs the maintainer's production Farhelm: the released `farhelm` binary at `~/.local/bin/farhelm` (put
there by `scripts/install.sh`) with its state under `~/.local/state/farhelm/`, the `farhelm-helm.service` and
`farhelm-supervisor.service` units that `farhelm helm setup` wrote to run it (plus the machine-local tmux pin under
`~/.local/lib/farhelm-local/` and the `*.service.d/` drop-ins that wire it in), and a separate dev-loop deployment under
`/home/scode/farhelm-live/` (its own source, binaries, and state) behind `farhelm-live-supervisor.service`. Take no
state-changing action against any of that: no running `install.sh` or `farhelm helm setup` here, no rebuilding,
redeploying, or modifying files in those trees, no start/stop/restart/reload/enable/disable/mask/kill of those units,
and no edits to their unit definitions or drop-ins — the verbs are examples, not an exhaustive list, and "it is not
literally named above" is not a loophole. Not to verify a fix, not as a finishing step, and not because the README or
release docs describe how: install documentation is addressed to the human operator, and reading it is not authorization
to run it. `farhelm helm setup --dry-run` is read-only and allowed. The only exception is the user explicitly asking, in
the current session, for a specific one of these things to be done.

This rule is a fence with a history: twice (2026-08-12 and 2026-08-15), sessions asked only to merge PR stacks went on
to rebuild and restart the live helm, the second time deploying a build that locked the operator's browser out the next
morning. Nothing in normal development needs the live install — the test suites create their own supervisors and helms
from temporary state directories, and stopping a transient `farhelm-<uuid>-*.scope` unit that belongs to such a test is
fine.

# Sharing the machine with other agents

Several agents work on this project at once, on this machine, as the same unix user, each in its own checkout or jj
workspace. The harnesses are built so that concurrent runs do not collide: every test picks its ports from the kernel
(the Rust e2e harness binds port 0, the Playwright config and the desktop smoke ask for a free port per run, the CentOS
script lets docker choose), state lives in per-run random directories under `/tmp` addressed by private tmux and
supervisor sockets, systemd scopes and units carry a UUID, and the CentOS script's ssh alias is suffixed per run. Do not
reintroduce a fixed port, a fixed path under `/tmp` or `$HOME`, a fixed unit or container name, or a fixed ssh alias in
a harness; when a harness genuinely needs a stable name, derive it from the run's own random directory.

What is NOT isolated, and what follows from it:

- One agent per checkout or workspace. Playwright's stack-info and auth files, `.ci-tmux/`, `.ci-nextest/`, and the dx
  output under `target/` are per checkout, and `scripts/build-tmux-assets.sh` deletes and rebuilds its python
  environment under `target/` without a lock. Two agents in one tree race on all of them. Sibling checkouts and
  `jj workspace` directories are the supported shape; a shared `CARGO_TARGET_DIR` across workspaces is fine for cargo
  (it locks) but not for the dx and tmux-asset builds: the desktop smoke serializes its own cargo and dx builds behind
  `/tmp/fh-build.lock`, and nothing serializes the rest. (Fixed lock files like that one, and the CentOS script's lock
  next to the ssh config, are the exception the fixed-path rule allows: a lock exists to be shared, and it names no
  state.)
- Processes you did not start are someone else's. A helm or supervisor answering on a port you did not choose, a
  `farhelm-<uuid>-*.scope` unit whose UUID no run of yours minted, an `fh-it.*` or `fh-e2e.*` directory holding a live
  flock, or a docker container from the CentOS script: leave them alone. The teststate sweep already reaps dead runs
  from any checkout, guarded by the flock, so nothing needs killing by hand.
- The `systemd --user` manager and the CPU are shared. Timing budgets, the transient-scope availability probe, and the
  RSS measurement in `terminal_backpressure` assume a quiet machine, and another agent's battery is exactly the load
  FLAKES.md records as tripping them. A failure under that load is an environment event: keep the retained run, re-run
  narrowly when the machine is quieter, and do not log it as a flake or a regression on one observation.

# lore/

`lore/` holds historical artifacts — decision records written when the decision was made. It is not part of the codebase
and is never updated to track code changes. See `lore/AGENTS.md` for its rules before touching anything in it.
