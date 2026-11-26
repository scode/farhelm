# Review feedback queue index

One line per open item. This file must always match the feedback files in this directory.

Each file identifies its reviewed commit, confidence, caveats, and provenance for triage. Findings retained from the
review at `bd8d5d76439f8f36bbc2637b44d7ba4026b65675` have not all been reassessed against later product changes; that
review also had known coverage gaps. An entry records an open finding, not a clean verdict for the surrounding code.

Collection record for the abbreviated review through `00de0b6c`:
[complete findings, candidates, withdrawals, leads and returned proposals](records/review-through-00de0b6c/README.md).
This record is bookkeeping, not an additional feedback item. Provisional buckets preserve unresolved stronger
consequences.

## Highest priority: security or data loss

## High priority: material UX degradation

- [`trailing-replace-with-recent-named-fill-enter-replaces.md`](trailing-replace-with-recent-named-fill-enter-replaces.md)
  — In Replace with, a screen reader announces a highlighted recent setup as "Fill from recent setup" while Enter on it
  replaces and deletes the source session; highest if the "⏎ Replace" key cap is not announced with it.

## Other: correctness, diagnostics, cleanup, or convenience

- [`trailing-launcher-new-clone-enter-after-vanished-pick.md`](trailing-launcher-new-clone-enter-after-vanished-pick.md)
  — In New and Clone, Return on a highlighted template or folder row can launch instead of picking when the highlight
  vanished before the redraw; only Replace with is guarded.

- [`trailing-launcher-held-escape-discards-draft.md`](trailing-launcher-held-escape-discards-draft.md) — Holding Escape
  with typed results or the Save as template panel open can close the whole launcher and discard its draft; the fix
  covered only the highlight case.

- [`trailing-launcher-stale-permanent-yolo-answer-silent.md`](trailing-launcher-stale-permanent-yolo-answer-silent.md) —
  A stale "Start, and don't ask again on this host" answer closes the YOLO question with nothing started and no reason,
  sometimes after the host setting was already saved.

- [`trailing-repository-history-failure-shown-as-no-matches.md`](trailing-repository-history-failure-shown-as-no-matches.md)
  — A failed repository-history read shows "No repository matches" and is not retried while the launcher is open.
