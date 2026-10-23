## What this was about

The triage outcome "duplicate uninstall advice points to a removed owner" (2026-10-21, decided without asking under your
instruction for clear-cut items). When two host entries reach the same machine, only one connects; the other is refused
as a duplicate, and choosing Uninstall on it is refused with advice to uninstall through the connected entry. If you
then removed that connected entry, the remaining one stays refused until you press Retry, but its uninstall refusal
still said "choose uninstall on another entry (host N), then remove this entry", pointing at an entry that no longer
exists. Triage decided to change only that advice, for only that case, without promising that uninstall then succeeds,
and to leave the name ("another entry (host N)"), the other refusal cases and the host-details status text in the host
list as they are. Size gate: about 100 lines; over it, block and bring it back.

## Things you should know

- The refusal for that case now ends its advice with: "That entry has since been removed: press retry so this entry can
  connect to the machine, then choose uninstall on this entry". The rest of the refusal is unchanged, so it still opens
  by saying this entry reaches the same machine as "another entry (host N)".
- "Can connect" rather than "connects", because Retry can still fail (the machine is unreachable, or the removed entry
  was added back as a new entry, which makes a fresh duplicate). It also makes no promise about uninstall itself: its
  usual checks, such as running sessions, still apply after the retry. This matches SPEC.md's duplicate remedy (remove
  the other entry or change this one's destination, then press Retry), and an existing helm test shows Retry connects
  the surviving entry once the other is removed.
- The new advice appears only when the helm's host list no longer holds the other entry; a failed read of the list is an
  error, not this case.
- The changelog entry this wording belongs to has not shipped in a release yet, so the PR adds a context paragraph to
  that entry's fragment instead of adding a new fragment.
- Size: about 70 changed lines of code and tests, within the gate.

## Open questions and possible follow-ups

- Possible follow-up: the refusal still opens by saying this entry reaches the same machine as the removed entry, which
  reads oddly next to "That entry has since been removed". Triage scoped that sentence out, so I left it.

## The PRs

- #2605 fix: suggest retry when uninstall refuses a removed entry's duplicate

## Checks

Run now:

- Recorded `cargo nextest run -p farhelm-helm --lib` selecting the uninstall refusal module, both duplicate uninstall
  tests and `retry_clears_a_duplicate_once_the_other_entry_is_gone`: run 1a429932-e5bd-41ae-973e-21cc7f52570a, 7 of 7
  passed.
- `cargo clippy -p farhelm-helm --all-targets -- -D warnings`, `cargo fmt --all -- --check`: pass.
- `python3 releasing/check-changelog.py format` and `dprint check` on the changed Markdown files: pass.
- `python -B scripts/check-test-sleeps.py`: 314 test delays, none without a rationale.

Reused: none. Skipped: other helm, UI and browser tests, because only the refusal's wording and its two tests changed,
as the plan's validation section says.

## Review gate

A fresh-context Opus 5.5 review at high effort found no blocker. I applied its two low findings: the live-entry case now
asserts that the new advice does not appear, and "connects" became "can connect". I left two nits: the refusal's
present-tense opening sentence before "has since been removed" (out of the triaged scope), and merging two adjacent
conditionals in a test (I kept the clearer order).
