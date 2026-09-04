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

Before creating or updating a PR, or claiming work is done, use judgment to select the checks below that can
meaningfully validate the change. The list is the authoritative inventory of CI-equivalent gates and what each one
covers, not a requirement to run every command for every diff. Run each command whose inputs, generated artifacts, or
exercised behavior may have changed. Select and order checks by the risks they cover, the signal they provide, their
cost, and the uncertainty that remains. Cost can favor a fast check over an equally credible slow one, but it cannot
excuse skipping the only check that covers a material risk. Widen when a change is cross-cutting, security-sensitive,
lifecycle-sensitive, installer- or release-sensitive, or otherwise uncertain, and use the whole list when that is the
most credible validation rather than as a mechanical default.

For a documentation-only change that cannot affect generated artifacts or executable examples — ordinary Markdown
documentation, a lore entry, agent instructions, or comments with no doctest or generation role — do NOT run Rust,
JavaScript, desktop, installer, provisioning, or browser tests. Run only formatters, linters, link checkers, or other
checks that actually inspect the changed files, if any exist. Do not run a check solely because it appears in the full
gate list.

When building a stack of reviewable changes, do not run the highest-cost relevant gates after every commit or PR by
default. At each review unit, run fast, high-signal checks that cover that unit and catch failures while they are cheap
to localize. Once the intended stack is complete, run the highest-cost relevant gates once at the stack tip so they
cover the combined diff. Run a highest-cost gate earlier when it is the only credible coverage for a material risk or
when later work depends on its result. If the stack changes after a tip result, apply the per-command reuse rule and
rerun only checks whose coverage is no longer valid.

Reuse is per command. Check the diff from the tested revision to the current head, rerun anything whose inputs or
exercised behavior may have changed, and rerun when there is real uncertainty about coverage. A failed, interrupted,
stale, or poorly identified run is not reusable. Report checks run now; checks reused, with the covered revision and why
the intervening diff leaves their coverage intact; and checks skipped, with the reason.

- `cargo fmt --all -- --check`
- `cargo clippy --all-targets -- -D warnings`
- `PATH="$(scripts/build-pinned-tmux-ci.sh):$PATH" cargo test -- --show-output` — the suite drives the pinned tmux from
  `.github/release/source-pins.env`, which the build script makes available under `.ci-tmux/` (one-time build, then
  cached); a run against whatever tmux is on your PATH exercises a different substrate than CI does. `--show-output` is
  what makes a loudly-skipped test's reason visible; the cgroup tests skip themselves where no systemd user manager
  exists, and libtest hides a passing test's output otherwise. CI adds `--test-threads=4` to match its 4-vCPU runners
  and keep the process-heavy tmux integration suite's aggregate load predictable; a beefier local machine does not need
  the cap.
- `scripts/test-tmux-pinned-shutdown.sh` — builds the exact checksummed release named by
  `.github/release/source-pins.env` on cache miss, then runs every focused output-client teardown regression in its own
  test process. CI uses the same script separately from the distro tmux used by the full suite because silently
  substituting another version loses the affected-version coverage.
- `cd crates/farhelm-ui/js-tests && node --test` — the JS unit harness for the asset-JS layer's pure functions
  (PLAN_M6_5.md item 1); node is already a CI requirement for Playwright below, so this adds no dependency. Run from
  inside the directory rather than as a glob from the repo root: node's no-argument default discovery (every `*.test.js`
  in cwd) is the oldest, most version-portable form the test runner has, whereas quoted-glob CLI arguments are newer and
  CI pins no node version.
- `cargo check -p farhelm-ui --features desktop` — the desktop renderer compiles nowhere else; needs the webkit2gtk/gtk
  dev packages (see the CI job for the apt list).
- `cargo test -p farhelm-ui --features desktop` — exercises the desktop-only persistence and IPC seams; needs the same
  webkit2gtk/gtk dev packages as the desktop compile check.
- `cargo check -p farhelm-desktop` — the shipped desktop binary sits outside `default-members` (so ordinary builds never
  compile WebKit), which means `-p` is the only thing that ever compiles it.
- `scripts/check-desktop-assets.sh` — holds the desktop build's `asset!()` set and the web bundle's files to the same
  set, in both directions. Needs `dx` and the wasm32 target; takes a few minutes, since it wipes `target/dx` and
  rebuilds both bundles.
