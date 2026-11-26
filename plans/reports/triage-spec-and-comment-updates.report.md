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
