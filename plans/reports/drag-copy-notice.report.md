### What this was about

When a program in a terminal handles mouse selection itself and a drag copies nothing, Farhelm explains how to copy
instead. That guidance used to appear briefly in the terminal's top-right corner, once per page for each text. It was
easy to miss while dragging in a prompt box elsewhere in the terminal.

### Things you should know

The notice now appears just above where you released the mouse, clamped inside the window. It appears on every
qualifying drag and stays for up to 30 seconds, until you click ×, or until the next mouse press in that terminal.
Clicking × preserves terminal keyboard focus and sends no input to the program. The rest of the notice lets pointer
input through, and a hidden notice cannot take clicks.

Switching terminal tabs hides a notice belonging to the tab you left. Returning before its deadline shows it again;
switching tabs does not reset the deadline. A new qualifying drag replaces the position and starts a fresh deadline.

The rule that decides whether to show the notice and its advice are unchanged, as agreed during planning. The earlier
concern about notices after successful copying was not reproducible then and was explicitly excluded from this change.
SPEC.md and SPEC_impl.md describe the new behavior; the TODO entry is removed and a new changelog fragment is included.

### Open questions and possible follow-ups

None identified. If the separate suspected false-positive problem recurs, the planning decision was to record it as new
work with a reproduction.

### PRs

[#1731](https://github.com/scode/farhelm/pull/1731/changes) — the terminal copy notice at the release pointer, with
dismissal and repeat display. Draft, pushed at `9e7a434811a8875dbf30bd9afe66b72ba2ada97c`; it has not been merged.

### Checks run, reused and skipped

The recorded `mouse-modes.spec.ts` selection passed all six scenarios, three on Chromium and three on WebKit, in run
`770c0352-200e-46e6-bd18-bce5ac0e9172`. One worker, zero retries; no skips, flaky results, interruptions or global
errors. The report, source identity and retained console are complete, and the actual tmux 3.7c binary matched the
pinned substrate. This covers real pointer placement, × hover text, focus and input isolation, next-press dismissal,
hidden hit testing, terminal-tab visibility, the real timeout, repeated display, the unchanged copying predicate and
Codex advice. The fixture's separate SSH host was unavailable; every selected scenario used a local session and ran
fully, so this is not remote-host coverage.

The full UI JavaScript unit harness (`node --test` from its test directory) passed 203 tests in recorded run
`ccaa2f51-5f5c-42a1-ae31-ae56487a7f3d`. It includes exact clock-boundary and replacement checks without wall-clock
delays. Earlier run `ec171fcc-b418-49a2-a684-85b784fcd548` passed 202 and failed one stylesheet-token check; the new
style used an undefined color token and a literal radius. Those were corrected to existing theme tokens. That failure
remains retained; the later pass does not erase it, and this same-session authoring issue is not a latent-flake entry.

`cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
`cargo check -p farhelm-ui --features desktop`, the CLI build and release web build passed. The isolated source-delay
checker inspected 269 delays with zero missing reasons. JavaScript syntax checks, changelog format and changed-Markdown
dprint checks passed. Dprint does not select this repository's JavaScript or TypeScript files; no formatting verdict is
claimed for those.

Reused these results after the clean rebase from main `192a280e` to `15d43efd`. Every upstream diff was inspected: plan
claims, an upload-plan report and delivery, and a new transcript-read plan with TODO links. No executable code or
product specification changed. All changed code, tests, specifications and the changelog fragment remain byte-identical
to tested revision `e05c9ead`; the TODO keeps the upstream links. Changed-Markdown and changelog checks passed again
after rebase. A separate documentation pass covered all touched files.

Skipped full workspace runtime, desktop runtime, the full browser suite, installer, provisioning and website batteries:
the selected browser and unit proofs cover the affected terminal behavior, and compilation covers the small Rust view
change. The asset file set is unchanged, so asset-set parity needs no new check. No hosted CI or deployment was
requested.

### Review gate outcome

The required fresh gpt-6.1-sol high code and test review passed after correcting an asynchronous asset-loading
dependency and notice visibility when switching terminal tabs. No outstanding findings. The commit and PR wording passed
a separate fresh cold read. The resume check reconciled the source, reviews and completed browser evidence. Reviewers
were requested on the specified models, but the harness did not expose actual model identity or token counters; exact
attribution and usage could not be independently verified.
