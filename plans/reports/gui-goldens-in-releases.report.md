### What this was about

The desktop's screenshot tests compare reference images pixel for pixel. The maintainer kept those checks in development
and pre-merge landing validation only, for changes to the app's drawing code, bundled fonts, or GUI toolkit. The
validation guide now records that decision and the TODO entry is closed.

### Things you should know

Exact comparisons depend on hosted runners' Mesa and font versions, and fetching the reference images directly requires
access to a private repository. Those dependencies are why pixel comparisons stay out of CI and release jobs. Release
coverage comes from Linux and macOS desktop unit tests and a headless Wayland smoke test on Linux.

### Open questions and possible follow-ups

None.

### PRs

- Records when to run the desktop screenshot comparisons and removes the settled TODO entry.
  [#4692](https://github.com/scode/farhelm/pull/4692/changes)

### Checks run, reused and skipped

`dprint check .agents/validation.md TODO.md` and `python3 scripts/check-agents-md.py` passed. The instruction check
reported one existing warning about a heading cited in the triage archive. The change is Markdown only, so validation
was limited to documentation checks.

### Review gate

The requested independent source review passed after clarifying that private repository access is needed for a direct
fetch. A comparison can also use a local copy of the references. The PR text passed a separate cold read.

### Landing

#### What landed with it

The plan's PR (#4692) landed fourth in a round of six plans; it changes only the validation instructions. Other changes
in the round and on main since its base edited other paragraphs of the same guide, none of them this plan's. The rebase
met only a TODO.md entry removal.

The landing's independent reviewer checked the new text against the release workflow's setup and found it accurate.

#### Fixes made while landing

None.

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

- The report says the validation guide now records the decision to keep the reference picture comparison out of CI and
  the release workflow. It does give the rule (compare only during development and landing) and the reasons (runner
  graphics and font updates change exact pixels, and the references sit in a private repository), but not that this was
  a deliberate decision or when it was made. One sentence saying so would keep it from being reopened; optional.