- `PATH="$(scripts/build-pinned-tmux-ci.sh):$PATH" scripts/desktop-smoke.sh` — before any of it needs Xvfb, four
  pre-display process legs drive `farhelm-desktop`'s own entry point directly: a missing tmux and a below-floor tmux
  each refuse with one exact plain stderr line and exit status 1 (the app's own preflight, run before it would spawn its
  managed supervisor), and a non-tmux bootstrap failure (an unusable state directory) exits 1 through the ordinary
  fallback path with no panic. The non-pixel Xvfb integration gate that follows covers the embedded helm, managed
  supervisor, desktop authentication, the tmux override reaching the managed supervisor, hard-exit tether, restart
  persistence, and that every asset the window requested went through the desktop asset handler. The optional
  coordinate-driven leg is not part of CI.
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
- `scripts/test-provision-centos.sh` — boots a systemd CentOS Stream 9 container and makes the helm provision it over
  ssh, which is the only coverage of a helm installing onto a distribution other than its own. Needs docker and
  `musl-tools`: the payloads it pushes are the release's musl-static `farhelm` and static tmux, because the workspace's
  glibc debug binary cannot exec on CentOS 9. A few minutes, most of it the musl build; the container image and the tmux
  build are both cached after the first run.
- `dprint check`
- `dist generate --check` — `release.yml` is generated from `dist-workspace.toml` plus `.github/dist-build-setup.yml`,
  and the release `plan` job refuses a stale one; this asks the same question before a tag has to. Needs the pinned
  cargo-dist (`cargo install --locked cargo-dist --version 0.32.0`, the version `dist-workspace.toml` names).

These commands mirror `.github/workflows/ci.yml`; if CI changes, update this list in the same change (and vice versa).

CI does not run on DRAFT pull requests, and every PR in a stack is opened as a draft. A stack gets rewritten and
re-pushed many times before anyone wants a verdict, and each rewrite used to cost a full run per PR (73 runs in one day
on 2026-09-02). Marking a PR ready (`gh pr ready <n>`) is the request for CI; `gh workflow run ci.yml --ref <branch>`
runs it on demand for any ref, draft or not. PRs are marked ready when the user asks to publish them, or as part of
landing them, never on the agent's own initiative.

The RELEASE workflow is not on that list and has nothing to run locally. `.github/workflows/release.yml` is generated by
cargo-dist from `dist-workspace.toml` — never hand-edit it; change the config and run `dist init --yes && dprint fmt` to
regenerate. It runs on tag pushes only (`pr-run-mode = "skip"`, D19), so a PR exercises none of it: the release path is
validated when a tag is cut, by the gate `.github/dist-build-setup.yml` puts at the top of every build job — every test
target except the tmux-driven e2e suite on the x86_64 Linux one (that suite is out of the gate until its load-sensitive
tests are deflaked; TODO.md lists them and the condition for putting it back), an Apple compile on the macOS one. That
gate lives inside the build jobs rather than in a `plan-jobs` workflow because dist 0.32 lets a failed plan job SKIP the
build jobs, and its `host` job accepts a skip; a failure inside a build job is the only kind it refuses. What a change
to release plumbing CAN be checked locally is `dist plan` (the config parses and the asset list is what you expect), the
release scripts' own `--self-test` modes (`scripts/check-release-archive.py`, `scripts/check-static-elf.sh`,
`scripts/check-desktop-assets.sh`), and `shellcheck` over the scripts the workflow calls.

When a tag produces a public release that never got its `SHA256SUMS`, the recovery procedure is in
`dist-workspace.toml`'s header ("RECOVERY: a release that exists but was never signed"). It is maintainer-run: delete
the release, never the tag, then re-run the workflow.

# Cutting an RC release

An RC exists so the maintainer can `curl | sh` a candidate onto a real machine before anything merges. The TAG, not any
merge, is what triggers the release workflow, so an RC can be cut from the tip of an unmerged PR stack — that is a
normal move here, not a trick (v0.2.1-rc.1 and rc.2 shipped this way on 2026-09-01, trialing the Farhelm.app bundle and
the clipboard fix before either landed).

When asked for an RC, settle TWO choices first, and ASK about each unless the request states it explicitly — a bare "cut
an rc" states neither, and guessing wrong publishes the wrong binaries, or the wrong version number, to a real, public
prerelease:

- The BASE: cut from main, or from the current in-flight PR stack's tip? "cut an rc with this stack" is explicit; "cut
  an rc" is not.
- The VERSION: is this the next attempt at the SAME candidate — a previous `X.Y.Z-rc.N` exists for the target version
  and this continues it, so it is `X.Y.Z-rc.N+1` — or the FIRST rc of a new target version? If the latter, which
  component bumps is the maintainer's semantic call, not something to infer from the diff: patch (`X.Y.Z+1-rc.1`), minor
  (`X.Y+1.0-rc.1`), or major. Name the exact resulting version string when asking, so the answer is a version, not a
  category.

With both settled, the process is:

