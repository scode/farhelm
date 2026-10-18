# Validation: choosing and running checks

The full "Finishing work" text from the root `AGENTS.md`, moved here with its references rewritten: how to choose
checks, the inventory of every CI-equivalent check and what it covers, reuse after a rebase, CI and the release
workflow, and browser end-to-end validation. Read it before choosing validation, running any runtime test, creating or
updating a PR, merging one, or reporting work as done (`AGENTS.md`, "Tests and validation"). The principles that apply
to every task are also in that root section, word for word; when you change one copy, change both.

## Finishing work

For changes to tests or their fixtures, apply `.agents/test-authoring.md` during authoring and supply its full text
verbatim to the independent reviewer. Use the review process selected for the task; this checklist does not request an
additional reviewer or a review swarm. Report concrete violations of the relevant contracts, not mechanical demands for
every bullet to appear in every test.

Run `python -B scripts/check-test-sleeps.py` with the isolated interpreter prepared in `docs/test-sleep-check.md` when
changing Rust or browser tests, their helpers, or the checker. The CI workflow's formatting job runs this source-only
check and requires zero unannotated delays. A `sleep-ok` reason explains a deliberate observation window, scheduling
stimulus, or polling interval; it does not replace a readiness oracle or excuse an unverified fixture premise.

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
- `cargo clippy -p farhelm --bins -- -D warnings` — lints the shipped binary's configuration, in which the supervisor's
  `test-seams` feature is off; the `--all-targets` run above always has it on, because the e2e tests enable it through a
  dev-dependency.
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
- `cd website/feedback-tests && node --test` — the feedback endpoint's handler (`website/feedback/handler.js`, deployed
  as `website/api/feedback.js`): method, configuration and size refusals, field caps counted in code points, fences that
  user text cannot close, and that no response or log line carries the token, the inbox's name, the message or the
  contact. Configuration and the GitHub client are injected; nothing touches the network or `process.env`. Run it when
  the handler or its tests change; CI's website job runs it beside the website build.
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
- `bash scripts/test-install-sh.sh` — drives the macOS-only installer as a real child against a fixture HTTP server,
  using a Darwin/arm64 `uname` shim on Linux and BusyBox. Covers fresh install, updates side by side and stopped at each
  step, the forwarder, 404, checksums, malformed archives, version normalization, missing prerequisites, exact closing
  messages and tmux advice, terminal styling/progress and plain redirected output, app layout and ownership/recovery.
  Linux refuses before downloading or writing; old releases (without app resources, or from before the side-by-side
  layout) refuse before anything changes. Nothing outside `~/.local/bin` and `~/Applications/Farhelm.app` changes, and
  no service manager runs. Every invocation uses `env -i` with an explicit environment, never the harness process's own.
  Refused `/latest` cases attribute requests by their scenario's origin path and report unexpected request lines, so
  delayed requests from another fixture cannot silently become this install's request count.
- `python3 scripts/record-test-run.py --kind development --selection 'installed uninstall acceptance' --concurrency 'one fixture at a time' --tmux none -- python3 scripts/test-uninstall.py --binary target/debug/farhelm --installer scripts/install.sh`
  — macOS-only, after building the CLI: runs the actual installer and installed uninstaller against private fixture
  homes and release archives. Covers updates, confirmation, dry-run preservation, foreign files and native bundles.
  Other platforms report a skip; Linux no longer has end-to-end coverage of installer-made installations because the
  installer refuses Linux. The focused macOS CI job and release gate also run
  `cargo nextest run -p farhelm --bin farhelm -E 'test(uninstall::)'` through the recorder with four slots and
  `--tmux none`.
- `scripts/test-provision-centos.sh` — the x86_64 Linux release gate boots a systemd CentOS Stream 9 container and makes
  the helm provision it over ssh, which is the only coverage of a helm installing onto a distribution other than its
  own. Needs docker and `musl-tools`: the payloads it pushes are the release's musl-static `farhelm` and static tmux,
  because the workspace's glibc debug binary cannot exec on CentOS 9. A few minutes, most of it the musl build; the
  container image and the tmux build are both cached after the first run.
