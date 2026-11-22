## What this was about

A wide terminal character that does not fit in the last column wraps to the next row. The last column used to lose the
program's background color. Farhelm also guessed whether a space in that column was padding, so copying sometimes
dropped a space the program printed. Erasing just the right half of a wide character could hide the new background
behind the full character.

## Things you should know

The program's background now reaches the right edge at a wide wrap, on screen and in scrollback. Copying keeps printed
spaces and skips the padding. Word selection and links can still cross the wrap. An erase that changes the right half's
background shows the left half of the glyph and the new color on the right. An erase with a matching background keeps
the full character visible. Printed text shifted into the right column by a delete stays visible.

The helm and supervisor protocol moves from 85 to 86 so Farhelm can distinguish a printed space from wrap padding. A
remote host on the old version shows Needs update until its supervisor is upgraded. Existing terminals and retained
scrollback are expected to reattach after an ordinary supervisor upgrade. The real-host check covered attach and replay
on one build. No running supervisor was upgraded in these checks.
[#4588](https://github.com/scode/farhelm/pull/4588/changes)

## Open questions and possible follow-ups

Erasing the left half can still replace the right cell's original background with the terminal's default background.
Separately, the agreed rule keeps the full glyph when the erase leaves the same background. A plain space shifted next
to a wide character can be covered by the glyph if its appearance and attributes match that erased cell.

## Pull requests

- Share the row traversal used by text selection and printed links.
  [#4580](https://github.com/scode/farhelm/pull/4580/changes)
- Mark the wrap padding so copying and link detection keep printed spaces.
  [#4588](https://github.com/scode/farhelm/pull/4588/changes)
- Keep the background color on the terminal's last column at a wide wrap.
  [#4592](https://github.com/scode/farhelm/pull/4592/changes)
- Show the new background after erasing the right half of a wide glyph.
  [#4598](https://github.com/scode/farhelm/pull/4598/changes)

## Checks

- `cargo nextest run --locked -p farhelm-grid-model` passed 44 existing traversal and model tests at `1822939fefbc`
  (recorder `6edbdf6d-a5b8-4f13-9ba4-84113362e10a`).
- `cargo nextest run --locked -p farhelm-proto -p farhelm-grid-model -p farhelm-supervisor -E 'package(farhelm-grid-model) | test(/^grid::tests::|^grid_output::tests::|protocol_version_is_pinned/)'`
  passed 98 selected wire-format, encoder and model tests at `3b23ed58fe8a` (recorder
  `aa1eb640-aa8f-4250-b376-29ab360e1eab`). The two older link-oracle inputs now state where padding is present. Their
  expected results are unchanged.
- `cargo nextest run --locked -p farhelm-proto --lib --run-ignored only -E 'test(=grid::tests::write_binary_grid_fixtures)'`
  regenerated the binary fixtures from the encoder (recorder `7d99fa29-438d-46ed-84b0-bfca9359c379`).
- `cargo nextest run --locked -p farhelm-grid-model -p farhelm-supervisor -E 'package(farhelm-grid-model) | (package(farhelm-supervisor) & test(grid_output::tests))'`
  passed 79 tests at `65adfc84ed7b` (recorder `8c8bcbf4-54ad-4002-895b-ccca48fdd5bf`).
  `cargo nextest run --locked -p farhelm-supervisor --lib -E 'test(=grid_output::tests::colored_wrap_padding_is_one_unlinked_run_on_screen_and_in_history)'`
  then passed with both linked and unlinked text at `3b565e31491c` (recorder `663042f1-9e72-4853-a819-9529b01253be`).
- `cargo nextest run --locked -p farhelm-proto -p farhelm-supervisor -p farhelm-grid-model -E 'package(farhelm-grid-model) | (package(farhelm-proto) & (test(grid) | test(protocol_version_is_pinned_at_85))) | (package(farhelm-supervisor) & test(grid_output::tests))'`
  ran 102 tests at `dc414698d22f`. Of those, 101 passed and the new cross-wrap link assertion looked on the wrong row
  (recorder `b6102205-e32d-4de7-aef9-07f5de1a046c`).
  `cargo nextest run --locked -p farhelm-grid-model --test links -E 'test(=osc_continuation_looks_past_the_styled_padding_run)'`
  passed after the correction at `f19fcd6423e0` (recorder `56126342-4bb3-4cf4-8237-7ad57b42a4d4`). The final passing
  group run is below.
- `cargo nextest run --locked -p farhelm --test e2e -E 'test(=host_terminals::a_host_agent_attaches_as_a_grid)'` passed
  at `f19fcd6423e0`. It decoded binary grid rows from a real host and reached replay completion (recorder
  `c88bcb92-8eef-418d-9052-77590bae0c07`).
- `python3 -B scripts/check-gui-goldens.py --update` rendered the full catalog for each visual draft (recorders
  `99665dda-475f-463f-9c67-49fe0b4322b9` and `c39f591c-c093-46e9-8601-99f2033d6dc4`). Each update changed only
  `terminal-wide-glyphs` in light and dark. Published those references with `python3 scripts/gui-goldens.py publish`.
- `cargo clippy --locked -p farhelm-proto -p farhelm-supervisor -p farhelm-grid-model -p farhelm-gui -p farhelm --all-targets -- -D warnings`
  passed at `f19fcd6423e0`. Ran `cargo fmt -p farhelm-proto -p farhelm-supervisor -p farhelm-grid-model -p farhelm-gui`
  and `dprint fmt TODO.md docs/gpui/screenshot-scenes.json releasing/changelog.d/wide-terminal-glyphs.md`.
  `git diff --check`, `python3 releasing/check-changelog.py format` and `python -B scripts/check-test-sleeps.py` passed.
- The rebase onto `63054db97844` brought in bulk controls, the window command-queue limit and interrupted-upload
  notices. The TODO conflict combined the adjacent removals from main and this stack. The protocol, encoder, grid model
  and terminal scene were unchanged by that rebase, so the targeted test and lint evidence carries forward. At
  `17e3c2b4c29f`,
  `python3 -B scripts/check-gui-goldens.py --scene terminal-wide-glyphs --scene interrupted-upload-notices` matched all
  four reference images (recorder `4dbdebd0-fca9-48bf-8d30-d6d7c1158189`).
  `python3 -B scripts/test-gui-screenshot-batch.py` passed all 47 catalog and batch harness tests (recorder
  `7ca9e1a5-28f6-479c-b588-9c07e4a15676`).
- The recipe-credentials change used version 85 while this was in review. The rebase onto `824b1f9d53` kept its version
  history and moved this stack's pin to 86. Recipe settings and launch changes leave the terminal scene and renderer
  unchanged, so the approved references and prior image comparison still apply.
- `cargo nextest run --locked -p farhelm-proto -p farhelm-supervisor -p farhelm-grid-model -E 'package(farhelm-grid-model) | (package(farhelm-proto) & (test(grid) | test(=tests::protocol_version_is_pinned_at_86))) | (package(farhelm-supervisor) & test(grid_output::tests))'`
  passed all 102 selected tests, including the version-86 pin, at final tip `92c2321f410b` (recorder
  `efa1777f-7518-49ff-9a73-52d89fd56488`). No selected tests returned an early runtime skip.
- Skipped a full workspace sweep because the targeted suites and real terminal attach cover the changed grid behavior.
  Skipped the dedicated terminal-host and snapshot suites because their formats and code are unchanged.

## Review

Independent Sol high source reviews passed after correcting two text readers that still included padding, extending the
encoder test for unlinked text, and fixing the cross-wrap link test's row. Independent Opus 5.5 high reviews approved
the wrap and right-half erase scenes in both appearances. Fresh readers confirmed the wording of the four drafts.

### Landing

#### What landed with it

No other plan landed in this round. The plan's four PRs (#4580, #4588, #4592, #4598) landed with no landing fix. Since
the plan's work was written, main had gained the compact managed-checkout launcher and a dependency fix, neither near
the terminal grid. The rebase conflicted only in TODO.md, where this plan and another landed plan had each removed their
own part of the same shared entry; the landing kept main's version and applied exactly this plan's removals. Main's
supervisor protocol was 85, and this plan alone moves it to 86.

An independent reviewer found no blocker; two minor leftovers, listed at the end, were left as follow-ups:

- **The new row marker.** The supervisor marks wrap padding only when the last cell really is the placeholder of a wide
  character that wrapped, and sends padding that carries a background color as a plain space in the last column. The
  desktop window no longer guesses padding anywhere; it reads the marker.
- **Version skew.** The helm passes the terminal stream through without decoding it, and the desktop app embeds its
  helm, so the only mismatch possible is a helm against a supervisor of another version, which the version check
  refuses, showing the host as Needs update. Scrollback is kept as terminal state, not in the new row format, so nothing
  needs migrating.
- **Tests.** The supervisor tests drive the real terminal emulator for every case the plan lists, on screen and in
  scrollback, and every desktop window test has a control case without the marker.

#### Fixes made while landing

None.

#### Checks

The plan changes the terminal stream's format between the supervisor and the window (protocol 86), the supervisor's
encoder, the desktop window's terminal model and a terminal screenshot, so the landing ran the Rust workspace suite, the
lint runs, the desktop app's tests, the end-to-end smoke test and every reference screenshot. The worker runs were on a
separate arm64 Linux test worker, the rest locally on x86_64.

On the plan as delivered, rebased onto main:

- `cargo nextest run --workspace --exclude farhelm-desktop`, with four nextest slots, no retries and four build jobs, on
  the test worker (recorder run `b105555d-3798-4f47-8d36-28b8b9052941`): all 8332 passed.
- The four clippy runs (`--all-targets`, `-p farhelm --bins`, and the desktop app with and without `--features agent`),
  on the test worker, not through the recorder: passed.
- `cargo nextest run -p farhelm-desktop`, on the test worker (recorder run `af7c77b4-42d0-4136-8e83-41827d140e3d`): all
  185 passed.
- `python3 -B scripts/gui-e2e-smoke.py`, on the test worker (recorder run `5b3e91e0-7237-4fa0-99ca-409f1980aec9`): 49 of
  its 50 steps passed and the paste step was skipped, as on every Linux run.
- `python3 scripts/check-gui-goldens.py`, every scene, locally (recorder run `1f5d1552-7ba8-46dd-bbb4-50d036a266da`):
  passed, including the redrawn wide-character scene.
- `python3 -B scripts/test-gui-screenshot-batch.py`, `cargo fmt --all -- --check`,
  `python -B scripts/check-test-sleeps.py` (in its isolated environment), `dprint check` and
  `python3 releasing/check-changelog.py format`, locally: passed.

Skipped: upgrading a running supervisor across the protocol change, which the plan's own real-host check also did not
do; macOS, since no Mac was part of this landing; CI, which runs only on demand.

#### Things you should know that the report does not say

- A comment in the protocol code still says the padding cell after a wide character is never sent, right beside the new
  paragraph saying colored padding is sent as a space; one clause would reconcile them.
- The report says two text readers were fixed to skip padding; the end-to-end tests' helper that turns a terminal row
  into text is a third one that still ignores the new marker, so a future test of a colored wide wrap would see a
  trailing space. No current test depends on it.