- Bump the version to `X.Y.Z-rc.N` (N increments per attempt; never reuse a tag name) in the root `Cargo.toml`'s
  `[workspace.package]` and `packaging/farhelm-desktop/dist.toml`, and refresh `Cargo.lock` (running `cargo metadata`
  suffices). The release commit is exactly those three files with the message `chore: release X.Y.Z-rc.N` — the shape
  every release commit here has (#304, #311, #314).
- Before tagging, sanity-check the announce: the version-parity tests
  (`cargo test -p farhelm-helm --lib provisioning::assets`) and `dist plan` naming the rc version with BOTH packages
  under it — a version mismatch makes the desktop archive silently vanish from the release.
- Give the bump its own PR like any other commit (stacked on the stack tip, or based on main), but do not merge anything
  for the release's sake: push the tag `vX.Y.Z-rc.N` at the bump commit and the workflow runs from the tag. Its build
  gate runs every test target except the tmux e2e suite on the tagged commit (see "Finishing work" above); that gate is
  the release's validation, so local slow-battery reruns are not a prerequisite for tagging.
- Watch the workflow run to completion rather than fire-and-forgetting it, then verify the published release: every
  asset present including `SHA256SUMS` and `SHA256SUMS.minisig`, and the release marked prerelease (cargo-dist does that
  for `-rc.N` versions on its own — `releases/latest` must still point at the last stable, so ordinary installs are
  unaffected).
- Finish by handing the maintainer the exact copy-paste command, with the installer fetched FROM THE TAG — when the rc
  comes from a stack, main does not have the rc's installer — and the version pinned on the far side of the pipe:

  ```
  curl -fsSL https://raw.githubusercontent.com/scode/farhelm/vX.Y.Z-rc.N/scripts/install.sh | FARHELM_VERSION=vX.Y.Z-rc.N sh
  ```

  Remind the maintainer to quit the desktop app before updating and relaunch after.
- A failed tag build publishes nothing; fix on the stack and cut `rc.N+1`. The stale tag stays (tags are never deleted;
  the unsigned-release recovery above is the one exception's procedure, and even it keeps the tag).
- One jj side effect to expect: once the rc tag is fetched, jj treats every commit under it as immutable, so a later
  mid-stack rewrite of those commits (a fixup round after the trial, say) needs `--ignore-immutable`. That is safe —
  rewriting creates new commits and the tag keeps pointing at what it tagged — but it will otherwise refuse with
  "immutable commits are used to protect shared history" at exactly the moment a trial's feedback wants applying.

# Cutting a dev release

A dev release is an RC under another name: `X.Y.Z-dev.N`, tagged `vX.Y.Z-dev.N`, cut by exactly the procedure above with
`dev` in place of `rc` everywhere — the bump commit is `chore: release X.Y.Z-dev.N`, N increments per attempt and a tag
name is never reused, the workflow runs from the tag and marks the release a prerelease (any semver prerelease suffix
does; `releases/latest` still points at the last stable), and `scripts/install.sh` accepts
`FARHELM_VERSION=vX.Y.Z-dev.N` the same way it accepts an `-rc.N`. Settle the same two choices first, base and version,
and ask when the request does not state them; the `-dev.N` and `-rc.N` counters are independent, so `0.3.0-dev.2` and
`0.3.0-rc.1` can both exist. The name is the whole difference: it tells whoever reads the tag list later that the build
was a trial of work in progress, not a claim that this is what will ship as `X.Y.Z`.

The browser end-to-end suite is deliberately NOT in that per-change list, and its CI job is disabled (`if: false` in
ci.yml): it is far too slow to pay on every PR. It gates MERGING instead — before landing a PR stack on main, run
`cd e2e && npx playwright test` (Chromium and WebKit; WebKit stands in for the desktop app's actual engine family). It
needs `cargo build` and `cd crates/farhelm-ui && dx build --package farhelm-ui --platform web --release` first (it
drives the built web UI against a real helm and supervisor), plus a one-time
`cd e2e && npm install && npx playwright install chromium webkit`. This split lets changes accumulate across a stack and
surface bugs once, before merge, without each PR paying the suite's cost — but it also means NOTHING else runs it: CI
green does not include e2e. The reuse rule above applies here too. A successful run covering the stack may stand when
later changes are provably outside the browser suite's inputs and exercised behavior (documentation-only changes are the
obvious case); compare the tested revision to the landing head and rerun for any relevant or uncertain change.

# Reproducing failures: narrow tests first

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

# TODO.md

`TODO.md` is the maintainer's running list of wanted fixes and features. When a PR addresses an entry, remove that entry
in the same PR — the file only ever describes what is still wanted. Do not add entries on your own initiative; they are
the maintainer's.

"tldr todo", "what's in the todo", and similar requests mean the FULL list, grouped under the file's own bucket
headings, in the file's order, one to two sentences per entry: what it is and, when the entry says so, why or the first
step. Every entry, not a selection — the point is to see the whole board at a glance. Bold a short handle at the start
of each line so an entry can be referred to by name afterwards.

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

# Desktop/web UI bug triage

`docs/desktop-web-triage.md` is the recipe: which engine comparison localizes a UI bug, where the unified log lives, and
what a bridge-death line looks like. Start there before investigating any "the desktop UI is broken" report.

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

# lore/

`lore/` holds historical artifacts — decision records written when the decision was made. It is not part of the codebase
and is never updated to track code changes. See `lore/AGENTS.md` for its rules before touching anything in it.
