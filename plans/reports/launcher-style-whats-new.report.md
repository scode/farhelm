# Give What's new the session launcher's look

## What this was about

The upgrade dialog previously grouped highlights under category headings and showed one count across all releases. The
maintainer chose the launcher sibling design and asked to keep every entry in full. The new dialog shows the upgrade
range, highlights by category, and the date and total changes for each release. Each entry has its category mark and the
full text, with a separate headline when release curation supplies one.

## Things you should know

Release curation now proposes a headline for each highlight. For wording already supplied by the maintainer, the release
draft keeps that wording while the headline awaits approval. The format checker requires headlines only in new finalized
sections. Before landing the first PR, confirm the cutoff still matches the newest section on main, including any
in-progress draft. At delivery it is v0.45.0.

The Screenshot link stays on a separate line at the end of its entry in both appearances, so it remains independently
focusable when the paragraph wraps. The version pill uses the launcher's terminal font in both appearances. The visual
review found these details fit the written design and accepted both images. The dialog continues to open and dismiss
under the same startup, focus and saved-preference rules.

## Open questions and possible follow-ups

For a future planning-system change, the planning questions page could propose a headline when it proposes a plan's
release-note entry. The existing release review is where that suggestion is made today.

## Pull requests

- Curated headlines for new finalized highlights. [#5004](https://github.com/scode/farhelm/pull/5004/changes)
- The redesigned What's new dialog and both appearance references.
  [#5008](https://github.com/scode/farhelm/pull/5008/changes)

## Checks run, reused and skipped

- `python3 releasing/check-changelog.py --self-test` through the recorder passed, including draft and historical
  compatibility and the finalized headline rule. Run `703e39d7-de20-49c7-a88f-c7411a39db1e`.
  `python3 releasing/check-changelog.py format`, `python3 scripts/check-agents-md.py`, and `git diff --check` passed.
  The instructions check reported one pre-existing archive citation warning.
- `cargo nextest run --locked -p farhelm-gui --lib -E 'test(=assets::tests::every_icon_resolves_to_bundled_svg_bytes)'`
  passed through the recorder, run `320c9af6-10b0-40d0-a3d2-96f22658ed7b`. The pinned nextest selection
  `cargo nextest run --locked -p farhelm-gui --lib -E 'test(/whats_new::|session_list::rename|session_list::quick_switcher|launcher::view_tests::/)'`
  passed 153 cases, run `9ccbbbc0-17a1-4172-83b5-1f910d003ce1`. Both used the repository's four-slot budget and no
  retries.
- `python3 scripts/check-gui-goldens.py --update --scene whats-new-dialog --jobs 1` and the full
  `python3 scripts/check-gui-goldens.py --update` both ran through the recorder, runs
  `2b912038-23e4-44bc-b49d-206a5b6ddd1d` and `c020a9d6-1829-4886-9843-714565223621`. All 540 Linux reference images were
  accounted for. A byte-level comparison with the previous manifest found exactly the two reviewed What's new changes,
  and the full run produced the same bytes as the selected captures.
- `cargo fmt --all -- --check`, touched-file `dprint check`, and `python -B scripts/check-test-sleeps.py` with the
  isolated interpreter passed. Whole-tree dprint flagged the untouched triage archive.
- Focused and full render checks cover window revision `2e81b8c1f26ee594e204bd74c52e9ee8ee530c9c`. The later reference
  manifest update preserves their coverage. The release checker and window fixtures cover the changed paths, so broader
  workspace, native Mac updater, and live release tests were skipped.

## Review gate

Independent Opus 5.5 high source reviews covered both steps. The window review led to one shared Enter-key chip for the
primary actions and to restored heading navigation, per-release change totals and spoken version ranges. A separate Opus
5.5 high visual review checked the light and dark dialog against direction 2 and accepted both.

### Landing

#### What landed with it

This plan landed first in its round; the supervisor's Restart cleanup fix (`supervisor-stop-and-restart-cleanup-fixes`)
landed right after it and shares no code with it. Since the plan's PRs were based, main gained only the host
provisioning fixes for Tailscale Uninstall and SSH sign-in priority, all in the helm. The rebase applied without
conflicts. The plan was built on top of the quick switcher, template list and Rename restyle, and its shared Enter-key
piece is an exact extraction of what that restyle drew inline, so those dialogs look the same (the full screenshot
comparison found no other picture changed). One small change reaches the quick switcher: a screen reader now hears its
section heading with the count, for example "Recent sessions, 12 · every host", which is what the restyle's own comment
intended.

The report asked to confirm, before landing, that the headline rule's cutoff still matches the newest release section on
main: it does (v0.45.0, finalized, with no draft after it).

#### Fixes made while landing

- **The app's code did not pass clippy.** The pinned toolchain's clippy refuses one helper in the new window (a lazy
  `then` where a plain value does). Fixed inside [#5008](https://github.com/scode/farhelm/pull/5008/changes); it behaves
  the same.
- **A comment in the release notes checker spoke to this landing.** It told the lander to set the headline cutoff to the
  newest section; once landed it would have read as a rule to keep moving it. Fixed inside
  [#5004](https://github.com/scode/farhelm/pull/5004/changes) to say v0.45.0 was the newest section when the rule
  landed.
- **The changelog draft claimed a change that was not one.** It said What's new now shows the full text of every
  highlight, which the old window already did. Reworded inside #5008 to say what changed: the launcher's look, the
  header counts and per-release totals, and, for curation, the category marks in place of bullets, headlines, and
  "Screenshot" in place of "See screenshot".

#### Checks

Worker runs were on a separate arm64 Linux test worker; the rest ran locally on x86_64 Linux. All the checks below ran
on both plans' PRs stacked in queue order on main as it stood at the start of the landing (`3236d5b7`), which applied
without conflicts; every file's change was exactly what the executors pushed until the landing fixes, which this round
made inside the plans' own PRs: #5004 and #5008 for What's new, and #5005, documentation only, for the Restart cleanup
fix.

- The workspace Rust suite, `cargo nextest run --workspace --exclude farhelm-desktop`, on the test worker (recorder run
  `4c1d2119-e93c-4914-8853-ee0f72a604fc`), before the fixes: all 8726 passed, with no runtime `SKIPPED` lines. This
  includes the GUI crate's tests and every supervisor test, the Restart cleanup and orphan reclaim tests among them. The
  fixes since then change one What's new helper to an equivalent form, a checker comment, and documents, so this run
  still applies.
- `cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings`,
  `cargo clippy -p farhelm-desktop --all-targets -- -D warnings` and
  `cargo clippy -p farhelm-desktop --all-targets --features agent -- -D warnings`, on the test worker: the first run
  failed on the What's new helper; after the fix all four passed.
- After the fix, the What's new tests, `cargo nextest run -p farhelm-gui --lib -E 'test(/whats_new::/)'`, on the test
  worker (recorder run `410e532f-a1dd-4630-bfe7-e9d08bafd6e2`): all 5 passed.
- The desktop app's end-to-end smoke, `python3 -B scripts/gui-e2e-smoke.py`, on the test worker (recorder run
  `0c6809d4-fc0c-4d4d-984f-6733e66273db`), before the fixes: 50 steps passed, one skipped by design on Linux (paste).
- The desktop app's screenshot comparison, `python3 scripts/check-gui-goldens.py`, all scenes, locally (recorder run
  `a7a2a9b6-2f76-4259-8cc2-c641e5b2cb98`), before the fixes: passed, including the two What's new pictures the plan
  updated.
- The screenshot batch tool's test, including the committed scene list (recorder run
  `805b2d42-fadf-473c-87ff-a16fc3f1a3c1`), locally, before the fixes: passed.
- `python3 releasing/check-changelog.py --self-test` and `python3 releasing/check-changelog.py format`, locally, before
  and after the fixes: passed.
- `python3 scripts/check-agents-md.py`, `cargo fmt --all -- --check` and `python -B scripts/check-test-sleeps.py`,
  locally: passed. `dprint check`, locally: these plans' files pass; it fails only on over-long lines in the triage
  outcomes archive, which main has had since an earlier triage record (#4978) landed. No PR fixes them yet; a
  formatting-only one would, and I recommend it, as earlier rounds' notes did.

Reused: the workspace, smoke, screenshot comparison and scene list runs above across the fixes, for the reason given
(nothing the fixes change draws differently). The executors' runs were focused selections on older bases, and everything
they covered ran again above.

Skipped: the tests of the desktop app's outer shell (`cargo nextest run -p farhelm-desktop`), since the What's new
window lives in the app's shared GUI crate, whose tests ran in the workspace suite, and nothing in the shell changed; a
Mac run, since nothing changed is Mac-specific; and CI, which runs only on demand.

#### What the report says that is no longer true

- Nothing.

#### Things you should know that the report does not say

- **The header counts every category the design names, even at zero.** A fix-only upgrade reads "0 new, 0 changed, 2
  fixed". The plan asks for "N new", "N changed" and "N fixed" always and the others only when present, so this matches
  what you approved; say if you would rather hide zero counts.
- **Release dates show no year** ("NOV 24"), as the design specifies. An upgrade across New Year shows "DEC 30" above
  "JAN 3" without saying which year either is.
- **Older highlights can gain a headline line.** Any entry, old or new, that opens with a bold sentence followed by more
  text shows that sentence as its headline. This is deliberate and matches SPEC_impl.md.
