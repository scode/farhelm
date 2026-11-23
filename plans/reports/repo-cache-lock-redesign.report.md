# Repository cache recovery

## What this was about

A session can fail during checkout if an interrupted Git operation leaves the host's repository cache stuck. Clearing
the locks directly could interfere with Git processes that survived the interruption. The queue decision approved a
separate path for each replacement because those processes can still reopen files at the original path.

If preparing the selected cache fails, Farhelm tries once in a fresh directory. It switches to the replacement after Git
fetches successfully. A failed retry leaves the previous cache selected, so an unavailable remote does not cost the host
the objects it already has. Checkouts and prepared spare checkouts can continue using their copied Git objects after the
cache is removed.

## Things you should know

- The first checkout for each repository after upgrade fetches a fresh cache before retiring the legacy one. It can take
  longer even if the legacy cache was healthy. If the fetch fails, Farhelm keeps the legacy cache on disk.
- Startup removes unfinished replacements and expired caches while respecting the per-repository lock. This includes the
  earlier fix for an expired legacy cache left as a plain file. Background Git cleanup leaves replacement to the
  checkout path.

## Open questions and possible follow-ups

None.

## PRs

- [#4697](https://github.com/scode/farhelm/pull/4697/changes) recovers checkouts and migrates older caches, including
  the startup cleanup fix from the earlier round.

## Checks run, reused and skipped

- On `8a78e870`, 126 selected supervisor tests passed with no runtime skips in recorded run
  `c1832d5b-ab2f-4c49-ac1d-e7e090134599`, using
  `cargo nextest run -p farhelm-supervisor --lib -E 'test(/^repo_cache::|^launch::|^spare_checkouts::|^spare_build::|^service::spare_builder::/)'`.
  The cases cover recovering from packed and loose ref locks, preserving the cache on an unavailable remote, independent
  checkout objects, startup reclaim skipping held locks, and background consolidation taking the same lock and
  compacting packs.
- On `8a78e870`, `cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings`,
  `cargo fmt --all -- --check`, and the test-delay checker passed. The tests and Rust checks still apply after rebasing
  onto `afe81073`. The incoming Codex credential changes affect agent arguments and environment. The cache patch,
  checkout preparation inputs and Git child setup stayed the same.
- Changed Markdown formatting, changelog format and `git diff --check` passed again after the rebase.
- The full workspace runtime suite was skipped because the selected tests cover the affected behavior. Runtime
  validation ran locally on Linux, so it does not cover macOS filesystems or the Git shipped with macOS.

## Review gate

The required fresh source review found two issues. An interruption during expiry could leave a half-deleted legacy cache
at its original path, and Git failure diagnostics could omit the directory Git opened. Both were corrected and rechecked
by the source reviewer. A separate cold read checked the commit and PR wording.

### Landing

#### What landed with it

The plan's PR (#4697) and one landing fix (#4709) landed fifth in a round of six plans; none of the others touches
repository caches. The rebase met only a TODO.md entry removal. This plan blocked earlier on whether to give each
replacement of a repository's cache a directory of its own; the landing monitor answered it on your behalf with that
option (recorded under "Plan decisions to revisit" in TODO.md), and the landing's independent reviewer confirmed the
plan built what that answer chose.

The reviewer found that no Git process surviving an interrupted cleanup can touch a replacement cache, that a failed
replacement keeps the previous cache selected, and that checkouts and spare checkouts keep working after a cache is
removed.

#### Fixes made while landing

- The landing's full test run failed three spare checkout builder tests, in the farhelm command's own tests, which the
  plan's runs (limited to the supervisor's library) had not included. They still looked for the cache at its old single
  path. #4709 makes them follow the new layout; the product code was right.
- The docs page on starting a session still gave the old cache path; #4709 describes the new one.
- #4709 adds the caveat the report and SPEC_impl.md state to the release-notes draft: after updating, the first managed
  checkout of each repository on a host downloads it again in full.

#### Checks

