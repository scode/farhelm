## What this was about

Reopening a session could leave the desktop stuck reconnecting to its remembered shell tab after the supervisor could no
longer read that terminal. The maintainer wanted the main terminal to appear after an explicit refusal, with automatic
retries for temporary connection failures.

## Things you should know

- The desktop shows the main terminal if the remembered shell tab is reported closed or unavailable before it attaches.
  The tab stays selectable for as long as it is listed. Selecting an unavailable tab deliberately shows the supervisor’s
  reason and a **Reconnect** button. Both kinds of fallback preserve the remembered choice for the next opening.
- If a connection reset interrupts a file upload, the notice appears in the terminal that ends up visible. If you select
  another session or clear the selection before the remembered shell attaches, the interruption goes to the window’s
  notice center. Review caught two races that could hide the notice when you close the restoring shell or open a new one
  as it disappears. Both are covered by regression tests.
- The helm and desktop exchange one optional flag. The supervisor protocol is unchanged. The fallback for an explicit
  unavailable reply requires the updated desktop and helm. The existing closed-tab signal still works.

## Open questions and possible follow-ups

None from execution.

## PRs

- Distinguish an unavailable shell tab from a connection failure so the desktop can stop retrying an explicit refusal.
  [#4690](https://github.com/scode/farhelm/pull/4690/changes)
- Show the main terminal on an initial refusal and deliver upload notices to the visible terminal.
  [#4695](https://github.com/scode/farhelm/pull/4695/changes)

## Checks

- At `ed6a1b59585a`, pinned nextest on Linux ran 64 cases across the protocol, helm and client. All passed, including
  the real WebSocket and fake-terminal parity case (`829af4f5-850c-4f02-b870-5adc30ec3041`). The selection in
  `cargo nextest run -p farhelm-proto -p farhelm-helm -p farhelm-helm-client --lib` covered `terminal_socket::tests`,
  `terminal::tests`, `fake::terminal::tests`, and the down-host terminal route. The earlier transport-loss fixture
  failed to close the full connection. Its corrected narrow rerun passed (`557c3e6d-d758-4ee1-9e08-9baca45abc24`). Both
  nextest runs and Clippy (`4dbd4baf-70f2-4048-bf37-bc3009d6a2f2`) still cover the first PR because the rebase preserved
  its patch and the desktop follow-up only changes how the GUI uses the result.
- At `41e192d8d713`,
  `cargo nextest run -p farhelm-gui --lib -E 'test(/^terminal::set::tests::/) or test(/^terminal::attached::tests::/) or test(/^terminal::attachments::tests::/)'`
  passed all 249 selected cases on Linux (`23be641d-94bc-44f1-ab87-2bf8345f5193`). The first run failed two new tests
  during setup (`cbde7c21-807f-4fd5-91c7-2dbdeffbd323`). Their fake helm now handles the ordinary session-selection
  request after reconnecting. Both corrected cases passed narrowly before the full selection passed
  (`1e328a59-ceb7-4639-94f2-083962386215`).
- `cargo clippy --all-targets -- -D warnings` passed on the corrected desktop patch
  (`ac652d36-5b80-47f8-9bb4-eda2b0ed2db1`). Test-file `rustfmt --check`, the Rust test-delay audit, and
  `git diff --check` also passed.
- Reused `cargo fmt --all -- --check`, `dprint check SPEC_impl.md TODO.md`, and
  `python3 releasing/check-changelog.py format` from `63dc32c2`. The later desktop revision only corrected the two test
  fixtures, whose formatting was rechecked.
- Skipped the full test battery and manual mixed-version testing. The focused checks exercise legacy wire decoding and
  the desktop restore behavior through a fake helm.

## Review gate

One independent reviewer reviewed each patch. The first review tightened the attach fixture and its assertions. The
second found the two notice-routing races, which were fixed and covered before running the desktop checks. Both commit
messages passed cold reads.

### Landing

#### What landed with it

The plan's two PRs (#4690, #4695) landed second in a round of six plans; none of the others touches terminal tabs or the
helm's terminal relay. The rebase met only TODO.md entry removals.

The landing's independent reviewer confirmed that the new "unavailable" signal works between any mix of old and new
helms and desktops (an old desktop ignores it, an old helm never sends it), that the fallback never shows another
session's terminal, and that closing the tab, reconnecting and changes to the session's tabs all end a pending restore.

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

- **A regression against what the TODO entry asked for, landed as built; I recommend a follow-up.** The report says the
  fallback applies before a tab attaches and that temporary failures keep retrying. In fact the desktop's helm client
  (the part of the desktop app that keeps its terminal connections to the helm) now stops retrying on any "unavailable"
  answer, also for a shell tab that was already attached and working. A supervisor gives that answer while it has not
  yet taken a tab's shell back after losing hold of it: the shell keeps running in a small process of its own (its
  terminal host), and the supervisor keeps trying to reattach to it, for example while the machine is too loaded to
  answer in time, or while a restarted supervisor is still reattaching to its shells at startup. Before this plan an
  open tab caught by that came back by itself once the supervisor had it again. Now the user sees "Detached" with a
  Reconnect button, which works but has to be pressed. I could not establish from the code how often an ordinary update
  hits this; it needs a tab that reconnects inside the supervisor's startup window, which is short on an idle machine
  and longer on a loaded one.
- Why it landed rather than going back: the plan's outline says to stop automatic retries in the helm client whenever
  the helm marks a tab unavailable, which is what was built, and the Decisions cover only reopening a session onto a
  refused tab and choosing a refused tab by hand. Neither covers a tab that was already attached, so this is a gap in
  the plan rather than a defect against it, and changing the retry rule was more than an obvious landing fix.
- The recommended follow-up: the helm client stops on that answer only before a tab's first attachment, and after one
  treats it like any other connection problem and retries. That keeps both decided behaviors, since restoring a tab and
  choosing a refused tab both happen before the first attachment. The helm needs no change, and one of the helm client's
  tests needs adjusting to match.
