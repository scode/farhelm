# Cutover probe ignores output before its asserted boundary

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

The cutover probe ignores early output between reply blocks.

## Details

F270 — **definite** — `scripts/check-tmux-cutover.py:191–196` — Cutover probe ignores output before its asserted
boundary

An asynchronous pane-output notification between the two reply blocks is excluded from the probe's relevant assertions.
It can therefore pass while violating the required boundary against output before cutover. Reject pane-output
notifications throughout the entire prefix ending with the final refresh reply, so the probe actually enforces the
boundary it is cited to establish.

## Evidence and triage context

- scripts/check-tmux-cutover.py:143–148 represents notifications outside blocks as LineEvent. Lines 182–186 inspect only
  block bodies. Lines 191–196 discard all notifications preceding the final block. A snapshot ending at N, an
  intervening duplicate output N, and contiguous post-block output starting at N+1 pass :198–205. The real caller at
  :268 executes this oracle and :297 reports success.

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- None identified in completed collection evidence.

Caveats:

- This proves an oracle defect, not that supported tmux versions produce the violating sequence.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `automation_website_08_cor:p1:F2`.

- `automation_website_08_cor:p1:F2`: confidence as filed: definite; suggested bucket as filed: other.