This round landed toolkit-reduced-motion (the vendored GPUI core, the desktop app's outer program and the desktop
window), fallback-unattachable-remembered-tab (the helm's terminal relay, its message format, the helm client and the
desktop's terminal), fix-muse-conversation-reporting (the supervisor's Muse hooks and the farhelm command's hook),
gui-goldens-in-releases (instructions only), repo-cache-lock-redesign (the supervisor's repository caches) and
brick-test-remote-mac-uninstall (the release brick test's harness) together. Between them they reach every crate, so the
landing ran the whole workspace suite, the desktop app's own tests, all lints, the end-to-end smoke and Linux lifecycle
suites, the vendored GPUI core's own tests, the brick test harness's portable tests, every reference picture and the
docs site build on the six plans stacked on main, then narrower runs after the landing fixes. The worker runs were on a
separate arm64 Linux test worker with a systemd user manager, rootless Podman and SSH to itself; the rest ran locally on
x86_64 Linux. None of them used a real Muse Code, a real Mac or a real GitHub repository; the landing's tests use
stand-ins for those (the Muse plan's own report also ran a real Muse Code offline).

On the six plans as delivered, stacked on main:

- `cargo nextest run --workspace --exclude farhelm-desktop`, on the test worker (recorder run
  `94c1ab85-1040-4d08-bd23-7e079a2f1ad7`): 8451 of 8454 passed and none printed a runtime `SKIPPED`. The three failures
  are spare checkout builder tests that repo-cache-lock-redesign broke, which its landing fix corrects; the failed run
  is kept.
- `cargo clippy --all-targets -- -D warnings` on the test worker failed once, on a function the landing itself had given
  an eighth argument while merging fix-muse-conversation-reporting with main's Codex credential work; see that plan's
  notes. After that was corrected it passed, with the other lints, as listed below.
- `cargo nextest run -p farhelm-desktop`, on the test worker (recorder run `4d9ea5a4-7ef3-4809-af75-6c2ee1a832f2`): all
  184 passed.
- The vendored GPUI core's tests of Farhelm's patches, `cargo test --manifest-path vendor/gpui-pre/Cargo.toml` with
  `--features test-support` and the four test binaries `.agents/validation.md` names (the new `reduce_motion` among
  them), on the test worker (recorder run `21c42e62-e85d-40eb-87a1-ad67ca5b23f0`): passed.
- The brick test harness's portable tests, `python3 -B -m unittest discover -s releasing/mac-vm-test -p 'test_*.py'`, on
  the test worker (recorder run `7be643a1-4eab-4d1c-88c2-fab46002f967`): all 315 passed.
- `python3 -B scripts/gui-e2e-smoke.py` and `python3 -B scripts/gui-lifecycle.py`, on the test worker (recorder runs
  `33d62eb8-f64c-4de5-8c8d-76496785f56d` and `f451571c-c20a-45a5-b779-256fd6f1d490`): of the smoke test's 50 steps, 49
  passed and one was skipped as always on Linux (paste, which headless Mutter cannot give the app); every lifecycle step
  passed.
- `python3 scripts/check-gui-goldens.py`, locally (recorder run `6a8c0f83-8308-4330-873a-afc10d70e032`): every picture
  matched its reference on the combined stack.
- `python3 -B scripts/test-gui-screenshot-batch.py` (a self-test of a script that starts nothing, which the validation
  guide lists without the recorder, so it has no run id), `cargo fmt --all -- --check`,
  `python -B scripts/check-test-sleeps.py`, `dprint check`, `python3 releasing/check-changelog.py format`,
  `python3 scripts/check-agents-md.py` and the docs site build (`bun run build` in `website/`), locally: passed.

After the landing fixes:

- The three vendored terminal crate suites, which the validation guide asks for whenever anything under `vendor/`
  changes (`cargo test` for `vendor/vte` with `--features ansi`, `vendor/alacritty_terminal` and `vendor/pair-tests`),
  on the test worker (recorder runs `1a1b556e-ae84-4955-862f-6194d6a7666e`, `a4d64bd6-a402-4686-b307-4be230c334ed` and
  `a322826a-3d5a-4799-bf98-898c9926208b`): 62, 166 and 3 passed. The round's own change under `vendor/` is in the GPUI
  tree and its notes, which these crates do not build from.

- `cargo nextest run -p farhelm --test spare_builder`, on the test worker (recorder run
  `77ae841d-669f-4777-8e54-c5b326d2496b`): all 13 passed, the three corrected tests among them.
- `cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings` and the desktop app's two
  `cargo clippy -p farhelm-desktop --all-targets` runs (with and without `--features agent`), on the test worker:
  passed.
- `cargo fmt --all -- --check`, `python -B scripts/check-test-sleeps.py`, `dprint check`,
  `python3 releasing/check-changelog.py format`, `python3 scripts/check-agents-md.py` and the docs site build, locally:
  passed.

Reused: the workspace run, the desktop app's tests, the smoke and lifecycle suites and the reference pictures above for
everything else, since the fixes change only the spare builder tests, comments, docs, release-notes drafts and one lint
attribute. Skipped: macOS, where the reduced-motion and remote Mac uninstall changes have their own paths that no run
here reaches, and where the repository caches meet macOS filesystems and Apple's Git; real Muse Code, a real Mac and
real GitHub repositories; CI, which runs only on demand.

#### Things you should know that the report does not say

- A replacement that is cut short leaves its half-built directory, up to the size of the repository, until the
  supervisor next starts, and each further cut-short replacement adds another. That happens when a session start that is
  preparing a managed checkout is cancelled, or when a background spare checkout build hits its time limit. The
  replacement holds the repository's lock, so it could clear such leftovers before starting; a possible follow-up.
- Each repository's cache now has a small file beside it naming the directory in use. Farhelm writes it in one atomic
  step, so a crash or a full disk cannot leave it half-written; only something outside Farhelm (a damaged disk, a hand
  edit) can damage it. If that happens, every managed checkout of that repository on that host fails with "invalid
  repository cache selector" until someone deletes the file by hand, and the error does not say to. Low likelihood;
  treating a damaged file as missing would be a cheap follow-up.
- The supervisor's startup cleanup of old caches still stops at the first cache it fails to remove, as it did before
  this plan, so the caches after it wait for the next start. A known limitation, not new.
- After updating, each repository is downloaded again rather than its old cache being moved into a new directory. I
  would keep that. Moving would save one full download per repository per host, but a Git process left running by the
  old supervisor inside the old cache would move along with it into the cache now in use, which is the situation the
  separate directories exist to rule out.
