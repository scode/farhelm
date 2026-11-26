# Spec and code-comment accuracy updates

## What this was about

Triage approved nine corrections to align the specifications and code comments with shipped behavior. They cover legacy
Tensorlake credentials, Claude Code first-run setup, launch confirmations, repository suggestions, registered agent IDs,
and terminal input and rendering. The PR graduates all nine decisions from the review queue to the archive.

## Things you should know

- Tensorlake is still experimental. Farhelm accepts that v0.36.0 Tensorlake hosts with no credential delivery record can
  retain the old Claude settings and Codex config if they never reconnected under v0.37 through v0.43. The document
  keeps the existing warning about stale values taking precedence at launch.
- On a recipe host, a symlinked or non-regular Claude Code settings destination can leave first-run setup pending.
  Farhelm leaves the path in place, reports the generic settings warning, and retries on reconnect. This is an accepted
  trade-off alongside the existing limits for large settings files and slow transfers.
- The launcher's YOLO confirmation can keep a launch pending while Farhelm saves the host preference or loads the
  checkout preview. Answering the hook-setup prompt can also wait for the preference to save. If the checkout preview is
  still loading when the hook answer proceeds, the launcher refuses the attempt. Press **Launch** again after the
  preview is ready.
- When you hover a file path in the terminal, the card showing its host, full path, and size must be visible when you
  press the link to download or open it. The press closes the card. Release on the same link before a new card replaces
  it. Farhelm checks that the link text stayed the same and that another surface did not cover the card at either event.
- While you are composing a character in the Linux terminal, Ctrl+Shift+/ can commit the unfinished text and open the
  Keyboard shortcuts dialog if XKB cancels composition before Farhelm gets the key. Separately, terminal programs that
  insert or delete starting halfway through a two-column character, such as a CJK glyph, can leave one cell with the
  wrong background color. The specs record both limitations.
- Registered custom agent IDs can start with lowercase letters or digits. The remaining references to repository
  discovery on the host now describe the shipped behavior. Repository suggestions answer from the helm's launch history,
  even while the selected host is silent.

## Open questions and possible follow-ups

The approved decisions are covered. The Linux compose limitation was verified from source, without a live Wayland or X11
reproduction.

## Pull requests

- Nine spec and comment corrections with their archived triage decisions.
  [PR #5040](https://github.com/scode/farhelm/pull/5040/changes)

## Checks run, reused, and skipped

- `git diff --check` and `dprint check SPEC.md SPEC_impl.md TRIAGE_OUTCOMES.md review_feedback_queue/INDEX.md` passed.
  The appended archive entries matched the Markdown formatter. A byte comparison verified that the older archive is
  intact. Whole-file formatting flags older archive entries.
- `python3 scripts/check-agents-md.py` passed with one pre-existing archive warning about a missing heading. The review
  queue index matches the remaining feedback files one to one.
- Reused `cargo fmt --all -- --check` from `042e351b`, which covers all four edited Rust files. Subsequent revisions
  changed only Markdown. Rust, JavaScript, and desktop runtime suites were skipped because the changes are to prose and
  review bookkeeping.

## Review gate

One fresh GPT-6.2 Sol high source review. Corrected the timing of cover checks for terminal file clicks and linked the
product spec to the Linux compose exception. The reviewer confirmed both corrections and found no remaining issue.

### Landing

#### What landed with it

This plan landed second of three in its round, after the provisioning, Podman and runtime triage
(`triage-provisioning-podman-and-runtime`) and before the queue discards (`triage-queue-discards`). It changes SPEC.md,
SPEC_impl.md and code and test comments, and no code. The rebase met only conflicts in the shared triage records. The
landing's independent review confirmed all nine of its items are present and the stale references to the removed
repository search are gone.

#### Fixes made while landing

- **One SPEC.md sentence dropped a requirement the previous round had just implemented (the review's should-fix).** The
  rewrite of the terminal file-click paragraph turned "is refused with a visible explanation" into "a refusal ...
  explains", so a silent refusal read as allowed, while the host menu and terminal triage, which landed one round
  earlier, exists to show that refusal. Fixed inside [#5040](https://github.com/scode/farhelm/pull/5040/changes): a
  click on a file whose disclosure painted but is clipped or covered is again refused with a visible explanation.

#### Checks

Worker runs were on a separate arm64 Linux test worker; the rest ran locally on x86_64 Linux. All the checks below ran
on the round's three plans stacked in queue order on main as it stood at the start of the landing (`491df5c5`), with the
shared triage records' conflicts kept both ways (by hand for one review queue index block in the spec plan's PR and the
open outcomes file in the discards plan's PR, then checked: the queue's 21 feedback files, its 21 index entries and the
21 open outcomes agree). Every other change is exactly what the executors pushed until the landing fixes, which change
only a SPEC.md sentence and two changelog drafts.

- The workspace Rust suite, `cargo nextest run --workspace --exclude farhelm-desktop`, on the test worker (recorder run
  `0368f541-87a9-4191-8302-5b438567d5bf`): all 8765 passed, with no runtime `SKIPPED` lines. That includes the
  provisioning, Podman, upload, UI channel and runtime tests the plans added or changed, on the rebased tree.
- `cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings`,
  `cargo clippy -p farhelm-desktop --all-targets -- -D warnings` and
  `cargo clippy -p farhelm-desktop --all-targets --features agent -- -D warnings`, on the test worker: passed.
- The desktop app's end-to-end smoke, `python3 -B scripts/gui-e2e-smoke.py`, on the test worker (recorder run
  `19013678-79f1-4d2e-9e1b-0271274e62f8`): of its 49 steps, 48 passed and one (paste) was skipped by design on Linux.
- `python3 scripts/check-agents-md.py`, `python3 releasing/check-changelog.py format` (again after the fixes),
  `cargo fmt --all -- --check` and `python -B scripts/check-test-sleeps.py`, locally: passed. `dprint check`, locally:
  it fails only on over-long lines in the triage outcomes archive: main's earlier ones and the new entries of the
  provisioning, Podman and runtime plan (the other two plans' entries pass); no PR fixes them yet; a formatting-only one
  would, and I recommend it, as earlier rounds' notes did.

Reused across the fixes: every run above, since they change no code. The executors' runs were on older bases, and
everything they covered ran again above.

Skipped: the screenshot comparison and the desktop app's outer shell tests, since no drawing code or desktop shell code
changed; real Podman and Tensorlake hosts and a Mac run, which no automated check here reaches; and CI, which runs only
on demand.

#### What the report says that is no longer true

- Nothing.

#### Things you should know that the report does not say

- **The launcher consent plan, now back with an executor, must reconcile with this plan's rewritten SPEC_impl.md
  paragraph** on how the YOLO and hook setup answers continue through a pending checkout preview, since that plan
  changes the behavior the paragraph describes. Its executor does that when it rebases, and its next landing review
  checks it; nothing needs your action.
- **Smaller points the review noticed.** The file disclosure is called a "card" next to the approval card; SPEC.md now
  links into SPEC_impl.md for the first time; and the Linux input-method gap is documented for the Keyboard shortcuts
  chord only, though it probably also applies to the tab switching and Rename and Delete chords (unverified). None needs
  a follow-up on its own.
