## What this was about

Uninstalling Farhelm from a remote Mac could refuse because an unrelated mounted volume had " (" or " on " in its name.
The final directory-removal check could hit this after the supervisor's background job was already gone, leaving
uninstall partly finished. The mount check now follows the maintainer's chosen best-effort policy: a clearly detected
filesystem mounted on or inside the installation still prevents removal; ambiguous or unreadable mount information does
not prevent it.

## Things you should know

This deliberately accepts that a nested mount with an ambiguous name can go undetected, and removal can then delete the
files it exposes. Malformed and non-absolute mount lines are skipped silently; an empty or failed mount read reports no
mounts. Linux keeps its existing mount check and read-failure refusal. The existing ownership and path guards remain.
The original unreleased remote-Mac changelog entry records the caveat, and the completed TODO entry is removed.

## Open questions and possible follow-ups

None required for this plan. No real-Mac run was required or performed; the regression evidence uses controlled shell
tools and the actual removal backend on Linux.

## The PRs

- [#3267 — unblock remote Mac uninstall on mount parse errors](https://github.com/scode/farhelm/pull/3267/changes),
  draft on main, head `940ff2ce08a938f3abb2ad4409e1327e79edec16`.

## Checks run, reused and skipped

Run now: recorded `cargo nextest run -p farhelm-helm --lib` filtered to the Mac mount parser, Mac final removal,
uninstall inspection, uninstall planning, uninstall refusals and host-command removal guards. Recorder run
`8bbf3851-75af-41c0-b5c9-850f96ecf8e8` completed successfully: 24 passed, zero failures or flaky results, 1,639 selected
out. Four shared nextest slots, zero retries, pinned nextest 0.9.148, no tmux needed. The complete retained output has
no runtime `SKIPPED` messages; the new final-removal case first proves that a known nested mount preserves the
installation, then proves that unrelated ambiguous volumes permit removal. Source capture and JUnit evidence are
complete, with no output truncation or forced cleanup.

Also passed: `cargo fmt --all -- --check`, `python3 releasing/check-changelog.py format`, dprint on the changed
Markdown, and the required isolated-interpreter `python -B scripts/check-test-sleeps.py` (429 delays inspected, zero
missing reasons).

Reused: the recorded runtime result covers the executable and test changes in the final PR unchanged. Its source was the
reviewed working copy on main `71dfc8b992f8c357e038b350b18749e720fe2ccd`; the only later source edit moved the changelog
caveat into the original feature's fragment. A fresh fetch found no incoming main changes to reconcile.

Skipped: browser, desktop, real SSH/Mac, the full workspace suite, broad Clippy and CI dispatch. The changed behavior is
an embedded POSIX-shell check, covered directly and through the production removal backend with child-only fixture
tools; those broader checks add no identified coverage for this change. No live installation was changed.

## Review gate's outcome

The required fresh gpt-6.1-sol/high reviewer found no correctness, design, idiomatic-code or test-authoring findings.
The same reviewer also checked the final fragment-only correction and found no issues. The commit/PR wording passed its
separate cold read. Every touched file received a documentation pass; no scope expansion or omitted required behavior
remains.