- `dprint check`
- `python3 scripts/check-agents-md.py` and `python3 scripts/check-agents-md.py --self-test` — keeps the agent
  instructions loadable: the root `AGENTS.md` within the 28 KiB budget (Codex injects only its first 32 KiB and drops
  the rest silently) and ending with its truncation sentinel, every path an instruction file names and every
  `` `file`, "Heading" `` citation resolvable, every `.agents/` file reachable from the root, and no `CLAUDE.md`
  anywhere (one disables Claude Code's native `AGENTS.md` loading). Run it whenever an instruction file changes or a
  Markdown heading is renamed or a file moved; the self-test when the script changes. Seconds, no network. CI runs it in
  the dprint job, which, unlike the formatting job, also runs for docs-only changes.
- `cd website && bun install --frozen-lockfile && bun run build` — the docs website (Astro + Starlight) must build; this
  is what catches a page with missing frontmatter, a sidebar slug that names no page, or a broken internal link or
  anchor. Bun is the package manager the Vercel project is configured with, so the lockfile is `bun.lock`; commit it
  with any dependency change. Vercel itself never builds on a push; see `.agents/vercel.md`. The build itself runs under
  Node (the `astro` binary's shebang), and Astro refuses Node older than 22.12; the CI job pins Node for that reason.
- `dist generate --check` — `release.yml` is generated from `dist-workspace.toml` plus `.github/dist-build-setup.yml`,
  and the release `plan` job refuses a stale one; this asks the same question before a tag has to. Needs the pinned
  cargo-dist (`cargo install --locked cargo-dist --version 0.32.0`, the version `dist-workspace.toml` names).
- `scripts/publish-docs-shots.sh --self-test` — the docs screenshot publisher's whole validation: against a bare
  repository standing in for GitHub, it checks that refused publishes push nothing, that only `refs/docs-assets/keep` is
  ever created (never a branch), that a snapshot holds exactly the shots the docs pages reference, that partial,
  incomplete, or since-modified captures are refused, that the manifest records the capture's main commit, and that a
  snapshot stays until six weeks after it was replaced (the one main pins, always). Run it, with `shellcheck`, when the
  script changes; seconds, no network, needs ImageMagick's `convert`.
- `bash scripts/test-plans-watch.sh` — drives the plans monitor's watcher (`scripts/plans-watch.sh`) against a strict
  stub `gh` that runs the watcher's own `jq` filter over GitHub-shaped tree listings: change detection, baselines read
  from a commit, the wake check that keeps claims from waking idle executors, the idle cap, retried and persistent
  failures, malformed and truncated responses, hung requests, argument errors, and prompt exit on SIGTERM without
  leaving children behind. Run it, with `shellcheck` on both scripts, when either changes; under a minute, no network,
  needs `jq`.
- `python3 scripts/test-plans-queue.py` — the plans queue script (`scripts/plans-queue.py`), which is the lock that
  keeps concurrent plan executors off each other's plans: its line grammar, every state transition and refusal, the
  invariants between a state and its files, the rules a planning PR must keep, the wake check, and the compare-and-swap
  loop against an in-memory GitHub that injects racing writers and lost responses. One case runs the real `dprint` when
  it is on PATH, which downloads dprint's plugins if its cache is cold. Run it when the script changes; seconds, no
  other network.
- `python3 scripts/test-todo-picker.py` — the todo planning flow's checklist page (`scripts/todo-picker.py`): how
  TODO.md buckets and entries are read, that entries with a plan are left out, that a summaries file written against an
  older TODO.md is refused, and that summaries are escaped into the page. Run it when the script changes; seconds, no
  network.
- `python3 releasing/check-changelog.py format` and `python3 releasing/check-changelog.py --self-test` — the changelog
  layout lint the release gate runs on every tag, and the checker's own fixtures (format variants, a synthetic git
  history for the fragment sweep, synthetic dist manifests for the announcement comparison). Run the lint when
  `CHANGELOG.md` or a fragment changes, the self-test when the checker does.
- `python3 releasing/finalize-release-notes.py --self-test` — the body swap that replaces a stable release's draft notes
  on its GitHub release with the approved text, against bodies shaped like dist's, and the refusals that need no
  repository or release. Run it when that script or the checker's parser changes; seconds, no network.
- `python3 releasing/notes-review-page.py --self-test` — the interactive page a stable release's draft notes are
  reviewed on: entry ids, escaping and PR links, and, under node, that the copied feedback block carries each note and
  that a regenerated page drops notes typed on its previous revision. Without node it says `SKIPPED` for the page script
  part. Run it when that script changes; seconds, no network.

This inventory names the relevant local checks. The CI workflow (`ci.yml`) covers formatter, Clippy, desktop
compilation, the JS harness, the website build and its feedback endpoint tests, installer validation, generated-workflow
validation, and the agent-instructions check; costly Rust, pinned-tmux, desktop runtime, and CentOS gates run in the
x86_64 Linux release artifact job. If either workflow changes, update this list in the same change.

The CI workflow runs ONLY on demand: it has no push or pull-request trigger (removed 2026-09-12), so neither a PR, a
`gh pr ready`, nor a merge to main starts a run. The release build gate is the validation that decides whether a build
ships; the local checks above are what validates a change before it lands. The focused headless macOS uninstall suite
runs with `gh workflow run ci.yml --ref <branch> -f suite=uninstall-macos`. `gh workflow run ci.yml --ref <branch>` runs
the whole baseline for any ref when a hosted verdict is wanted. PRs are still opened as drafts and marked ready when the
user asks to publish them, or as part of landing them, never on the agent's own initiative.

The RELEASE workflow is not on that list: it is generated, runs on tag pushes only, and carries its own gate, which is
not something to run locally. `releasing/.agents/workflow.md` describes it, the parts of it that CAN be checked locally
(the changelog checks above among them), and the unsigned-release recovery procedure.

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
